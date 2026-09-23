use crate::model::*;
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
pub struct Storage(pub Connection);
impl Storage {
    pub fn open(path: &Path) -> Result<Self, String> {
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS state(key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY, value TEXT NOT NULL);").map_err(|e|e.to_string())?;
        Ok(Self(c))
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.0
            .query_row("SELECT value FROM state WHERE key=?1", [key], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR REPLACE INTO state(key,value) VALUES(?1,?2)",
                params![
                    key,
                    serde_json::to_string(value).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn operation(&self, value: &Operation) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR REPLACE INTO operations(id,value) VALUES(?1,?2)",
                params![
                    value.id,
                    serde_json::to_string(value).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let transaction = self.0.unchecked_transaction().map_err(|e| e.to_string())?;
        self.put("settings", settings)?;
        self.put("inventory", &Inventory::default())?;
        transaction.commit().map_err(|e| e.to_string())
    }
    pub fn history(&self) -> Result<Vec<Operation>, String> {
        let mut q = self
            .0
            .prepare("SELECT value FROM operations ORDER BY rowid DESC LIMIT 100")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            r.map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_change_and_inventory_invalidation_are_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let db = Storage::open(&temp.path().join("state.sqlite")).unwrap();
        let mut settings = Settings {
            home: temp.path().display().to_string(),
            projects: vec![],
            codex_home: None,
            claude_home: None,
        };
        db.save_settings(&settings).unwrap();
        db.put(
            "inventory",
            &Inventory {
                scanned_at: 42,
                ..Default::default()
            },
        )
        .unwrap();
        db.0.execute_batch("CREATE TRIGGER fail_inventory BEFORE INSERT ON state WHEN NEW.key = 'inventory' BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        settings.codex_home = Some(temp.path().join("changed-root").display().to_string());
        assert!(db.save_settings(&settings).is_err());
        assert!(db.get::<Settings>("settings").unwrap().codex_home.is_none());
        assert_eq!(db.get::<Inventory>("inventory").unwrap().scanned_at, 42);
        db.0.execute_batch("DROP TRIGGER fail_inventory;").unwrap();
        db.save_settings(&settings).unwrap();
        assert_eq!(
            db.get::<Settings>("settings").unwrap().codex_home,
            settings.codex_home
        );
        assert_eq!(db.get::<Inventory>("inventory").unwrap().scanned_at, 0);
    }

    #[test]
    fn settings_inventory_and_history_survive_reopen_and_rescan() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("inventory.sqlite");
        let settings = Settings {
            home: temp.path().display().to_string(),
            projects: vec![],
            codex_home: Some(temp.path().join("codex").display().to_string()),
            claude_home: Some(temp.path().join("claude").display().to_string()),
        };
        let operation = Operation {
            id: "fixture".into(),
            title: "Fixture import".into(),
            status: "completed".into(),
            message: "Verified".into(),
            at: 42,
        };
        {
            let db = Storage::open(&path).unwrap();
            db.put("settings", &settings).unwrap();
            db.put(
                "inventory",
                &Inventory {
                    scanned_at: 123,
                    ..Default::default()
                },
            )
            .unwrap();
            db.operation(&operation).unwrap();
        }
        {
            let db = Storage::open(&path).unwrap();
            assert_eq!(db.get::<Settings>("settings").unwrap().home, settings.home);
            assert_eq!(
                db.get::<Settings>("settings").unwrap().codex_home,
                settings.codex_home
            );
            assert_eq!(
                db.get::<Settings>("settings").unwrap().claude_home,
                settings.claude_home
            );
            assert_eq!(db.get::<Inventory>("inventory").unwrap().scanned_at, 123);
            assert_eq!(db.history().unwrap()[0].id, operation.id);
            db.put("inventory", &Inventory::default()).unwrap();
        }
        let db = Storage::open(&path).unwrap();
        assert_eq!(db.get::<Inventory>("inventory").unwrap().scanned_at, 0);
        assert_eq!(db.history().unwrap()[0].message, "Verified");
    }
}
