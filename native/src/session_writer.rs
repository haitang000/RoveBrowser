//! A single writer keeps session and navigation writes off the window thread.
use crate::{model::State, storage::Storage};
use std::{
    path::Path,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

#[derive(Default)]
struct Pending {
    state: Option<State>,
    history: Vec<HistoryWrite>,
    requested: u64,
    completed: u64,
    error: Option<String>,
    stopping: bool,
}

enum HistoryWrite {
    Visit {
        title: String,
        url: String,
        visited: i64,
    },
    Clear(Box<dyn FnOnce(bool) + Send>),
}

impl HistoryWrite {
    fn apply(self, storage: &Storage) -> rusqlite::Result<()> {
        match self {
            Self::Visit {
                title,
                url,
                visited,
            } => storage.visit(&title, &url, visited).map(|_| ()),
            Self::Clear(done) => {
                let result = storage.clear_history().map(|_| ());
                done(result.is_ok());
                result
            }
        }
    }
}

pub struct SessionWriter {
    shared: Arc<(Mutex<Pending>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl SessionWriter {
    pub fn new(
        path: &Path,
        on_error: impl Fn(String) + Send + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let storage = Storage::open(path)?;
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let work = shared.clone();
        let worker = thread::Builder::new()
            .name("rove-session-writer".into())
            .spawn(move || {
                let (lock, ready) = &*work;
                loop {
                    let (state, history, generation) = {
                        let mut pending = lock.lock().unwrap();
                        while pending.state.is_none()
                            && pending.history.is_empty()
                            && !pending.stopping
                        {
                            pending = ready.wait(pending).unwrap();
                        }
                        if pending.state.is_none() && pending.history.is_empty() {
                            break;
                        }
                        (
                            pending.state.take(),
                            std::mem::take(&mut pending.history),
                            pending.requested,
                        )
                    };
                    // Never hold the queue lock while SQLite waits for another
                    // writer. Reload/title callbacks must be able to enqueue.
                    let mut error = state
                        .as_ref()
                        .and_then(|state| storage.save(state).err())
                        .map(|error| error.to_string());
                    for write in history {
                        if let Err(failure) = write.apply(&storage) {
                            error.get_or_insert_with(|| failure.to_string());
                        }
                    }
                    if let Some(error) = &error {
                        on_error(error.clone());
                    }
                    let mut pending = lock.lock().unwrap();
                    pending.completed = generation;
                    pending.error = error;
                    ready.notify_all();
                }
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Replace the pending snapshot instead of accumulating stale sessions.
    pub fn save(&self, state: State) {
        let (lock, ready) = &*self.shared;
        let mut pending = lock.lock().unwrap();
        pending.state = Some(state);
        pending.requested += 1;
        ready.notify_all();
    }

    pub fn visit(&self, title: &str, url: &str) {
        self.history(HistoryWrite::Visit {
            title: title.into(),
            url: url.into(),
            visited: crate::model::now_secs() as i64,
        });
    }

    /// Ordered after all earlier visits, including the batch already on disk.
    /// Completion is asynchronous so clearing cannot block the window either.
    pub fn clear_history(&self, done: impl FnOnce(bool) + Send + 'static) {
        self.history(HistoryWrite::Clear(Box::new(done)));
    }

    fn history(&self, write: HistoryWrite) {
        let (lock, ready) = &*self.shared;
        let mut pending = lock.lock().unwrap();
        pending.history.push(write);
        pending.requested += 1;
        ready.notify_all();
    }

    /// A barrier for shutdown and imports, which also write the session table.
    pub fn flush(&self) -> Result<(), String> {
        let (lock, ready) = &*self.shared;
        let mut pending = lock.lock().unwrap();
        let generation = pending.requested;
        while pending.completed < generation {
            pending = ready.wait(pending).unwrap();
        }
        pending.error.clone().map_or(Ok(()), Err)
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        let (lock, ready) = &*self.shared;
        lock.lock().unwrap().stopping = true;
        ready.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_and_drop_preserve_the_latest_snapshot() {
        let path = std::env::temp_dir().join(format!("rove-writer-{}", uuid::Uuid::new_v4()));
        let reader = Storage::open(&path).unwrap();
        let writer = SessionWriter::new(&path, |error| panic!("{error}")).unwrap();
        let mut state = State::default();
        for i in 0..100 {
            state.spaces[0].name = format!("Space {i}");
            writer.save(state.clone());
        }
        writer.flush().unwrap();
        assert_eq!(reader.load().unwrap().spaces[0].name, "Space 99");
        state.spaces[0].name = "Final update".into();
        writer.save(state);
        drop(writer);
        assert_eq!(reader.load().unwrap().spaces[0].name, "Final update");
        drop(reader);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn a_flushed_writer_cannot_overwrite_a_later_import() {
        let path =
            std::env::temp_dir().join(format!("rove-writer-import-{}", uuid::Uuid::new_v4()));
        let db = Storage::open(&path).unwrap();
        let writer = SessionWriter::new(&path, |_| {}).unwrap();
        let mut state = State::default();
        writer.save(state.clone());
        writer.flush().unwrap();
        state.spaces[0].name = "Imported".into();
        db.save(&state).unwrap();
        drop(writer);
        assert_eq!(db.load().unwrap().spaces[0].name, "Imported");
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn write_failures_reach_both_the_barrier_and_error_handler() {
        let path = std::env::temp_dir().join(format!("rove-writer-error-{}", uuid::Uuid::new_v4()));
        let db = Storage::open(&path).unwrap();
        db.conn.execute_batch("CREATE TRIGGER reject_session BEFORE INSERT ON session BEGIN SELECT RAISE(ABORT, 'write rejected'); END;").unwrap();
        let (errors, received) = std::sync::mpsc::channel();
        let writer = SessionWriter::new(&path, move |error| {
            let _ = errors.send(error);
        })
        .unwrap();
        writer.save(State::default());
        assert!(writer.flush().unwrap_err().contains("write rejected"));
        assert!(received.recv().unwrap().contains("write rejected"));
        db.conn
            .execute_batch("DROP TRIGGER reject_session")
            .unwrap();
        writer.save(State::default());
        writer.flush().unwrap();
        assert!(db.load().is_ok());
        drop(writer);
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn locked_database_does_not_block_enqueueing_and_clear_orders_pending_visits() {
        use std::sync::mpsc;
        use std::time::Duration;

        let path =
            std::env::temp_dir().join(format!("rove-writer-locked-{}", uuid::Uuid::new_v4()));
        let db = Storage::open(&path).unwrap();
        let writer = SessionWriter::new(&path, |error| panic!("{error}")).unwrap();
        db.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        writer.visit("Before clear", "https://example.com/before");
        // Let the worker reach the locked write before adding another batch.
        std::thread::sleep(Duration::from_millis(100));
        let (enqueued, queued) = mpsc::channel();
        let (cleared, clear_done) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            writer.save(State::default());
            writer.visit("Also before clear", "https://example.com/pending");
            writer.clear_history(move |ok| cleared.send(ok).unwrap());
            writer.visit("After clear", "https://example.com/after");
            enqueued.send(()).unwrap();
            writer.flush().unwrap();
        });
        let responsive = queued.recv_timeout(Duration::from_secs(1));
        // Always unlock before assertions/join, including on a regression.
        db.conn.execute_batch("ROLLBACK").unwrap();
        responsive.expect("enqueueing must not wait for SQLite's busy timeout");
        assert!(clear_done.recv_timeout(Duration::from_secs(5)).unwrap());
        worker.join().unwrap();
        let urls: Vec<String> = db
            .conn
            .prepare("SELECT url FROM history")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(urls, ["https://example.com/after"]);
        assert!(db.load().is_ok());
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn shutdown_drains_history_and_preserves_the_visit_time() {
        let path =
            std::env::temp_dir().join(format!("rove-writer-history-{}", uuid::Uuid::new_v4()));
        let db = Storage::open(&path).unwrap();
        let writer = SessionWriter::new(&path, |error| panic!("{error}")).unwrap();
        writer.history(HistoryWrite::Visit {
            title: "Original".into(),
            url: "https://example.com".into(),
            visited: 123,
        });
        writer.history(HistoryWrite::Visit {
            title: "Latest title".into(),
            url: "https://example.com".into(),
            visited: 456,
        });
        drop(writer);
        let row: (String, i64) = db
            .conn
            .query_row("SELECT title,visited FROM history", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(row, ("Latest title".into(), 456));
        assert_eq!(db.search("Latest title").unwrap().len(), 1);
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn history_write_errors_are_reported() {
        let path = std::env::temp_dir().join(format!(
            "rove-writer-history-error-{}",
            uuid::Uuid::new_v4()
        ));
        let db = Storage::open(&path).unwrap();
        db.conn.execute_batch("CREATE TRIGGER reject_history BEFORE INSERT ON history BEGIN SELECT RAISE(ABORT, 'history rejected'); END;").unwrap();
        let (errors, received) = std::sync::mpsc::channel();
        let writer = SessionWriter::new(&path, move |error| {
            let _ = errors.send(error);
        })
        .unwrap();
        writer.visit("Page", "https://example.com");
        assert!(writer.flush().unwrap_err().contains("history rejected"));
        assert!(received
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .contains("history rejected"));
        drop(writer);
        drop(db);
        std::fs::remove_dir_all(path).unwrap();
    }
}
