//! Browser discovery, SQLite snapshots and import readers. Adapted from Zigzag
//! ImportCoordinator / ChromiumImporter / FirefoxImporter / CSVImporter,
//! Copyright 2026 Zigao Wang, Apache-2.0; Windows paths and failure handling added.
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_BOOKMARKS: usize = 20_000;
const MAX_HISTORY: usize = 50_000;
const MAX_FILE: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub browser: String,
    pub name: String,
    pub bookmarks: bool,
    pub history: bool,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(skip)]
    firefox: bool,
    #[serde(skip)]
    modified: u64,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub sources: Vec<String>,
    pub bookmarks: bool,
    pub history: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub id: String,
    pub name: String,
    pub bookmarks: usize,
    pub history: usize,
    pub passwords: usize,
    pub skipped: usize,
    pub problems: Vec<String>,
}
#[derive(Debug)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub folders: Vec<String>,
}
#[derive(Debug)]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub visited: i64,
}
#[derive(Serialize, Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
    pub notes: String,
    pub otp: String,
}
pub struct Batch {
    pub id: String,
    pub name: String,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<Visit>,
    pub logins: Vec<(String, Login)>,
    pub skipped: usize,
    pub problems: Vec<String>,
}
impl std::fmt::Debug for Batch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportBatch")
            .field("bookmarks", &self.bookmarks.len())
            .field("history", &self.history.len())
            .field("passwords", &self.logins.len())
            .finish()
    }
}
impl Batch {
    fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            bookmarks: vec![],
            history: vec![],
            logins: vec![],
            skipped: 0,
            problems: vec![],
        }
    }
}
fn hash(value: &Path) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    format!("{:x}", h.finish())
}
pub fn web_url(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw.trim()).ok()?;
    (matches!(u.scheme(), "http" | "https")
        && u.host_str().is_some()
        && u.username().is_empty()
        && u.password().is_none()
        && raw.len() <= 8192)
        .then(|| u.to_string())
}
pub fn origin(raw: &str) -> Option<String> {
    Some(
        url::Url::parse(&web_url(raw)?)
            .ok()?
            .origin()
            .ascii_serialization(),
    )
}
fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|t| t.as_secs())
        .unwrap_or(0)
}

pub fn detect() -> Vec<Source> {
    #[cfg(debug_assertions)]
    if let Some(base) = std::env::var_os("ERA_IMPORT_FIXTURES") {
        let base = PathBuf::from(base);
        return detect_at(&base.join("Local"), &base.join("Roaming"));
    }
    detect_at(
        &PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()),
        &PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()),
    )
}
pub fn detect_at(local: &Path, roaming: &Path) -> Vec<Source> {
    let mut result = vec![];
    for (key, name, root) in [
        (
            "edge",
            "Microsoft Edge",
            local.join("Microsoft/Edge/User Data"),
        ),
        (
            "chrome",
            "Google Chrome",
            local.join("Google/Chrome/User Data"),
        ),
        (
            "brave",
            "Brave",
            local.join("BraveSoftware/Brave-Browser/User Data"),
        ),
        ("vivaldi", "Vivaldi", local.join("Vivaldi/User Data")),
        (
            "opera",
            "Opera",
            roaming.join("Opera Software/Opera Stable"),
        ),
    ] {
        let state = read_json(&root.join("Local State")).unwrap_or(Value::Null);
        let mut profiles = vec![root.clone()];
        if let Ok(entries) = std::fs::read_dir(&root) {
            profiles.extend(
                entries
                    .flatten()
                    .take(200)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.path()),
            );
        }
        for path in profiles {
            let bookmarks = path.join("Bookmarks").is_file();
            let history = path.join("History").is_file();
            if !bookmarks && !history {
                continue;
            }
            let folder = path.file_name().unwrap_or_default().to_string_lossy();
            let label = state
                .pointer("/profile/info_cache")
                .and_then(|v| v.get(folder.as_ref()))
                .and_then(|v| v.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(&folder);
            result.push(Source {
                id: format!("{key}-{}", hash(&path)),
                browser: name.into(),
                name: if path == root {
                    name.into()
                } else {
                    format!("{name} · {}", label.chars().take(80).collect::<String>())
                },
                bookmarks,
                history,
                modified: modified(&path.join("History")),
                path,
                firefox: false,
            });
        }
    }
    let root = roaming.join("Mozilla/Firefox");
    let mut paths = vec![];
    if let Ok(entries) = std::fs::read_dir(root.join("Profiles")) {
        paths.extend(entries.flatten().take(200).map(|e| e.path()));
    }
    if let Ok(ini) = std::fs::read_to_string(root.join("profiles.ini")) {
        let mut relative = true;
        let mut path: Option<String> = None;
        for line in ini.lines().chain(std::iter::once("[end]")) {
            let line = line.trim();
            if line.starts_with('[') {
                if let Some(p) = path.take() {
                    paths.push(if relative {
                        root.join(p)
                    } else {
                        PathBuf::from(p)
                    });
                }
                relative = true;
            }
            if let Some(p) = line.strip_prefix("Path=") {
                path = Some(p.into());
            }
            if let Some(r) = line.strip_prefix("IsRelative=") {
                relative = r != "0";
            }
        }
    }
    let mut seen = HashSet::new();
    for path in paths {
        if !path.join("places.sqlite").is_file() || !seen.insert(path.clone()) {
            continue;
        }
        let label = path.file_name().unwrap_or_default().to_string_lossy();
        let label = label.split_once('.').map(|p| p.1).unwrap_or(&label);
        result.push(Source {
            id: format!("firefox-{}", hash(&path)),
            browser: "Firefox".into(),
            name: format!("Firefox · {}", label.chars().take(80).collect::<String>()),
            bookmarks: true,
            history: true,
            modified: modified(&path.join("places.sqlite")),
            path,
            firefox: true,
        });
    }
    result.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    result
}
fn read_json(path: &Path) -> Result<Value, String> {
    let size = std::fs::metadata(path)
        .map_err(|_| "无法读取文件，请关闭原浏览器后重试".to_string())?
        .len();
    if size > MAX_FILE {
        return Err("文件超过 16 MiB，未读取".into());
    }
    let data = std::fs::read(path).map_err(|_| "无法读取文件，请关闭原浏览器后重试".to_string())?;
    serde_json::from_slice(&data).map_err(|_| "数据格式损坏，未读取".into())
}
fn snapshot(path: &Path) -> Result<Connection, String> {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > 256 * 1024 * 1024) {
        return Err("数据库超过 256 MiB，未读取".into());
    }
    let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "无法打开数据库，请关闭原浏览器后重试".to_string())?;
    source
        .busy_timeout(Duration::from_millis(100))
        .map_err(|e| e.to_string())?;
    if dest_size(&source) > 256 * 1024 * 1024 {
        return Err("数据库超过 256 MiB，未读取".into());
    }
    let page_size: i64 = source
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .unwrap_or(65536);
    let mut dest = Connection::open_in_memory().map_err(|e| e.to_string())?;
    {
        let backup = Backup::new(&source, &mut dest)
            .map_err(|_| "无法创建数据库快照，请关闭原浏览器后重试".to_string())?;
        let start = Instant::now();
        loop {
            match backup
                .step(256)
                .map_err(|_| "数据库正在使用或已损坏，请关闭原浏览器后重试".to_string())?
            {
                StepResult::Done => break,
                _ if start.elapsed() > Duration::from_secs(5)
                    || i64::from(backup.progress().pagecount).saturating_mul(page_size)
                        > 256 * 1024 * 1024 =>
                {
                    return Err("数据库繁忙或过大，请关闭原浏览器后重试".into())
                }
                StepResult::Busy | StepResult::Locked => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                _ => {}
            }
        }
    }
    Ok(dest)
}
fn dest_size(conn: &Connection) -> i64 {
    let pages: i64 = conn
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap_or(0);
    let size: i64 = conn
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .unwrap_or(0);
    pages.saturating_mul(size)
}
fn walk_bookmarks(value: &Value, folders: &[String], out: &mut Vec<Bookmark>, depth: usize) {
    if depth > 32 || out.len() >= MAX_BOOKMARKS {
        return;
    }
    if let Some(raw) = value.get("url").and_then(Value::as_str) {
        if let Some(url) = web_url(raw) {
            out.push(Bookmark {
                title: value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&url)
                    .chars()
                    .take(512)
                    .collect(),
                url,
                folders: folders.to_vec(),
            });
        }
    } else if let Some(children) = value.get("children").and_then(Value::as_array) {
        let mut folders = folders.to_vec();
        if let Some(name) = value
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            folders.push(name.chars().take(120).collect());
        }
        for child in children {
            walk_bookmarks(child, &folders, out, depth + 1);
        }
    }
}
pub fn read_browser(source: &Source, request: &Request) -> Batch {
    let mut batch = Batch::new(source.id.clone(), source.name.clone());
    if !source.firefox {
        if request.bookmarks && source.bookmarks {
            match read_json(&source.path.join("Bookmarks")) {
                Ok(value) => {
                    if let Some(roots) = value.get("roots").and_then(Value::as_object) {
                        for root in roots.values() {
                            walk_bookmarks(root, &[], &mut batch.bookmarks, 0);
                        }
                    } else {
                        batch.problems.push("书签格式无法识别".into());
                    }
                }
                Err(e) => batch.problems.push(format!("书签：{e}")),
            }
        }
        if request.history && source.history {
            match snapshot(&source.path.join("History")).and_then(|db| read_history(&db, "SELECT url, COALESCE(title,''), last_visit_time / 1000000 - 11644473600 FROM urls WHERE last_visit_time > 0 ORDER BY last_visit_time DESC LIMIT 50001")) {
                Ok(rows) => batch.history = rows, Err(e) => batch.problems.push(format!("历史：{e}")),
            }
        }
    } else {
        match snapshot(&source.path.join("places.sqlite")) {
            Ok(db) => {
                if request.history {
                    match read_history(&db, "SELECT url, COALESCE(title,''), last_visit_date / 1000000 FROM moz_places WHERE last_visit_date > 0 ORDER BY last_visit_date DESC LIMIT 50001") { Ok(rows) => batch.history = rows, Err(e) => batch.problems.push(format!("历史：{e}")) }
                }
                if request.bookmarks {
                    match firefox_bookmarks(&db) {
                        Ok(rows) => batch.bookmarks = rows,
                        Err(e) => batch.problems.push(format!("书签：{e}")),
                    }
                }
            }
            Err(e) => batch.problems.push(e),
        }
    }
    if batch.bookmarks.len() >= MAX_BOOKMARKS {
        batch.problems.push("本次最多读取 20,000 个书签".into());
    }
    if batch.history.len() > MAX_HISTORY {
        batch.history.truncate(MAX_HISTORY);
        batch
            .problems
            .push("本次只读取最近 50,000 条历史记录".into());
    }
    batch
}
fn read_history(db: &Connection, sql: &str) -> Result<Vec<Visit>, String> {
    let mut q = db
        .prepare(sql)
        .map_err(|_| "数据库格式无法识别".to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut result = vec![];
    for row in rows {
        let (raw, title, visited) = row.map_err(|_| "部分历史数据损坏".to_string())?;
        if let Some(url) = web_url(&raw) {
            result.push(Visit {
                url,
                title: title.chars().take(512).collect(),
                visited: visited.clamp(0, crate::model::now_secs() as i64),
            });
        }
    }
    Ok(result)
}
fn firefox_bookmarks(db: &Connection) -> Result<Vec<Bookmark>, String> {
    let mut q = db.prepare("SELECT b.id,b.parent,COALESCE(b.title,''),COALESCE(p.url,''),b.type FROM moz_bookmarks b LEFT JOIN moz_places p ON p.id=b.fk ORDER BY b.position,b.id LIMIT 40000").map_err(|_| "书签数据库格式无法识别".to_string())?;
    let rows: Vec<_> = q
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let folders: HashMap<_, _> = rows
        .iter()
        .filter(|r| r.4 == 2)
        .map(|r| (r.0, (r.1, r.2.clone())))
        .collect();
    let mut result = vec![];
    for (_, mut parent, title, raw, kind) in rows {
        if kind != 1 {
            continue;
        }
        if let Some(url) = web_url(&raw) {
            let mut path = vec![];
            let mut seen = HashSet::new();
            while path.len() < 32 && seen.insert(parent) {
                if let Some((next, name)) = folders.get(&parent) {
                    if !name.is_empty() {
                        path.push(name.chars().take(120).collect());
                    }
                    parent = *next;
                } else {
                    break;
                }
            }
            path.reverse();
            result.push(Bookmark {
                url,
                title: title.chars().take(512).collect(),
                folders: path,
            });
            if result.len() >= MAX_BOOKMARKS {
                break;
            }
        }
    }
    Ok(result)
}
pub fn read_csv(path: &Path) -> Batch {
    let mut batch = Batch::new("password-file".into(), "密码文件".into());
    let read = || -> Result<Vec<u8>, String> {
        if std::fs::metadata(path)
            .map_err(|_| "无法读取密码文件".to_string())?
            .len()
            > MAX_FILE
        {
            return Err("密码文件超过 16 MiB".into());
        }
        std::fs::read(path).map_err(|_| "无法读取密码文件".into())
    };
    match read().and_then(|bytes| parse_csv(&bytes)) {
        Ok((logins, skipped)) => {
            batch.logins = logins;
            batch.skipped = skipped;
        }
        Err(e) => batch.problems.push(e),
    }
    batch
}
pub fn parse_csv(bytes: &[u8]) -> Result<(Vec<(String, Login)>, usize), String> {
    if bytes.len() as u64 > MAX_FILE {
        return Err("密码文件超过 16 MiB".into());
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) {
        if !bytes.len().is_multiple_of(2) {
            return Err("密码文件编码损坏".into());
        }
        String::from_utf16(
            &bytes[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| "密码文件编码损坏".to_string())?
    } else {
        String::from_utf8(
            bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(bytes)
                .to_vec(),
        )
        .map_err(|_| "请使用 UTF-8 编码的密码 CSV 文件".to_string())?
    };
    let mut reader = csv::ReaderBuilder::new().from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| "无法读取 CSV 表头".to_string())?
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let find = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let (Some(u), Some(n), Some(p)) = (
        find(&["url", "login_uri"]),
        find(&["username", "login_username"]),
        find(&["password", "login_password"]),
    ) else {
        return Err("请选择浏览器或密码管理器导出的 CSV（需包含网址、用户名、密码列）".into());
    };
    let notes = find(&["notes", "note", "extra"]);
    let otp = find(&["otpauth", "login_totp", "totp"]);
    let mut logins = vec![];
    let mut skipped = 0;
    for row in reader.records() {
        let row = row.map_err(|_| "CSV 列数或引号格式错误，整个文件未导入".to_string())?;
        if logins.len() + skipped >= 50_000 {
            return Err("密码文件超过 50,000 条，请分批导出".into());
        }
        if let Some(origin) = origin(&row[u])
            .filter(|_| !row[p].is_empty() && row[p].len() <= 16384 && row[n].len() <= 2048)
        {
            logins.push((
                origin,
                Login {
                    username: row[n].into(),
                    password: row[p].into(),
                    notes: notes.map(|i| row[i].to_string()).unwrap_or_default(),
                    otp: otp.map(|i| row[i].to_string()).unwrap_or_default(),
                },
            ));
        } else {
            skipped += 1;
        }
    }
    if logins.is_empty() {
        return Err("文件中没有可导入的网页密码".into());
    }
    Ok((logins, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("rove-import-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn source(&self, firefox: bool) -> Source {
            Source {
                id: "test".into(),
                browser: "Test".into(),
                name: "Test".into(),
                bookmarks: true,
                history: true,
                path: self.0.clone(),
                firefox,
                modified: 0,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn request(bookmarks: bool, history: bool) -> Request {
        Request {
            sources: vec![],
            bookmarks,
            history,
        }
    }

    #[test]
    fn chromium_nested_bookmarks_filter_unsafe_urls_and_honor_type_selection() {
        let f = Fixture::new();
        std::fs::write(f.0.join("Bookmarks"), r#"{"roots":{"bookmark_bar":{"name":"收藏夹栏","children":[{"name":"工作","children":[{"name":"文档","url":"https://example.com/docs"},{"name":"bad","url":"javascript:alert(1)"},{"url":"file:///c:/secret"},{"url":"https://user:pass@example.com"}]}]}}}"#).unwrap();
        let batch = read_browser(&f.source(false), &request(true, false));
        assert!(batch.problems.is_empty());
        assert_eq!(batch.bookmarks.len(), 1);
        assert_eq!(batch.bookmarks[0].folders, ["收藏夹栏", "工作"]);
        assert_eq!(batch.bookmarks[0].title, "文档");
        assert!(batch.history.is_empty());
        assert_eq!(
            origin("https://example.com:8443/login?q=1").as_deref(),
            Some("https://example.com:8443")
        );
        assert!(web_url("data:text/plain,secret").is_none());
    }

    #[test]
    fn sqlite_snapshot_reads_committed_wal_and_preserves_source() {
        let f = Fixture::new();
        let path = f.0.join("History");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE urls(url TEXT,title TEXT,last_visit_time INTEGER);").unwrap();
        db.execute(
            "INSERT INTO urls VALUES('https://example.com/','recent',?1)",
            [(1_700_000_000i64 + 11_644_473_600) * 1_000_000],
        )
        .unwrap();
        let batch = read_browser(&f.source(false), &request(false, true));
        assert!(batch.problems.is_empty());
        assert!(batch.bookmarks.is_empty());
        assert_eq!(batch.history.len(), 1);
        assert_eq!(batch.history[0].visited, 1_700_000_000);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn broken_or_locked_history_does_not_discard_bookmarks() {
        let f = Fixture::new();
        std::fs::write(
            f.0.join("Bookmarks"),
            r#"{"roots":{"other":{"children":[{"url":"https://example.com/","name":"OK"}]}}}"#,
        )
        .unwrap();
        std::fs::write(f.0.join("History"), b"not a database").unwrap();
        let batch = read_browser(&f.source(false), &request(true, true));
        assert_eq!(batch.bookmarks.len(), 1);
        assert_eq!(batch.problems.len(), 1);
        std::fs::remove_file(f.0.join("History")).unwrap();
        let db = Connection::open(f.0.join("History")).unwrap();
        db.execute_batch(
            "CREATE TABLE urls(url TEXT,title TEXT,last_visit_time INTEGER); BEGIN EXCLUSIVE;",
        )
        .unwrap();
        assert!(snapshot(&f.0.join("History")).is_err());
        db.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn firefox_bookmark_ancestors_and_microsecond_history_are_read() {
        let f = Fixture::new();
        let db = Connection::open(f.0.join("places.sqlite")).unwrap();
        db.execute_batch("CREATE TABLE moz_places(id INTEGER,url TEXT,title TEXT,last_visit_date INTEGER); CREATE TABLE moz_bookmarks(id INTEGER,parent INTEGER,title TEXT,fk INTEGER,type INTEGER,position INTEGER); INSERT INTO moz_places VALUES(1,'https://example.org/','Firefox page',1700000000000000); INSERT INTO moz_bookmarks VALUES(1,0,'',NULL,2,0),(2,1,'Bookmarks Toolbar',NULL,2,0),(3,2,'研究',NULL,2,0),(4,3,'论文',1,1,0);").unwrap();
        let batch = read_browser(&f.source(true), &request(true, true));
        assert!(batch.problems.is_empty());
        assert_eq!(batch.history[0].visited, 1_700_000_000);
        assert_eq!(batch.bookmarks[0].folders, ["Bookmarks Toolbar", "研究"]);
        assert_eq!(batch.bookmarks[0].title, "论文");
    }

    #[test]
    fn discovers_multiple_profiles_and_firefox_ini_without_exposing_paths() {
        let f = Fixture::new();
        let local = f.0.join("Local");
        let roaming = f.0.join("Roaming");
        let edge = local.join("Microsoft/Edge/User Data");
        for profile in ["Default", "Profile 1"] {
            std::fs::create_dir_all(edge.join(profile)).unwrap();
            std::fs::write(edge.join(profile).join("Bookmarks"), "{}").unwrap();
        }
        std::fs::write(
            edge.join("Local State"),
            r#"{"profile":{"info_cache":{"Profile 1":{"name":"工作账户"}}}}"#,
        )
        .unwrap();
        let firefox = roaming.join("Mozilla/Firefox");
        let custom = f.0.join("outside-profile");
        std::fs::create_dir_all(&firefox).unwrap();
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(custom.join("places.sqlite"), []).unwrap();
        std::fs::write(
            firefox.join("profiles.ini"),
            format!(
                "[Profile0]\nName=custom\nIsRelative=0\nPath={}\n",
                custom.display()
            ),
        )
        .unwrap();
        let sources = detect_at(&local, &roaming);
        assert_eq!(sources.len(), 3);
        assert!(sources.iter().any(|s| s.name.contains("工作账户")));
        assert_eq!(
            sources.iter().map(|s| &s.id).collect::<HashSet<_>>().len(),
            3
        );
        assert_eq!(
            sources.iter().map(|s| &s.id).collect::<Vec<_>>(),
            detect_at(&local, &roaming)
                .iter()
                .map(|s| &s.id)
                .collect::<Vec<_>>()
        );
        let json = serde_json::to_string(&sources).unwrap();
        assert!(!json.contains("path"));
        assert!(!json.contains(&f.0.to_string_lossy().to_string()));
    }

    #[test]
    fn csv_supports_bom_unicode_quoted_newlines_and_manager_headers() {
        let csv = "\u{feff}name,url,username,password,note\r\n网站,https://example.com/login,用户,\"p,\"\"quote\"\"\",\"line1\nline2\"\r\nunsafe,javascript:alert(1),a,b,c\r\n";
        let (rows, skipped) = parse_csv(csv.as_bytes()).unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "https://example.com");
        assert_eq!(rows[0].1.username, "用户");
        assert_eq!(rows[0].1.password, "p,\"quote\"");
        assert_eq!(rows[0].1.notes, "line1\nline2");
        let text = "login_uri,login_username,login_password,login_totp\nhttps://example.org,用户,密码,otp\n";
        let bytes: Vec<_> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let (rows, _) = parse_csv(&bytes).unwrap();
        assert_eq!(rows[0].1.otp, "otp");
    }

    #[test]
    fn malformed_csv_rejects_entire_file_and_never_echoes_secrets() {
        let secret = "private-test-password";
        let csv = format!("url,username,password\nhttps://example.com,user,{secret}\nhttps://example.org,incomplete\n");
        let error = parse_csv(csv.as_bytes()).err().unwrap();
        assert!(!error.contains(secret));
        assert!(parse_csv(b"name,value\na,b").is_err());
        assert!(parse_csv(b"url,username,password\njavascript:bad,u,p").is_err());
        assert!(parse_csv(&[0xff, 0xfe, 0]).is_err());
        let batch = read_csv(Path::new("missing-password-file.csv"));
        assert!(!batch.problems.is_empty());
    }
}
