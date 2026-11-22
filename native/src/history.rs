//! The internal history tab uses the same URL-level history as address suggestions.
use rusqlite::{params, params_from_iter, Connection, ToSql, TransactionBehavior};
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
    #[serde(default = "one_visit", rename = "visitCount")]
    pub visit_count: i64,
}

fn one_visit() -> i64 {
    1
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    title: String,
    url: String,
    visited: i64,
    visit_count: i64,
}

impl Visit {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            title: row.get(0)?,
            url: row.get(1)?,
            visited: row.get(2)?,
            visit_count: row.get(3)?,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Page {
    entries: Vec<Visit>,
    next: Option<Cursor>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    #[default]
    Date,
    Site,
    Popular,
    Last,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Query {
    pub query: String,
    pub before: Option<Cursor>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub sort: Sort,
    pub urls: Option<Vec<String>>,
}

pub fn migrate(db: &mut Connection) -> rusqlite::Result<()> {
    let has_count = |db: &Connection| {
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('history') WHERE name='visit_count')",
            [],
            |row| row.get::<_, bool>(0),
        )
    };
    if !has_count(db)? {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !has_count(&tx)? {
            tx.execute(
                "ALTER TABLE history ADD COLUMN visit_count INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
        }
        tx.commit()?;
    }
    Ok(())
}

pub fn page(db: &Connection, query: &Query) -> rusqlite::Result<Page> {
    let words = query
        .query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    let limit = if words.is_empty() { 200_i64 } else { 300 };
    let fetch = limit + 1;
    let urls = query
        .urls
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let mut conditions = Vec::new();
    let mut values: Vec<&dyn ToSql> = Vec::new();
    if !words.is_empty() {
        conditions.push("history_matches(title, url, ?)");
        values.push(&words);
    }
    if let Some(since) = query.since.as_ref() {
        conditions.push("visited >= ?");
        values.push(since);
    }
    if let Some(until) = query.until.as_ref() {
        conditions.push("visited < ?");
        values.push(until);
    }
    if let Some(urls) = urls.as_ref() {
        conditions.push("url IN (SELECT value FROM json_each(?))");
        values.push(urls);
    }
    let site = query
        .before
        .as_ref()
        .and_then(|cursor| crate::model::site_key(&cursor.url))
        .unwrap_or_default();
    if let Some(cursor) = &query.before {
        match query.sort {
            Sort::Popular => {
                conditions.push("(visit_count,visited,url) < (?,?,?)");
                values.push(&cursor.visit_count);
            }
            Sort::Site => {
                conditions.push(
                    "(history_site(url) > ? OR (history_site(url) = ? AND (visited,url) < (?,?)))",
                );
                values.extend([&site as &dyn ToSql, &site]);
            }
            Sort::Date | Sort::Last => conditions.push("(visited,url) < (?,?)"),
        }
        values.extend([&cursor.visited as &dyn ToSql, &cursor.url]);
    }
    let order = match query.sort {
        Sort::Site => "history_site(url), visited DESC, url DESC",
        Sort::Popular => "visit_count DESC, visited DESC, url DESC",
        Sort::Date | Sort::Last => "visited DESC, url DESC",
    };
    let mut sql = String::from("SELECT title,url,visited,visit_count FROM history");
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(&format!(" ORDER BY {order} LIMIT ?"));
    values.push(&fetch);
    let mut statement = db.prepare_cached(&sql)?;
    let mut entries: Vec<_> = statement
        .query_map(params_from_iter(values), Visit::read)?
        .collect::<rusqlite::Result<_>>()?;
    let next = if entries.len() > limit as usize {
        entries.pop();
        entries.last().map(|visit| Cursor {
            visited: visit.visited,
            url: visit.url.clone(),
            visit_count: visit.visit_count,
        })
    } else {
        None
    };
    Ok(Page { entries, next })
}

pub fn register_search(db: &Connection) -> rusqlite::Result<()> {
    use rusqlite::functions::FunctionFlags;
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    db.create_scalar_function("history_site", 1, flags, |ctx| {
        Ok(crate::model::site_key(ctx.get_raw(0).as_str()?).unwrap_or_default())
    })?;
    db.create_scalar_function("history_matches", 3, flags, |ctx| {
        let title = ctx.get_raw(0).as_str()?.to_lowercase();
        let url = ctx.get_raw(1).as_str()?.to_lowercase();
        let words = ctx.get_raw(2).as_str()?;
        Ok(words
            .split_whitespace()
            .all(|word| title.contains(word) || url.contains(word)))
    })
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
                "DELETE FROM history WHERE url IN (SELECT value FROM json_each(?1)) RETURNING title,url,visited,visit_count",
            )?;
            let removed = statement
                .query_map([urls], Visit::read)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            removed
        }
        Edit::Clear { since } => {
            let sql = if since.is_some() {
                "DELETE FROM history WHERE visited >= ?1 RETURNING title,url,visited,visit_count"
            } else {
                "DELETE FROM history RETURNING title,url,visited,visit_count"
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
                let mut statement = tx.prepare(
                    "INSERT INTO history(url,title,visited,visit_count) VALUES(?1,?2,?3,?4)
                     ON CONFLICT(url) DO UPDATE SET visit_count=history.visit_count+excluded.visit_count",
                )?;
                for visit in undo.iter() {
                    statement.execute(params![
                        visit.url,
                        visit.title,
                        visit.visited,
                        visit.visit_count
                    ])?;
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
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE history(url TEXT PRIMARY KEY,title TEXT NOT NULL,visited INTEGER NOT NULL);").unwrap();
        migrate(&mut db).unwrap();
        register_search(&db).unwrap();
        db
    }

    #[test]
    fn pagination_keeps_every_legacy_row_with_the_same_timestamp() {
        let db = legacy_history();
        for index in 0..401 {
            db.execute(
                "INSERT INTO history(url,title,visited) VALUES (?1,'Édition',100)",
                [format!("https://example.com/chapter/{index:03}")],
            )
            .unwrap();
        }
        let results = page(
            &db,
            &Query {
                query: "éDITION chapter/399".into(),
                ..Query::default()
            },
        )
        .unwrap();
        assert_eq!(results.entries.len(), 1);
        let mut query = Query::default();
        let first = page(&db, &query).unwrap();
        query.before = first.next;
        let second = page(&db, &query).unwrap();
        query.before = second.next;
        let third = page(&db, &query).unwrap();
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
        db.execute_batch("INSERT INTO history VALUES ('https://a.test/','Older',99,1),('https://b.test/','At the limit',100,23),('https://c.test/','Recent',101,4);").unwrap();
        let mut undo = vec![];
        assert!(edit(&mut db, Edit::Clear { since: Some(100) }, &mut undo).unwrap());
        assert_eq!(
            page(&db, &Query::default()).unwrap().entries[0].title,
            "Older"
        );
        db.execute(
            "INSERT INTO history(url,title,visited) VALUES ('https://c.test/','Revisited',110)",
            [],
        )
        .unwrap();
        assert!(edit(&mut db, Edit::Undo, &mut undo).unwrap());
        assert!(undo.is_empty());
        let page = page(&db, &Query::default()).unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(|visit| visit.visited)
                .collect::<Vec<_>>(),
            [110, 100, 99]
        );
        assert_eq!(page.entries[0].title, "Revisited");
        assert_eq!(page.entries[0].visit_count, 5);
        assert_eq!(page.entries[1].visit_count, 23);
        let urls = vec!["https://a.test/".into(), "https://c.test/".into()];
        assert!(edit(&mut db, Edit::Remove { urls }, &mut undo).unwrap());
        assert_eq!(undo.len(), 2);
    }

    #[test]
    fn date_bounds_and_search_apply_before_sorting_across_pages() {
        let db = legacy_history();
        for number in 0..205 {
            db.execute(
                "INSERT INTO history VALUES (?1,'Match',100,3)",
                [format!("https://www.a.test/{number:03}")],
            )
            .unwrap();
        }
        db.execute_batch("INSERT INTO history VALUES ('https://b.test/','Match earlier',100,9),
            ('https://b.test/other','Unrelated',101,12),('https://a.test/too-recent','Match',102,50),
            ('https://a.test/too-old','Match',99,50);").unwrap();
        for (sort, first, last) in [
            (Sort::Site, "https://www.a.test/204", "https://b.test/"),
            (Sort::Popular, "https://b.test/", "https://www.a.test/000"),
            (Sort::Last, "https://www.a.test/204", "https://b.test/"),
        ] {
            let mut query = Query {
                query: "match".into(),
                since: Some(100),
                until: Some(102),
                sort,
                ..Query::default()
            };
            let result = page(&db, &query).unwrap();
            assert_eq!(result.entries.len(), 206);
            assert_eq!(result.entries[0].url, first);
            assert_eq!(result.entries.last().unwrap().url, last);

            query.query.clear();
            let result = page(&db, &query).unwrap();
            query.before = result.next;
            let remainder = page(&db, &query).unwrap();
            assert_eq!(result.entries.len() + remainder.entries.len(), 207);
            assert!(result
                .entries
                .iter()
                .all(|entry| remainder.entries.iter().all(|other| entry.url != other.url)));
        }
    }

    #[test]
    fn upgrading_legacy_counts_preserves_the_latest_visit_and_counts_new_visits() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE history(url TEXT PRIMARY KEY,title TEXT NOT NULL,visited INTEGER NOT NULL);
            INSERT INTO history VALUES ('https://a.test/','Original',100)").unwrap();
        migrate(&mut db).unwrap();
        let storage = crate::storage::Storage { conn: db };
        let original = page(&storage.conn, &Query::default()).unwrap();
        assert_eq!(
            (original.entries[0].visited, original.entries[0].visit_count),
            (100, 1)
        );
        storage.visit("Revisited", "https://a.test/", 200).unwrap();
        storage
            .update_history_title("New title", "https://a.test/")
            .unwrap();
        let revisited = page(&storage.conn, &Query::default()).unwrap();
        assert_eq!(
            (
                revisited.entries[0].visited,
                revisited.entries[0].visit_count
            ),
            (200, 2)
        );
        assert_eq!(revisited.entries[0].title, "New title");
    }
}
