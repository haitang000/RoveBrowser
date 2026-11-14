//! Import commit is one SQLite transaction, including the corresponding session.
use crate::{
    import::{Batch, Login, Report},
    model::{new_id, Favorite, Space, State},
    passwords,
    storage::Storage,
};
use rusqlite::params;
use std::collections::HashSet;

impl Storage {
    pub fn import_batch(&mut self, state: &mut State, batch: Batch) -> Result<(), String> {
        let mut next = state.clone();
        let mut report = Report {
            id: batch.id.clone(),
            name: batch.name.clone(),
            skipped: batch.skipped,
            problems: batch.problems,
            ..Default::default()
        };
        let space_id = format!("import-{}", batch.id);
        let mut bookmarks: HashSet<_> = next
            .pins
            .iter()
            .filter(|f| f.space_id.as_deref() == Some(&space_id))
            .map(|f| (f.url.clone(), f.title.clone()))
            .collect();
        for bookmark in batch.bookmarks {
            let title = if bookmark.folders.is_empty() {
                bookmark.title
            } else {
                format!("{} / {}", bookmark.folders.join(" / "), bookmark.title)
            };
            if !bookmarks.insert((bookmark.url.clone(), title.clone())) {
                report.skipped += 1;
                continue;
            }
            if !next.spaces.iter().any(|s| s.id == space_id) {
                next.spaces.push(Space {
                    id: space_id.clone(),
                    name: format!("导入 · {}", batch.name),
                    color: "sand".into(),
                    translucency: None,
                    active_tab: None,
                    icon: None,
                });
            }
            next.pins.push(Favorite {
                id: new_id(),
                title,
                url: bookmark.url,
                favicon: None,
                space_id: Some(space_id.clone()),
                split_with: None,
            });
            report.bookmarks += 1;
        }
        let tx = self
            .conn
            .transaction()
            .map_err(|_| "无法开始导入事务，未写入本次数据".to_string())?;
        {
            let mut insert = tx.prepare("INSERT INTO history(url,title,visited) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET title=CASE WHEN excluded.title='' THEN history.title ELSE excluded.title END,visited=excluded.visited WHERE excluded.visited > history.visited").map_err(|e| e.to_string())?;
            for visit in batch.history {
                let count = insert
                    .execute(params![visit.url, visit.title, visit.visited])
                    .map_err(|_| "历史记录写入失败，本次数据未写入".to_string())?;
                report.history += count;
                if count == 0 {
                    report.skipped += 1;
                }
            }
        }
        let mut existing = HashSet::new();
        if !batch.logins.is_empty() {
            let mut q = tx
                .prepare("SELECT origin,secret FROM imported_logins")
                .map_err(|e| e.to_string())?;
            let records = q
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })
                .map_err(|e| e.to_string())?;
            for record in records {
                let (origin, secret) = record.map_err(|e| e.to_string())?;
                // An unreadable existing vault must not be silently overwritten.
                let login = passwords::unprotect(&secret)?;
                existing.insert((origin, login.username.clone()));
            }
        }
        for (origin, login) in batch.logins {
            if !existing.insert((origin.clone(), login.username.clone())) {
                report.skipped += 1;
                continue;
            }
            let secret = passwords::protect(&login)?;
            tx.execute(
                "INSERT INTO imported_logins(id,origin,secret) VALUES(?1,?2,?3)",
                params![new_id(), origin, secret],
            )
            .map_err(|_| "密码保存失败，本次数据未写入".to_string())?;
            report.passwords += 1;
        }
        next.onboarding.imports.retain(|r| r.id != report.id);
        next.onboarding.imports.push(report);
        tx.execute("INSERT INTO session(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json", [serde_json::to_string(&next).map_err(|e| e.to_string())?]).map_err(|_| "会话保存失败，本次数据未写入".to_string())?;
        tx.commit()
            .map_err(|_| "无法完成导入，本次数据未写入".to_string())?;
        *state = next;
        Ok(())
    }
    pub fn imported_logins(&self, origin: &str) -> Vec<Login> {
        let Ok(mut q) = self
            .conn
            .prepare("SELECT secret FROM imported_logins WHERE origin=?1 LIMIT 30")
        else {
            return vec![];
        };
        let Ok(rows) = q.query_map([origin], |r| r.get::<_, Vec<u8>>(0)) else {
            return vec![];
        };
        rows.filter_map(Result::ok)
            .filter_map(|secret| passwords::unprotect(&secret).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{Bookmark, Visit};
    fn batch(password: &str, visited: i64) -> Batch {
        Batch {
            id: "edge-test".into(),
            name: "Edge Test".into(),
            bookmarks: vec![Bookmark {
                url: "https://example.com/".into(),
                title: "Docs".into(),
                folders: vec!["Work".into()],
            }],
            history: vec![Visit {
                url: "https://example.com/".into(),
                title: "Imported".into(),
                visited,
            }],
            logins: vec![(
                "https://example.com".into(),
                Login {
                    username: "test-user".into(),
                    password: password.into(),
                    notes: String::new(),
                    otp: String::new(),
                },
            )],
            skipped: 0,
            problems: vec![],
        }
    }
    #[test]
    fn imports_are_deduplicated_encrypted_and_restored_without_overwriting_newer_data() {
        let path = std::env::temp_dir().join(format!("rove-import-store-{}", uuid::Uuid::new_v4()));
        {
            let mut db = Storage::open(&path).unwrap();
            let mut state = State::first_run();
            db.conn
                .execute(
                    "INSERT INTO history VALUES('https://example.com/','Existing',200)",
                    [],
                )
                .unwrap();
            db.import_batch(&mut state, batch("first-secret", 100))
                .unwrap();
            assert_eq!(state.pins.len(), 1);
            assert_eq!(state.pins[0].title, "Work / Docs");
            let spaces = state.spaces.len();
            db.import_batch(&mut state, batch("replacement-secret", 100))
                .unwrap();
            assert_eq!(state.pins.len(), 1);
            assert_eq!(state.spaces.len(), spaces);
            let report = &state.onboarding.imports[0];
            assert_eq!(report.skipped, 3);
            assert_eq!(report.passwords, 0);
            let history: (String, i64) = db
                .conn
                .query_row("SELECT title,visited FROM history", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
            assert_eq!(history, ("Existing".into(), 200));
            let secret: Vec<u8> = db
                .conn
                .query_row("SELECT secret FROM imported_logins", [], |r| r.get(0))
                .unwrap();
            assert!(!secret.windows(12).any(|w| w == b"first-secret"));
            let session: String = db
                .conn
                .query_row("SELECT json FROM session", [], |r| r.get(0))
                .unwrap();
            assert!(!session.contains("first-secret"));
            assert!(!session.contains("test-user"));
        }
        {
            let db = Storage::open(&path).unwrap();
            let state = db.load().unwrap();
            assert_eq!(state.pins.len(), 1);
            assert_eq!(state.onboarding.imports.len(), 1);
            let logins = db.imported_logins("https://example.com");
            assert_eq!(logins.len(), 1);
            assert_eq!(logins[0].password, "first-secret");
            assert!(db.imported_logins("https://other.example.com").is_empty());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn session_write_failure_rolls_back_history_vault_and_in_memory_bookmarks() {
        let path =
            std::env::temp_dir().join(format!("rove-import-rollback-{}", uuid::Uuid::new_v4()));
        {
            let mut db = Storage::open(&path).unwrap();
            let mut state = State::first_run();
            let before = serde_json::to_string(&state).unwrap();
            db.conn.execute_batch("CREATE TRIGGER fail_session BEFORE INSERT ON session BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
            assert!(db.import_batch(&mut state, batch("secret", 100)).is_err());
            assert_eq!(serde_json::to_string(&state).unwrap(), before);
            for table in ["history", "imported_logins", "session"] {
                assert_eq!(
                    db.conn
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn damaged_vault_does_not_block_bookmarks_or_history() {
        let path = std::env::temp_dir().join(format!("rove-import-vault-{}", uuid::Uuid::new_v4()));
        {
            let mut db = Storage::open(&path).unwrap();
            let mut state = State::first_run();
            db.conn
                .execute(
                    "INSERT INTO imported_logins VALUES('broken','https://example.com',X'00')",
                    [],
                )
                .unwrap();
            let mut data = batch("unused", 100);
            data.logins.clear();
            db.import_batch(&mut state, data).unwrap();
            assert_eq!(state.pins.len(), 1);
            assert!(db.import_batch(&mut state, batch("secret", 200)).is_err());
            assert_eq!(
                db.conn
                    .query_row("SELECT visited FROM history", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                100
            );
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}
