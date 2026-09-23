use crate::model::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[path = "backup_security.rs"]
mod backup_security;

fn change(path: PathBuf, after: Option<Vec<u8>>, secret: bool) -> Result<FileChange, String> {
    validate_path(&path)?;
    let before = if path.exists() {
        Some(fs::read(&path).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let display = |bytes: &Option<Vec<u8>>| {
        bytes.as_ref().map(|b| {
            if secret {
                redacted_config(b, &path)
            } else {
                String::from_utf8(b.clone()).unwrap_or_else(|_| {
                    format!("[Binary file: {} bytes, SHA256 {}]", b.len(), hash(b))
                })
            }
        })
    };
    let mut after_display = display(&after);
    if secret {
        let fields = changed_config_fields(before.as_deref(), after.as_deref(), &path);
        if let Some(text) = after_display.as_mut() {
            text.push_str(&format!(
                "\nChanged fields (values hidden): {}",
                fields.join(", ")
            ));
        }
    }
    Ok(FileChange {
        path: path.display().to_string(),
        before_hash: before.as_ref().map(|b| hash(b)),
        before: display(&before),
        after: after_display,
        before_bytes: before,
        after_bytes: after,
    })
}
fn parse_config(bytes: Option<&[u8]>, path: &Path) -> Value {
    bytes
        .and_then(|b| {
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                toml::from_str::<toml::Value>(std::str::from_utf8(b).ok()?)
                    .ok()
                    .and_then(|v| serde_json::to_value(v).ok())
            } else {
                serde_json::from_slice(b).ok()
            }
        })
        .unwrap_or(Value::Null)
}
fn changed_config_fields(before: Option<&[u8]>, after: Option<&[u8]>, path: &Path) -> Vec<String> {
    fn visit(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
        if a == b {
            return;
        }
        if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                // Credential map keys are not exposed. Report their parent instead.
                if ["env", "headers", "http_headers", "env_http_headers"].contains(&key.as_str()) {
                    if a.get(key) != b.get(key) {
                        out.push(format!("{path}/{key}"));
                    }
                } else {
                    visit(
                        a.get(key).unwrap_or(&Value::Null),
                        b.get(key).unwrap_or(&Value::Null),
                        &format!("{path}/{key}"),
                        out,
                    );
                }
            }
        } else {
            out.push(path.to_string());
        }
    }
    let a = parse_config(before, path);
    let b = parse_config(after, path);
    let mut result = vec![];
    for key in ["mcpServers", "mcp_servers", "skills"] {
        visit(&a[key], &b[key], key, &mut result);
    }
    result
}
pub fn validate_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("An absolute path without parent traversal is required".into());
    }
    #[cfg(windows)]
    for component in path.components() {
        use std::path::{Component, Prefix};
        match component {
            Component::Prefix(p)
                if !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) =>
            {
                return Err("Only local drive paths support writes".into())
            }
            Component::Normal(value) => {
                let name = value.to_str().ok_or("Non-Unicode write path")?;
                let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                if name.contains(':')
                    || name.ends_with(['.', ' '])
                    || name.chars().any(|c| c.is_control())
                    || [
                        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                        "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
                        "LPT7", "LPT8", "LPT9",
                    ]
                    .contains(&stem.as_str())
                {
                    return Err("Ambiguous or reserved Windows write path".into());
                }
            }
            _ => {}
        }
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(
                        "Linked paths are read-only; select the original installation".into(),
                    );
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err("Junction/reparse-point paths are read-only".into());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Cannot verify write path {}: {error}",
                    ancestor.display()
                ))
            }
        }
    }
    Ok(())
}
fn same_location(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.replace('\\', "/")
            .trim_start_matches("//?/")
            .eq_ignore_ascii_case(b.replace('\\', "/").trim_start_matches("//?/"))
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
fn plan(title: String, changes: Vec<FileChange>, warnings: Vec<String>) -> Plan {
    Plan {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        changes,
        warnings,
        created_at: now(),
        package_snapshot: None,
    }
}
fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Use 1-80 ASCII letters, digits, hyphens or underscores".into());
    }
    let upper = name.to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&upper.as_str())
    {
        return Err("Reserved Windows filename".into());
    }
    Ok(())
}
pub fn validate_skill(content: &str) -> Result<(), String> {
    if content.len() > 1024 * 1024 {
        return Err("Skill manifest exceeds 1 MiB".into());
    }
    let normalized = content.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let yaml = normalized
        .strip_prefix("---\n")
        .and_then(|v| v.split_once("\n---"))
        .ok_or("SKILL.md needs YAML frontmatter")?
        .0;
    let v: Value = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
    if v["name"].as_str().unwrap_or("").is_empty()
        || v["description"].as_str().unwrap_or("").is_empty()
    {
        return Err("Skill name and description are required".into());
    }
    Ok(())
}
pub fn package_files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    fn visit(
        root: &Path,
        dir: &Path,
        depth: usize,
        total: &mut usize,
        result: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), String> {
        validate_path(dir)?;
        if depth > 32 {
            return Err("Package depth exceeds 32".into());
        }
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            validate_path(&path)?;
            if entry.file_name() == ".git" || entry.file_name() == "node_modules" {
                return Err(
                    "Package contains a VCS/dependency tree; select a standalone skill directory"
                        .into(),
                );
            }
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                visit(root, &path, depth + 1, total, result)?;
            } else if meta.is_file() {
                if meta.len() > 16 * 1024 * 1024 || result.len() >= 10000 {
                    return Err("Package file limit exceeded".into());
                }
                let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                *total += bytes.len();
                if *total > 64 * 1024 * 1024 {
                    return Err("Package exceeds 64 MiB".into());
                }
                result.insert(
                    path.strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_path_buf(),
                    bytes,
                );
            } else {
                return Err("Unsupported special file".into());
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    visit(root, root, 0, &mut 0, &mut result)?;
    Ok(result)
}
fn owned_skill<'a>(inventory: &'a Inventory, id: Option<&str>) -> Result<&'a Component, String> {
    let c = inventory
        .components
        .iter()
        .find(|c| Some(c.id.as_str()) == id)
        .ok_or("Component no longer exists; rescan")?;
    let p = c.path.replace('\\', "/").to_lowercase();
    let canonical = c.canonical_path.replace('\\', "/").to_lowercase();
    let protected =
        |p: &str| p.contains("/synced/") || p.contains("/plugins/") || p.contains("/.system/");
    let registered_owner = inventory
        .components
        .iter()
        .filter(|p| p.kind == "plugin")
        .any(|p| {
            canonical.starts_with(&format!(
                "{}/",
                p.canonical_path
                    .replace('\\', "/")
                    .trim_end_matches('/')
                    .to_lowercase()
            ))
        });
    if c.kind != "skill"
        || c.owner_id.is_some()
        || matches!(c.binding_kind.as_deref(), Some("inherited" | "local"))
        || c.status.iter().any(|status| status == "managed")
        || c.scope == "cache"
        || protected(&p)
        || protected(&canonical)
        || registered_owner
    {
        return Err("Managed by parent package or agent; use its owning manager".into());
    }
    Ok(c)
}
fn redacted_config(bytes: &[u8], path: &Path) -> String {
    let parsed = if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| toml::from_str::<toml::Value>(s).ok())
            .and_then(|v| serde_json::to_value(v).ok())
    } else {
        serde_json::from_slice::<Value>(bytes).ok()
    };
    let Some(value) = parsed else {
        return "[Unparseable configuration omitted]".into();
    };
    let mut safe =
        json!({"note":"Unrelated fields, arguments and credentials are omitted from this preview"});
    for key in ["mcpServers", "mcp_servers"] {
        if let Some(servers) = value[key].as_object() {
            let mut result = serde_json::Map::new();
            for (name, config) in servers {
                let mut entry = json!({});
                for flag in ["enabled", "disabled"] {
                    if let Some(v) = config[flag].as_bool() {
                        entry[flag] = json!(v)
                    }
                }
                if let Some(kind) = config["type"].as_str() {
                    entry["type"] = json!(if ["stdio", "http", "sse", "streamable-http"]
                        .contains(&kind)
                    {
                        kind
                    } else {
                        "[unknown transport]"
                    })
                }
                if config.get("command").is_some() {
                    entry["command"] = json!("[command configured; review supplied editor input]")
                }
                if let Some(url) = config["url"].as_str() {
                    entry["urlOrigin"] = json!(reqwest::Url::parse(url)
                        .ok()
                        .map(|u| u.origin().ascii_serialization())
                        .unwrap_or_else(|| "[invalid URL]".into()))
                }
                for field in ["env", "headers"] {
                    if let Some(map) = config[field].as_object() {
                        entry[field] = Value::Object(
                            map.keys()
                                .map(|k| (k.clone(), json!("[redacted]")))
                                .collect(),
                        )
                    }
                }
                if config.get("args").is_some() {
                    entry["args"] = json!("[redacted]")
                }
                result.insert(name.clone(), entry);
            }
            safe[key] = Value::Object(result)
        }
    }
    if let Some(entries) = value["skills"]["config"].as_array() {
        safe["skills"] = json!({"config":entries.iter().map(|e|json!({"path":e["path"],"enabled":e["enabled"]})).collect::<Vec<_>>()})
    }
    serde_json::to_string_pretty(&safe).unwrap_or_else(|_| "[Configuration omitted]".into())
}
pub fn plan_skill(
    settings: &Settings,
    inventory: &Inventory,
    action: &str,
    id: Option<&str>,
    name: &str,
    content: &str,
) -> Result<Plan, String> {
    if action == "install" {
        valid_name(name)?;
        validate_skill(content)?;
        let root = Path::new(&settings.home).join(".agents/skills").join(name);
        if root.exists() {
            return Err("Installation directory already exists; use update".into());
        }
        return Ok(plan(
            format!("Install skill {name}"),
            vec![change(
                root.join("SKILL.md"),
                Some(content.as_bytes().to_vec()),
                false,
            )?],
            vec![
                "Creates a manifest-only skill. Use package import to include scripts/resources."
                    .into(),
            ],
        ));
    }
    let c = owned_skill(inventory, id)?;
    if action == "enable" || action == "disable" {
        return skill_binding(settings, c, action == "enable");
    }
    let path = PathBuf::from(&c.path);
    validate_path(&path)?;
    if inventory
        .components
        .iter()
        .any(|other| other.id != c.id && same_location(&other.canonical_path, &c.canonical_path))
    {
        return Err(
            "Shared payload has other bindings; resolve bindings before editing/removing its files"
                .into(),
        );
    }
    let changes = match action {
        "update" => {
            validate_skill(content)?;
            vec![change(
                path.clone(),
                Some(content.as_bytes().to_vec()),
                false,
            )?]
        }
        "uninstall" => package_files(path.parent().ok_or("Missing skill directory")?)?
            .into_keys()
            .map(|relative| change(path.parent().unwrap().join(relative), None, false))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("Unsupported skill action".into()),
    };
    let mut result = plan(
        format!("{action} skill {}", c.name),
        changes,
        vec![
            "All listed files are backed up. Empty directories are retained. No commands run."
                .into(),
        ],
    );
    if action == "uninstall" {
        result.package_snapshot = Some(package_snapshot(path.parent().ok_or("Missing parent")?)?);
    }
    Ok(result)
}
pub fn plan_package(
    settings: &Settings,
    inventory: &Inventory,
    action: &str,
    id: Option<&str>,
    name: &str,
    source: &Path,
    agent: &str,
) -> Result<Plan, String> {
    valid_name(name)?;
    let files = package_files(source)?;
    let manifest = files
        .get(Path::new("SKILL.md"))
        .ok_or("Package must contain SKILL.md at its root")?;
    validate_skill(std::str::from_utf8(manifest).map_err(|_| "SKILL.md must be UTF-8")?)?;
    let target = if action == "install" {
        let folder = match agent {
            "Codex" | "Shared" => Path::new(&settings.home).join(".agents/skills"),
            "Claude Code" => settings.claude_root().join("skills"),
            _ => return Err("Unsupported agent".into()),
        };
        let target = folder.join(name);
        if target.exists() {
            return Err("Installation directory already exists".into());
        }
        target
    } else if action == "update" {
        let c = owned_skill(inventory, id)?;
        if inventory.components.iter().any(|other| {
            other.id != c.id && same_location(&other.canonical_path, &c.canonical_path)
        }) {
            return Err("Cannot update a shared payload with other bindings".into());
        }
        Path::new(&c.path)
            .parent()
            .ok_or("Missing parent")?
            .to_path_buf()
    } else {
        return Err("Package import supports install/update".into());
    };
    validate_path(&target)?;
    if target == source || source.starts_with(&target) || target.starts_with(source) {
        return Err("Source and target directories must be separate".into());
    }
    let mut changes = vec![];
    if target.exists() {
        for relative in package_files(&target)?.into_keys() {
            if !files.contains_key(&relative) {
                changes.push(change(target.join(relative), None, false)?);
            }
        }
    }
    for (relative, bytes) in files {
        changes.push(change(target.join(relative), Some(bytes), false)?);
    }
    let mut result = plan(
        format!("{action} complete skill package {name}"),
        changes,
        vec![format!(
            "Source snapshot: {}. No scripts executed.",
            source.display()
        )],
    );
    result.package_snapshot = Some(package_snapshot(&target)?);
    Ok(result)
}
fn package_snapshot(root: &Path) -> Result<(String, BTreeMap<PathBuf, String>), String> {
    let files = if root.exists() {
        package_files(root)?
    } else {
        BTreeMap::new()
    };
    Ok((
        root.display().to_string(),
        files.into_iter().map(|(p, b)| (p, hash(&b))).collect(),
    ))
}
fn codex_root(settings: &Settings) -> PathBuf {
    settings.codex_root()
}
fn read_toml(path: &Path) -> Result<toml::Table, String> {
    if !path.exists() {
        return Ok(toml::Table::new());
    }
    toml::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|_| "Invalid TOML; no changes made (source hidden to protect credentials)".into())
}
fn read_json(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let v: Value = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|_| "Invalid JSON; no changes made (source hidden to protect credentials)")?;
    if !v.is_object() {
        return Err("Configuration must be a JSON object".into());
    }
    Ok(v)
}
fn skill_binding(settings: &Settings, c: &Component, enabled: bool) -> Result<Plan, String> {
    if c.agent != "Codex" && c.agent != "Shared" {
        return Err(
            "Claude Code has no documented per-skill enable switch; use its native manager".into(),
        );
    }
    let path = codex_root(settings).join("config.toml");
    let mut doc = read_toml(&path)?;
    let skills = doc
        .entry("skills")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or("Invalid skills configuration")?;
    let entries = skills
        .entry("config")
        .or_insert_with(|| toml::Value::Array(vec![]))
        .as_array_mut()
        .ok_or("Invalid skills.config")?;
    if let Some(entry) = entries
        .iter_mut()
        .find(|v| v.get("path").and_then(|p| p.as_str()) == Some(c.path.as_str()))
    {
        entry
            .as_table_mut()
            .ok_or("Invalid skill binding")?
            .insert("enabled".into(), toml::Value::Boolean(enabled));
    } else {
        let mut table = toml::Table::new();
        table.insert("path".into(), toml::Value::String(c.path.clone()));
        table.insert("enabled".into(), toml::Value::Boolean(enabled));
        entries.push(toml::Value::Table(table));
    }
    Ok(plan(format!("{} Codex binding: {}", if enabled { "Enable" } else { "Disable" }, c.name), vec![change(path, Some(toml::to_string_pretty(&doc).map_err(|e| e.to_string())?.into_bytes()), true)?], vec!["Only Codex's binding changes. Shared payload remains intact. TOML comments are not retained.".into()]))
}
pub fn plan_mcp(
    settings: &Settings,
    agent: &str,
    project: Option<&str>,
    action: &str,
    name: &str,
    content: &str,
) -> Result<Plan, String> {
    valid_name(name)?;
    if let Some(project) = project {
        if !settings.projects.iter().any(|p| p == project) {
            return Err("Select a configured project root".into());
        }
    }
    let supplied: Option<Value> = if action == "install" || action == "update" {
        let v: Value =
            serde_json::from_str(content).map_err(|e| format!("Invalid MCP JSON: {e}"))?;
        if !v.is_object()
            || (v["command"].as_str().unwrap_or("").is_empty()
                && v["url"].as_str().unwrap_or("").is_empty())
        {
            return Err("MCP entry needs command or url".into());
        }
        if v.get("command").is_some() && v.get("url").is_some() {
            return Err("Choose stdio command or HTTP URL, not both".into());
        }
        if let Some(url) = v["url"].as_str() {
            if !url.starts_with("https://")
                && !url.starts_with("http://localhost:")
                && !url.starts_with("http://127.0.0.1:")
            {
                return Err("Remote URL must use HTTPS (loopback HTTP allowed)".into());
            }
        }
        Some(v)
    } else {
        None
    };
    let (path, after) = if agent == "Codex" {
        let path = project
            .map(|p| Path::new(p).join(".codex/config.toml"))
            .unwrap_or_else(|| codex_root(settings).join("config.toml"));
        let mut doc = read_toml(&path)?;
        let servers = doc
            .entry("mcp_servers")
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .ok_or("Invalid mcp_servers table")?;
        validate_action(action, servers.contains_key(name))?;
        match action {
            "install" | "update" => {
                let mut merged = servers
                    .get(name)
                    .map(|v| serde_json::to_value(v).unwrap_or(json!({})))
                    .unwrap_or(json!({}));
                merge_config(&mut merged, supplied.unwrap());
                if merged.get("url").is_some() && merged.get("command").is_some() {
                    return Err("Changing transport requires uninstall then install".into());
                }
                let v: toml::Value = serde_json::from_value(merged)
                    .map_err(|_| "MCP entry contains unsupported TOML values")?;
                servers.insert(name.into(), v);
            }
            "uninstall" => {
                servers.remove(name);
            }
            "enable" | "disable" => {
                servers
                    .get_mut(name)
                    .and_then(|v| v.as_table_mut())
                    .ok_or("Invalid MCP entry")?
                    .insert("enabled".into(), toml::Value::Boolean(action == "enable"));
            }
            _ => return Err("Unsupported MCP action".into()),
        }
        (
            path,
            toml::to_string_pretty(&doc).map_err(|e| e.to_string())?,
        )
    } else if agent == "Claude Code" {
        if action == "enable" || action == "disable" {
            return Err("Claude MCP disablement is project-context dependent; use Claude /mcp for the intended project".into());
        }
        let path = project
            .map(|p| Path::new(p).join(".mcp.json"))
            .unwrap_or_else(|| settings.claude_user_config());
        let mut doc = read_json(&path)?;
        let servers = doc
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("Invalid mcpServers object")?;
        validate_action(action, servers.contains_key(name))?;
        if action == "uninstall" {
            servers.remove(name);
        } else {
            let mut v = servers.get(name).cloned().unwrap_or(json!({}));
            merge_config(&mut v, supplied.ok_or("Unsupported MCP action")?);
            if v.get("url").is_some() && v.get("type").is_none() {
                v["type"] = json!("http");
            }
            if v.get("url").is_some() && v.get("command").is_some() {
                return Err("Changing transport requires uninstall then install".into());
            }
            servers.insert(name.into(), v);
        }
        (
            path,
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?,
        )
    } else {
        return Err("Unsupported agent".into());
    };
    Ok(plan(format!("{action} {agent} MCP registration: {name}"), vec![change(path, Some(after.into_bytes()), true)?], vec!["Only registration changes. No packages are installed and no server commands run. Restart/reload your agent.".into(), "Credentials hidden in preview; raw configuration stored only in local backups. Formatting/comments may change.".into()]))
}
fn merge_config(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (k, v) in patch {
                merge_config(target.entry(k).or_insert(Value::Null), v)
            }
        }
        (target, patch) => *target = patch,
    }
}
fn validate_action(action: &str, exists: bool) -> Result<(), String> {
    if action == "install" && exists {
        return Err("Entry exists; use update".into());
    }
    if action != "install" && !exists {
        return Err("Entry does not exist; rescan".into());
    }
    if !["install", "update", "uninstall", "enable", "disable"].contains(&action) {
        return Err("Unsupported action".into());
    }
    Ok(())
}
// This journal contains hashes and paths only; backup bytes never cross IPC.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub message: String,
    pub at: u64,
    entries: Vec<RecoveryEntry>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
struct RecoveryEntry {
    path: String,
    before_hash: Option<String>,
    after_hash: Option<String>,
    attempted: bool,
}
fn durable_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    validate_path(path)?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| e.to_string())
}
fn save_recovery(dir: &Path, record: &RecoveryRecord) -> Result<(), String> {
    durable_write(
        &dir.join("recovery.json"),
        &serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
    )
}
fn recovery_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid recovery ID")?;
    let dir = root.join(id);
    validate_path(&dir)?;
    Ok(dir)
}
fn load_recovery(dir: &Path) -> Result<RecoveryRecord, String> {
    let journal = dir.join("recovery.json");
    validate_path(&journal)?;
    if fs::metadata(&journal).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("Recovery journal exceeds size limit".into());
    }
    let record: RecoveryRecord =
        serde_json::from_slice(&fs::read(journal).map_err(|e| e.to_string())?)
            .map_err(|_| "Invalid recovery journal")?;
    uuid::Uuid::parse_str(&record.id).map_err(|_| "Invalid recovery ID")?;
    if dir.file_name().and_then(|p| p.to_str()) != Some(record.id.as_str()) {
        return Err("Recovery journal identity mismatch".into());
    }
    let mut paths: Vec<&str> = Vec::new();
    for entry in &record.entries {
        validate_path(Path::new(&entry.path))?;
        if paths.iter().any(|p| same_location(p, &entry.path)) {
            return Err("Duplicate recovery target".into());
        }
        paths.push(&entry.path);
        for digest in [&entry.before_hash, &entry.after_hash]
            .into_iter()
            .flatten()
        {
            if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Invalid recovery checksum".into());
            }
        }
    }
    Ok(record)
}
pub fn recovery_records(root: &Path) -> Result<Vec<RecoveryRecord>, String> {
    validate_path(root)?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut records = vec![];
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry
            .map_err(|e| e.to_string())?
            .path()
            .join("recovery.json");
        validate_path(&path)?;
        if path.exists() {
            records.push(load_recovery(path.parent().ok_or("Invalid journal path")?)?);
        }
    }
    Ok(records)
}
pub fn recover_pending(root: &Path) -> Result<Vec<Operation>, String> {
    Ok(recovery_records(root)?
        .into_iter()
        .filter(|r| r.status != "completed" && r.status != "rolled_back")
        .map(|r| Operation {
            id: r.id,
            title: r.title,
            status: r.status,
            message: format!("{}. Backup root: {}", r.message, root.display()),
            at: r.at,
        })
        .collect())
}
/// Mark complete only AFTER inventory and operation history have committed.
pub fn finish_recovery(root: &Path, id: &str) -> Result<(), String> {
    let dir = recovery_dir(root, id)?;
    if !dir.join("recovery.json").exists() {
        return Ok(());
    }
    let mut record = load_recovery(&dir)?;
    if record.status == "applied_pending_persistence" {
        record.status = "completed".into();
        record.message = "Files verified; inventory and history persisted".into();
        save_recovery(&dir, &record)?;
    }
    Ok(())
}
/// Explicit user-confirmed recovery only. Never overwrite an external edit.
pub fn recover(root: &Path, id: &str) -> Result<RecoveryRecord, String> {
    let dir = recovery_dir(root, id)?;
    let mut record = load_recovery(&dir)?;
    backup_security::secure_backup_directory(&dir)?;
    if record.status == "completed" || record.status == "rolled_back" {
        return Err("Recovery already finalized".into());
    }
    let mut failures = vec![];
    for (i, entry) in record
        .entries
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, e)| e.attempted)
    {
        let restored = (|| -> Result<(), String> {
            let path = Path::new(&entry.path);
            validate_path(path)?;
            let current = if path.exists() {
                Some(hash(&fs::read(path).map_err(|e| e.to_string())?))
            } else {
                None
            };
            if current == entry.before_hash {
                return Ok(());
            }
            if current != entry.after_hash {
                return Err("External edit detected; manual recovery required".into());
            }
            let before = if entry.before_hash.is_some() {
                let backup_path = dir.join(format!("{i}.bak"));
                validate_path(&backup_path)?;
                let bytes = fs::read(backup_path).map_err(|e| e.to_string())?;
                if Some(hash(&bytes)) != entry.before_hash {
                    return Err("Backup checksum mismatch".into());
                }
                Some(bytes)
            } else {
                None
            };
            write_change(path, before.as_deref())?;
            verify_bytes(path, before.as_deref())
        })();
        if let Err(error) = restored {
            failures.push(format!("{}: {error}", entry.path));
        }
    }
    record.status = if failures.is_empty() {
        "rolled_back"
    } else {
        "recovery_required"
    }
    .into();
    record.message = if failures.is_empty() {
        "Rollback: completed".into()
    } else {
        failures.join("; ")
    };
    record.at = now();
    save_recovery(&dir, &record)?;
    Ok(record)
}
pub fn apply(plan: &Plan, backup_root: &Path) -> Result<(), String> {
    if plan.changes.is_empty() {
        return Err("Plan has no executable file changes".into());
    }
    if let Some((root, expected)) = &plan.package_snapshot {
        if &package_snapshot(Path::new(root))?.1 != expected {
            return Err(
                "Stale plan: package membership or contents changed; create a new preview".into(),
            );
        }
    }
    for c in &plan.changes {
        check_current(c)?;
    }
    let backup = recovery_dir(backup_root, &plan.id)?;
    fs::create_dir_all(backup_root).map_err(|e| e.to_string())?;
    // Backups can contain credentials. Protect both the configured backup root
    // and this operation's directory before writing the first byte.
    backup_security::secure_backup_directory(backup_root)?;
    fs::create_dir(&backup).map_err(|e| e.to_string())?;
    backup_security::secure_backup_directory(&backup)?;
    for (i, c) in plan.changes.iter().enumerate() {
        if let Some(before) = &c.before_bytes {
            durable_write(&backup.join(format!("{i}.bak")), before)?;
        }
    }
    durable_write(
        &backup.join("plan.json"),
        &serde_json::to_vec_pretty(plan).map_err(|e| e.to_string())?,
    )?;
    let mut record = RecoveryRecord {
        id: plan.id.clone(),
        title: plan.title.clone(),
        status: "applying".into(),
        message: "Confirmed operation has a durable backup".into(),
        at: now(),
        entries: plan
            .changes
            .iter()
            .map(|c| RecoveryEntry {
                path: c.path.clone(),
                before_hash: c.before_hash.clone(),
                after_hash: c.after_bytes.as_ref().map(|b| hash(b)),
                attempted: false,
            })
            .collect(),
    };
    save_recovery(&backup, &record)?;
    let mut expected_package = plan.package_snapshot.clone();
    for (i, c) in plan.changes.iter().enumerate() {
        let result = (|| -> Result<(), String> {
            if let Some((root, expected)) = &expected_package {
                if &package_snapshot(Path::new(root))?.1 != expected {
                    return Err("Stale plan: package membership changed during apply".into());
                }
            }
            check_current(c)?;
            record.entries[i].attempted = true;
            // Persist intent before the corresponding file write, including crash windows.
            save_recovery(&backup, &record)?;
            write_change(Path::new(&c.path), c.after_bytes.as_deref())?;
            verify_bytes(Path::new(&c.path), c.after_bytes.as_deref())
        })();
        if let Err(error) = result {
            let restored = recover(backup_root, &plan.id)
                .map(|r| r.message)
                .unwrap_or_else(|e| format!("Recovery required: {e}"));
            return Err(format!(
                "Apply failed: {error}. {restored}. Backup: {}",
                backup.display()
            ));
        }
        if let Some((root, expected)) = &mut expected_package {
            let relative = Path::new(&c.path)
                .strip_prefix(Path::new(root))
                .map_err(|e| e.to_string())?
                .to_path_buf();
            if let Some(bytes) = &c.after_bytes {
                expected.insert(relative, hash(bytes));
            } else {
                expected.remove(&relative);
            }
        }
    }
    record.status = "applied_pending_persistence".into();
    record.message = "Files applied; inventory/history persistence not yet confirmed".into();
    save_recovery(&backup, &record).map_err(|e| {
        format!("Files applied but recovery journal finalization failed: {e}; recovery required")
    })?;
    Ok(())
}
fn verify_bytes(path: &Path, expected: Option<&[u8]>) -> Result<(), String> {
    match expected {
        Some(bytes) => {
            if fs::read(path).map_err(|e| e.to_string())? != bytes {
                return Err(format!(
                    "Post-write verification failed: {}",
                    path.display()
                ));
            }
        }
        None => {
            if fs::symlink_metadata(path).is_ok() {
                return Err(format!("Deletion verification failed: {}", path.display()));
            }
        }
    }
    Ok(())
}
fn check_current(c: &FileChange) -> Result<(), String> {
    let path = Path::new(&c.path);
    validate_path(path)?;
    let current = if path.exists() {
        Some(hash(&fs::read(path).map_err(|e| e.to_string())?))
    } else {
        None
    };
    if current != c.before_hash {
        return Err(format!(
            "Stale plan: {} changed; create a new preview",
            c.path
        ));
    }
    Ok(())
}
fn write_change(path: &Path, content: Option<&[u8]>) -> Result<(), String> {
    validate_path(path)?;
    match content {
        Some(bytes) => {
            let parent = path.parent().ok_or("Missing parent")?;
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let temp = parent.join(format!(".acm-{}.tmp", uuid::Uuid::new_v4()));
            fs::write(&temp, bytes).map_err(|e| e.to_string())?;
            let result = fs::rename(&temp, path).map_err(|e| e.to_string());
            if result.is_err() {
                let _ = fs::remove_file(&temp);
            }
            result
        }
        None => {
            if path.exists() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_paths_reject_streams_devices_and_case_aliases_match() {
        let t = tempfile::tempdir().unwrap();
        for name in ["config.json:secret", "NUL.txt", "name.", "name "] {
            assert!(validate_path(&t.path().join(name)).is_err(), "{name}");
        }
        assert!(same_location(
            r"C:\Skills\Demo\SKILL.md",
            "c:/skills/demo/skill.md"
        ));
    }
    #[cfg(windows)]
    #[test]
    fn junction_ancestor_is_read_only_and_keeps_original_files() {
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("original");
        let alias = t.path().join("alias");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("SKILL.md"), "fixture").unwrap();
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:ACM_TEST_ALIAS -Target $env:ACM_TEST_TARGET | Out-Null"])
            .env("ACM_TEST_ALIAS", &alias).env("ACM_TEST_TARGET", &target)
            .status().unwrap();
        assert!(status.success());
        assert!(validate_path(&alias.join("SKILL.md")).is_err());
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "fixture"
        );
        // Remove only the junction itself, never recursively traverse it.
        fs::remove_dir(alias).unwrap();
    }
    #[test]
    fn recovery_rejects_mismatched_identity_before_touching_targets() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("config.json");
        fs::write(&path, "before").unwrap();
        let p = plan(
            "test".into(),
            vec![change(path.clone(), Some(b"after".to_vec()), false).unwrap()],
            vec![],
        );
        let root = t.path().join("backups");
        apply(&p, &root).unwrap();
        let dir = root.join(&p.id);
        let mut record = load_recovery(&dir).unwrap();
        record.id = uuid::Uuid::new_v4().to_string();
        save_recovery(&dir, &record).unwrap();
        assert!(recover(&root, &p.id).unwrap_err().contains("identity"));
        assert_eq!(fs::read_to_string(path).unwrap(), "after");
    }
    fn settings(root: &Path) -> Settings {
        Settings {
            home: root.display().to_string(),
            projects: vec![],
            codex_home: Some(root.join(".codex").display().to_string()),
            claude_home: None,
        }
    }
    #[test]
    fn custom_roots_are_shared_by_planning_and_scanning() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        settings.codex_home = Some(t.path().join("custom-codex").display().to_string());
        settings.claude_home = Some(t.path().join("custom-claude").display().to_string());
        let p = plan_mcp(
            &settings,
            "Codex",
            None,
            "install",
            "demo",
            r#"{"command":"example"}"#,
        )
        .unwrap();
        assert_eq!(
            Path::new(&p.changes[0].path),
            settings.codex_root().join("config.toml")
        );
        let source = t.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: example\n---\nBody",
        )
        .unwrap();
        let skill = plan_package(
            &settings,
            &Inventory::default(),
            "install",
            None,
            "demo",
            &source,
            "Claude Code",
        )
        .unwrap();
        assert_eq!(
            Path::new(&skill.changes[0].path),
            settings.claude_root().join("skills/demo/SKILL.md")
        );
        let claude = plan_mcp(
            &settings,
            "Claude Code",
            None,
            "install",
            "demo",
            r#"{"command":"example"}"#,
        )
        .unwrap();
        assert_eq!(
            Path::new(&claude.changes[0].path),
            settings.claude_user_config()
        );
        fs::create_dir_all(settings.codex_root()).unwrap();
        fs::write(
            settings.codex_root().join("config.toml"),
            p.changes[0].after_bytes.as_ref().unwrap(),
        )
        .unwrap();
        let inv = crate::scanner::scan(&settings, &Inventory::default());
        assert!(inv
            .components
            .iter()
            .any(|c| c.name == "demo" && c.agent == "Codex"));
    }
    #[test]
    fn hidden_changes_have_field_indicators() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join(".claude.json");
        fs::write(&path,r#"{"mcpServers":{"demo":{"url":"https://example.com/old?token=old-secret","headers":{"Authorization":"old-secret"}}}}"#).unwrap();
        let p = plan_mcp(&settings(t.path()),"Claude Code",None,"update","demo",r#"{"url":"https://example.com/new?token=new-secret","headers":{"Authorization":"new-secret"}}"#).unwrap();
        let after = p.changes[0].after.as_ref().unwrap();
        assert!(after.contains("mcpServers/demo/url"));
        assert!(after.contains("mcpServers/demo/headers"));
        let serialized = serde_json::to_string(&p).unwrap();
        assert!(!serialized.contains("old-secret"));
        assert!(!serialized.contains("new-secret"));
    }
    #[test]
    fn journal_survives_reopen_and_restores_without_in_memory_plan() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("config");
        let root = t.path().join("backups");
        fs::write(&path, "private-before").unwrap();
        let p = plan(
            "recovery".into(),
            vec![change(path.clone(), Some(b"private-after".to_vec()), true).unwrap()],
            vec![],
        );
        apply(&p, &root).unwrap();
        let id = p.id.clone();
        drop(p);
        let entries = recover_pending(&root).unwrap();
        assert_eq!(entries[0].status, "applied_pending_persistence");
        assert!(!serde_json::to_string(&entries)
            .unwrap()
            .contains("private-before"));
        assert_eq!(recover(&root, &id).unwrap().status, "rolled_back");
        assert!(recover_pending(&root).unwrap().is_empty());
        assert_eq!(fs::read_to_string(path).unwrap(), "private-before");
    }
    #[test]
    fn recovery_does_not_overwrite_external_edits_or_accept_bad_backups() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("config");
        let root = t.path().join("backups");
        fs::write(&path, "before").unwrap();
        let p = plan(
            "recovery".into(),
            vec![change(path.clone(), Some(b"after".to_vec()), false).unwrap()],
            vec![],
        );
        apply(&p, &root).unwrap();
        fs::write(&path, "external").unwrap();
        assert_eq!(recover(&root, &p.id).unwrap().status, "recovery_required");
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        fs::write(&path, "after").unwrap();
        fs::write(root.join(&p.id).join("0.bak"), "tampered").unwrap();
        assert_eq!(recover(&root, &p.id).unwrap().status, "recovery_required");
        assert_eq!(fs::read_to_string(path).unwrap(), "after");
    }
    #[test]
    fn completed_journal_is_not_offered_as_pending_recovery() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("backups");
        let p = plan(
            "completed".into(),
            vec![change(t.path().join("file"), Some(b"new".to_vec()), false).unwrap()],
            vec![],
        );
        apply(&p, &root).unwrap();
        finish_recovery(&root, &p.id).unwrap();
        assert!(recover_pending(&root).unwrap().is_empty());
        assert!(recover(&root, &p.id).is_err());
    }
    #[test]
    fn package_added_after_preview_blocks_apply() {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: test\n---\n",
        )
        .unwrap();
        let p = plan_package(
            &settings(t.path()),
            &Inventory::default(),
            "install",
            None,
            "demo",
            &source,
            "Codex",
        )
        .unwrap();
        let target = t.path().join(".agents/skills/demo");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("external.txt"), "external").unwrap();
        assert!(apply(&p, &t.path().join("backups"))
            .unwrap_err()
            .contains("membership"));
        assert!(!target.join("SKILL.md").exists());
    }
    #[test]
    fn stale_plan_never_overwrites() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("SKILL.md");
        fs::write(&p, "first").unwrap();
        let plan = plan(
            "test".into(),
            vec![change(p.clone(), Some(b"next".to_vec()), false).unwrap()],
            vec![],
        );
        fs::write(&p, "external edit").unwrap();
        assert!(apply(&plan, &t.path().join("backup"))
            .unwrap_err()
            .contains("Stale"));
        assert_eq!(fs::read_to_string(p).unwrap(), "external edit");
    }
    #[test]
    fn full_binary_package_and_utf8_survive() {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("source");
        fs::create_dir_all(source.join("assets")).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: example\ndescription: \u{4e2d}\u{6587}\n---\nBody",
        )
        .unwrap();
        fs::write(source.join("assets/image.bin"), [0, 255, 128]).unwrap();
        let plan = plan_package(
            &settings(t.path()),
            &Inventory::default(),
            "install",
            None,
            "example",
            &source,
            "Codex",
        )
        .unwrap();
        apply(&plan, &t.path().join("backup")).unwrap();
        assert_eq!(
            fs::read(t.path().join(".agents/skills/example/assets/image.bin")).unwrap(),
            [0, 255, 128]
        );
    }
    #[test]
    fn mcp_redacts_preserves_unknown_and_rejects_bad_config() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join(".claude.json");
        fs::write(&p, r#"{"unknown":42,"token":"very-secret"}"#).unwrap();
        let plan = plan_mcp(
            &settings(t.path()),
            "Claude Code",
            None,
            "install",
            "server",
            r#"{"url":"https://example.com/mcp","headers":{"Authorization":"secret"}}"#,
        )
        .unwrap();
        assert!(!serde_json::to_string(&plan)
            .unwrap()
            .contains("very-secret"));
        apply(&plan, &t.path().join("backup")).unwrap();
        let v = read_json(&p).unwrap();
        assert_eq!(v["unknown"], 42);
        assert_eq!(v["mcpServers"]["server"]["type"], "http");
        fs::write(&p, "{broken").unwrap();
        assert!(plan_mcp(
            &settings(t.path()),
            "Claude Code",
            None,
            "install",
            "second",
            r#"{"command":"example"}"#
        )
        .is_err());
    }
    #[test]
    fn rollback_restores_prior_write_when_later_parent_is_file() {
        let t = tempfile::tempdir().unwrap();
        let first = t.path().join("first");
        let obstruction = t.path().join("file");
        fs::write(&first, "before").unwrap();
        fs::write(&obstruction, "blocked").unwrap();
        let p = plan(
            "rollback".into(),
            vec![
                change(first.clone(), Some(b"after".to_vec()), false).unwrap(),
                change(obstruction.join("child"), Some(vec![1]), false).unwrap(),
            ],
            vec![],
        );
        assert!(apply(&p, &t.path().join("backup"))
            .unwrap_err()
            .contains("Rollback: completed"));
        assert_eq!(fs::read_to_string(first).unwrap(), "before");
    }
    #[test]
    fn plugin_owned_skill_cannot_be_removed() {
        let t = tempfile::tempdir().unwrap();
        let p = t
            .path()
            .join(".claude/plugins/cache/example/v1/skills/test");
        fs::create_dir_all(&p).unwrap();
        fs::write(
            p.join("SKILL.md"),
            "---\nname: child\ndescription: owned\n---\n",
        )
        .unwrap();
        let s = settings(t.path());
        let inv = crate::scanner::scan(&s, &Inventory::default());
        let c = inv.components.iter().find(|c| c.name == "child").unwrap();
        assert!(plan_skill(&s, &inv, "uninstall", Some(&c.id), "child", "")
            .unwrap_err()
            .contains("parent"));
    }
    #[test]
    fn mcp_update_keeps_existing_credentials() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join(".claude.json");
        fs::write(&p,r#"{"mcpServers":{"server":{"url":"https://old.example/mcp","headers":{"Authorization":"keep-me"}}}}"#).unwrap();
        let plan = plan_mcp(
            &settings(t.path()),
            "Claude Code",
            None,
            "update",
            "server",
            r#"{"url":"https://new.example/mcp"}"#,
        )
        .unwrap();
        assert!(!serde_json::to_string(&plan).unwrap().contains("keep-me"));
        apply(&plan, &t.path().join("backup")).unwrap();
        assert_eq!(
            read_json(&p).unwrap()["mcpServers"]["server"]["headers"]["Authorization"],
            "keep-me"
        );
    }
    #[test]
    fn config_review_shows_changes_without_secret_values() {
        let t = tempfile::tempdir().unwrap();
        let p=plan_mcp(&settings(t.path()),"Claude Code",None,"install","visible-server",r#"{"url":"https://alice:password@example.com/private?token=secret","headers":{"Authorization":"keep-secret"}}"#).unwrap();
        let review = serde_json::to_string(&p).unwrap();
        assert!(review.contains("visible-server"));
        assert!(review.contains("example.com"));
        for secret in [
            "alice",
            "password",
            "private",
            "token=secret",
            "keep-secret",
        ] {
            assert!(!review.contains(secret))
        }
    }
    #[test]
    fn verification_detects_changed_bytes() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("file");
        fs::write(&p, "actual").unwrap();
        assert!(verify_bytes(&p, Some(b"expected")).is_err());
        assert!(verify_bytes(&p, None).is_err());
    }
    #[test]
    fn registered_plugin_outside_cache_owns_its_skill() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join(".agents/skills/owned");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("SKILL.md"),
            "---\nname: owned\ndescription: plugin child\n---\n",
        )
        .unwrap();
        let registry = t.path().join(".claude/plugins");
        fs::create_dir_all(&registry).unwrap();
        fs::write(
            registry.join("installed_plugins.json"),
            serde_json::to_vec(&json!({"plugins":{"parent@market":[{"installPath":root}]}}))
                .unwrap(),
        )
        .unwrap();
        let s = settings(t.path());
        let inv = crate::scanner::scan(&s, &Inventory::default());
        let child = inv
            .components
            .iter()
            .find(|c| c.kind == "skill" && c.name == "owned")
            .unwrap();
        assert!(plan_skill(&s, &inv, "uninstall", Some(&child.id), "owned", "").is_err());
    }
}
