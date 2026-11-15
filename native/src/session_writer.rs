//! A single writer keeps serialization and SQLite commits off the window thread.
use crate::{model::State, storage::Storage};
use std::{
    path::Path,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

#[derive(Default)]
struct Pending {
    state: Option<State>,
    requested: u64,
    completed: u64,
    error: Option<String>,
    stopping: bool,
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
                    let (state, generation) = {
                        let mut pending = lock.lock().unwrap();
                        while pending.state.is_none() && !pending.stopping {
                            pending = ready.wait(pending).unwrap();
                        }
                        let Some(state) = pending.state.take() else {
                            break;
                        };
                        (state, pending.requested)
                    };
                    let error = storage.save(&state).err().map(|error| error.to_string());
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
}
