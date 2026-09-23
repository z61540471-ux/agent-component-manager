use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub home: String,
    pub projects: Vec<String>,
    #[serde(default)]
    pub codex_home: Option<String>,
    #[serde(default)]
    pub claude_home: Option<String>,
}
impl Settings {
    pub fn codex_root(&self) -> std::path::PathBuf {
        self.codex_root_with_env(std::env::var_os("CODEX_HOME"))
    }
    fn codex_root_with_env(&self, environment: Option<std::ffi::OsString>) -> std::path::PathBuf {
        self.codex_home
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| {
                environment
                    .filter(|p| !p.is_empty())
                    .map(std::path::PathBuf::from)
            })
            .unwrap_or_else(|| std::path::Path::new(&self.home).join(".codex"))
    }
    pub fn claude_root(&self) -> std::path::PathBuf {
        self.claude_home
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::Path::new(&self.home).join(".claude"))
    }
    // This user configuration is separate from Claude's skills/plugin directory.
    pub fn claude_user_config(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.home).join(".claude.json")
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub agent: String,
    pub scope: String,
    pub path: String,
    pub canonical_path: String,
    pub hash: String,
    pub version: Option<String>,
    pub status: Vec<String>,
    pub effective: String,
    pub source: Option<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub binding_kind: Option<String>,
    #[serde(default)]
    pub owner_id: Option<String>,
    pub last_seen: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Issue {
    pub path: String,
    pub message: String,
}
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub components: Vec<Component>,
    pub issues: Vec<Issue>,
    pub scanned_at: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub before_hash: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    #[serde(skip)]
    pub before_bytes: Option<Vec<u8>>,
    #[serde(skip)]
    pub after_bytes: Option<Vec<u8>>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub changes: Vec<FileChange>,
    pub warnings: Vec<String>,
    pub created_at: u64,
    #[serde(skip)]
    pub package_snapshot: Option<(
        String,
        std::collections::BTreeMap<std::path::PathBuf, String>,
    )>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub title: String,
    pub status: String,
    pub message: String,
    pub at: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketItem {
    pub name: String,
    pub description: String,
    pub source: String,
    pub url: String,
    pub version: Option<String>,
    pub stars: Option<u64>,
    pub fetched_at: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarketResult {
    pub items: Vec<MarketItem>,
    pub stale: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn legacy_settings_and_root_precedence() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"home":"fixture-home","projects":[]}"#).unwrap();
        assert!(settings.codex_home.is_none());
        assert_eq!(
            settings.codex_root_with_env(None),
            Path::new("fixture-home").join(".codex")
        );
        assert_eq!(
            settings.codex_root_with_env(Some("environment-root".into())),
            Path::new("environment-root")
        );
        settings.codex_home = Some("explicit-root".into());
        assert_eq!(
            settings.codex_root_with_env(Some("environment-root".into())),
            Path::new("explicit-root")
        );
        settings.claude_home = Some("claude-custom".into());
        assert_eq!(settings.claude_root(), Path::new("claude-custom"));
        assert_eq!(
            settings.claude_user_config(),
            Path::new("fixture-home").join(".claude.json")
        );
        let value = serde_json::to_value(settings).unwrap();
        assert_eq!(value["codexHome"], "explicit-root");
        assert_eq!(value["claudeHome"], "claude-custom");
    }
}
