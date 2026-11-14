//! Download history and state, ported from Zigzag's `Web/DownloadManager.swift`
//! (Apache-2.0): one row per started download, the five-state machine the
//! panel renders, a 50-item recent list and Clear that never throws away a
//! half-finished file. The WebView2 operation lives in the host; this module is
//! pure data so it can be unit tested without a browser.
use serde::{Deserialize, Serialize};
use webview2_com::Microsoft::Web::WebView2::Win32::*;

/// Zigzag keeps the newest 50 finished/paused downloads between launches.
pub const MAX_ITEMS: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DownloadState {
    InProgress,
    Paused,
    Finished,
    Failed,
    Cancelled,
}

impl DownloadState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::InProgress | Self::Paused)
    }
    pub fn is_clearable(self) -> bool {
        !self.is_active()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    /// The address the browser fetched, as WebView2 reports it.
    pub url: String,
    /// Where the file is (or was being) written.
    pub path: String,
    pub filename: String,
    pub received: u64,
    /// 0 when the server did not send a length.
    pub total: u64,
    pub state: DownloadState,
    pub failure: Option<String>,
    /// Unix seconds, for the newest-first list.
    pub started: u64,
    pub finished: Option<u64>,
    /// Filled in when the list is published: a finished file may have been
    /// moved or deleted since it landed.
    #[serde(default)]
    pub exists: bool,
}

impl DownloadItem {
    /// Only the tests need the ratio; the UI computes its own from the same fields.
    #[cfg(test)]
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.received as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }
    pub fn is_clearable(&self) -> bool {
        self.state.is_clearable()
    }
}

/// Recent downloads, newest first, with Zigzag's clear rules.
#[derive(Default)]
pub struct Downloads {
    pub items: Vec<DownloadItem>,
}

impl Downloads {
    /// Restores the saved list. A download that was running when the app quit
    /// cannot be resumed (WebView2 keeps no resume data across processes), so
    /// Zigzag's paused-with-resume data case becomes a plain interruption.
    pub fn load(items: Vec<DownloadItem>) -> Self {
        let mut downloads = Self { items };
        downloads.items.truncate(MAX_ITEMS);
        let now = crate::model::now_secs();
        for item in &mut downloads.items {
            if item.is_active() {
                item.state = DownloadState::Failed;
                item.failure = Some("上次退出时下载未完成".into());
                item.finished = Some(now);
            }
        }
        downloads
    }
    pub fn insert(&mut self, item: DownloadItem) {
        self.items.retain(|existing| existing.id != item.id);
        self.items.insert(0, item);
        // Prefer dropping the oldest finished row; if everything is active the
        // oldest active one still goes, so the list stays bounded.
        if self.items.len() > MAX_ITEMS {
            let index = self
                .items
                .iter()
                .rposition(|item| !item.is_active())
                .unwrap_or(self.items.len() - 1);
            self.items.remove(index);
        }
    }
    pub fn get(&self, id: &str) -> Option<&DownloadItem> {
        self.items.iter().find(|item| item.id == id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut DownloadItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }
    pub fn set_progress(&mut self, id: &str, received: u64, total: u64) {
        if let Some(item) = self.get_mut(id).filter(|item| item.is_active()) {
            item.received = received;
            if total > 0 {
                item.total = total;
            }
            item.exists = false;
        }
    }
    pub fn finish(&mut self, id: &str, state: DownloadState, failure: Option<String>) -> bool {
        let now = crate::model::now_secs();
        if let Some(item) = self.get_mut(id) {
            item.state = state;
            item.finished = Some(now);
            item.failure = failure;
            if state == DownloadState::Finished {
                if item.total > 0 {
                    item.received = item.total;
                }
                item.total = item.received;
            }
            return true;
        }
        false
    }
    pub fn clear_clearable(&mut self) -> Vec<DownloadItem> {
        let mut removed = Vec::new();
        self.items.retain(|item| {
            if item.is_clearable() {
                removed.push(item.clone());
                false
            } else {
                true
            }
        });
        removed
    }
    pub fn remove(&mut self, id: &str) -> Option<DownloadItem> {
        let index = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(index))
    }
    /// The list as the UI sees it: `exists` refreshed from disk.
    pub fn publish_items(&mut self) -> Vec<DownloadItem> {
        for item in &mut self.items {
            item.exists = !item.path.is_empty()
                && item.state == DownloadState::Finished
                && std::path::Path::new(&item.path).is_file();
        }
        self.items.clone()
    }
}

/// Zigzag shows the reason in a few plain words; WebView2's enum is much wider,
/// so keep the common causes specific and fall back to a calm generic line.
pub fn interrupt_reason_text(reason: COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON) -> String {
    let text = if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
        "已取消"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
        "已暂停"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_SHUTDOWN {
        "应用退出"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_DISCONNECTED {
        "网络已断开"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_FAILED {
        "网络错误"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_TIMEOUT {
        "网络超时"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_SERVER_DOWN {
        "服务器不可用"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_INVALID_REQUEST {
        "请求无效"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_FAILED {
        "服务器错误"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_FORBIDDEN
        || reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_UNAUTHORIZED
    {
        "服务器拒绝访问"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CERTIFICATE_PROBLEM {
        "证书问题"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CONTENT_LENGTH_MISMATCH
        || reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_BAD_CONTENT
    {
        "服务器内容异常"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CROSS_ORIGIN_REDIRECT {
        "跨域重定向"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_NO_RANGE {
        "服务器不支持断点续传"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_UNEXPECTED_RESPONSE {
        "服务器响应异常"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_ACCESS_DENIED {
        "没有写入权限"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NO_SPACE {
        "磁盘空间不足"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NAME_TOO_LONG {
        "文件名过长"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_LARGE {
        "文件过大"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_SHORT {
        "文件不完整"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_MALICIOUS {
        "文件被安全拦截"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_BLOCKED_BY_POLICY {
        "被策略阻止"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_HASH_MISMATCH
        || reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_SECURITY_CHECK_FAILED
    {
        "文件校验失败"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TRANSIENT_ERROR {
        "文件暂时不可用"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_FAILED {
        "文件写入失败"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_DOWNLOAD_PROCESS_CRASHED {
        "下载进程崩溃"
    } else {
        "下载失败"
    };
    text.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, state: DownloadState, started: u64) -> DownloadItem {
        DownloadItem {
            id: id.into(),
            url: format!("https://example.com/{id}"),
            path: std::env::temp_dir().join(id).to_string_lossy().into(),
            filename: id.into(),
            received: 10,
            total: 100,
            state,
            failure: None,
            started,
            finished: None,
            exists: false,
        }
    }
    #[test]
    fn new_items_go_first_and_the_list_stays_bounded() {
        let mut downloads = Downloads::default();
        for index in 0..(MAX_ITEMS + 5) {
            downloads.insert(item(&format!("done-{index}"), DownloadState::Finished, index as u64));
        }
        assert_eq!(downloads.items.len(), MAX_ITEMS);
        assert_eq!(downloads.items[0].id, format!("done-{}", MAX_ITEMS + 4));
        // An active item is never the one dropped while a finished one exists.
        downloads.insert(item("active", DownloadState::InProgress, 0));
        assert!(downloads.items.iter().any(|item| item.id == "active"));
        assert_eq!(downloads.items.len(), MAX_ITEMS);
    }
    #[test]
    fn clear_keeps_running_and_paused_downloads() {
        let mut downloads = Downloads::default();
        downloads.insert(item("running", DownloadState::InProgress, 3));
        downloads.insert(item("paused", DownloadState::Paused, 2));
        downloads.insert(item("finished", DownloadState::Finished, 1));
        downloads.insert(item("failed", DownloadState::Failed, 0));
        let cleared = downloads.clear_clearable();
        assert_eq!(cleared.len(), 2);
        assert_eq!(downloads.items.len(), 2);
        assert!(downloads.items.iter().all(|item| item.is_active()));
    }
    #[test]
    fn loading_a_session_turns_running_rows_into_interruptions() {
        let mut running = item("running", DownloadState::InProgress, 5);
        running.received = 40;
        let downloads = Downloads::load(vec![running, item("done", DownloadState::Finished, 1)]);
        let restored = downloads.get("running").unwrap();
        assert_eq!(restored.state, DownloadState::Failed);
        assert_eq!(restored.failure.as_deref(), Some("上次退出时下载未完成"));
        assert_eq!(downloads.get("done").unwrap().state, DownloadState::Finished);
    }
    #[test]
    fn finishing_marks_the_full_size_and_fractions_are_bounded() {
        let mut downloads = Downloads::default();
        downloads.insert(item("file", DownloadState::InProgress, 1));
        assert!((downloads.get("file").unwrap().fraction() - 0.1).abs() < f64::EPSILON);
        downloads.set_progress("file", 500, 100);
        assert!((downloads.get("file").unwrap().fraction() - 1.0).abs() < f64::EPSILON);
        downloads.set_progress("file", 100, 100);
        downloads.finish("file", DownloadState::Finished, None);
        let done = downloads.get("file").unwrap();
        assert_eq!(done.received, 100);
        assert_eq!(done.total, 100);
        assert!(done.finished.is_some());
    }
    #[test]
    fn full_progress_waits_for_completion_and_late_bytes_cannot_change_finished_items() {
        let mut downloads = Downloads::default();
        downloads.insert(item("file", DownloadState::InProgress, 1));
        downloads.set_progress("file", 100, 100);
        assert_eq!(downloads.get("file").unwrap().state, DownloadState::InProgress);
        assert!(downloads.get("file").unwrap().finished.is_none());

        downloads.finish("file", DownloadState::Finished, None);
        downloads.get_mut("file").unwrap().exists = true;
        downloads.set_progress("file", 90, 200);
        let done = downloads.get("file").unwrap();
        assert_eq!((done.received, done.total), (100, 100));
        assert!(done.exists);
    }
    #[test]
    fn interrupt_reasons_are_plain_words() {
        assert_eq!(
            interrupt_reason_text(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED),
            "已取消"
        );
        assert_eq!(
            interrupt_reason_text(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_NO_SPACE),
            "磁盘空间不足"
        );
        assert_eq!(
            interrupt_reason_text(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE),
            "下载失败"
        );
    }
}
