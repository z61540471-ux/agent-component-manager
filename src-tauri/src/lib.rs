mod export;
mod model;
mod planner;
mod registry;
mod scanner;
mod storage;
use model::*;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tauri::Manager;
struct State {
    db: storage::Storage,
    settings: Settings,
    inventory: Inventory,
    plans: HashMap<String, Plan>,
    data_dir: PathBuf,
    pending_operations: Vec<Operation>,
}
type Shared = Mutex<State>;
#[tauri::command]
fn settings(state: tauri::State<Shared>) -> Result<Settings, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.settings.clone())
}
#[tauri::command]
fn save_settings(value: Settings, state: tauri::State<Shared>) -> Result<(), String> {
    let value = normalize_settings(value)?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.db.save_settings(&value)?;
    s.settings = value;
    s.plans.clear();
    s.inventory = Inventory::default();
    Ok(())
}
fn normalize_settings(mut value: Settings) -> Result<Settings, String> {
    for root in [&mut value.codex_home, &mut value.claude_home] {
        *root = root.take().filter(|path| !path.trim().is_empty());
    }
    if !std::path::Path::new(&value.home).is_absolute()
        || value
            .projects
            .iter()
            .any(|p| !std::path::Path::new(p).is_absolute())
        || [&value.codex_home, &value.claude_home]
            .into_iter()
            .flatten()
            .any(|p| !std::path::Path::new(p).is_absolute())
    {
        return Err("Use absolute home, agent root and project paths".into());
    }
    Ok(value)
}
#[tauri::command]
fn inventory(state: tauri::State<Shared>) -> Result<Inventory, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.inventory.clone())
}
#[tauri::command]
fn export_inventory(destination: String, state: tauri::State<Shared>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    export::write_inventory(&s.inventory, std::path::Path::new(&destination))
}
#[tauri::command]
async fn scan(state: tauri::State<'_, Shared>) -> Result<Inventory, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.plans.clear();
    let next = scanner::scan(&s.settings, &s.inventory);
    s.db.put("inventory", &next)?;
    s.inventory = next.clone();
    Ok(next)
}
#[tauri::command]
fn history(state: tauri::State<Shared>) -> Result<Vec<Operation>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let mut history = match s.db.history() {
        Ok(history) => history,
        Err(error) if !s.pending_operations.is_empty() => {
            let mut pending = s.pending_operations.clone();
            for op in &mut pending {
                op.message
                    .push_str(&format!(". Stored history unavailable: {error}"));
            }
            return Ok(pending);
        }
        Err(error) => return Err(error),
    };
    for pending in &s.pending_operations {
        history.retain(|op| op.id != pending.id);
        history.insert(0, pending.clone());
    }
    Ok(history)
}
#[tauri::command]
async fn market(
    source: String,
    query: String,
    cursor: Option<String>,
    state: tauri::State<'_, Shared>,
) -> Result<MarketResult, String> {
    let query = query.trim().to_string();
    let key = registry::cache_key(&source, &query, cursor.as_deref());
    let result = tauri::async_runtime::spawn_blocking(move || {
        registry::search(&source, &query, cursor.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?;
    let s = state.lock().map_err(|e| e.to_string())?;
    registry::cache_page(&s.db, &key, result)
}
#[tauri::command]
fn preview(
    action: String,
    id: Option<String>,
    name: String,
    content: String,
    state: tauri::State<Shared>,
) -> Result<Plan, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let p = planner::plan_skill(
        &s.settings,
        &s.inventory,
        &action,
        id.as_deref(),
        &name,
        &content,
    )?;
    s.plans.insert(p.id.clone(), p.clone());
    Ok(p)
}
#[tauri::command]
fn preview_package(
    action: String,
    id: Option<String>,
    name: String,
    source_path: String,
    agent: String,
    state: tauri::State<Shared>,
) -> Result<Plan, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;

    let p = planner::plan_package(
        &s.settings,
        &s.inventory,
        &action,
        id.as_deref(),
        &name,
        std::path::Path::new(&source_path),
        &agent,
    )?;
    s.plans.insert(p.id.clone(), p.clone());
    Ok(p)
}
#[tauri::command]
fn preview_mcp(
    action: String,
    id: Option<String>,
    name: String,
    content: String,
    agent: String,
    mut project: Option<String>,
    state: tauri::State<Shared>,
) -> Result<Plan, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(id) = id {
        let c = s
            .inventory
            .components
            .iter()
            .find(|c| c.id == id)
            .ok_or("Rescan: component not found")?;
        if c.kind != "mcp" || c.agent != agent || c.name != name {
            return Err("MCP identity mismatch".into());
        }
        if matches!(c.binding_kind.as_deref(), Some("inherited" | "local"))
            || c.owner_id.is_some()
            || (c.binding_kind.is_none() && c.description.starts_with("Project binding:"))
        {
            return Err("Claude local-scope entries are managed through the Claude CLI; project .mcp.json is a separate registration".into());
        }
        if project.is_none() && c.scope == "project" {
            project = c.project_path.clone();
        }
        if project.is_none() && c.scope == "project" {
            project = s
                .settings
                .projects
                .iter()
                .find(|p| {
                    PathBuf::from(p).join(if agent == "Codex" {
                        ".codex/config.toml"
                    } else {
                        ".mcp.json"
                    }) == PathBuf::from(&c.path)
                })
                .cloned();
            if project.is_none() {
                return Err("Project registration root is no longer configured; rescan".into());
            }
        }
        let expected = if agent == "Codex" {
            project
                .as_ref()
                .map(|p| PathBuf::from(p).join(".codex/config.toml"))
                .unwrap_or_else(|| s.settings.codex_root().join("config.toml"))
        } else {
            project
                .as_ref()
                .map(|p| PathBuf::from(p).join(".mcp.json"))
                .unwrap_or_else(|| s.settings.claude_user_config())
        };
        if std::fs::canonicalize(expected).ok() != std::fs::canonicalize(&c.path).ok() {
            return Err("Selected project does not match the observed registration path".into());
        }
    }
    let p = planner::plan_mcp(
        &s.settings,
        &agent,
        project.as_deref(),
        &action,
        &name,
        &content,
    )?;
    s.plans.insert(p.id.clone(), p.clone());
    Ok(p)
}
#[tauri::command]
fn read_skill(id: String, state: tauri::State<Shared>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let c = s
        .inventory
        .components
        .iter()
        .find(|c| c.id == id && c.kind == "skill")
        .ok_or("Skill not found; rescan")?;
    let meta = std::fs::metadata(&c.path).map_err(|e| e.to_string())?;
    if meta.len() > 1024 * 1024 {
        return Err("Skill manifest exceeds 1 MiB".into());
    }
    std::fs::read_to_string(&c.path).map_err(|e| e.to_string())
}
#[tauri::command]
async fn preview_github(
    repository: String,
    revision: String,
    subdirectory: String,
    name: String,
    action: String,
    id: Option<String>,
    state: tauri::State<'_, Shared>,
) -> Result<Plan, String> {
    let repo = repository.clone();
    let (sha, files) = tauri::async_runtime::spawn_blocking(move || {
        registry::github_package(&repo, &revision, &subdirectory)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let root = s
        .data_dir
        .join("downloads")
        .join(uuid::Uuid::new_v4().to_string());
    for (relative, bytes) in files {
        let path = root.join(relative);
        planner::validate_path(&path)?;
        std::fs::create_dir_all(path.parent().ok_or("Invalid payload path")?)
            .map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    let provenance = serde_json::json!({"source":format!("https://github.com/{repository}/tree/{sha}"),"commit":sha,"fetchedAt":now()});
    std::fs::write(
        root.join(".acm-source.json"),
        serde_json::to_vec_pretty(&provenance).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut p = planner::plan_package(
        &s.settings,
        &s.inventory,
        &action,
        id.as_deref(),
        &name,
        &root,
        "Codex",
    )?;
    p.warnings.push(format!("Pinned source: https://github.com/{repository}/tree/{sha}. GitHub stars refer to the repository, not this skill."));
    s.plans.insert(p.id.clone(), p.clone());
    Ok(p)
}
#[tauri::command]
fn preview_plugin(action: String, name: String, scope: String) -> Result<Plan, String> {
    if !["install", "update", "uninstall", "enable", "disable"].contains(&action.as_str())
        || !["user", "project", "local"].contains(&scope.as_str())
    {
        return Err("Unsupported plugin action/scope".into());
    }
    if name.is_empty()
        || name.starts_with('-')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_@.".contains(c))
    {
        return Err("Use a plugin@marketplace identifier".into());
    }
    Ok(Plan{id:uuid::Uuid::new_v4().to_string(),title:format!("{action} Claude plugin {name}"),changes:vec![],warnings:vec![format!("Manual command: claude plugin {action} {name} --scope {scope}"),"Run from the intended project directory. Command contract verified against installed Claude CLI help. Recheck --help for your CLI version.".into(),"Native plugin manager owns registration/cache/dependencies. This application does not execute the command or promise rollback of its side effects. Review any marketplace command prompt; then rescan.".into()],created_at:now(),package_snapshot:None})
}
// Only the verified Windows implementation may apply confirmed retained plans.
#[tauri::command]
fn apply_plan(
    id: String,
    confirmed: bool,
    state: tauri::State<Shared>,
) -> Result<Operation, String> {
    if !MUTATIONS_VERIFIED {
        return Err("Component writes are disabled pending native verification.".into());
    }
    if !confirmed {
        return Err("Explicit preview confirmation is required".into());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    apply_retained_plan(&mut s, &id)
}

fn apply_retained_plan(s: &mut State, id: &str) -> Result<Operation, String> {
    if !s.plans.contains_key(id) {
        return Err("Plan expired; generate a new preview".into());
    }
    let observed = scanner::scan(&s.settings, &s.inventory);
    if inventory_signature(&observed) != inventory_signature(&s.inventory) {
        s.inventory = observed;
        s.plans.clear();
        return Err(
            "Component content or ownership changed; rescan and create a new preview".into(),
        );
    }
    if observed.issues.iter().any(|issue| {
        issue
            .path
            .replace('\\', "/")
            .to_lowercase()
            .contains("/plugins/")
    }) {
        s.plans.clear();
        return Err(
            "Plugin ownership could not be verified; resolve scan errors before writing".into(),
        );
    }
    let p = s
        .plans
        .remove(id)
        .ok_or("Plan expired; generate a new preview")?;
    let op = Operation {
        id: p.id.clone(),
        title: p.title.clone(),
        status: "started".into(),
        message: "Applying confirmed plan; backup location is recorded in the plan directory"
            .into(),
        at: now(),
    };
    s.db.put(&format!("plan:{}", p.id), &p)?;
    s.db.operation(&op)?;
    let outcome = planner::apply(&p, &s.data_dir.join("backups"));
    finish_operation(s, op, outcome)
}

fn inventory_signature(inventory: &Inventory) -> Vec<(String, String, String, String)> {
    let mut signature: Vec<_> = inventory
        .components
        .iter()
        .map(|component| {
            (
                component.id.clone(),
                component.hash.clone(),
                component.canonical_path.clone(),
                component.effective.clone(),
            )
        })
        .collect();
    signature.sort();
    signature
}

// After filesystem work, persistence failure must never hide the actual outcome.
fn finish_operation(
    s: &mut State,
    mut op: Operation,
    outcome: Result<(), String>,
) -> Result<Operation, String> {
    op.status = if outcome.is_ok() {
        "completed"
    } else {
        "failed"
    }
    .into();
    op.message = outcome.err().unwrap_or_else(|| {
        "Files verified and applied; refresh/restart affected agent as needed".into()
    });
    persist_outcome(s, op)
}

fn persist_outcome(s: &mut State, mut op: Operation) -> Result<Operation, String> {
    let inv = scanner::scan(&s.settings, &s.inventory);
    s.inventory = inv;
    s.plans.clear();
    op.message.push_str(&format!(
        ". Backup/recovery directory: {}",
        s.data_dir.join("backups").join(&op.id).display()
    ));
    let mut warnings = vec![];
    if let Err(error) = s.db.put("inventory", &s.inventory) {
        warnings.push(format!("Inventory persistence failed: {error}"));
    }
    if let Err(error) = s.db.operation(&op) {
        warnings.push(format!("Operation persistence failed: {error}"));
    }
    if warnings.is_empty() && op.status == "completed" {
        if let Err(error) = planner::finish_recovery(&s.data_dir.join("backups"), &op.id) {
            warnings.push(format!("Recovery journal finalization failed: {error}"));
        }
    }
    if !warnings.is_empty() {
        op.status.push_str("_with_warnings");
        op.message.push_str(&format!(
            ". {}. Recovery evidence retained; inspect history before making more changes.",
            warnings.join("; ")
        ));
        s.pending_operations.retain(|previous| previous.id != op.id);
        s.pending_operations.push(op.clone());
    }
    Ok(op)
}
const MUTATIONS_VERIFIED: bool = cfg!(windows);
#[tauri::command]
fn capabilities() -> serde_json::Value {
    serde_json::json!({"mutationsEnabled":MUTATIONS_VERIFIED,"skillPackages":true,"mcpConfiguration":true,"githubPinnedImport":true,"plugins":"manual-cli","claudeSkillEnablement":false})
}
#[tauri::command]
fn recovery(state: tauri::State<Shared>) -> Result<Vec<planner::RecoveryRecord>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    planner::recovery_records(&s.data_dir.join("backups"))
}
#[tauri::command]
fn recover_operation(
    id: String,
    confirmed: bool,
    state: tauri::State<Shared>,
) -> Result<Operation, String> {
    if !MUTATIONS_VERIFIED || !confirmed {
        return Err("Verified writes and explicit recovery confirmation are required".into());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let record = planner::recovery_records(&s.data_dir.join("backups"))?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or("Recovery record not found")?;
    let outcome = planner::recover(&s.data_dir.join("backups"), &id);
    let op = match outcome {
        Ok(record) => Operation {
        id: record.id,
        title: record.title,
        status: record.status,
        message: record.message,
        at: record.at,
        },
        Err(error) => Operation {
            id: record.id, title: record.title,
            status: "recovery_required".into(),
            message: format!("Recovery did not finish: {error}. Inspect backups and refreshed inventory before retrying"),
            at: now(),
        },
    };
    s.pending_operations.retain(|previous| previous.id != op.id);
    persist_outcome(&mut s, op)
}
#[tauri::command]
async fn mcp_detail(name: String, version: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || registry::mcp_detail(&name, &version))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = storage::Storage::open(&dir.join("inventory.sqlite"))
                .map_err(std::io::Error::other)?;
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            let settings = db.get("settings").unwrap_or(Settings {
                home,
                projects: vec![],
                codex_home: None,
                claude_home: None,
            });
            let mut inventory: Inventory = db.get("inventory").unwrap_or_default();
            let pending_operations = match planner::recover_pending(&dir.join("backups")) {
                Ok(operations) => operations,
                Err(error) => {
                    inventory.issues.push(Issue {
                        path: dir.join("backups").display().to_string(),
                        message: format!(
                            "Recovery journal inspection failed; no files changed: {error}"
                        ),
                    });
                    vec![]
                }
            };
            if !pending_operations.is_empty() {
                inventory = scanner::scan(&settings, &inventory);
            }
            app.manage(Mutex::new(State {
                db,
                settings,
                inventory,
                plans: HashMap::new(),
                data_dir: dir,
                pending_operations,
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings,
            save_settings,
            inventory,
            export_inventory,
            scan,
            history,
            market,
            preview,
            preview_package,
            preview_mcp,
            read_skill,
            preview_github,
            preview_plugin,
            capabilities,
            mcp_detail,
            apply_plan,
            recovery,
            recover_operation
        ])
        .run(tauri::generate_context!())
        .expect("Desktop application failed to start");
}

#[cfg(test)]
mod orchestration_tests {
    use super::*;

    fn fixture_state(root: &std::path::Path) -> State {
        State {
            db: storage::Storage::open(&root.join("inventory.sqlite")).unwrap(),
            settings: Settings {
                home: root.join("home").display().to_string(),
                projects: vec![],
                codex_home: Some(root.join("home/.codex").display().to_string()),
                claude_home: Some(root.join("home/.claude").display().to_string()),
            },
            inventory: Inventory::default(),
            plans: HashMap::new(),
            data_dir: root.join("app-data"),
            pending_operations: vec![],
        }
    }

    fn install_plan(s: &State) -> Plan {
        planner::plan_skill(
            &s.settings,
            &s.inventory,
            "install",
            None,
            "fixture",
            "---\nname: fixture\ndescription: fixture description\n---\nHello",
        )
        .unwrap()
    }

    #[test]
    fn settings_normalize_blank_roots_and_reject_relative_overrides() {
        let root = tempfile::tempdir().unwrap();
        let mut value = fixture_state(root.path()).settings;
        value.codex_home = Some(" \t".into());
        value.claude_home = Some(String::new());
        let mut value = normalize_settings(value).unwrap();
        assert!(value.codex_home.is_none());
        assert!(value.claude_home.is_none());
        value.codex_home = Some("relative/codex".into());
        assert!(normalize_settings(value.clone()).is_err());
        value.codex_home = None;
        value.claude_home = Some("relative/claude".into());
        assert!(normalize_settings(value).is_err());
    }

    #[test]
    fn database_failure_after_apply_preserves_result_and_refreshes_memory() {
        let root = tempfile::tempdir().unwrap();
        let mut s = fixture_state(root.path());
        let plan = install_plan(&s);
        let id = plan.id.clone();
        let target = plan.changes[0].path.clone();
        s.plans.insert(id.clone(), plan.clone());
        let mut other = plan.clone();
        other.id = uuid::Uuid::new_v4().to_string();
        s.plans.insert(other.id.clone(), other);
        // Fail only the post-write inventory persistence, not the intent write.
        s.db.0.execute_batch("CREATE TRIGGER fail_snapshot BEFORE INSERT ON state WHEN NEW.key='inventory' BEGIN SELECT RAISE(FAIL,'injected snapshot failure'); END;").unwrap();
        let op = apply_retained_plan(&mut s, &id).unwrap();
        assert_eq!(op.status, "completed_with_warnings");
        assert!(std::path::Path::new(&target).exists());
        assert!(s.plans.is_empty());
        assert!(s.inventory.components.iter().any(|c| c.name == "fixture"));
        assert!(op.message.contains("Files verified and applied"));
        assert!(op.message.contains("injected snapshot failure"));
        assert!(op.message.contains(&id));
        assert_eq!(s.pending_operations[0].id, id);
        assert!(planner::recover_pending(&s.data_dir.join("backups"))
            .unwrap()
            .iter()
            .any(|op| op.id == id && op.status == "applied_pending_persistence"));
    }

    #[test]
    fn history_failure_after_apply_does_not_report_generic_error() {
        let root = tempfile::tempdir().unwrap();
        let mut s = fixture_state(root.path());
        let plan = install_plan(&s);
        let id = plan.id.clone();
        s.plans.insert(id.clone(), plan);
        s.db.0.execute_batch("CREATE TRIGGER fail_result BEFORE INSERT ON operations WHEN json_extract(NEW.value,'$.status') != 'started' BEGIN SELECT RAISE(FAIL,'injected history failure'); END;").unwrap();
        let op = apply_retained_plan(&mut s, &id).unwrap();
        assert_eq!(op.status, "completed_with_warnings");
        assert!(op.message.contains("injected history failure"));
        assert!(s.plans.is_empty());
        assert!(s
            .db
            .get::<Inventory>("inventory")
            .unwrap()
            .components
            .iter()
            .any(|c| c.name == "fixture"));
    }

    #[test]
    fn intent_persistence_failure_prevents_component_writes() {
        let root = tempfile::tempdir().unwrap();
        let mut s = fixture_state(root.path());
        let plan = install_plan(&s);
        let id = plan.id.clone();
        let target = plan.changes[0].path.clone();
        s.plans.insert(id.clone(), plan);
        s.db.0.execute_batch("PRAGMA query_only=ON;").unwrap();
        assert!(apply_retained_plan(&mut s, &id).is_err());
        assert!(!std::path::Path::new(&target).exists());
    }

    #[test]
    fn newly_registered_plugin_invalidates_a_retained_preview() {
        let root = tempfile::tempdir().unwrap();
        let mut s = fixture_state(root.path());
        let plan = install_plan(&s);
        let target = plan.changes[0].path.clone();
        let id = plan.id.clone();
        s.plans.insert(id.clone(), plan);
        let registry = PathBuf::from(&s.settings.home).join(".claude/plugins");
        std::fs::create_dir_all(&registry).unwrap();
        let owner = PathBuf::from(&target).parent().unwrap().to_path_buf();
        std::fs::write(registry.join("installed_plugins.json"), serde_json::to_vec(
            &serde_json::json!({"plugins":{"fixture@market":[{"installPath":owner,"scope":"user"}]}})
        ).unwrap()).unwrap();
        let error = apply_retained_plan(&mut s, &id).unwrap_err();
        assert!(error.contains("ownership changed"));
        assert!(s.plans.is_empty());
        assert!(!std::path::Path::new(&target).exists());
    }

    #[test]
    fn skill_lifecycle_applies_and_records_every_confirmed_plan() {
        let root = tempfile::tempdir().unwrap();
        let mut state = fixture_state(root.path());
        let install = install_plan(&state);
        let target = install.changes[0].path.clone();
        let id = install.id.clone();
        state.plans.insert(id.clone(), install);
        assert_eq!(
            apply_retained_plan(&mut state, &id).unwrap().status,
            "completed"
        );
        for action in ["update", "uninstall"] {
            let component = state
                .inventory
                .components
                .iter()
                .find(|c| c.name == "fixture")
                .unwrap();
            let plan = planner::plan_skill(
                &state.settings,
                &state.inventory,
                action,
                Some(&component.id),
                "fixture",
                "---\nname: fixture\ndescription: updated\n---\nChanged content",
            )
            .unwrap();
            let id = plan.id.clone();
            state.plans.insert(id.clone(), plan);
            assert_eq!(
                apply_retained_plan(&mut state, &id).unwrap().status,
                "completed"
            );
            if action == "update" {
                assert!(std::fs::read_to_string(&target)
                    .unwrap()
                    .contains("Changed content"));
            }
        }
        assert!(!std::path::Path::new(&target).exists());
        assert!(state.inventory.components.is_empty());
        assert_eq!(state.db.history().unwrap().len(), 3);
        assert!(planner::recover_pending(&state.data_dir.join("backups"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn project_mcp_lifecycle_preserves_other_configuration() {
        let root = tempfile::tempdir().unwrap();
        let mut state = fixture_state(root.path());
        let project = root.path().join("project");
        std::fs::create_dir_all(project.join(".codex")).unwrap();
        let config = project.join(".codex/config.toml");
        std::fs::write(&config, "model = 'fixture-model'\n").unwrap();
        let project = project.display().to_string();
        state.settings.projects.push(project.clone());
        state.inventory = scanner::scan(&state.settings, &state.inventory);
        for action in ["install", "disable", "enable", "update", "uninstall"] {
            let plan = planner::plan_mcp(
                &state.settings,
                "Codex",
                Some(&project),
                action,
                "fixture-server",
                r#"{"command":"fixture-runtime","args":["never-executed"]}"#,
            )
            .unwrap();
            let id = plan.id.clone();
            state.plans.insert(id.clone(), plan);
            assert_eq!(
                apply_retained_plan(&mut state, &id).unwrap().status,
                "completed"
            );
            let content: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
            assert_eq!(content["model"].as_str(), Some("fixture-model"));
            if action == "disable" {
                assert_eq!(
                    content["mcp_servers"]["fixture-server"]["enabled"].as_bool(),
                    Some(false)
                );
            }
        }
        assert!(state.inventory.components.is_empty());
        assert_eq!(state.db.history().unwrap().len(), 5);
    }
}
