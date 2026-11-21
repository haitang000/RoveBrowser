//! Bounded HTTPS downloads shared by app updates and Chrome Web Store installs.
use std::{ffi::c_void, ptr::null_mut};
use url::{Position, Url};
use windows::{
    core::{w, PCWSTR},
    Win32::Networking::WinHttp::*,
};

#[derive(Debug)]
pub enum Error {
    InvalidUrl,
    RedirectLimit,
    Transport,
    Status(u32),
    TooLarge,
    Write(String),
}

pub fn target(url: &Url) -> Result<&str, Error> {
    let value = &url[Position::BeforePath..];
    if value.starts_with('/') && !value.contains([' ', '\r', '\n', '#']) && value.len() <= 2048 {
        Ok(value)
    } else {
        Err(Error::InvalidUrl)
    }
}

pub fn fetch(
    start: &Url,
    allowed: impl Fn(&Url) -> bool,
    accept: &str,
    max: u64,
    timeouts: [i32; 4],
    write: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), Error> {
    fetch_with_progress(start, allowed, accept, max, timeouts, write, &mut |_, _| {})
}

/// `progress` receives bytes saved so far and the declared length, when the
/// response included Content-Length. It runs only for the final response.
pub fn fetch_with_progress(
    start: &Url,
    allowed: impl Fn(&Url) -> bool,
    accept: &str,
    max: u64,
    timeouts: [i32; 4],
    write: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), Error> {
    let mut current = start.clone();
    for _ in 0..5 {
        if !allowed(&current)
            || current.scheme() != "https"
            || current.port().is_some()
            || !current.username().is_empty()
            || current.password().is_some()
        {
            return Err(Error::InvalidUrl);
        }
        match exchange(&current, accept, max, timeouts, write, progress)? {
            Hop::Done => return Ok(()),
            Hop::Redirect(location) => {
                current = current.join(&location).map_err(|_| Error::InvalidUrl)?;
            }
        }
    }
    Err(Error::RedirectLimit)
}

enum Hop {
    Done,
    Redirect(String),
}

struct Handles {
    session: *mut c_void,
    connection: *mut c_void,
    request: *mut c_void,
}
impl Drop for Handles {
    fn drop(&mut self) {
        for handle in [self.request, self.connection, self.session] {
            if !handle.is_null() {
                let _ = unsafe { WinHttpCloseHandle(handle) };
            }
        }
    }
}

fn exchange(
    url: &Url,
    accept: &str,
    max: u64,
    timeouts: [i32; 4],
    write: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Hop, Error> {
    let host = wide(url.host_str().ok_or(Error::InvalidUrl)?);
    let request_target = wide(target(url)?);
    let headers = wide(&format!(
        "User-Agent: Rove/{}\r\nAccept: {accept}\r\nAccept-Encoding: identity\r\n",
        env!("CARGO_PKG_VERSION"),
    ));
    unsafe {
        let mut handles = Handles {
            session: WinHttpOpen(
                w!("Rove"),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ),
            connection: null_mut(),
            request: null_mut(),
        };
        if handles.session.is_null() {
            return Err(Error::Transport);
        }
        handles.connection = WinHttpConnect(
            handles.session,
            PCWSTR(host.as_ptr()),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        );
        if handles.connection.is_null() {
            return Err(Error::Transport);
        }
        handles.request = WinHttpOpenRequest(
            handles.connection,
            w!("GET"),
            PCWSTR(request_target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            null_mut(),
            WINHTTP_FLAG_SECURE,
        );
        if handles.request.is_null() {
            return Err(Error::Transport);
        }
        let disable = (WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES).to_ne_bytes();
        WinHttpSetOption(
            Some(handles.request),
            WINHTTP_OPTION_DISABLE_FEATURE,
            Some(&disable),
        )
        .map_err(|_| Error::Transport)?;
        WinHttpSetTimeouts(
            handles.request,
            timeouts[0],
            timeouts[1],
            timeouts[2],
            timeouts[3],
        )
        .map_err(|_| Error::Transport)?;
        WinHttpSendRequest(
            handles.request,
            Some(&headers[..headers.len() - 1]),
            None,
            0,
            0,
            0,
        )
        .map_err(|_| Error::Transport)?;
        WinHttpReceiveResponse(handles.request, null_mut()).map_err(|_| Error::Transport)?;
        let status = status_code(handles.request).ok_or(Error::Transport)?;
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            return header(handles.request, WINHTTP_QUERY_LOCATION)
                .filter(|location| !location.is_empty())
                .map(Hop::Redirect)
                .ok_or(Error::InvalidUrl);
        }
        if status != 200 {
            return Err(Error::Status(status));
        }
        let declared = header(handles.request, WINHTTP_QUERY_CONTENT_LENGTH)
            .and_then(|value| value.trim().parse::<u64>().ok());
        if declared.is_some_and(|length| length > max) {
            return Err(Error::TooLarge);
        }
        let mut received = 0u64;
        progress(received, declared);
        let mut chunk = [0u8; 16384];
        loop {
            let mut read = 0;
            WinHttpReadData(
                handles.request,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            )
            .map_err(|_| Error::Transport)?;
            if read == 0 {
                return Ok(Hop::Done);
            }
            received += u64::from(read);
            if received > max {
                return Err(Error::TooLarge);
            }
            write(&chunk[..read as usize]).map_err(Error::Write)?;
            progress(received, declared);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn status_code(request: *mut c_void) -> Option<u32> {
    unsafe {
        let mut status = 0u32;
        let mut length = 4u32;
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut length,
            null_mut(),
        )
        .ok()?;
        Some(status)
    }
}

fn header(request: *mut c_void, level: u32) -> Option<String> {
    unsafe {
        let mut size = 0u32;
        let _ = WinHttpQueryHeaders(request, level, PCWSTR::null(), None, &mut size, null_mut());
        if size < 2 || size > 8192 {
            return None;
        }
        let mut buffer = vec![0u16; size as usize / 2 + 1];
        WinHttpQueryHeaders(
            request,
            level,
            PCWSTR::null(),
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            null_mut(),
        )
        .ok()?;
        let chars = (size as usize / 2).min(buffer.len());
        let end = buffer[..chars]
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(chars);
        String::from_utf16(&buffer[..end]).ok()
    }
}
