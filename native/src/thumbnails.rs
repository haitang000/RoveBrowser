//! Session-only thumbnails; no screenshots enter SQLite/history/profile files.
use std::collections::HashMap;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;
#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, (String, String, u64)>,
    clock: u64,
    bytes: usize,
}
impl Cache {
    pub fn insert(&mut self, id: String, url: String, image: String) {
        self.remove(&id);
        if image.len() > 220 * 1024 {
            return;
        }
        self.clock += 1;
        self.bytes += image.len();
        self.entries.insert(id, (url, image, self.clock));
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let Some(id) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.2)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            self.remove(&id);
        }
    }
    pub fn remove(&mut self, id: &str) {
        if let Some((_, image, _)) = self.entries.remove(id) {
            self.bytes -= image.len()
        }
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.entries.retain(|id, (_, image, _)| {
            let yes = keep(id);
            if !yes {
                self.bytes -= image.len()
            }
            yes
        });
    }
    pub fn images(&self) -> HashMap<&str, &str> {
        self.entries
            .iter()
            .map(|(id, (_, image, _))| (id.as_str(), image.as_str()))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caps_memory_and_discards_closed_tabs() {
        let mut c = Cache::default();
        for i in 0..80 {
            c.insert(
                i.to_string(),
                "https://example.com".into(),
                "x".repeat(200 * 1024),
            );
        }
        assert!(c.bytes <= MAX_BYTES && c.entries.len() <= MAX_ENTRIES);
        assert!(!c.entries.contains_key("0"));
        c.retain(|id| id == "79");
        assert_eq!(c.entries.len(), 1);
        c.remove("79");
        assert_eq!(c.bytes, 0);
    }
}
