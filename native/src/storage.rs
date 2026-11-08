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
    conn: Connection,
}
impl Storage {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(path)?;
        let conn = Connection::open(path.join("era.sqlite"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS history(url TEXT PRIMARY KEY,title TEXT NOT NULL,visited INTEGER NOT NULL);")?;
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
            Ok(State::default())
        }
    }
    pub fn save(&self, s: &State) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.execute("INSERT INTO session(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",[serde_json::to_string(s)?])?;
        Ok(())
    }
    pub fn visit(&self, title: &str, url: &str) -> rusqlite::Result<usize> {
        self.conn.execute("INSERT INTO history(url,title,visited) VALUES(?1,?2,unixepoch()) ON CONFLICT(url) DO UPDATE SET title=excluded.title,visited=excluded.visited",params![url,title])
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
    fn session_and_history_roundtrip() {
        let path = std::env::temp_dir().join(format!("era-test-{}", uuid::Uuid::new_v4()));
        {
            let db = Storage::open(&path).unwrap();
            let s = State::default();
            db.save(&s).unwrap();
            assert_eq!(db.load().unwrap().active_tab, s.active_tab);
            db.visit("A title", "https://example.com").unwrap();
            assert_eq!(db.search("title").unwrap().len(), 1);
            assert_eq!(db.search("' OR 1=1 --").unwrap().len(), 0);
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}
