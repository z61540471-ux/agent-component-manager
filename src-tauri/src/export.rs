use crate::{model::Inventory, planner};
use std::{fs::OpenOptions, io::Write, path::Path};

// Export the retained normalized snapshot, never configuration or backup bytes.
pub fn write_inventory(inventory: &Inventory, destination: &Path) -> Result<String, String> {
    if inventory.scanned_at == 0 {
        return Err("Scan the local inventory before exporting".into());
    }
    planner::validate_path(destination)?;
    if !destination
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("json"))
    {
        return Err("Choose a destination with a .json extension".into());
    }
    let parent = destination
        .parent()
        .ok_or("Export destination has no parent directory")?;
    if !parent.is_dir() {
        return Err("The export parent directory must already exist".into());
    }
    let mut bytes = serde_json::to_vec_pretty(inventory).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    // Exclusive creation preserves existing files, including Agent configs.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| {
            format!(
                "Cannot create export {} (existing files are never overwritten): {e}",
                destination.display()
            )
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        return Err(format!("Export could not finish: {error}. An incomplete file may remain at {}; choose another destination after inspecting it", destination.display()));
    }
    Ok(destination.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::Settings, scanner};
    use std::fs;

    #[test]
    fn exports_unicode_snapshot_without_mcp_credentials() {
        let root = tempfile::tempdir().unwrap();
        let settings = Settings {
            home: root.path().display().to_string(),
            projects: vec![],
            codex_home: Some(root.path().join("codex").display().to_string()),
            claude_home: Some(root.path().join("claude").display().to_string()),
        };
        let skill = settings.claude_root().join("skills/example");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: example\ndescription: \u{4e2d}\u{6587}\u{6587}\u{6863}\n---\nHello",
        )
        .unwrap();
        fs::write(settings.claude_user_config(), r#"{"mcpServers":{"fixture":{"url":"https://example.invalid/mcp?token=url-secret","headers":{"Authorization":"header-secret"},"env":{"TOKEN":"env-secret"},"args":["argument-secret"]}}}"#).unwrap();
        let inventory = scanner::scan(&settings, &Inventory::default());
        assert!(inventory.components.iter().any(|c| c.kind == "mcp"));
        let destination = root.path().join("\u{6e05}\u{5355}.json");
        assert_eq!(
            write_inventory(&inventory, &destination).unwrap(),
            destination.display().to_string()
        );
        let contents = fs::read_to_string(&destination).unwrap();
        assert!(contents.contains("\u{4e2d}\u{6587}\u{6587}\u{6863}"));
        for secret in [
            "url-secret",
            "header-secret",
            "env-secret",
            "argument-secret",
        ] {
            assert!(!contents.contains(secret));
        }
        let exported: Inventory = serde_json::from_str(&contents).unwrap();
        assert_eq!(exported.scanned_at, inventory.scanned_at);
        assert_eq!(exported.components.len(), inventory.components.len());
    }

    #[test]
    fn refuses_unscanned_invalid_and_existing_destinations() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("inventory.json");
        assert!(write_inventory(&Inventory::default(), &destination)
            .unwrap_err()
            .contains("Scan"));
        assert!(!destination.exists());
        let inventory = Inventory {
            scanned_at: 1,
            ..Default::default()
        };
        for invalid in [
            root.path().join("inventory.txt"),
            root.path().join("missing/inventory.json"),
            "relative.json".into(),
            root.path().join("../traversal.json"),
        ] {
            assert!(write_inventory(&inventory, &invalid).is_err());
        }
        fs::write(&destination, "keep original").unwrap();
        assert!(write_inventory(&inventory, &destination)
            .unwrap_err()
            .contains("never overwritten"));
        assert_eq!(fs::read_to_string(destination).unwrap(), "keep original");
    }
}
