use super::*;
use std::collections::BTreeMap;

fn declarations(inv: &mut Inventory, path: &Path) -> BTreeMap<String, Value> {
    if !path.exists() {
        return BTreeMap::new();
    }
    let parsed = fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            toml::from_str::<toml::Value>(&text)
                .map_err(|_| "Invalid Codex plugin configuration (content hidden)".into())
        });
    match parsed {
        Ok(value) => value
            .get("plugins")
            .and_then(|v| v.as_table())
            .map(|table| {
                table
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            serde_json::to_value(value).unwrap_or(Value::Null),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(error) => {
            if !inv.issues.iter().any(|item| Path::new(&item.path) == path) {
                issue(inv, path, error);
            }
            BTreeMap::new()
        }
    }
}

fn registration(
    inv: &mut Inventory,
    path: &Path,
    name: &str,
    value: &Value,
    project: Option<&str>,
    inherited: bool,
) {
    if !value.is_object() {
        issue(
            inv,
            path,
            "Unsupported Codex plugin setting; expected a table",
        );
        return;
    }
    record(inv, path, "plugin", name.into(),
        "Declared Codex plugin settings; package selection, project trust and runtime loading are unverified".into(),
        "Codex", if project.is_some() { "project" } else { "global" },
        &serde_json::to_vec(value).unwrap_or_default(), None,
        if value["enabled"].as_bool() == Some(false) { "disabled" } else { "configured" }, None);
    bind(
        inv.components.last_mut().unwrap(),
        project,
        if inherited {
            "inherited"
        } else {
            "registration"
        },
        None,
    );
}

pub(super) fn scan(inv: &mut Inventory, settings: &Settings) {
    let root = settings.codex_root();
    let config = root.join("config.toml");
    let user = declarations(inv, &config);
    for (name, value) in &user {
        registration(inv, &config, name, value, None, false);
    }
    for project in &settings.projects {
        let path = Path::new(project).join(".codex/config.toml");
        let local = declarations(inv, &path);
        for (name, value) in &user {
            if !local.contains_key(name) {
                registration(inv, &config, name, value, Some(project), true);
            }
        }
        for (name, value) in &local {
            // A project table overrides fields it declares, not omitted user settings.
            let mut merged = user
                .get(name)
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if let (Some(base), Some(overrides)) = (merged.as_object_mut(), value.as_object()) {
                base.extend(overrides.clone());
            } else {
                merged = value.clone();
            }
            registration(inv, &path, name, &merged, Some(project), false);
        }
    }
    let cache = root.join("plugins/cache");
    walk(
        inv,
        &cache,
        &cache,
        0,
        &mut HashSet::new(),
        &mut 0,
        "cache",
        None,
    );
    // Codex also supports project-local plugin caches. Keep these separate
    // from the user cache so inventory scope and project bindings remain
    // accurate when a project vendors its own plugin package.
    for project in &settings.projects {
        catalog(
            inv,
            Path::new(project),
            Path::new(project).join(".agents/plugins/marketplace.json"),
            Some(project),
        );
        let cache = Path::new(project).join(".codex/plugins/cache");
        walk(
            inv,
            &cache,
            &cache,
            0,
            &mut HashSet::new(),
            &mut 0,
            "project",
            Some(project),
        );
    }
    catalog(
        inv,
        Path::new(&settings.home),
        Path::new(&settings.home).join(".agents/plugins/marketplace.json"),
        None,
    );
}

fn catalog(inv: &mut Inventory, marketplace_root: &Path, path: PathBuf, project: Option<&str>) {
    if !path.is_file() {
        return;
    }
    let value = match json_file(inv, &path) {
        Some(value) => value,
        None => return,
    };
    let Some(entries) = value["plugins"].as_array() else {
        issue(inv, &path, "Codex marketplace catalog has no plugins array");
        return;
    };
    for entry in entries {
        let Some(name) = entry["name"].as_str() else {
            issue(inv, &path, "Codex marketplace entry has no name");
            continue;
        };
        let source = &entry["source"];
        let relative = source.as_str().or_else(|| source["path"].as_str());
        let Some(relative) = relative.filter(|p| p.starts_with("./")) else {
            issue(
                inv,
                &path,
                format!("Codex marketplace entry {name} has an unsupported local source"),
            );
            continue;
        };
        let root = marketplace_root.join(&relative[2..]);
        let canonical_root = match fs::canonicalize(&root) {
            Ok(root)
                if root.starts_with(fs::canonicalize(marketplace_root).unwrap_or_default()) =>
            {
                root
            }
            _ => {
                issue(
                    inv,
                    &path,
                    format!("Codex marketplace entry {name} escapes its root"),
                );
                continue;
            }
        };
        let manifest = if canonical_root.join("plugin.json").is_file() {
            canonical_root.join("plugin.json")
        } else {
            canonical_root.join(".codex-plugin/plugin.json")
        };
        let Some(manifest) = json_file(inv, &manifest) else {
            issue(
                inv,
                &root,
                format!("Codex marketplace entry {name} has no plugin manifest"),
            );
            continue;
        };
        package(
            inv,
            marketplace_root,
            &canonical_root,
            &manifest,
            canonical_root.join("plugin.json").is_file(),
            "catalog",
            project,
        );
        if let Some(component) = inv.components.last_mut() {
            component.source = Some(path.display().to_string());
        }
    }
}

fn walk(
    inv: &mut Inventory,
    cache: &Path,
    root: &Path,
    depth: usize,
    seen: &mut HashSet<PathBuf>,
    count: &mut usize,
    scope: &str,
    project: Option<&str>,
) {
    if !root.exists() {
        return;
    }
    if depth > 6 || *count >= 2000 {
        issue(inv, root, "Plugin cache traversal limit reached");
        return;
    }
    *count += 1;
    let canonical = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(e) => {
            issue(inv, root, e);
            return;
        }
    };
    if !seen.insert(canonical) {
        return;
    }
    for relative in [
        "plugin.json",
        ".codex-plugin/plugin.json",
        ".claude-plugin/plugin.json",
    ] {
        let manifest_path = root.join(relative);
        if !manifest_path.is_file() {
            continue;
        }
        if let Some(manifest) = json_file(inv, &manifest_path) {
            package(
                inv,
                cache,
                root,
                &manifest,
                relative == "plugin.json",
                scope,
                project,
            );
        }
        return;
    }
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) if entry.path().is_dir() => walk(
                        inv,
                        cache,
                        &entry.path(),
                        depth + 1,
                        seen,
                        count,
                        scope,
                        project,
                    ),
                    Err(e) => issue(inv, root, e),
                    _ => (),
                }
            }
        }
        Err(e) => issue(inv, root, e),
    }
}

fn resource(root: &Path, relative: &str) -> Option<PathBuf> {
    if !relative.starts_with("./") || relative.contains('\\') || relative.contains(':') {
        return None;
    }
    if relative.split('/').skip(1).any(|p| p == ".." || p == ".") {
        return None;
    }
    let tail = &relative[2..];
    if Path::new(tail).has_root() {
        return None;
    }
    let path = root.join(tail);
    // A declared resource must stay within the package, including linked paths.
    if path.exists()
        && !fs::canonicalize(&path)
            .ok()?
            .starts_with(fs::canonicalize(root).ok()?)
    {
        return None;
    }
    Some(path)
}

fn package(
    inv: &mut Inventory,
    cache: &Path,
    root: &Path,
    manifest: &Value,
    portable: bool,
    scope: &str,
    project: Option<&str>,
) {
    let Some(name) = manifest["name"].as_str() else {
        issue(inv, root, "Plugin manifest has no name");
        return;
    };
    let cache_parts: Vec<_> = root
        .strip_prefix(cache)
        .unwrap_or(root)
        .components()
        .collect();
    let identity = if cache_parts.len() == 3 {
        format!(
            "{}@{}",
            cache_parts[1].as_os_str().to_string_lossy(),
            cache_parts[0].as_os_str().to_string_lossy()
        )
    } else {
        name.into()
    };
    record(
        inv,
        root,
        "plugin",
        identity,
        manifest["description"]
            .as_str()
            .unwrap_or("Observed Codex plugin cache; active version unknown")
            .into(),
        "Codex",
        scope,
        &serde_json::to_vec(manifest).unwrap_or_default(),
        manifest["version"].as_str().map(String::from),
        "cached",
        None,
    );
    bind(
        inv.components.last_mut().unwrap(),
        project,
        "manifest",
        None,
    );
    let owner = inv.components.last().unwrap().id.clone();
    let start = inv.components.len();
    let overlay = &manifest["extensions"]["com.openai"];
    let compatibility = if portable && !overlay.is_object() {
        json_file(inv, &root.join(".codex-plugin/plugin.json"))
    } else {
        None
    };
    let settings = if portable {
        compatibility.as_ref().unwrap_or(overlay)
    } else {
        manifest
    };
    if !portable && !settings["skills"].is_null() && !settings["skills"].is_string() {
        issue(
            inv,
            root,
            "Unsupported plugin skills declaration; only relative directory strings are scanned",
        );
    }
    let skills = if portable {
        Some("./skills/")
    } else {
        settings["skills"].as_str().or(Some("./skills/"))
    };
    if let Some(relative) = skills {
        if let Some(path) = resource(root, relative) {
            skill_tree(inv, &path, "Codex", scope, &mut HashSet::new(), 0);
        } else {
            issue(
                inv,
                root,
                "Plugin skills path escapes package or is unsupported",
            );
        }
    }
    let declared_mcp = settings["mcpServers"].as_str();
    if !settings["mcpServers"].is_null() && declared_mcp.is_none() {
        issue(
            inv,
            root,
            "Unsupported plugin MCP declaration; inline mappings are not scanned",
        );
    }
    let relative = declared_mcp.unwrap_or(if portable {
        "./mcp.json"
    } else {
        "./.mcp.json"
    });
    if let Some(path) = resource(root, relative) {
        mcp(inv, &path, "Codex", scope, false);
    } else {
        issue(
            inv,
            root,
            "Plugin MCP path escapes package or is unsupported",
        );
    }
    for child in &mut inv.components[start..] {
        child.status.push("managed".into());
        child.effective = if scope == "catalog" {
            "catalog"
        } else {
            "cached"
        }
        .into();
        bind(child, project, "manifest", Some(&owner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, value: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    #[test]
    fn portable_and_legacy_cache_children_are_owned_and_config_is_not_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("codex");
        let project = temp.path().join("project");
        let settings = Settings {
            home: temp.path().display().to_string(),
            projects: vec![project.display().to_string()],
            codex_home: Some(root.display().to_string()),
            claude_home: None,
        };
        write(
            &root.join("config.toml"),
            "[plugins.'example@market']\nenabled = true\n",
        );
        write(
            &project.join(".codex/config.toml"),
            "[plugins.'example@market']\nenabled = false\n",
        );
        let portable = root.join("plugins/cache/market/example/2");
        write(
            &portable.join("plugin.json"),
            r#"{"name":"example","version":"2"}"#,
        );
        write(
            &portable.join("skills/child/SKILL.md"),
            "---\nname: child\ndescription: fixture\n---\nHello",
        );
        write(
            &portable.join("mcp.json"),
            r#"{"mcpServers":{"remote":{"url":"https://example.invalid","headers":{"Authorization":"fixture-secret"}}}}"#,
        );
        let legacy = root.join("plugins/cache/market/legacy/1");
        write(
            &legacy.join(".codex-plugin/plugin.json"),
            r#"{"name":"legacy","version":"1","skills":"./workflows"}"#,
        );
        write(
            &legacy.join("workflows/child/SKILL.md"),
            "---\nname: legacy-child\ndescription: fixture\n---\nHello",
        );
        let project_plugin = project.join(".codex/plugins/cache/local/project/1");
        write(
            &project_plugin.join("plugin.json"),
            r#"{"name":"project-plugin","version":"1"}"#,
        );
        let catalog_plugin = project.join("plugins/catalog-plugin");
        write(
            &catalog_plugin.join("plugin.json"),
            r#"{"name":"catalog-plugin","version":"local"}"#,
        );
        write(
            &catalog_plugin.join("skills/catalog-skill/SKILL.md"),
            "---\nname: catalog-skill\ndescription: fixture\n---\nHello",
        );
        write(
            &project.join(".agents/plugins/marketplace.json"),
            r#"{"name":"local-repo","plugins":[{"name":"catalog-plugin","source":{"source":"local","path":"./plugins/catalog-plugin"}}]}"#,
        );
        let inventory = super::super::scan(&settings, &Inventory::default());
        assert!(inventory
            .components
            .iter()
            .any(|c| c.name == "example@market"
                && c.scope == "global"
                && c.effective == "configured"));
        assert!(inventory
            .components
            .iter()
            .any(|c| c.name == "example@market"
                && c.scope == "project"
                && c.effective == "disabled"));
        let children: Vec<_> = inventory
            .components
            .iter()
            .filter(|c| c.kind != "plugin" && c.scope == "cache")
            .collect();
        assert_eq!(children.len(), 3);
        assert!(children.iter().all(|c| c.owner_id.is_some()
            && c.effective == "cached"
            && c.status.contains(&"managed".into())));
        assert!(inventory.components.iter().any(|c| {
            c.kind == "plugin"
                && c.name == "project-plugin"
                && c.scope == "project"
                && c.project_path.as_deref() == Some(project.to_str().unwrap())
        }));
        assert!(inventory.components.iter().any(|c| {
            c.kind == "skill"
                && c.name == "catalog-skill"
                && c.scope == "catalog"
                && c.effective == "catalog"
                && c.owner_id.is_some()
        }));
        assert!(inventory.components.iter().any(|c| {
            c.kind == "plugin"
                && c.name == "catalog-plugin"
                && c.scope == "catalog"
                && c.project_path.as_deref() == Some(project.to_str().unwrap())
                && c.source
                    .as_deref()
                    .unwrap_or("")
                    .ends_with("marketplace.json")
        }));
        assert!(!serde_json::to_string(&inventory)
            .unwrap()
            .contains("fixture-secret"));
    }
    #[test]
    fn package_paths_reject_parent_and_absolute_resources() {
        let root = tempfile::tempdir().unwrap();
        assert!(resource(root.path(), "./../external").is_none());
        assert!(resource(root.path(), "C:/external").is_none());
        assert!(resource(root.path(), ".//external").is_none());
        assert!(resource(root.path(), "./safe/skills").is_some());
    }
}
