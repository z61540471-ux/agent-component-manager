#[path = "codex_plugins.rs"]
mod codex_plugins;
use crate::model::*;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

fn issue(inv: &mut Inventory, path: &Path, message: impl ToString) {
    inv.issues.push(Issue {
        path: path.display().to_string(),
        message: message.to_string(),
    });
}
fn package_bytes(root: &Path) -> Result<Vec<u8>, String> {
    fn collect(
        root: &Path,
        current: &Path,
        out: &mut Vec<u8>,
        count: &mut usize,
    ) -> Result<(), String> {
        let mut entries = fs::read_dir(current)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if entry.file_name() == ".git" {
                continue;
            }
            *count += 1;
            if *count > 10000 {
                return Err("Skill package exceeds 10000 files".into());
            }
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err("Nested links: package equality cannot be established".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("Nested reparse point: package equality unknown".into());
                }
            }
            if meta.is_dir() {
                collect(root, &path, out, count)?
            } else if meta.is_file() {
                if meta.len() > 16 * 1024 * 1024 || out.len() > 64 * 1024 * 1024 {
                    return Err("Skill package exceeds hashing budget".into());
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                out.extend_from_slice(&(relative.len() as u64).to_le_bytes());
                out.extend_from_slice(relative.as_bytes());
                out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                out.extend_from_slice(&bytes);
            }
        }
        Ok(())
    }
    let mut bytes = vec![];
    let mut count = 0;
    collect(root, root, &mut bytes, &mut count)?;
    Ok(bytes)
}
fn record(
    inv: &mut Inventory,
    path: &Path,
    kind: &str,
    name: String,
    description: String,
    agent: &str,
    scope: &str,
    bytes: &[u8],
    version: Option<String>,
    effective: &str,
    source: Option<String>,
) {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    inv.components.push(Component {
        id: hash(format!("{}:{agent}:{scope}:{name}", path.display()).as_bytes()),
        kind: kind.into(),
        name,
        description,
        agent: agent.into(),
        scope: scope.into(),
        path: path.display().to_string(),
        canonical_path: canonical.display().to_string(),
        hash: hash(bytes),
        version,
        status: vec![],
        effective: effective.into(),
        source,
        project_path: None,
        binding_kind: Some(
            if kind == "mcp" {
                "registration"
            } else {
                "manifest"
            }
            .into(),
        ),
        owner_id: None,
        last_seen: inv.scanned_at,
    });
}
// Binding identity is independent of mutable manifest prose and observed state.
fn bind(c: &mut Component, project: Option<&str>, kind: &str, owner: Option<&str>) {
    c.project_path = project.map(String::from);
    c.binding_kind = Some(kind.into());
    c.owner_id = owner.map(String::from);
    c.id = hash(
        &serde_json::to_vec(&(
            &c.path,
            &c.agent,
            &c.scope,
            &c.name,
            &c.project_path,
            kind,
            &c.owner_id,
        ))
        .unwrap(),
    );
}
fn skill_tree(
    inv: &mut Inventory,
    root: &Path,
    agent: &str,
    scope: &str,
    seen: &mut HashSet<PathBuf>,
    depth: usize,
) {
    if !root.exists() {
        return;
    }
    if depth > 8 {
        issue(inv, root, "Directory depth limit reached");
        return;
    }
    let canonical = match fs::canonicalize(root) {
        Ok(v) => v,
        Err(e) => {
            issue(inv, root, e);
            return;
        }
    };
    if !seen.insert(canonical.clone()) {
        issue(inv, root, "Link traversal cycle");
        return;
    }
    let manifest = root.join("SKILL.md");
    if manifest.is_file() {
        match fs::read_to_string(&manifest) {
            Ok(text) => {
                let normalized = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
                let front = normalized
                    .strip_prefix("---\n")
                    .and_then(|s| s.split_once("\n---"))
                    .map(|(s, _)| s);
                match front
                    .ok_or_else(|| "Missing YAML frontmatter".to_string())
                    .and_then(|f| serde_yaml::from_str::<Value>(f).map_err(|e| e.to_string()))
                {
                    Ok(v) => {
                        let name = v["name"]
                            .as_str()
                            .unwrap_or_else(|| {
                                root.file_name()
                                    .and_then(|v| v.to_str())
                                    .unwrap_or("unknown")
                            })
                            .to_string();
                        let desc = v["description"].as_str().unwrap_or("").to_string();
                        let effective = if root.components().any(|c| c.as_os_str() == "cache") {
                            "cached"
                        } else {
                            "discovered"
                        };
                        let bytes = match package_bytes(root) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                issue(inv, root, e);
                                format!("unverified:{}", root.display()).into_bytes()
                            }
                        };
                        let provenance = fs::read_to_string(root.join(".acm-source.json"))
                            .ok()
                            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
                        let source = provenance
                            .as_ref()
                            .and_then(|v| v["source"].as_str())
                            .map(String::from);
                        record(
                            inv,
                            &manifest,
                            "skill",
                            name,
                            desc,
                            agent,
                            scope,
                            &bytes,
                            v["version"].as_str().map(String::from),
                            effective,
                            source,
                        );
                    }
                    Err(e) => issue(inv, &manifest, e),
                }
            }
            Err(e) => issue(inv, &manifest, e),
        }
        seen.remove(&canonical);
        return;
    }
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(e) => {
                        if ["node_modules", ".git", "target"]
                            .iter()
                            .any(|n| e.file_name() == *n)
                        {
                            continue;
                        }
                        if e.path().is_dir() {
                            skill_tree(inv, &e.path(), agent, scope, seen, depth + 1)
                        } else if e.file_type().map(|v| v.is_symlink()).unwrap_or(false)
                            && !e.path().exists()
                        {
                            issue(inv, &e.path(), "Broken symbolic link")
                        }
                    }
                    Err(e) => issue(inv, root, e),
                }
            }
        }
        Err(e) => issue(inv, root, e),
    }
    seen.remove(&canonical);
}
fn json_file(inv: &mut Inventory, path: &Path) -> Option<Value> {
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
    {
        Ok(v) => Some(v),
        Err(e) => {
            issue(inv, path, e);
            None
        }
    }
}
fn mcp(inv: &mut Inventory, path: &Path, agent: &str, scope: &str, toml_format: bool) {
    if !path.exists() {
        return;
    }
    let value = if toml_format {
        match fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| {
                toml::from_str::<toml::Value>(&s).map_err(|_| {
                    "Invalid TOML configuration (content hidden to protect credentials)".to_string()
                })
            })
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                issue(inv, path, e);
                return;
            }
        }
    } else {
        match json_file(inv, path) {
            Some(v) => v,
            None => return,
        }
    };
    let key = if toml_format {
        "mcp_servers"
    } else {
        "mcpServers"
    };
    if let Some(entries) = value[key].as_object() {
        for (name, config) in entries {
            let enabled = config["enabled"].as_bool() != Some(false)
                && config["disabled"].as_bool() != Some(true);
            // Deliberately omit arguments and environment values: they may contain credentials.
            record(
                inv,
                path,
                "mcp",
                name.clone(),
                "MCP configuration (credentials and arguments hidden)".into(),
                agent,
                scope,
                &serde_json::to_vec(config).unwrap_or_default(),
                None,
                if enabled { "configured" } else { "disabled" },
                None,
            );
        }
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let a = fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let b = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .replace('\\', "/")
            .eq_ignore_ascii_case(&b.to_string_lossy().replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
fn plugin_enabled(inv: &mut Inventory, path: &Path, name: &str) -> Option<bool> {
    json_file(inv, path).and_then(|v| v["enabledPlugins"][name].as_bool())
}
fn project_plugin(inv: &mut Inventory, root: &Path) {
    let path = root.join(".claude-plugin/plugin.json");
    if let Some(v) = json_file(inv, &path) {
        record(
            inv,
            root,
            "plugin",
            v["name"].as_str().unwrap_or("unknown").into(),
            v["description"]
                .as_str()
                .unwrap_or("Project plugin manifest; registration and runtime unknown")
                .into(),
            "Claude Code",
            "project",
            &serde_json::to_vec(&v).unwrap_or_default(),
            v["version"].as_str().map(String::from),
            "discovered",
            None,
        );
        let plugin = inv.components.last_mut().unwrap();
        bind(plugin, root.to_str(), "manifest", None);
        let owner = plugin.id.clone();
        let before = inv.components.len();
        skill_tree(
            inv,
            &root.join("skills"),
            "Claude Code",
            "project",
            &mut HashSet::new(),
            0,
        );
        for child in &mut inv.components[before..] {
            child.status.push("managed".into());
            bind(child, root.to_str(), "manifest", Some(&owner));
        }
    }
}

fn claude_catalog(
    inv: &mut Inventory,
    marketplace_root: &Path,
    path: &Path,
    project: Option<&str>,
) {
    let Some(value) = json_file(inv, path) else {
        return;
    };
    let Some(entries) = value["plugins"].as_array() else {
        issue(inv, path, "Claude marketplace catalog has no plugins array");
        return;
    };
    let Some(root_canonical) = fs::canonicalize(marketplace_root).ok() else {
        issue(
            inv,
            marketplace_root,
            "Claude marketplace root is unavailable",
        );
        return;
    };
    for entry in entries {
        let Some(name) = entry["name"].as_str() else {
            issue(inv, path, "Claude marketplace entry has no name");
            continue;
        };
        let source = &entry["source"];
        let relative = source.as_str().or_else(|| source["path"].as_str());
        let Some(relative) = relative.filter(|p| p.starts_with("./")) else {
            issue(
                inv,
                path,
                format!("Claude marketplace entry {name} has an unsupported local source"),
            );
            continue;
        };
        let candidate = marketplace_root.join(&relative[2..]);
        let Ok(root) = fs::canonicalize(&candidate) else {
            issue(
                inv,
                &candidate,
                format!("Claude marketplace entry {name} source is missing"),
            );
            continue;
        };
        if !root.starts_with(&root_canonical) {
            issue(
                inv,
                &candidate,
                format!("Claude marketplace entry {name} escapes its root"),
            );
            continue;
        }
        let manifest_path = if root.join(".claude-plugin/plugin.json").is_file() {
            root.join(".claude-plugin/plugin.json")
        } else {
            root.join("plugin.json")
        };
        let Some(manifest) = json_file(inv, &manifest_path) else {
            issue(
                inv,
                &root,
                format!("Claude marketplace entry {name} has no plugin manifest"),
            );
            continue;
        };
        record(
            inv,
            &root,
            "plugin",
            manifest["name"].as_str().unwrap_or(name).into(),
            manifest["description"]
                .as_str()
                .unwrap_or("Local marketplace plugin; runtime unknown")
                .into(),
            "Claude Code",
            "catalog",
            &serde_json::to_vec(&manifest).unwrap_or_default(),
            manifest["version"].as_str().map(String::from),
            "catalog",
            Some(path.display().to_string()),
        );
        let plugin = inv.components.last_mut().unwrap();
        bind(plugin, project, "catalog", None);
        let owner = plugin.id.clone();
        let before = inv.components.len();
        skill_tree(
            inv,
            &root.join("skills"),
            "Claude Code",
            "catalog",
            &mut HashSet::new(),
            0,
        );
        for child in &mut inv.components[before..] {
            child.status.push("managed".into());
            child.effective = "catalog".into();
            bind(child, project, "catalog", Some(&owner));
        }
    }
}
fn listed(value: &Value, field: &str, name: &str) -> bool {
    value[field]
        .as_array()
        .map(|a| a.iter().any(|v| v.as_str() == Some(name)))
        .unwrap_or(false)
}
fn claude_contexts(inv: &mut Inventory, settings: &Settings) {
    let path = settings.claude_user_config();
    let Some(user) = json_file(inv, &path) else {
        return;
    };
    let global: Vec<_> = inv
        .components
        .iter()
        .filter(|c| c.kind == "mcp" && c.agent == "Claude Code" && c.scope == "global")
        .cloned()
        .collect();
    for project in &settings.projects {
        let root = Path::new(project);
        let context = user["projects"]
            .as_object()
            .and_then(|entries| entries.iter().find(|(p, _)| same_path(Path::new(p), root)))
            .map(|(_, v)| v)
            .unwrap_or(&Value::Null);
        let local = context["mcpServers"].as_object();
        let shared_path = root.join(".mcp.json");
        let shared_names: HashSet<_> = inv
            .components
            .iter()
            .filter(|c| {
                c.kind == "mcp"
                    && c.agent == "Claude Code"
                    && same_path(Path::new(&c.path), &shared_path)
            })
            .map(|c| c.name.clone())
            .collect();
        for c in inv.components.iter_mut().filter(|c| {
            c.kind == "mcp"
                && c.agent == "Claude Code"
                && same_path(Path::new(&c.path), &shared_path)
        }) {
            if listed(context, "disabledMcpServers", &c.name) {
                c.effective = "disabled".into();
            }
            if listed(context, "disabledMcpjsonServers", &c.name) {
                c.status.push("approval-denied".into());
            }
            if local.map(|m| m.contains_key(&c.name)).unwrap_or(false) {
                c.status.push("shadowed".into());
            }
        }
        for original in &global {
            let mut c = original.clone();
            c.description = "Inherited user MCP registration (runtime unknown)".into();
            c.scope = "project".into();
            bind(&mut c, Some(project), "inherited", None);
            if listed(context, "disabledMcpServers", &c.name) {
                c.effective = "disabled".into();
            }
            if shared_names.contains(&c.name)
                || local.map(|m| m.contains_key(&c.name)).unwrap_or(false)
            {
                c.status.push("shadowed".into());
            }
            inv.components.push(c);
        }
        if let Some(entries) = local {
            for (name, cfg) in entries {
                record(
                    inv,
                    &path,
                    "mcp",
                    name.clone(),
                    "Local MCP registration (runtime unknown)".into(),
                    "Claude Code",
                    "project",
                    &serde_json::to_vec(cfg).unwrap_or_default(),
                    None,
                    if listed(context, "disabledMcpServers", name) {
                        "disabled"
                    } else {
                        "configured"
                    },
                    None,
                );
                bind(
                    inv.components.last_mut().unwrap(),
                    Some(project),
                    "local",
                    None,
                );
            }
        }
    }
}
fn plugins(inv: &mut Inventory, root: &Path, agent: &str, scope: &str, settings: &Settings) {
    let path = root.join("plugins/installed_plugins.json");
    let Some(v) = json_file(inv, &path) else {
        return;
    };
    let Some(entries) = v["plugins"].as_object() else {
        return;
    };
    for (name, installations) in entries {
        let Some(list) = installations.as_array() else {
            continue;
        };
        for install in list {
            let scope = install["scope"].as_str().unwrap_or(scope);
            let project = install["projectPath"].as_str();
            if matches!(scope, "project" | "local")
                && !project
                    .map(|p| {
                        settings
                            .projects
                            .iter()
                            .any(|selected| same_path(Path::new(p), Path::new(selected)))
                    })
                    .unwrap_or(false)
            {
                continue;
            }
            let Some(install_path) = install["installPath"].as_str().map(PathBuf::from) else {
                issue(inv, &path, "Plugin registration has no installPath");
                continue;
            };
            let metadata = json_file(inv, &install_path.join(".claude-plugin/plugin.json"))
                .unwrap_or(Value::Null);
            // Retain the original user registration as well as each selected project view.
            let mut contexts = vec![(
                scope,
                project.filter(|_| matches!(scope, "project" | "local")),
                "registration",
            )];
            if matches!(scope, "global" | "user") {
                contexts.extend(
                    settings
                        .projects
                        .iter()
                        .map(|p| ("project", Some(p.as_str()), "inherited")),
                );
            }
            for (context_scope, project, binding) in contexts {
                let mut enabled = plugin_enabled(inv, &root.join("settings.json"), name);
                if let Some(project) = project {
                    for file in ["settings.json", "settings.local.json"] {
                        if let Some(value) = plugin_enabled(
                            inv,
                            &Path::new(project).join(".claude").join(file),
                            name,
                        ) {
                            enabled = Some(value);
                        }
                    }
                }
                let effective = if !install_path.is_dir() {
                    "broken"
                } else if enabled == Some(false) {
                    "disabled"
                } else {
                    "installed"
                };
                record(
                    inv,
                    &install_path,
                    "plugin",
                    name.clone(),
                    metadata["description"]
                        .as_str()
                        .unwrap_or("Registered plugin; runtime unknown")
                        .into(),
                    agent,
                    context_scope,
                    &serde_json::to_vec(&metadata).unwrap_or_default(),
                    install["version"].as_str().map(String::from),
                    effective,
                    None,
                );
                let plugin = inv.components.last_mut().unwrap();
                bind(plugin, project, binding, None);
                let owner = plugin.id.clone();
                let before = inv.components.len();
                skill_tree(
                    inv,
                    &install_path.join("skills"),
                    agent,
                    context_scope,
                    &mut HashSet::new(),
                    0,
                );
                for child in &mut inv.components[before..] {
                    child.status.push("managed".into());
                    // Enabled configuration does not establish runtime loading.
                    child.effective = if effective == "disabled" {
                        "disabled"
                    } else {
                        "discovered"
                    }
                    .into();
                    bind(child, project, binding, Some(&owner));
                }
            }
        }
    }
}
pub fn classify(inv: &mut Inventory, previous: &Inventory) {
    fn path_key(path: &str) -> String {
        #[cfg(windows)]
        {
            path.replace('\\', "/")
                .trim_start_matches("//?/")
                .to_ascii_lowercase()
        }
        #[cfg(not(windows))]
        {
            path.to_string()
        }
    }
    let mut hashes: HashMap<(String, String), HashSet<String>> = HashMap::new();
    let mut names: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for c in &inv.components {
        hashes
            .entry((c.kind.clone(), c.hash.clone()))
            .or_default()
            .insert(path_key(&c.canonical_path));
        names
            .entry((c.kind.clone(), c.name.clone()))
            .or_default()
            .insert(c.hash.clone());
    }
    let mut aliases: HashMap<String, HashSet<String>> = HashMap::new();
    for c in &inv.components {
        aliases
            .entry(path_key(&c.canonical_path))
            .or_default()
            .insert(path_key(&c.path));
    }
    for c in &mut inv.components {
        if hashes[&(c.kind.clone(), c.hash.clone())].len() > 1 {
            c.status.push("duplicate".into())
        }
        if names[&(c.kind.clone(), c.name.clone())].len() > 1 {
            c.status.push("conflict".into())
        }
        if c.kind == "skill" && aliases[&path_key(&c.canonical_path)].len() > 1 {
            c.status.push("alias".into())
        }
        if previous
            .components
            .iter()
            .any(|p| p.id == c.id && p.hash != c.hash)
        {
            c.status.push("drift".into())
        }
        c.status.sort();
        c.status.dedup();
    }
}

/// Keep disappearance visible across scans without resurrecting a component in
/// the live inventory.  A successful uninstall should leave the component list
/// empty, while a deleted or moved file that was present in the last snapshot
/// still needs an actionable diagnostic for the user.
fn report_stale(previous: &Inventory, current: &mut Inventory) {
    let present: HashSet<String> = current
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect();
    for component in &previous.components {
        if present.contains(&component.id) {
            continue;
        }
        let path = Path::new(&component.path);
        if !path.exists() {
            current.issues.push(Issue {
                path: component.path.clone(),
                message: format!(
                    "Stale inventory entry: {} {} was not found during this scan",
                    component.kind, component.name
                ),
            });
        }
    }
}
pub fn scan(settings: &Settings, previous: &Inventory) -> Inventory {
    let mut inv = Inventory {
        scanned_at: now(),
        ..Default::default()
    };
    let home = Path::new(&settings.home);
    let roots = vec![
        (settings.codex_root(), "Codex"),
        (settings.claude_root(), "Claude Code"),
    ];
    for (root, agent) in roots {
        skill_tree(
            &mut inv,
            &root.join("skills"),
            agent,
            "global",
            &mut HashSet::new(),
            0,
        );
        if agent != "Codex" {
            skill_tree(
                &mut inv,
                &root.join("plugins/cache"),
                agent,
                "cache",
                &mut HashSet::new(),
                0,
            );
        }
        if agent == "Codex" {
            mcp(&mut inv, &root.join("config.toml"), agent, "global", true);
        }
        if agent == "Claude Code" {
            plugins(&mut inv, &root, agent, "global", settings)
        }
    }
    skill_tree(
        &mut inv,
        &home.join(".agents/skills"),
        "Codex",
        "global",
        &mut HashSet::new(),
        0,
    );
    mcp(
        &mut inv,
        &settings.claude_user_config(),
        "Claude Code",
        "global",
        false,
    );
    for project in &settings.projects {
        let root = Path::new(project);
        if !root.is_dir() {
            issue(&mut inv, root, "Project root is missing");
            continue;
        }
        let project_start = inv.components.len();
        project_plugin(&mut inv, root);
        claude_catalog(
            &mut inv,
            root,
            &root.join(".claude-plugin/marketplace.json"),
            Some(project),
        );
        for (dir, agent) in [
            (".agents", "Shared"),
            (".codex", "Codex"),
            (".claude", "Claude Code"),
        ] {
            skill_tree(
                &mut inv,
                &root.join(dir).join("skills"),
                agent,
                "project",
                &mut HashSet::new(),
                0,
            )
        }
        mcp(
            &mut inv,
            &root.join(".mcp.json"),
            "Claude Code",
            "project",
            false,
        );
        mcp(
            &mut inv,
            &root.join(".codex/config.toml"),
            "Codex",
            "project",
            true,
        );
        for component in &mut inv.components[project_start..] {
            if component.project_path.is_none() {
                let kind = component
                    .binding_kind
                    .clone()
                    .unwrap_or_else(|| "manifest".into());
                bind(component, Some(project), &kind, None);
            }
        }
    }
    codex_plugins::scan(&mut inv, settings);
    claude_contexts(&mut inv, settings);
    reconcile_bindings(&mut inv, settings);
    classify(&mut inv, previous);
    report_stale(previous, &mut inv);
    inv
}

fn reconcile_bindings(inv: &mut Inventory, settings: &Settings) {
    for c in &mut inv.components {
        if c.kind != "skill" {
            continue;
        }
        let path = c.path.replace('\\', "/");
        if c.scope == "cache"
            || path.contains("/plugins/")
            || path.contains("/.system/")
            || path.contains("/synced/")
        {
            c.status.push("managed".into());
        }
        if c.agent == "Codex" || c.agent == "Shared" {
            // Project bindings are governed by that project's config.toml;
            // falling back to the user config here would incorrectly mark a
            // global disablement on every selected project (and would miss a
            // project-only disablement entirely).
            let config_path = c
                .project_path
                .as_deref()
                .map(|project| Path::new(project).join(".codex/config.toml"))
                .unwrap_or_else(|| settings.codex_root().join("config.toml"));
            let config = fs::read_to_string(config_path)
                .ok()
                .and_then(|s| toml::from_str::<toml::Value>(&s).ok());
            if let Some(entries) = config
                .as_ref()
                .and_then(|v| v.get("skills"))
                .and_then(|v| v.get("config"))
                .and_then(|v| v.as_array())
            {
                for entry in entries {
                    if entry
                        .get("path")
                        .and_then(|v| v.as_str())
                        .map(|p| same_path(Path::new(p), Path::new(&c.path)))
                        .unwrap_or(false)
                        && entry.get("enabled").and_then(|v| v.as_bool()) == Some(false)
                    {
                        c.effective = "disabled".into();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(root: &Path) -> Settings {
        Settings {
            home: root.display().to_string(),
            projects: vec![],
            codex_home: Some(root.join("custom-codex").display().to_string()),
            claude_home: Some(root.join("custom-claude").display().to_string()),
        }
    }
    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }
    #[test]
    fn inherited_plugin_bindings_preserve_user_state_and_stable_child_identity() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        let first = t.path().join("first");
        let second = t.path().join("second");
        settings.projects = vec![first.display().to_string(), second.display().to_string()];
        let package = settings.claude_root().join("plugins/cache/example/v1");
        let manifest = package.join(".claude-plugin/plugin.json");
        write(
            &manifest,
            &serde_json::json!({"name":"different-manifest-name", "description":"initial prose"}),
        );
        let skill = package.join("skills/child/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(
            &skill,
            "---\nname: child\ndescription: first description\n---\nBody",
        )
        .unwrap();
        write(
            &settings
                .claude_root()
                .join("plugins/installed_plugins.json"),
            &serde_json::json!({"version":2,"plugins":{"example@market":[{"scope":"user","installPath":package}]}}),
        );
        write(
            &settings.claude_root().join("settings.json"),
            &serde_json::json!({"enabledPlugins":{"example@market":false}}),
        );
        write(
            &first.join(".claude/settings.json"),
            &serde_json::json!({"enabledPlugins":{"example@market":true}}),
        );
        write(
            &second.join(".claude/settings.json"),
            &serde_json::json!({"enabledPlugins":{"example@market":true}}),
        );
        write(
            &second.join(".claude/settings.local.json"),
            &serde_json::json!({"enabledPlugins":{"example@market":false}}),
        );
        let initial = scan(&settings, &Inventory::default());
        let plugins: Vec<_> = initial
            .components
            .iter()
            .filter(|c| c.kind == "plugin")
            .collect();
        assert_eq!(plugins.len(), 3);
        assert_eq!(
            plugins
                .iter()
                .find(|c| c.project_path.is_none())
                .unwrap()
                .effective,
            "disabled"
        );
        for (project, expected) in [
            (&settings.projects[0], "installed"),
            (&settings.projects[1], "disabled"),
        ] {
            let plugin = plugins
                .iter()
                .find(|c| c.project_path.as_ref() == Some(project))
                .unwrap();
            assert_eq!(plugin.binding_kind.as_deref(), Some("inherited"));
            assert_eq!(plugin.effective, expected);
            let child = initial
                .components
                .iter()
                .find(|c| c.owner_id.as_ref() == Some(&plugin.id))
                .unwrap();
            assert_eq!(child.project_path.as_ref(), Some(project));
            assert_eq!(
                child.effective,
                if expected == "disabled" {
                    "disabled"
                } else {
                    "discovered"
                }
            );
            assert!(!child
                .status
                .iter()
                .any(|s| s == "alias" || s == "duplicate"));
            assert!(crate::planner::plan_skill(
                &settings,
                &initial,
                "uninstall",
                Some(&child.id),
                &child.name,
                ""
            )
            .is_err());
        }
        write(
            &manifest,
            &serde_json::json!({"name":"different-manifest-name", "description":"changed prose"}),
        );
        fs::write(
            &skill,
            "---\nname: child\ndescription: changed description\n---\nBody",
        )
        .unwrap();
        let changed = scan(&settings, &initial);
        assert_eq!(
            initial
                .components
                .iter()
                .map(|c| &c.id)
                .collect::<HashSet<_>>(),
            changed
                .components
                .iter()
                .map(|c| &c.id)
                .collect::<HashSet<_>>()
        );
        assert_eq!(
            changed
                .components
                .iter()
                .map(|c| &c.id)
                .collect::<HashSet<_>>()
                .len(),
            changed.components.len()
        );
        assert!(changed
            .components
            .iter()
            .filter(|c| c.owner_id.is_some())
            .all(|c| c.status.iter().any(|s| s == "drift")));
    }
    #[test]
    fn explicit_ownership_blocks_payload_without_parent_row() {
        let t = tempfile::tempdir().unwrap();
        let settings = settings(t.path());
        let skill = settings.codex_root().join("skills/example/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(
            &skill,
            "---\nname: example\ndescription: standalone\n---\nBody",
        )
        .unwrap();
        let mut inventory = scan(&settings, &Inventory::default());
        let id = inventory.components[0].id.clone();
        assert!(crate::planner::plan_skill(
            &settings,
            &inventory,
            "uninstall",
            Some(&id),
            "example",
            ""
        )
        .is_ok());
        inventory.components[0].owner_id = Some("unavailable-parent".into());
        assert!(crate::planner::plan_skill(
            &settings,
            &inventory,
            "uninstall",
            Some(&id),
            "example",
            ""
        )
        .is_err());
        inventory.components[0].owner_id = None;
        inventory.components[0].binding_kind = Some("inherited".into());
        assert!(crate::planner::plan_skill(
            &settings,
            &inventory,
            "uninstall",
            Some(&id),
            "example",
            ""
        )
        .is_err());
        let mut old = serde_json::to_value(&inventory.components[0]).unwrap();
        for key in ["ownerId", "bindingKind", "projectPath"] {
            old.as_object_mut().unwrap().remove(key);
        }
        let old: Component = serde_json::from_value(old).unwrap();
        assert!(old.owner_id.is_none() && old.binding_kind.is_none() && old.project_path.is_none());
    }
    #[test]
    fn custom_roots_and_selected_claude_contexts_are_distinct() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        let project = t.path().join("selected");
        let ignored = t.path().join("unselected");
        fs::create_dir_all(&project).unwrap();
        settings.projects.push(project.display().to_string());
        fs::create_dir_all(settings.codex_root()).unwrap();
        fs::write(
            settings.codex_root().join("config.toml"),
            "[mcp_servers.custom]\ncommand = 'example'\n",
        )
        .unwrap();
        write(
            &settings.claude_user_config(),
            &serde_json::json!({"mcpServers":{"shared":{"command":"user"}},"projects":{project.display().to_string():{"mcpServers":{"shared":{"command":"local"}},"disabledMcpServers":["shared"],"disabledMcpjsonServers":["approval"]},ignored.display().to_string():{"mcpServers":{"ignored":{"command":"never"}}}}}),
        );
        write(
            &project.join(".mcp.json"),
            &serde_json::json!({"mcpServers":{"shared":{"command":"project"},"approval":{"command":"approval"}}}),
        );
        let inv = scan(&settings, &Inventory::default());
        assert!(inv.components.iter().any(|c| c.name == "custom"
            && Path::new(&c.path) == settings.codex_root().join("config.toml")));
        assert!(!inv.components.iter().any(|c| c.name == "ignored"));
        let shared: Vec<_> = inv
            .components
            .iter()
            .filter(|c| c.name == "shared")
            .collect();
        assert_eq!(shared.len(), 4);
        assert_eq!(
            shared.iter().filter(|c| c.effective == "disabled").count(),
            3
        );
        assert_eq!(
            shared.iter().map(|c| &c.id).collect::<HashSet<_>>().len(),
            4
        );
        assert_eq!(
            shared
                .iter()
                .filter(|c| c.status.contains(&"shadowed".into()))
                .count(),
            2
        );
        let approval = inv
            .components
            .iter()
            .find(|c| c.name == "approval")
            .unwrap();
        assert_eq!(approval.effective, "configured");
        assert!(approval.status.contains(&"approval-denied".into()));
    }
    #[test]
    fn registered_project_plugins_are_filtered_and_project_manifests_are_observations() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        let project = t.path().join("selected");
        let package = t.path().join("package");
        let ignored = t.path().join("unselected");
        settings.projects.push(project.display().to_string());
        write(
            &project.join(".claude-plugin/plugin.json"),
            &serde_json::json!({"name":"development"}),
        );
        write(
            &project.join("catalog-plugin/.claude-plugin/plugin.json"),
            &serde_json::json!({"name":"catalog-plugin","version":"local"}),
        );
        write(
            &project.join(".claude-plugin/marketplace.json"),
            &serde_json::json!({"name":"local","plugins":[{"name":"catalog-plugin","source":{"source":"local","path":"./catalog-plugin"}}]}),
        );
        write(
            &package.join(".claude-plugin/plugin.json"),
            &serde_json::json!({"name":"manifest-name"}),
        );
        write(
            &settings
                .claude_root()
                .join("plugins/installed_plugins.json"),
            &serde_json::json!({"version":2,"plugins":{"entry@market":[{"installPath":package,"scope":"project","projectPath":project}],"ignored@market":[{"installPath":package,"scope":"project","projectPath":ignored}]}}),
        );
        write(
            &project.join(".claude/settings.json"),
            &serde_json::json!({"enabledPlugins":{"entry@market":false}}),
        );
        fs::create_dir_all(package.join("skills/child")).unwrap();
        fs::write(
            package.join("skills/child/SKILL.md"),
            "---\nname: child\n---\nBody",
        )
        .unwrap();
        let inv = scan(&settings, &Inventory::default());
        let child = inv.components.iter().find(|c| c.name == "child").unwrap();
        assert_eq!(child.effective, "disabled");
        assert!(child.status.iter().any(|s| s == "managed"));
        assert!(!inv.components.iter().any(|c| c.name == "ignored@market"));
        assert_eq!(
            inv.components
                .iter()
                .find(|c| c.name == "entry@market")
                .unwrap()
                .effective,
            "disabled"
        );
        assert_eq!(
            inv.components
                .iter()
                .find(|c| c.name == "development")
                .unwrap()
                .effective,
            "discovered"
        );
        let catalog = inv
            .components
            .iter()
            .find(|c| c.name == "catalog-plugin")
            .unwrap();
        assert_eq!(catalog.scope, "catalog");
        assert_eq!(
            catalog.project_path.as_deref(),
            Some(project.to_str().unwrap())
        );
    }

    #[test]
    fn claude_catalog_rejects_paths_outside_marketplace_root() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        let project = t.path().join("selected");
        let outside = t.path().join("outside");
        settings.projects = vec![project.display().to_string()];
        write(
            &outside.join(".claude-plugin/plugin.json"),
            &serde_json::json!({"name":"outside-plugin"}),
        );
        write(
            &project.join(".claude-plugin/marketplace.json"),
            &serde_json::json!({"name":"local","plugins":[{"name":"outside-plugin","source":{"source":"local","path":"../outside"}}]}),
        );
        let inv = scan(&settings, &Inventory::default());
        assert!(!inv.components.iter().any(|c| c.name == "outside-plugin"));
        assert!(inv
            .issues
            .iter()
            .any(|i| i.message.contains("escapes its root")));
    }

    #[test]
    fn project_skills_are_scanned_and_project_codex_bindings_classify_conflicts() {
        let t = tempfile::tempdir().unwrap();
        let mut settings = settings(t.path());
        let first = t.path().join("project-one");
        let second = t.path().join("project-two");
        settings.projects = vec![first.display().to_string(), second.display().to_string()];

        for (project, body) in [(&first, "one"), (&second, "two")] {
            let skill = project.join(".codex/skills/shared/SKILL.md");
            fs::create_dir_all(skill.parent().unwrap()).unwrap();
            fs::write(
                &skill,
                format!("---\nname: shared\ndescription: project skill\n---\n{body}"),
            )
            .unwrap();
        }
        let disabled_path = first.join(".codex/skills/shared/SKILL.md");
        fs::write(
            first.join(".codex/config.toml"),
            format!(
                "[[skills.config]]\npath = '{}'\nenabled = false\n",
                disabled_path.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();

        let inv = scan(&settings, &Inventory::default());
        let project_skills: Vec<_> = inv
            .components
            .iter()
            .filter(|c| c.kind == "skill" && c.name == "shared")
            .collect();
        assert_eq!(project_skills.len(), 2);
        assert!(project_skills.iter().all(|c| c.scope == "project"));
        assert!(project_skills.iter().all(|c| c.project_path.is_some()));
        assert!(project_skills
            .iter()
            .all(|c| c.status.contains(&"conflict".into())));
        assert_eq!(
            project_skills
                .iter()
                .find(|c| c.project_path.as_deref() == Some(first.to_str().unwrap()))
                .unwrap()
                .effective,
            "disabled"
        );
        assert_eq!(
            project_skills
                .iter()
                .find(|c| c.project_path.as_deref() == Some(second.to_str().unwrap()))
                .unwrap()
                .effective,
            "discovered"
        );
    }

    #[test]
    fn parses_unicode_and_classifies_copies() {
        let t = tempfile::tempdir().unwrap();
        for name in ["one", "two"] {
            let p = t.path().join(".agents/skills").join(name);
            fs::create_dir_all(&p).unwrap();
            fs::write(
                p.join("SKILL.md"),
                "---\nname: 文档\ndescription: 中文描述\n---\nBody",
            )
            .unwrap();
        }
        let inv = scan(
            &Settings {
                home: t.path().display().to_string(),
                projects: vec![],
                codex_home: Some(t.path().join(".codex").display().to_string()),
                claude_home: None,
            },
            &Inventory::default(),
        );
        assert_eq!(inv.components.len(), 2);
        assert!(inv
            .components
            .iter()
            .all(|c| c.status.contains(&"duplicate".into())));
        assert_eq!(inv.components[0].description, "中文描述");
    }

    #[test]
    fn missing_previous_component_is_reported_as_stale_issue() {
        let t = tempfile::tempdir().unwrap();
        let settings = settings(t.path());
        let missing = t.path().join(".agents/skills/removed/SKILL.md");
        let previous = Inventory {
            components: vec![Component {
                id: "removed-id".into(),
                kind: "skill".into(),
                name: "removed".into(),
                description: String::new(),
                agent: "Codex".into(),
                scope: "global".into(),
                path: missing.display().to_string(),
                canonical_path: missing.display().to_string(),
                hash: "old-hash".into(),
                version: None,
                status: vec![],
                effective: "discovered".into(),
                source: None,
                project_path: None,
                binding_kind: Some("manifest".into()),
                owner_id: None,
                last_seen: 1,
            }],
            issues: vec![],
            scanned_at: 1,
        };
        let inventory = scan(&settings, &previous);
        assert!(inventory.issues.iter().any(|issue| {
            issue.path == missing.display().to_string() && issue.message.contains("Stale inventory")
        }));
    }
    #[test]
    fn malformed_file_does_not_abort_scan() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join(".codex")).unwrap();
        fs::write(t.path().join(".codex/config.toml"), "[broken").unwrap();
        let inv = scan(
            &Settings {
                home: t.path().display().to_string(),
                projects: vec![],
                codex_home: Some(t.path().join(".codex").display().to_string()),
                claude_home: None,
            },
            &Inventory::default(),
        );
        assert_eq!(inv.issues.len(), 1);
    }
}
