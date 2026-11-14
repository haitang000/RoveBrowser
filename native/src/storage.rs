use crate::downloads::DownloadItem;
use crate::model::State;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
#[derive(Serialize)]
pub struct Entry {
    title: String,
    url: String,
}
pub struct Storage {
    pub(crate) conn: Connection,
}
impl Storage {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(path)?;
        let conn = Connection::open(path.join("era.sqlite"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS history(url TEXT PRIMARY KEY,title TEXT NOT NULL,visited INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS downloads(id TEXT PRIMARY KEY,started INTEGER NOT NULL,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS imported_logins(id TEXT PRIMARY KEY, origin TEXT NOT NULL, secret BLOB NOT NULL); CREATE INDEX IF NOT EXISTS imported_logins_origin ON imported_logins(origin);")?;
        Ok(Self { conn })
    }
    pub fn load(&self) -> Result<State, Box<dyn std::error::Error>> {
        let mut q = self.conn.prepare("SELECT json FROM session WHERE id=1")?;
        let mut rows = q.query([])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let mut s: State = serde_json::from_str(&json)?;
            s.repair();
            Ok(s)
        } else {
            Ok(State::first_run())
        }
    }
    pub fn save(&self, s: &State) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.execute("INSERT INTO session(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",[serde_json::to_string(s)?])?;
        Ok(())
    }
    pub fn visit(&self, title: &str, url: &str) -> rusqlite::Result<usize> {
        self.conn.execute("INSERT INTO history(url,title,visited) VALUES(?1,?2,unixepoch()) ON CONFLICT(url) DO UPDATE SET title=excluded.title,visited=excluded.visited",params![url,title])
    }
    pub fn clear_history(&self) -> rusqlite::Result<usize> { self.conn.execute("DELETE FROM history", []) }
    /// Newest first, capped by the caller's model. Malformed rows are skipped so
    /// one broken download record cannot stop the browser from starting.
    pub fn load_downloads(&self) -> rusqlite::Result<Vec<DownloadItem>> {
        let mut q = self.conn.prepare("SELECT json FROM downloads ORDER BY started DESC LIMIT 50")?;
        let rows = q.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows
            .filter_map(|row| row.ok())
            .filter_map(|json| serde_json::from_str::<DownloadItem>(&json).ok())
            .collect())
    }
    /// The list is small (≤50), so a full rewrite keeps every transition simple.
    pub fn save_downloads(&mut self, items: &[DownloadItem]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM downloads", [])?;
        for item in items {
            let json = serde_json::to_string(item).unwrap_or_default();
            tx.execute(
                "INSERT OR REPLACE INTO downloads(id,started,json) VALUES(?1,?2,?3)",
                params![item.id, item.started as i64, json],
            )?;
        }
        tx.commit()
    }
    pub fn search(&self, text: &str) -> rusqlite::Result<Vec<Entry>> {
        let mut q=self.conn.prepare("SELECT title,url FROM history WHERE instr(lower(title),lower(?1))>0 OR instr(lower(url),lower(?1))>0 ORDER BY visited DESC LIMIT 30")?;
        let entries = q
            .query_map([text], |r| {
                Ok(Entry {
                    title: r.get(0)?,
                    url: r.get(1)?,
                })
            })?
            .collect();
        entries
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Command;
    #[test]
    fn sidebar_width_upgrades_legacy_sessions_and_survives_restart() {
        let path = std::env::temp_dir().join(format!("rove-sidebar-{}", uuid::Uuid::new_v4()));
        {
            let db = Storage::open(&path).unwrap();
            let mut legacy = serde_json::to_value(State::default()).unwrap();
            legacy.as_object_mut().unwrap().remove("sidebarWidth");
            db.conn.execute("INSERT INTO session(id,json) VALUES(1,?1)", [legacy.to_string()]).unwrap();
            let mut state = db.load().unwrap();
            assert_eq!(state.sidebar_width, None);
            let command: Command = serde_json::from_str(r#"{"type":"sidebarWidth","width":336}"#).unwrap();
            state.apply(&command).unwrap();
            db.save(&state).unwrap();
        }
        {
            let db = Storage::open(&path).unwrap();
            let mut state = db.load().unwrap();
            assert_eq!(state.sidebar_width, Some(336.));
            for (input, expected) in [(-100., 192.), (9999., 480.), (300.6, 301.)] {
                state.apply(&Command::SidebarWidth { width: Some(input) }).unwrap();
                assert_eq!(state.sidebar_width, Some(expected));
            }
            assert!(state.apply(&Command::SidebarWidth { width: Some(f64::NAN) }).is_err());
            assert_eq!(state.sidebar_width, Some(301.));
            state.sidebar_width = Some(9999.);
            db.save(&state).unwrap();
            assert_eq!(db.load().unwrap().sidebar_width, Some(480.));
            let reset: Command = serde_json::from_str(r#"{"type":"sidebarWidth","width":null}"#).unwrap();
            state.apply(&reset).unwrap();
            db.save(&state).unwrap();
        }
        assert_eq!(Storage::open(&path).unwrap().load().unwrap().sidebar_width, None);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn customized_and_empty_favorites_survive_restart() {
        let path = std::env::temp_dir().join(format!("era-favorites-{}", uuid::Uuid::new_v4()));
        {
            let db = Storage::open(&path).unwrap();
            let mut s = State::default();
            s.apply(&Command::SaveFavorite {
                title: "我的文档".into(),
                url: "example.com/guide".into(),
                previous_url: Some(s.favorites[0].url.clone()),
            })
            .unwrap();
            s.apply(&Command::MoveFavorite {
                url: "https://example.com/guide".into(),
                offset: 1,
            })
            .unwrap();
            db.save(&s).unwrap();
        }
        {
            let db = Storage::open(&path).unwrap();
            let mut s = db.load().unwrap();
            assert_eq!(s.favorites[1].title, "我的文档");
            assert_eq!(s.favorites[1].url, "https://example.com/guide");
            for url in s
                .favorites
                .iter()
                .map(|f| f.url.clone())
                .collect::<Vec<_>>()
            {
                s.apply(&Command::RemoveFavorite { url }).unwrap();
            }
            db.save(&s).unwrap();
        }
        {
            let db = Storage::open(&path).unwrap();
            assert!(db.load().unwrap().favorites.is_empty());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn download_history_roundtrips_and_skips_broken_rows() {
        use crate::downloads::DownloadState;
        let path = std::env::temp_dir().join(format!("era-downloads-{}", uuid::Uuid::new_v4()));
        let item = |id: &str, started: u64| DownloadItem {
            id: id.into(),
            url: format!("https://example.com/{id}"),
            path: format!("C:\\Downloads\\{id}.bin"),
            filename: format!("{id}.bin"),
            received: 10,
            total: 20,
            state: DownloadState::Finished,
            failure: None,
            started,
            finished: Some(started + 1),
            exists: true,
        };
        {
            let mut db = Storage::open(&path).unwrap();
            db.save_downloads(&[item("newer", 20), item("older", 10)]).unwrap();
            // A row from a future/damaged build must not poison the whole list.
            db.conn
                .execute(
                    "INSERT INTO downloads(id,started,json) VALUES('broken',30,'{not json')",
                    [],
                )
                .unwrap();
        }
        {
            let db = Storage::open(&path).unwrap();
            let restored = db.load_downloads().unwrap();
            assert_eq!(restored.len(), 2);
            assert_eq!(restored[0].id, "newer");
            assert_eq!(restored[1].id, "older");
            let db = Storage::open(&path).unwrap();
            db.conn.execute("DELETE FROM downloads", []).unwrap();
            assert!(db.load_downloads().unwrap().is_empty());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}
