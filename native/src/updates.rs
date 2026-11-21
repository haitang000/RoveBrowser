//! Update downloads for the two release channels.
//! Preview reads GitHub Releases. Canary follows the GitLab pipeline: every
//! commit on main builds the `windows-installer` job, so a new build is a new
//! commit even when the package version stays the same.

use crate::update_install::{PackageKind, PreparedUpdate};
use serde_json::Value;
use std::cmp::Ordering;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use url::Url;
use uuid::Uuid;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Networking::WinHttp::*,
        Security::Cryptography::{
            BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
            BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE,
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS, BCRYPT_SHA256_ALGORITHM,
        },
    },
};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Set by GitLab CI while `windows-installer` compiles this binary. Preview
/// builds leave it empty, so they are not mistaken for that pipeline commit.
pub const COMMIT: Option<&str> = option_env!("CI_COMMIT_SHA");
const PREVIEW_API: &str = "https://api.github.com/repos/haitang000/RoveBrowser/releases/latest";
const CANARY_PIPELINES: &str = "https://hub.haitang000.cn/api/v4/projects/haitang000%2Frovebrowser/pipelines?ref=main&status=success&order_by=id&sort=desc&per_page=5";
const JSON_LIMIT: u64 = 1024 * 1024;
const SUM_LIMIT: u64 = 64 * 1024;
const PACKAGE_LIMIT: u64 = 256 * 1024 * 1024;
const API_TIMEOUTS: [i32; 4] = [8_000, 8_000, 15_000, 20_000];
const DOWNLOAD_TIMEOUTS: [i32; 4] = [10_000, 10_000, 30_000, 60_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Preview,
    Canary,
}
impl Channel {
    pub fn parse(value: &str) -> Self {
        if value == "canary" { Self::Canary } else { Self::Preview }
    }
    pub fn as_str(self) -> &'static str {
        match self { Self::Preview => "preview", Self::Canary => "canary" }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Preview => "Preview", Self::Canary => "Canary" }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<Id>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Id {
    Num(u64),
    Text(String),
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => cmp_pre(&self.pre, &other.pre),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
fn cmp_pre(left: &[Id], right: &[Id]) -> Ordering {
    for (a, b) in left.iter().zip(right) {
        let order = match (a, b) {
            (Id::Num(a), Id::Num(b)) => a.cmp(b),
            (Id::Num(_), Id::Text(_)) => Ordering::Less,
            (Id::Text(_), Id::Num(_)) => Ordering::Greater,
            (Id::Text(a), Id::Text(b)) => a.cmp(b),
        };
        if order != Ordering::Equal { return order; }
    }
    left.len().cmp(&right.len())
}
fn ident(raw: &str) -> Option<Id> {
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return None; }
    if raw.chars().all(|c| c.is_ascii_digit()) { return raw.parse().ok().map(Id::Num); }
    Some(Id::Text(raw.to_string()))
}
fn parse_version(tag: &str) -> Option<(Version, String)> {
    let trimmed = tag.trim();
    let bare = trimmed.strip_prefix('v').or_else(|| trimmed.strip_prefix('V')).unwrap_or(trimmed);
    let bare = bare.split_once('+').map(|(version, _)| version).unwrap_or(bare);
    if bare.is_empty() || bare.len() > 80 { return None; }
    let (core, pre) = bare.split_once('-').map(|(core, pre)| (core, Some(pre))).unwrap_or((bare, None));
    let mut numbers = core.split('.');
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    let patch = numbers.next()?.parse().ok()?;
    if numbers.next().is_some() { return None; }
    let pre = match pre {
        Some(pre) => pre.split('.').map(ident).collect::<Option<Vec<_>>>()?,
        None => Vec::new(),
    };
    Some((Version { major, minor, patch, pre }, bare.to_string()))
}

#[derive(Clone, Debug)]
struct Release {
    version: Version,
    version_text: String,
    notes: String,
    asset_name: String,
    asset_url: String,
    checksum_url: Option<String>,
    /// SHA-256 already read from the canary job's checksum file.
    checksum: Option<String>,
    /// Git commit of a canary pipeline build.
    commit: Option<String>,
}
#[derive(Clone, Debug)]
enum Outcome {
    Current,
    Ahead { remote: String },
    Available(Release),
}
#[derive(Clone, Debug)]
pub struct Report {
    pub phase: &'static str,
    pub channel: &'static str,
    pub version: String,
    pub message: String,
}

impl Report {
    pub fn new(channel: Channel, phase: &'static str, message: impl Into<String>) -> Self {
        Self { phase, channel: channel.as_str(), version: String::new(), message: message.into() }
    }
}

#[derive(Debug)]
pub struct Offer {
    pub id: String,
    pub channel: Channel,
    pub report: Report,
    target: PathBuf,
    kind: PackageKind,
    release: Release,
}

pub fn check(channel: Channel) -> Result<Offer, Report> {
    let target = std::env::current_exe()
        .map_err(|_| Report::new(channel, "error", "无法定位当前的 Rove 程序。"))?;
    let kind = PackageKind::for_executable(&target);
    let outcome = lookup(channel, kind);
    let report = report(channel, outcome.as_ref().map_err(String::as_str));
    match outcome {
        Ok(Outcome::Available(release)) => Ok(Offer {
            id: Uuid::new_v4().to_string(),
            channel,
            report,
            target,
            kind,
            release,
        }),
        _ => Err(report),
    }
}

impl Offer {
    pub fn prepare(self, progress: impl Fn(Report)) -> Result<(PreparedUpdate, Report), Report> {
        let Self { channel, mut report, target, kind, release, .. } = self;
        report.phase = "downloading";
        report.message = "正在下载并校验更新包。".into();
        progress(report.clone());
        let result = (|| {
            let update = PreparedUpdate::new(target, kind)?;
            download(channel, &release, &update.package())?;
            if kind == PackageKind::Portable {
                report.phase = "extracting";
                report.message = "正在解压更新包。".into();
                progress(report.clone());
            }
            update.stage()?;
            Ok(update)
        })();
        match result {
            Ok(update) => {
                report.phase = "installing";
                report.message = "正在安装更新，Rove 即将重启。".into();
                Ok((update, report))
            }
            Err(message) => {
                report.phase = "error";
                report.message = message;
                Err(report)
            }
        }
    }
}

fn report(channel: Channel, outcome: Result<&Outcome, &str>) -> Report {
    if channel == Channel::Canary {
        return canary_report(outcome);
    }
    match outcome {
        Ok(Outcome::Current) => Report {
            phase: "current",
            channel: channel.as_str(),
            version: CURRENT.into(),
            message: format!("{} 通道已是最新版本 {CURRENT}。", channel.label()),
        },
        Ok(Outcome::Ahead { remote }) => Report {
            phase: "ahead",
            channel: channel.as_str(),
            version: remote.clone(),
            message: format!("当前版本 {CURRENT} 比 {} 通道的 {remote} 更新。", channel.label()),
        },
        Ok(Outcome::Available(release)) => {
            let notes = brief(&release.notes);
            let message = if notes.is_empty() {
                format!("发现新版本 {}。", release.version_text)
            } else {
                format!("发现新版本 {}。{notes}", release.version_text)
            };
            Report { phase: "available", channel: channel.as_str(), version: release.version_text.clone(), message }
        }
        Err(message) => Report::new(channel, "error", message),
    }
}
fn canary_report(outcome: Result<&Outcome, &str>) -> Report {
    match outcome {
        Ok(Outcome::Current) => {
            let sha = running_commit().map(short_sha).unwrap_or_else(|| "未知".into());
            Report { phase: "current", channel: "canary", version: sha.clone(), message: format!("Canary 已是最新构建 {sha}。") }
        }
        Ok(Outcome::Available(release)) => {
            let sha = release.commit.as_deref().map(short_sha).unwrap_or_else(|| "未知".into());
            Report {
                phase: "available",
                channel: "canary",
                version: sha.clone(),
                message: format!("发现新的 Canary 构建 {sha}（{}）。", release.version_text),
            }
        }
        Ok(Outcome::Ahead { remote }) => Report {
            phase: "ahead",
            channel: "canary",
            version: remote.clone(),
            message: "Canary 构建还没有新的安装包。".into(),
        },
        Err(message) => Report::new(Channel::Canary, "error", message),
    }
}
fn lookup(channel: Channel, kind: PackageKind) -> Result<Outcome, String> {
    match channel {
        Channel::Preview => {
            let body = get_text(PREVIEW_API, channel, true, "application/vnd.github+json", JSON_LIMIT)?;
            let release = parse_github(&body, kind)?;
            let current = parse_version(CURRENT).map(|(version, _)| version).ok_or("无法识别当前版本。")?;
            Ok(judge(&current, release))
        }
        Channel::Canary => lookup_canary(kind),
    }
}
fn lookup_canary(kind: PackageKind) -> Result<Outcome, String> {
    let pipelines = parse_pipelines(&get_text(CANARY_PIPELINES, Channel::Canary, true, "application/json", JSON_LIMIT)?)?;
    let mut last_error = "Canary 通道还没有 Windows 安装包。".to_string();
    for (id, sha) in pipelines.into_iter().take(5) {
        if running_commit().is_some_and(|running| running.eq_ignore_ascii_case(&sha)) {
            return Ok(Outcome::Current);
        }
        let jobs_url = canary_jobs_url(id)?;
        let jobs = match get_text(&jobs_url, Channel::Canary, true, "application/json", JSON_LIMIT) {
            Ok(body) => body,
            Err(error) => { last_error = error; continue; }
        };
        let Some(job_id) = installer_job_id(&jobs) else { continue };
        let sums_url = canary_sums_url(job_id)?;
        let sums = match get_text(&sums_url, Channel::Canary, true, "text/plain", SUM_LIMIT) {
            Ok(body) => body,
            Err(error) => { last_error = error; continue; }
        };
        let mut release = match parse_canary_sums(&sums, job_id, kind) {
            Ok(release) => release,
            Err(error) => { last_error = error; continue; }
        };
        release.commit = Some(sha);
        return Ok(Outcome::Available(release));
    }
    Err(last_error)
}
fn judge(current: &Version, release: Release) -> Outcome {
    match release.version.cmp(current) {
        Ordering::Greater => Outcome::Available(release),
        Ordering::Equal => Outcome::Current,
        Ordering::Less => Outcome::Ahead { remote: release.version_text },
    }
}

fn parse_github(body: &str, kind: PackageKind) -> Result<Release, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "无法读取 Preview 发布信息。")?;
    if value.get("tag_name").is_none() {
        let message = value.get("message").and_then(Value::as_str).unwrap_or("");
        if message.to_ascii_lowercase().contains("rate limit") {
            return Err("GitHub 暂时限制了更新检查，请稍后再试。".into());
        }
        return Err("Preview 通道还没有发布。".into());
    }
    release_from_github(&value, kind)
}
fn parse_pipelines(body: &str) -> Result<Vec<(u64, String)>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "无法读取 Canary 流水线。")?;
    let list = value.as_array().ok_or("Canary 通道还没有构建。")?;
    let mut pipelines = vec![];
    for item in list {
        if item.get("ref").and_then(Value::as_str) != Some("main") || item.get("status").and_then(Value::as_str) != Some("success") {
            continue;
        }
        let Some(id) = item.get("id").and_then(Value::as_u64) else { continue };
        let Some(sha) = item.get("sha").and_then(Value::as_str).filter(|sha| is_commit(sha)) else { continue };
        pipelines.push((id, sha.to_ascii_lowercase()));
    }
    pipelines.sort_by(|a, b| b.0.cmp(&a.0));
    if pipelines.is_empty() { return Err("Canary 通道还没有构建。".into()); }
    Ok(pipelines)
}
fn installer_job_id(body: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(body).ok()?;
    value.as_array()?.iter().find_map(|job| {
        if job.get("name").and_then(Value::as_str) == Some("windows-installer")
            && job.get("status").and_then(Value::as_str) == Some("success")
            && job.get("ref").and_then(Value::as_str) == Some("main")
        {
            job.get("id").and_then(Value::as_u64)
        } else {
            None
        }
    })
}
fn running_commit() -> Option<&'static str> {
    COMMIT.filter(|sha| is_commit(sha))
}
fn is_commit(sha: &str) -> bool {
    sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
}
fn short_sha(sha: &str) -> String {
    if is_commit(sha) { sha[..8].to_ascii_lowercase() } else { "未知".into() }
}
fn parse_canary_sums(body: &str, job_id: u64, kind: PackageKind) -> Result<Release, String> {
    let mut best: Option<Release> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else { continue };
        let Some(file) = parts.next() else { continue };
        let file = file.trim_start_matches('*');
        if parts.next().is_some() || !sha256_digest(hash) {
            continue;
        }
        let Some((version, version_text)) = package_version(file, kind).and_then(parse_version)
        else { continue };
        let release = Release {
            version,
            version_text,
            notes: "来自 GitLab CI 的 windows-installer。".into(),
            asset_name: file.to_string(),
            asset_url: canary_artifact_url(job_id, file)?,
            checksum_url: None,
            checksum: Some(hash.to_ascii_lowercase()),
            commit: None,
        };
        if best.as_ref().map(|current| release.version > current.version).unwrap_or(true) { best = Some(release); }
    }
    best.ok_or_else(|| "Canary 通道还没有 Windows 安装包。".into())
}
fn canary_jobs_url(pipeline_id: u64) -> Result<String, String> {
    pinned_canary_url(
        &format!("https://hub.haitang000.cn/api/v4/projects/haitang000%2Frovebrowser/pipelines/{pipeline_id}/jobs"),
        &format!("/api/v4/projects/haitang000%2Frovebrowser/pipelines/{pipeline_id}/jobs"),
    )
}
fn canary_sums_url(job_id: u64) -> Result<String, String> {
    pinned_canary_url(
        &format!("https://hub.haitang000.cn/api/v4/projects/haitang000%2Frovebrowser/jobs/{job_id}/artifacts/release/SHA256SUMS.txt"),
        &format!("/api/v4/projects/haitang000%2Frovebrowser/jobs/{job_id}/artifacts/release/SHA256SUMS.txt"),
    )
}
fn canary_artifact_url(job_id: u64, name: &str) -> Result<String, String> {
    pinned_canary_url(
        &format!("https://hub.haitang000.cn/api/v4/projects/haitang000%2Frovebrowser/jobs/{job_id}/artifacts/release/{name}"),
        &format!("/api/v4/projects/haitang000%2Frovebrowser/jobs/{job_id}/artifacts/release/{name}"),
    )
}
fn pinned_canary_url(raw: &str, expected: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|_| "发布文件地址无效。")?;
    if !allowed(Channel::Canary, true, &url) || request_target(&url)? != expected {
        return Err("发布文件地址无效。".into());
    }
    Ok(url.to_string())
}
fn release_from_github(item: &Value, kind: PackageKind) -> Result<Release, String> {
    let tag = item.get("tag_name").and_then(Value::as_str).ok_or("无法识别发布版本。")?;
    let (version, version_text) = parse_version(tag).ok_or("无法识别发布版本。")?;
    let notes = item.get("body").and_then(Value::as_str).unwrap_or("").to_string();
    let assets = item.get("assets").and_then(Value::as_array).ok_or("这个版本没有 Windows 安装包。")?;
    let mut package = None;
    let mut checksum_url = None;
    for asset in assets {
        let name = asset.get("name").and_then(Value::as_str).unwrap_or("");
        let raw = asset.get("browser_download_url").and_then(Value::as_str).unwrap_or("");
        if name == "SHA256SUMS.txt" {
            checksum_url = Some(preview_asset_url(raw, tag, name)?);
            continue;
        }
        if package_version(name, kind) != Some(version_text.as_str()) { continue; }
        let url = preview_asset_url(raw, tag, name)?;
        let digest = asset.get("digest").and_then(Value::as_str)
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .filter(|digest| sha256_digest(digest)).map(str::to_owned);
        package = Some((name.to_owned(), url, digest));
    }
    let Some((asset_name, asset_url, checksum)) = package else { return Err("这个版本没有适用于当前 Rove 的更新包。".into()); };
    Ok(Release { version, version_text, notes, asset_name, asset_url, checksum_url, checksum, commit: None })
}
fn package_version(name: &str, kind: PackageKind) -> Option<&str> {
    let version = name.strip_prefix("Rove-")?.strip_suffix(kind.suffix())?;
    (!version.is_empty()
        && version.len() <= 80
        && !version.contains("..")
        && version.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-')))
        .then_some(version)
}
fn preview_asset_url(raw: &str, tag: &str, name: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|_| "发布文件地址无效。")?;
    let expected = format!("/haitang000/RoveBrowser/releases/download/{tag}/{name}");
    if !allowed(Channel::Preview, false, &url) || url.host_str() != Some("github.com") || request_target(&url)? != expected {
        return Err("发布文件不在该版本的发布页上。".into());
    }
    Ok(url.to_string())
}
fn allowed(channel: Channel, api: bool, url: &Url) -> bool {
    if url.scheme() != "https" || !matches!(url.port(), None | Some(443)) || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else { return false; };
    match (channel, api) {
        (Channel::Preview, true) => host == "api.github.com",
        (Channel::Preview, false) => host == "github.com" || host.ends_with(".githubusercontent.com"),
        (Channel::Canary, _) => host == "hub.haitang000.cn",
    }
}
fn brief(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        let ch = if ch.is_control() { ' ' } else { ch };
        if ch == ' ' && out.ends_with(' ') { continue; }
        out.push(ch);
        if out.chars().count() == 160 { break; }
    }
    let trimmed = out.trim().to_string();
    if text.chars().count() > 160 && !trimmed.is_empty() { format!("{trimmed}…") } else { trimmed }
}

fn download(channel: Channel, release: &Release, path: &Path) -> Result<(), String> {
    let url = Url::parse(&release.asset_url).map_err(|_| "发布文件地址无效。")?;
    let expected = if let Some(expected) = &release.checksum {
        expected.clone()
    } else if let Some(checksum) = &release.checksum_url {
        let sums = get_text(checksum, channel, false, "text/plain", SUM_LIMIT)?;
        checksum_for(&sums, &release.asset_name).ok_or("找不到这个更新包的校验值。")?
    } else {
        return Err("发布页缺少更新包的 SHA-256 校验值。".into());
    };
    save_to(&url, channel, path, PACKAGE_LIMIT)?;
    if !sha256_file(path)?.eq_ignore_ascii_case(&expected) {
        return Err("更新包校验失败，已取消安装。".into());
    }
    Ok(())
}
fn sha256_digest(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    for line in sums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(file)) = (parts.next(), parts.next()) else { continue; };
        if parts.next().is_some() { continue; }
        if file.trim_start_matches('*') == name && sha256_digest(hash) {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|_| "无法读取安装包。")?;
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE(std::ptr::null_mut());
        if BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_SHA256_ALGORITHM, PCWSTR::null(), BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS::default()).is_err() {
            return Err("无法计算校验值。".into());
        }
        let mut hash = BCRYPT_HASH_HANDLE(std::ptr::null_mut());
        if BCryptCreateHash(alg, &mut hash, None, None, 0).is_err() {
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return Err("无法计算校验值。".into());
        }
        let mut buf = [0u8; 65536];
        loop {
            let read = match file.read(&mut buf) {
                Ok(read) => read,
                Err(_) => {
                    let _ = BCryptDestroyHash(hash);
                    let _ = BCryptCloseAlgorithmProvider(alg, 0);
                    return Err("无法读取安装包。".into());
                }
            };
            if read == 0 { break; }
            if BCryptHashData(hash, &buf[..read], 0).is_err() {
                let _ = BCryptDestroyHash(hash);
                let _ = BCryptCloseAlgorithmProvider(alg, 0);
                return Err("无法计算校验值。".into());
            }
        }
        let mut out = [0u8; 32];
        let finished = BCryptFinishHash(hash, &mut out, 0);
        let _ = BCryptDestroyHash(hash);
        let _ = BCryptCloseAlgorithmProvider(alg, 0);
        if finished.is_err() { return Err("无法计算校验值。".into()); }
        Ok(out.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}
fn get_text(raw: &str, channel: Channel, api: bool, accept: &str, max: u64) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|_| "更新地址无效。")?;
    let bytes = get_bytes(&url, channel, api, accept, max)?;
    String::from_utf8(bytes).map_err(|_| "更新服务器返回了无法识别的内容。".into())
}
fn get_bytes(url: &Url, channel: Channel, api: bool, accept: &str, max: u64) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    fetch(url, channel, api, accept, max, API_TIMEOUTS, &mut |chunk| { body.extend_from_slice(chunk); Ok(()) })?;
    if body.is_empty() { return Err("更新服务器没有返回发布信息。".into()); }
    Ok(body)
}
fn save_to(url: &Url, channel: Channel, path: &Path, max: u64) -> Result<(), String> {
    let mut file = std::fs::File::create_new(path).map_err(|_| "无法保存安装包。")?;
    let result = fetch(url, channel, false, "application/octet-stream", max, DOWNLOAD_TIMEOUTS, &mut |chunk| {
        file.write_all(chunk).map_err(|_| "无法保存安装包。".into())
    });
    drop(file);
    if result.is_err() { let _ = std::fs::remove_file(path); }
    result
}
fn fetch(start: &Url, channel: Channel, api: bool, accept: &str, max: u64, timeouts: [i32; 4], write: &mut dyn FnMut(&[u8]) -> Result<(), String>) -> Result<(), String> {
    let mut current = start.clone();
    for hop in 0..5 {
        if !allowed(channel, api, &current) { return Err("更新地址不在该通道的服务器上。".into()); }
        match exchange(&current, accept, max, timeouts, write)? {
            Hop::Done => return Ok(()),
            Hop::Redirect(location) => {
                current = current.join(&location).or_else(|_| Url::parse(&location)).map_err(|_| "更新地址无效。".to_string())?;
                if hop == 4 { return Err("更新地址重定向过多。".into()); }
            }
        }
    }
    Err("更新地址重定向过多。".into())
}
enum Hop { Done, Redirect(String) }
struct Session { session: *mut core::ffi::c_void, connection: *mut core::ffi::c_void, request: *mut core::ffi::c_void }
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            if !self.request.is_null() { let _ = WinHttpCloseHandle(self.request); }
            if !self.connection.is_null() { let _ = WinHttpCloseHandle(self.connection); }
            if !self.session.is_null() { let _ = WinHttpCloseHandle(self.session); }
        }
    }
}
fn exchange(url: &Url, accept: &str, max: u64, timeouts: [i32; 4], write: &mut dyn FnMut(&[u8]) -> Result<(), String>) -> Result<Hop, String> {
    let host = wide(url.host_str().ok_or("更新地址无效。")?);
    let target = request_target(url)?;
    let target = wide(&target);
    let headers = wide(&format!("User-Agent: Rove/{CURRENT}\r\nAccept: {accept}\r\nAccept-Encoding: identity\r\n"));
    unsafe {
        let mut handles = Session { session: std::ptr::null_mut(), connection: std::ptr::null_mut(), request: std::ptr::null_mut() };
        handles.session = WinHttpOpen(w!("Rove"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0);
        if handles.session.is_null() { return Err(offline()); }
        handles.connection = WinHttpConnect(handles.session, PCWSTR(host.as_ptr()), INTERNET_DEFAULT_HTTPS_PORT, 0);
        if handles.connection.is_null() { return Err(offline()); }
        handles.request = WinHttpOpenRequest(handles.connection, w!("GET"), PCWSTR(target.as_ptr()), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE);
        if handles.request.is_null() { return Err(offline()); }
        let disable = WINHTTP_DISABLE_REDIRECTS.to_ne_bytes();
        if WinHttpSetOption(Some(handles.request), WINHTTP_OPTION_DISABLE_FEATURE, Some(&disable)).is_err() {
            return Err(offline());
        }
        if WinHttpSetTimeouts(handles.request, timeouts[0], timeouts[1], timeouts[2], timeouts[3]).is_err() { return Err(offline()); }
        if WinHttpSendRequest(handles.request, Some(&headers[..headers.len() - 1]), None, 0, 0, 0).is_err()
            || WinHttpReceiveResponse(handles.request, std::ptr::null_mut()).is_err()
        {
            return Err(offline());
        }
        let status = status_code(handles.request).ok_or_else(offline)?;
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            return header_text(handles.request, WINHTTP_QUERY_LOCATION).filter(|location| !location.is_empty()).map(Hop::Redirect).ok_or_else(|| "更新地址无效。".into());
        }
        if status != 200 {
            eprintln!("Rove update: HTTP {status}");
            return Err(status_message(status));
        }
        if let Some(length) = header_text(handles.request, WINHTTP_QUERY_CONTENT_LENGTH).and_then(|text| text.trim().parse::<u64>().ok()) {
            if length > max { return Err("更新服务器返回的内容过大。".into()); }
        }
        let mut received = 0u64;
        loop {
            let mut read = 0u32;
            let mut chunk = [0u8; 16384];
            if WinHttpReadData(handles.request, chunk.as_mut_ptr().cast(), chunk.len() as u32, &mut read).is_err() { return Err(offline()); }
            if read == 0 { break; }
            received += read as u64;
            if received > max { return Err("更新服务器返回的内容过大。".into()); }
            write(&chunk[..read as usize])?;
        }
        Ok(Hop::Done)
    }
}
fn status_message(status: u32) -> String {
    if status == 403 || status == 429 { "更新服务器暂时限制了检查，请稍后再试。".into() }
    else if status == 404 { "这个通道还没有发布。".into() }
    else { "更新服务器没有返回发布信息。".into() }
}
fn offline() -> String {
    "无法连接更新服务器，请稍后再试。".into()
}
fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(Some(0)).collect() }
fn request_target(url: &Url) -> Result<String, String> {
    let Some((_, after)) = url.as_str().split_once("://") else { return Err("更新地址无效。".into()); };
    let target = after.find('/').map(|index| &after[index..]).unwrap_or("/");
    if target.is_empty() || target.len() > 2048 || target.contains([' ', '\r', '\n', '#']) { return Err("更新地址无效。".into()); }
    Ok(target.to_string())
}
fn status_code(request: *mut core::ffi::c_void) -> Option<u32> {
    unsafe {
        let mut status = 0u32;
        let mut length = 4u32;
        WinHttpQueryHeaders(request, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, PCWSTR::null(), Some(&mut status as *mut u32 as *mut _), &mut length, std::ptr::null_mut()).ok()?;
        Some(status)
    }
}
fn header_text(request: *mut core::ffi::c_void, level: u32) -> Option<String> {
    unsafe {
        let mut size = 0u32;
        let _ = WinHttpQueryHeaders(request, level, PCWSTR::null(), None, &mut size, std::ptr::null_mut());
        if size < 2 || size > 8192 { return None; }
        let mut buf = vec![0u16; size as usize / 2 + 1];
        WinHttpQueryHeaders(request, level, PCWSTR::null(), Some(buf.as_mut_ptr().cast()), &mut size, std::ptr::null_mut()).ok()?;
        let chars = (size as usize / 2).min(buf.len());
        let end = buf[..chars].iter().position(|c| *c == 0).unwrap_or(chars);
        String::from_utf16(&buf[..end]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn version(tag: &str) -> Version { parse_version(tag).unwrap().0 }
    #[test]
    fn versions_follow_semver_and_preview_does_not_downgrade_to_canary() {
        assert!(version("v0.1.3") > version("0.1.2"));
        assert!(version("0.1.2") > version("0.1.2-canary.1"));
        assert!(version("0.1.3-canary.1") > version("0.1.2"));
        assert!(version("0.1.3-canary.2") > version("0.1.3-canary.1"));
        assert!(version("1.0.0-alpha") < version("1.0.0-alpha.1"));
        assert!(version("1.0.0-alpha.1") < version("1.0.0-alpha.beta"));
        assert!(version("1.0.0-rc.1") < version("1.0.0"));
        assert!(parse_version("v0.1").is_none());
        assert!(parse_version("latest").is_none());
    }
    #[test]
    fn github_release_selects_the_package_for_each_installation() {
        let body = r#"{
            "tag_name":"v0.2.0",
            "body":"窗口拖动\n修复",
            "assets":[
                {"name":"Rove-0.2.0-windows-x64.zip","browser_download_url":"https://github.com/haitang000/RoveBrowser/releases/download/v0.2.0/Rove-0.2.0-windows-x64.zip","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                {"name":"Rove-0.2.0-windows-x64-setup.exe","browser_download_url":"https://github.com/haitang000/RoveBrowser/releases/download/v0.2.0/Rove-0.2.0-windows-x64-setup.exe"},
                {"name":"SHA256SUMS.txt","browser_download_url":"https://github.com/haitang000/RoveBrowser/releases/download/v0.2.0/SHA256SUMS.txt"}
            ]
        }"#;
        let release = parse_github(body, PackageKind::Setup).unwrap();
        assert_eq!(release.version_text, "0.2.0");
        assert!(release.asset_url.contains("setup.exe"));
        assert!(release.checksum_url.is_some());
        let portable = parse_github(body, PackageKind::Portable).unwrap();
        assert!(portable.asset_url.ends_with("-windows-x64.zip"));
        assert_eq!(portable.checksum, Some("a".repeat(64)));
        assert!(parse_github(r#"{"message":"API rate limit exceeded"}"#, PackageKind::Setup).unwrap_err().contains("稍后再试"));
        let hostile = r#"{
            "tag_name":"v9.9.9",
            "assets":[{"name":"Rove-9.9.9-windows-x64-setup.exe","browser_download_url":"https://evil.example/Rove-9.9.9-windows-x64-setup.exe"}]
        }"#;
        assert!(parse_github(hostile, PackageKind::Setup).is_err());
    }
    #[test]
    fn canary_follows_each_pipeline_commit_not_the_package_version() {
        let hash = "841f1241ba975d6927fcf5b7f045dfefaf2bc2001e212c25c6d6e0f10e6c0818";
        let sums = format!("{hash}  Rove-0.1.2-windows-x64-setup.exe\r\n{}  Rove-0.1.2-windows-x64.zip\r\n", "b".repeat(64));
        let release = parse_canary_sums(&sums, 147, PackageKind::Setup).unwrap();
        assert!(release.asset_url.contains("/jobs/147/artifacts/release/Rove-0.1.2-windows-x64-setup.exe"));
        assert!(!release.asset_url.contains("job="));
        let pipelines = parse_pipelines(r#"[
            {"id":70,"sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ref":"feature","status":"success"},
            {"id":72,"sha":"F99534F3508FDFF6F7D17FAF0A28D7CB54C99E38","ref":"main","status":"success"},
            {"id":71,"sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","ref":"main","status":"failed"}
        ]"#).unwrap();
        assert_eq!(pipelines, vec![(72, "f99534f3508fdff6f7d17faf0a28d7cb54c99e38".into())]);
        let jobs = r#"[{"id":151,"name":"frontend","status":"success","ref":"main"},{"id":152,"name":"windows-installer","status":"success","ref":"main"}]"#;
        assert_eq!(installer_job_id(jobs), Some(152));
        let built = parse_canary_sums(&sums, 152, PackageKind::Portable).unwrap();
        assert!(built.asset_url.ends_with("/jobs/152/artifacts/release/Rove-0.1.2-windows-x64.zip"));
        assert_eq!(built.checksum, Some("b".repeat(64)));
        assert_eq!(release.version, built.version);
        assert!(parse_canary_sums("abcd  Rove-0.2.0-windows-x64.zip\n", 1, PackageKind::Portable).is_err());
        let target = request_target(&Url::parse(CANARY_PIPELINES).unwrap()).unwrap();
        assert!(target.contains("haitang000%2Frovebrowser"));
        assert!(target.contains("ref=main"));
    }
    #[test]
    fn checksum_lines_match_the_installer_name() {
        let hash = "a".repeat(64);
        let sums = format!("{hash}  Rove-0.2.0-windows-x64-setup.exe\n{}  other.zip\n", "b".repeat(64));
        assert_eq!(checksum_for(&sums, "Rove-0.2.0-windows-x64-setup.exe").as_deref(), Some(hash.as_str()));
        assert!(checksum_for(&sums, "missing.exe").is_none());
    }
    #[test]
    fn judging_a_release_offers_only_a_newer_version() {
        let older = parse_github(r#"{
            "tag_name":"v0.0.1",
            "assets":[{"name":"Rove-0.0.1-windows-x64-setup.exe","browser_download_url":"https://github.com/haitang000/RoveBrowser/releases/download/v0.0.1/Rove-0.0.1-windows-x64-setup.exe"}]
        }"#, PackageKind::Setup).unwrap();
        let current = parse_version(CURRENT).unwrap().0;
        assert!(matches!(judge(&current, older), Outcome::Ahead { .. }));
    }
}
