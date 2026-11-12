//! One SQLite/WebView2 owner per profile. Shell launches deliver bounded URL
//! messages to the existing process through a local Windows named pipe.
use std::{
    fs::{File, OpenOptions},
    hash::{Hash, Hasher},
    io::{Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{GetLastError, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND},
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW,
            PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
        },
    },
};
const MAX_MESSAGE: usize = 65536;

pub fn urls(args: impl IntoIterator<Item = std::ffi::OsString>) -> Result<Vec<String>, String> {
    let mut urls = vec![];
    for arg in args {
        if arg == "--" {
            continue;
        }
        let raw = arg.to_str().ok_or("链接编码无效")?;
        if raw.chars().any(char::is_control) {
            return Err("链接包含无效字符".into());
        }
        urls.push(crate::import::web_url(raw).ok_or("Rove 只接受 HTTP 或 HTTPS 网页链接")?);
        if urls.len() > 16 {
            return Err("一次最多打开 16 个链接".into());
        }
    }
    if serde_json::to_vec(&urls)
        .map_err(|_| "无法编码启动链接")?
        .len()
        > MAX_MESSAGE
    {
        return Err("启动链接过长".into());
    }
    Ok(urls)
}
fn decode(bytes: &[u8]) -> Option<Vec<String>> {
    if bytes.len() > MAX_MESSAGE {
        return None;
    }
    let raw: Vec<String> = serde_json::from_slice(bytes).ok()?;
    urls(raw.into_iter().map(Into::into)).ok()
}
pub struct Instance {
    pipe: Option<File>,
    name: String,
    stopped: Arc<AtomicBool>,
}
impl Instance {
    pub fn claim(data: &Path, urls: &[String]) -> std::io::Result<Option<Self>> {
        // Canonicalize before hashing, including callers using relative paths.
        std::fs::create_dir_all(data)?;
        let canonical = std::fs::canonicalize(data)?
            .to_string_lossy()
            .to_lowercase();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        canonical.hash(&mut hash);
        let name = format!("\\\\.\\pipe\\Rove-{:x}", hash.finish());
        let handle = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(&name),
                PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_MESSAGE as u32,
                MAX_MESSAGE as u32,
                0,
                None,
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            let pipe = unsafe { File::from_raw_handle(handle.0) };
            return Ok(Some(Self {
                pipe: Some(pipe),
                name,
                stopped: Arc::new(AtomicBool::new(false)),
            }));
        }
        let payload = serde_json::to_vec(urls)?;
        let start = Instant::now();
        loop {
            match OpenOptions::new().write(true).open(&name) {
                Ok(mut pipe) => {
                    pipe.write_all(&payload)?;
                    return Ok(None);
                }
                Err(error) if start.elapsed() >= Duration::from_secs(10) => {
                    return Err(std::io::Error::new(
                        error.kind(),
                        "已有 Rove 进程未响应，请稍后重试",
                    ))
                }
                Err(_) => {
                    unsafe {
                        let _ = WaitNamedPipeW(&HSTRING::from(&name), 100);
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
        }
    }
    pub fn listen(&mut self, receive: impl Fn(Vec<String>) + Send + 'static) {
        let Some(mut pipe) = self.pipe.take() else {
            return;
        };
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let handle = HANDLE(pipe.as_raw_handle());
                let connected = unsafe {
                    ConnectNamedPipe(handle, None).is_ok() || GetLastError() == ERROR_PIPE_CONNECTED
                };
                if connected {
                    let mut bytes = vec![0; MAX_MESSAGE + 1];
                    if let Ok(len) = pipe.read(&mut bytes) {
                        if let Some(urls) = decode(&bytes[..len]) {
                            if !stopped.load(Ordering::Acquire) {
                                receive(urls);
                            }
                        }
                    }
                }
                unsafe {
                    let _ = DisconnectNamedPipe(handle);
                }
            }
        });
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if self.pipe.is_none() {
            if let Ok(mut pipe) = OpenOptions::new().write(true).open(&self.name) {
                let _ = pipe.write_all(b"[]");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_shell_arguments_and_bounded_messages() {
        assert_eq!(
            urls(["--".into(), "https://example.com/a?q=1&b=中文".into()])
                .unwrap()
                .len(),
            1
        );
        for raw in [
            "javascript:alert(1)",
            "file:///c:/secret",
            "--proxy-server=evil",
            "https://user:pass@example.com",
            "https://example.com/\nsecret",
        ] {
            assert!(urls([raw.into()]).is_err());
        }
        assert!(urls(std::iter::repeat_n("https://example.com".into(), 17)).is_err());
        assert!(decode(b"[\"file:///c:/secret\"]").is_none());
        assert!(decode(&vec![b' '; MAX_MESSAGE + 1]).is_none());
    }
    #[test]
    fn forwards_unicode_urls_and_activation_to_profile_owner() {
        let path = std::env::temp_dir().join(format!("rove-instance-{}", uuid::Uuid::new_v4()));
        let mut primary = Instance::claim(&path, &[]).unwrap().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        primary.listen(move |urls| {
            let _ = send.send(urls);
        });
        let links = urls(["https://example.com/文档?q=a&b=2".into()]).unwrap();
        assert!(Instance::claim(&path, &links).unwrap().is_none());
        assert_eq!(receive.recv_timeout(Duration::from_secs(3)).unwrap(), links);
        assert!(Instance::claim(&path, &[]).unwrap().is_none());
        assert!(receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .is_empty());
        drop(primary);
        std::fs::remove_dir_all(path).unwrap();
    }
}
