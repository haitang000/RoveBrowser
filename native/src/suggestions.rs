//! Search suggestions for the command bar. Only the typed query is sent, to Google's
//! public suggest endpoint, with no cookies. Adapted from Zigzag's SuggestionFetcher.
use serde_json::Value;
use windows::{
    core::{w, PCWSTR},
    Win32::Networking::WinHttp::*,
};

pub fn parse(bytes: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return vec![];
    };
    value
        .as_array()
        .and_then(|items| items.get(1))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .take(8)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn fetch(query: &str) -> Vec<String> {
    let query = query.trim();
    if !(2..=200).contains(&query.chars().count()) || query.chars().any(char::is_control) {
        return vec![];
    }
    let path: Vec<u16> = format!("/complete/search?client=firefox&q={}", encode(query)).encode_utf16().chain(Some(0)).collect();
    unsafe {
        let session = WinHttpOpen(w!("Rove"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0);
        if session.is_null() { return vec![]; }
        let connection = WinHttpConnect(session, w!("suggestqueries.google.com"), INTERNET_DEFAULT_HTTPS_PORT, 0);
        if connection.is_null() { let _ = WinHttpCloseHandle(session); return vec![]; }
        let request = WinHttpOpenRequest(
            connection,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connection);
            let _ = WinHttpCloseHandle(session);
            return vec![];
        }
        let disable = WINHTTP_DISABLE_REDIRECTS.to_ne_bytes();
        let _ = WinHttpSetOption(Some(request), WINHTTP_OPTION_DISABLE_FEATURE, Some(&disable));
        let _ = WinHttpSetTimeouts(request, 1000, 1000, 1500, 1500);
        let sent = WinHttpSendRequest(request, None, None, 0, 0, 0).is_ok()
            && WinHttpReceiveResponse(request, std::ptr::null_mut()).is_ok();
        let mut body = Vec::new();
        if sent {
            loop {
                let mut read = 0u32;
                let mut chunk = [0u8; 4096];
                if WinHttpReadData(request, chunk.as_mut_ptr().cast(), chunk.len() as u32, &mut read).is_err() || read == 0 { break; }
                body.extend_from_slice(&chunk[..read as usize]);
                if body.len() > 65_536 { body.clear(); break; }
            }
        }
        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connection);
        let _ = WinHttpCloseHandle(session);
        if body.is_empty() { vec![] } else { parse(&body) }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    #[test]
    fn parses_opensearch_suggestions() {
        assert_eq!(parse(br#"["cats",["cats musical","cats movie"]]"#), vec!["cats musical", "cats movie"]);
        assert!(parse(b"not json").is_empty());
    }
}
