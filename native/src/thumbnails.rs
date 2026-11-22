//! Session-only thumbnails; no screenshots enter SQLite/history/profile files.
use crate::capture::Cancellation;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const SETTLE: Duration = Duration::from_millis(140);
// Give the visible preview time to paint after DOMContentLoaded or load.
pub const PREVIEW_SETTLE: Duration = Duration::from_millis(50);
pub const PREVIEW_LOAD_TIMEOUT: Duration = Duration::from_secs(12);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Capture {
    pub ticket: u64,
    pub cancellation: Cancellation,
    deadline: Instant,
}

/// Only in-flight captures have deadlines. Each ticket belongs to one capture,
/// even if a controller is recreated with the same tab ID and URL.
#[derive(Default)]
pub struct Captures {
    pending: HashMap<String, Capture>,
    ticket: u64,
}

impl Captures {
    pub fn start(&mut self, id: String, now: Instant) -> &Capture {
        self.ticket += 1;
        self.pending
            .entry(id)
            .insert_entry(Capture {
                ticket: self.ticket,
                cancellation: Cancellation::default(),
                deadline: now + CAPTURE_TIMEOUT,
            })
            .into_mut()
    }

    pub fn complete(&mut self, id: &str, ticket: u64) -> bool {
        if self
            .pending
            .get(id)
            .is_some_and(|capture| capture.ticket == ticket)
        {
            self.cancel(id);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.pending.contains_key(id)
    }

    pub fn cancel(&mut self, id: &str) -> bool {
        self.pending.remove(id).is_some()
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) -> bool {
        let count = self.pending.len();
        self.pending.retain(|id, _| keep(id));
        count != self.pending.len()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending.values().map(|capture| capture.deadline).min()
    }

    pub fn expire(&mut self, now: Instant) -> Vec<String> {
        let mut timed_out = Vec::new();
        self.pending.retain(|id, capture| {
            if capture.deadline <= now {
                timed_out.push(id.clone());
                return false;
            }
            true
        });
        timed_out
    }
}

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;
struct Entry {
    url: String,
    image: String,
    sequence: u64,
    fully_loaded: bool,
}

#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, Entry>,
    clock: u64,
    bytes: usize,
}
impl Cache {
    /// Returns whether the displayed image changed.
    pub fn insert(&mut self, id: String, url: String, image: String, fully_loaded: bool) -> bool {
        if image.len() > 220 * 1024 {
            return false;
        }
        self.clock += 1;
        if let Some(entry) = self
            .entries
            .get_mut(&id)
            .filter(|e| e.url == url && e.image == image)
        {
            entry.sequence = self.clock;
            entry.fully_loaded |= fully_loaded;
            return false;
        }
        self.remove(&id);
        self.bytes += image.len();
        self.entries.insert(
            id,
            Entry {
                url,
                image,
                sequence: self.clock,
                fully_loaded,
            },
        );
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let Some(id) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.sequence)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            self.remove(&id);
        }
        true
    }
    pub fn remove(&mut self, id: &str) {
        if let Some(entry) = self.entries.remove(id) {
            self.bytes -= entry.image.len()
        }
    }
    pub fn retain_sources(&mut self, mut keep: impl FnMut(&str, &str) -> bool) {
        self.entries.retain(|id, entry| {
            let yes = keep(id, &entry.url);
            if !yes {
                self.bytes -= entry.image.len()
            }
            yes
        });
    }
    pub fn has_complete(&self, id: &str, url: &str) -> bool {
        self.entries
            .get(id)
            .is_some_and(|entry| entry.url == url && entry.fully_loaded)
    }
    pub fn image(&self, id: &str) -> Option<&str> {
        self.entries.get(id).map(|entry| entry.image.as_str())
    }
    pub fn ids(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }
    pub fn images(&self) -> HashMap<&str, &str> {
        self.entries
            .iter()
            .map(|(id, entry)| (id.as_str(), entry.image.as_str()))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_replaced_and_expired_captures_cannot_publish_later() {
        let mut captures = Captures::default();
        let now = Instant::now();
        let first = captures.start("tab".into(), now).ticket;
        assert_eq!(captures.next_deadline(), Some(now + CAPTURE_TIMEOUT));
        assert!(captures.complete("tab", first));
        assert_eq!(captures.next_deadline(), None);
        assert!(!captures.complete("tab", first));

        let previous = captures.start("tab".into(), now).ticket;
        let current = captures
            .start("tab".into(), now + Duration::from_secs(1))
            .ticket;
        assert!(!captures.complete("tab", previous));
        assert!(captures.expire(now + CAPTURE_TIMEOUT).is_empty());
        assert!(captures.contains("tab"));
        assert_eq!(
            captures.expire(now + CAPTURE_TIMEOUT + Duration::from_secs(1)),
            ["tab"]
        );
        assert!(!captures.complete("tab", current));
        assert_eq!(captures.next_deadline(), None);
    }

    #[test]
    fn caps_memory_and_discards_closed_tabs() {
        let mut c = Cache::default();
        for i in 0..80 {
            c.insert(
                i.to_string(),
                "https://example.com".into(),
                "x".repeat(200 * 1024),
                true,
            );
        }
        assert!(c.bytes <= MAX_BYTES && c.entries.len() <= MAX_ENTRIES);
        assert!(!c.entries.contains_key("0"));
        c.retain_sources(|id, _| id == "79");
        assert_eq!(c.entries.len(), 1);
        c.remove("79");
        assert_eq!(c.bytes, 0);
    }
    #[test]
    fn a_saved_preview_is_invalidated_when_its_source_url_changes() {
        let mut c = Cache::default();
        c.insert(
            "saved".into(),
            "https://example.com/one".into(),
            "image".into(),
            true,
        );
        assert!(c.has_complete("saved", "https://example.com/one"));
        assert!(!c.has_complete("saved", "https://example.com/two"));
        c.retain_sources(|_, url| url == "https://example.com/two");
        assert!(c.images().is_empty());
    }

    #[test]
    fn unchanged_image_can_complete_a_provisional_preview_without_a_new_message() {
        let mut cache = Cache::default();
        let url = "https://example.com";
        assert!(cache.insert("saved".into(), url.into(), "image".into(), false));
        assert_eq!(cache.image("saved"), Some("image"));
        assert!(!cache.has_complete("saved", url));
        assert!(!cache.insert("saved".into(), url.into(), "image".into(), true));
        assert!(cache.has_complete("saved", url));
    }
}
