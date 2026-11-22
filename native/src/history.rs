//! The internal history tab uses the same URL-level history as address suggestions.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub const URL: &str = "rove://history";

pub fn is_history(input: &str) -> bool {
    matches!(
        input.to_ascii_lowercase().as_str(),
        "rove://history" | "rove://history/" | "era://history" | "era://history/"
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cursor {
    pub visited: i64,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct Visit {
    title: String,
    url: String,
    visited: i64,
}

impl Visit {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            title: row.get(0)?,
            url: row.get(1)?,
            visited: row.get(2)?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Page {
    entries: Vec<Visit>,
    next: Option<Cursor>,
}

pub fn page(db: &Connection, query: &str, before: Option<&Cursor>) -> rusqlite::Result<Page> {
    if !query.trim().is_empty() {
        let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut statement = db.prepare_cached(
            "SELECT title,url,visited FROM history
             WHERE history_matches(title, url, ?1)
             ORDER BY visited DESC, url DESC LIMIT 300",
        )?;
        let entries = statement
            .query_map([words.join(" ")], Visit::read)?
            .collect::<rusqlite::Result<_>>()?;
        return Ok(Page {
            entries,
            next: None,
        });
    }
    let limit = 200_i64;
    let sql = if before.is_some() {
        "SELECT title,url,visited FROM history WHERE (visited,url) < (?2,?3)
         ORDER BY visited DESC, url DESC LIMIT ?1"
    } else {
        "SELECT title,url,visited FROM history ORDER BY visited DESC, url DESC LIMIT ?1"
    };
    let mut statement = db.prepare_cached(sql)?;
    let mut entries: Vec<_> = match before {
        Some(cursor) => statement
            .query_map(params![limit + 1, cursor.visited, cursor.url], Visit::read)?
            .collect::<rusqlite::Result<_>>()?,
        None => statement
            .query_map([limit + 1], Visit::read)?
            .collect::<rusqlite::Result<_>>()?,
    };
    let next = if entries.len() > limit as usize {
        entries.pop();
        entries.last().map(|visit| Cursor {
            visited: visit.visited,
            url: visit.url.clone(),
        })
    } else {
        None
    };
    Ok(Page { entries, next })
}

pub fn register_search(db: &Connection) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    db.create_scalar_function(
        "history_matches",
        3,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let title = ctx.get_raw(0).as_str()?.to_lowercase();
            let url = ctx.get_raw(1).as_str()?.to_lowercase();
            let words = ctx.get_raw(2).as_str()?;
            Ok(words
                .split_whitespace()
                .all(|word| title.contains(word) || url.contains(word)))
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Edit {
    Remove { urls: Vec<String> },
    Clear { since: Option<i64> },
    Undo,
}

pub fn edit(db: &mut Connection, change: Edit, undo: &mut Vec<Visit>) -> rusqlite::Result<bool> {
    let tx = db.transaction()?;
    let removed = match change {
        Edit::Remove { urls } => {
            let urls = serde_json::to_string(&urls)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            let mut statement = tx.prepare(
                "DELETE FROM history WHERE url IN (SELECT value FROM json_each(?1)) RETURNING title,url,visited",
            )?;
            let removed = statement
                .query_map([urls], Visit::read)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            removed
        }
        Edit::Clear { since } => {
            let sql = if since.is_some() {
                "DELETE FROM history WHERE visited >= ?1 RETURNING title,url,visited"
            } else {
                "DELETE FROM history RETURNING title,url,visited"
            };
            let mut statement = tx.prepare(sql)?;
            match since {
                Some(since) => statement
                    .query_map([since], Visit::read)?
                    .collect::<rusqlite::Result<_>>()?,
                None => statement
                    .query_map([], Visit::read)?
                    .collect::<rusqlite::Result<_>>()?,
            }
        }
        Edit::Undo => {
            {
                let mut statement = tx
                    .prepare("INSERT OR IGNORE INTO history(url,title,visited) VALUES(?1,?2,?3)")?;
                for visit in undo.iter() {
                    statement.execute(params![visit.url, visit.title, visit.visited])?;
                }
            }
            tx.commit()?;
            let changed = !undo.is_empty();
            undo.clear();
            return Ok(changed);
        }
    };
    tx.commit()?;
    let changed = !removed.is_empty();
    if changed {
        *undo = removed;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_history() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE history(url TEXT PRIMARY KEY,title TEXT NOT NULL,visited INTEGER NOT NULL);").unwrap();
        register_search(&db).unwrap();
        db
    }

    #[test]
    fn pagination_keeps_every_legacy_row_with_the_same_timestamp() {
        let db = legacy_history();
        for index in 0..401 {
            db.execute(
                "INSERT INTO history VALUES (?1,'Édition',100)",
                [format!("https://example.com/chapter/{index:03}")],
            )
            .unwrap();
        }
        let results = page(&db, "éDITION chapter/399", None).unwrap();
        assert_eq!(results.entries.len(), 1);
        let first = page(&db, "", None).unwrap();
        let second = page(&db, "", first.next.as_ref()).unwrap();
        let third = page(&db, "", second.next.as_ref()).unwrap();
        let addresses: Vec<_> = first
            .entries
            .into_iter()
            .chain(second.entries)
            .chain(third.entries)
            .map(|visit| visit.url)
            .collect();
        assert_eq!(addresses.len(), 401);
        assert!(addresses.windows(2).all(|pair| pair[0] > pair[1]));
        assert!(third.next.is_none());
    }

    #[test]
    fn range_clear_and_undo_preserve_older_rows_and_newer_revisits() {
        let mut db = legacy_history();
        db.execute_batch("INSERT INTO history VALUES ('https://a.test/','Older',99),('https://b.test/','At the limit',100),('https://c.test/','Recent',101);").unwrap();
        let mut undo = vec![];
        assert!(edit(&mut db, Edit::Clear { since: Some(100) }, &mut undo).unwrap());
        assert_eq!(page(&db, "", None).unwrap().entries[0].title, "Older");
        db.execute(
            "INSERT INTO history VALUES ('https://c.test/','Revisited',110)",
            [],
        )
        .unwrap();
        assert!(edit(&mut db, Edit::Undo, &mut undo).unwrap());
        assert!(undo.is_empty());
        let page = page(&db, "", None).unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(|visit| visit.visited)
                .collect::<Vec<_>>(),
            [110, 100, 99]
        );
        assert_eq!(page.entries[0].title, "Revisited");
        let urls = vec!["https://a.test/".into(), "https://c.test/".into()];
        assert!(edit(&mut db, Edit::Remove { urls }, &mut undo).unwrap());
        assert_eq!(undo.len(), 2);
    }
}
