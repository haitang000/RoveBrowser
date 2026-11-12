//! Site icons: keep WebView2's own favicon pixels, and fetch the icons a page
//! declares at their native resolution so a cached icon is never an enlargement.
//!
//! WebView2's favicon store only holds 16 and 32 pixel bitmaps. Caching those
//! after stretching them to 64 pixels made every icon soft; this pipeline keeps
//! native pixels, prefers a page's own SVG or large PNG, and only shrinks.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    time::{Duration, Instant},
};
use webview2_com::{
    ExecuteScriptCompletedHandler, GetFaviconCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2_15, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
    },
};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Networking::WinHttp::{
            WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
            WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
            INTERNET_DEFAULT_HTTPS_PORT, INTERNET_DEFAULT_HTTP_PORT,
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
            WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
        },
        System::Com::{IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET},
    },
};

/// The longest side a raster icon is stored at. Two device pixels per CSS pixel
/// at 200% covers every place the UI draws one; SVG keeps its own resolution.
pub const MAX_SIDE: u32 = 64;
/// Bumped when the cached-icon format changes; the host then re-fetches each
/// saved site once (icons saved before this build were upscaled and blurry).
pub const ICON_VERSION: u8 = 1;

const MAX_INPUT: u64 = 256 * 1024;
const MAX_ICON: usize = 512 * 1024;
const MAX_HTML: usize = 512 * 1024;
const MAX_SVG: usize = 48 * 1024;
/// Icons beyond this many candidates are not worth another request.
const MAX_CANDIDATES: usize = 8;
const MAX_LINKS: usize = 16;
const RETRY_AFTER: Duration = Duration::from_secs(10 * 60);
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub fn same_origin(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => matches!(a.scheme(), "http" | "https") && a.origin() == b.origin(),
        _ => false,
    }
}

/// The site key icons are cached by: scheme, host and port, never a path.
pub fn origin_of(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.origin().ascii_serialization())
}

/// Whether an icon a page declares speaks for its whole site (Zigzag's rule):
/// a front page's icon can replace a saved site icon, a deep page's only fills
/// a gap. A page deep in a site can wear its own icon (each Claude artifact has
/// one) and that must not take over the icon of the site's favorites and pins.
pub fn speaks_for_site(url: &str) -> bool {
    url::Url::parse(url)
        .map(|url| url.path().split('/').filter(|part| !part.is_empty()).count() <= 1)
        .unwrap_or(false)
}

/// One `<link>`-shaped icon declaration, as the page's own script reports it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Link {
    #[serde(default)]
    pub rel: String,
    #[serde(default)]
    pub href: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub sizes: String,
}

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(16 * 1024 * 1024);
    limits
}

/// A raster icon at its own size, shrunk only when larger than [`MAX_SIDE`];
/// never enlarged, so a small icon stays as sharp as its source.
fn fit(image: image::DynamicImage) -> image::DynamicImage {
    if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        image.thumbnail(MAX_SIDE, MAX_SIDE)
    } else {
        image
    }
}

fn trimmed(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("<svg") || lower.starts_with("<?xml") || lower.starts_with("<!--"))
        && lower.contains("<svg")
}

/// Drawable shapes make an SVG usable in an `<img>`; a stylesheet-only SVG
/// draws nothing there, so it is passed over for the next declared icon.
fn svg_draws(text: &str) -> bool {
    [
        "<path", "<circle", "<rect", "<polygon", "<polyline", "<ellipse", "<line", "<use",
        "<image", "<text",
    ]
    .iter()
    .any(|shape| text.contains(shape))
}

fn icon_url(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_ICON {
        return None;
    }
    let bytes = trimmed(bytes);
    if is_svg(bytes) {
        if bytes.len() > MAX_SVG || !svg_draws(&String::from_utf8_lossy(bytes).to_ascii_lowercase()) {
            return None;
        }
        return Some(format!(
            "data:image/svg+xml;base64,{}",
            STANDARD.encode(bytes)
        ));
    }
    let format = image::guess_format(bytes).ok()?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::Ico
    ) {
        return None;
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    reader.limits(limits());
    let image = fit(reader.decode().ok()?);
    let mut output = std::io::Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(output.into_inner())
    ))
}

/// Sharpness of a stored icon, in source pixels on its longest side. SVG is
/// vector, so it stays sharp at any size. An icon we cannot read is 0.
pub fn side(icon: &str) -> u32 {
    if icon.starts_with("data:image/svg+xml") {
        return u32::MAX;
    }
    let Some(rest) = icon.strip_prefix("data:image/png;base64,") else {
        return 0;
    };
    let Ok(bytes) = STANDARD.decode(rest) else {
        return 0;
    };
    // PNG signature, IHDR length and type, then the big-endian width and height.
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return 0;
    }
    u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]])
        .max(u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]))
}

/// Persist real WebView2 favicon pixels, rather than a URL that needs another request.
fn read_icon(stream: &IStream) -> Option<String> {
    let mut stat = STATSTG::default();
    // WebView2 owns this stream on the UI thread; cap input and decoded pixels.
    unsafe {
        stream.Stat(&mut stat, STATFLAG_NONAME).ok()?;
        if stat.cbSize == 0 || stat.cbSize > MAX_INPUT {
            return None;
        }
        stream.Seek(0, STREAM_SEEK_SET, None).ok()?;
        let mut bytes = vec![0; stat.cbSize as usize];
        let mut read = 0;
        stream
            .Read(
                bytes.as_mut_ptr().cast(),
                bytes.len() as u32,
                Some(&mut read),
            )
            .ok()
            .ok()?;
        bytes.truncate(read as usize);
        icon_url(&bytes)
    }
}

pub fn capture(
    core: &ICoreWebView2_15,
    done: impl FnOnce(String) + 'static,
) -> windows::core::Result<()> {
    let handler = GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
        if result.is_ok() {
            if let Some(icon) = stream.as_ref().and_then(read_icon) {
                done(icon);
            }
        }
        Ok(())
    }));
    unsafe { core.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &handler) }
}

/// Runs in the page's main frame. Returns the icon links the document declares;
/// protocol-relative and relative addresses arrive absolute through `.href`.
const LINK_SCRIPT: &str = r#"(function () {
  var out = [];
  var tags = document.querySelectorAll('link[rel]');
  for (var i = 0; i < tags.length && out.length < 16; i++) {
    var tag = tags[i];
    var rel = (tag.getAttribute('rel') || '').toLowerCase();
    var href = tag.href || tag.getAttribute('href') || '';
    if (!href || !/^(https?:|data:)/i.test(href)) continue;
    var tokens = rel.split(/\s+/);
    var isIcon = false;
    var isTouch = false;
    for (var j = 0; j < tokens.length; j++) {
      if (tokens[j] === 'icon' || tokens[j] === 'fluid-icon') isIcon = true;
      if (tokens[j] === 'apple-touch-icon' || tokens[j] === 'apple-touch-icon-precomposed') isTouch = true;
    }
    if (!isIcon && !isTouch) continue;
    out.push({
      rel: rel,
      href: href,
      type: (tag.getAttribute('type') || '').toLowerCase(),
      sizes: (tag.getAttribute('sizes') || '').toLowerCase()
    });
  }
  return out;
})()"#;

/// Reads the icon links a loaded page declares, without another page request.
pub fn discover_links(
    core: &ICoreWebView2,
    done: impl FnOnce(Vec<Link>) + 'static,
) -> windows::core::Result<()> {
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
        if result.is_ok() {
            done(parse_links(&json));
        }
        Ok(())
    }));
    unsafe { core.ExecuteScript(&HSTRING::from(LINK_SCRIPT), &handler) }
}

pub fn parse_links(json: &str) -> Vec<Link> {
    serde_json::from_str::<Vec<Link>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|link| !link.href.trim().is_empty() && link.href.len() <= 2048)
        .take(MAX_LINKS)
        .collect()
}

fn link_size(link: &Link) -> u32 {
    if link.sizes.trim().eq_ignore_ascii_case("any") {
        return 512;
    }
    link.sizes
        .split_ascii_whitespace()
        .filter_map(|token| token.split_once(['x', 'X']))
        .filter_map(|(width, _)| width.trim().parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

fn is_svg_link(link: &Link, href: &str) -> bool {
    link.kind
        .to_ascii_lowercase()
        .contains("svg")
        || href
            .split(['?', '#'])
            .next()
            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".svg"))
}

/// Where to look for a page's icon, best first (Zigzag's ranking): an SVG,
/// then the smallest icon of 64px or more, then of 32px or more, then the rest,
/// largest first. Home-screen tiles only stand in for a site with no icon of
/// its own; `/favicon.ico` and `/apple-touch-icon.png` stand in when the page
/// declares none.
pub fn rank(links: &[Link], page_url: &str) -> (Vec<String>, Vec<String>) {
    let Ok(base) = url::Url::parse(page_url) else {
        return (Vec::new(), Vec::new());
    };
    let mut icons: Vec<(u32, String)> = Vec::new();
    let mut touch: Vec<(u32, String)> = Vec::new();
    let mut seen = HashSet::new();
    for link in links.iter().filter(|link| !link.href.trim().is_empty()) {
        let href = link.href.trim();
        let rel = link.rel.to_ascii_lowercase();
        // A mask icon is a monochrome stencil meant to be tinted by the page,
        // not a picture to show on its own.
        if rel.contains("mask-icon") {
            continue;
        }
        let absolute = if href.starts_with("data:") {
            href.to_string()
        } else {
            let Ok(url) = base.join(href) else { continue };
            if !matches!(url.scheme(), "http" | "https") {
                continue;
            }
            url.to_string()
        };
        if !seen.insert(absolute.clone()) {
            continue;
        }
        if rel.contains("apple-touch") {
            let size = match link_size(link) {
                0 => 180,
                size => size,
            };
            touch.push((size, absolute));
        } else {
            let size = match link_size(link) {
                0 => 16,
                size => size,
            };
            let score = if is_svg_link(link, &absolute) {
                2100
            } else if size >= 64 {
                2000 - size.min(1000)
            } else if size >= 32 {
                1000 - size
            } else {
                size
            };
            icons.push((score, absolute));
        }
    }
    if icons.is_empty() {
        if let Ok(url) = base.join("/favicon.ico") {
            icons.push((0, url.to_string()));
        }
    }
    if touch.is_empty() {
        if let Ok(url) = base.join("/apple-touch-icon.png") {
            touch.push((0, url.to_string()));
        }
    }
    icons.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    touch.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    (
        icons.into_iter().map(|(_, href)| href).collect(),
        touch.into_iter().map(|(_, href)| href).collect(),
    )
}

/// Decodes a `data:` URL's base64 payload. Percent-encoded data URLs are not
/// used for icons in practice, so they are passed over.
fn data_url_bytes(href: &str) -> Option<Vec<u8>> {
    let (meta, payload) = href.strip_prefix("data:")?.split_once(',')?;
    if !meta.to_ascii_lowercase().contains(";base64") {
        return None;
    }
    STANDARD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload))
        .ok()
}

struct Handles {
    session: *mut c_void,
    connection: *mut c_void,
    request: *mut c_void,
}

impl Drop for Handles {
    fn drop(&mut self) {
        unsafe {
            if !self.request.is_null() {
                let _ = WinHttpCloseHandle(self.request);
            }
            if !self.connection.is_null() {
                let _ = WinHttpCloseHandle(self.connection);
            }
            if !self.session.is_null() {
                let _ = WinHttpCloseHandle(self.session);
            }
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A plain GET over WinHTTP with the system proxy, a browser user agent, a
/// byte cap and short timeouts. Redirects are followed; nothing is sent but
/// the address, so fetching an icon never leaks cookies or referrers.
fn get(url: &url::Url, accept: &str, max: usize) -> Option<Vec<u8>> {
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = wide(url.host_str()?);
    let port = url.port_or_known_default().unwrap_or_else(|| {
        if url.scheme() == "https" {
            INTERNET_DEFAULT_HTTPS_PORT
        } else {
            INTERNET_DEFAULT_HTTP_PORT
        }
    });
    let mut path = String::from(url.path());
    if path.is_empty() {
        path.push('/');
    }
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    let path = wide(&path);
    let headers = wide(&format!(
        "User-Agent: {UA}\r\nAccept: {accept}\r\nAccept-Encoding: identity\r\n"
    ));
    let mut handles = Handles {
        session: std::ptr::null_mut(),
        connection: std::ptr::null_mut(),
        request: std::ptr::null_mut(),
    };
    unsafe {
        handles.session = WinHttpOpen(
            windows::core::w!("Rove"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if handles.session.is_null() {
            return None;
        }
        let _ = WinHttpSetTimeouts(handles.session, 3000, 3000, 4000, 6000);
        handles.connection = WinHttpConnect(handles.session, PCWSTR(host.as_ptr()), port, 0);
        if handles.connection.is_null() {
            return None;
        }
        handles.request = WinHttpOpenRequest(
            handles.connection,
            windows::core::w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            if url.scheme() == "https" {
                WINHTTP_FLAG_SECURE
            } else {
                WINHTTP_OPEN_REQUEST_FLAGS(0)
            },
        );
        if handles.request.is_null() {
            return None;
        }
        let sent = WinHttpSendRequest(handles.request, Some(&headers[..headers.len() - 1]), None, 0, 0, 0)
            .is_ok()
            && WinHttpReceiveResponse(handles.request, std::ptr::null_mut()).is_ok();
        if !sent {
            return None;
        }
        let mut status = 0u32;
        let mut length = std::mem::size_of::<u32>() as u32;
        let status_ok = WinHttpQueryHeaders(
            handles.request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut length,
            std::ptr::null_mut(),
        )
        .is_ok()
            && (200..400).contains(&status);
        if !status_ok {
            return None;
        }
        let mut body = Vec::new();
        loop {
            let mut read = 0u32;
            let mut chunk = [0u8; 8192];
            if WinHttpReadData(
                handles.request,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            )
            .is_err()
                || read == 0
            {
                break;
            }
            body.extend_from_slice(&chunk[..read as usize]);
            // A huge page only needs its head: icon links live there. A truncated
            // icon just fails to decode and falls through to the next candidate.
            if body.len() >= max {
                body.truncate(max);
                break;
            }
        }
        (!body.is_empty()).then_some(body)
    }
}

fn fetch_icon(href: &str) -> Option<String> {
    if href.starts_with("data:") {
        return icon_url(&data_url_bytes(href)?);
    }
    let url = url::Url::parse(href).ok()?;
    let bytes = get(&url, "image/*,*/*;q=0.8", MAX_ICON)?;
    icon_url(&bytes)
}

/// The best icon a page declares that loads: a site's own icon wins, and a
/// home-screen tile (solid, made for a phone) only stands in when the site has
/// no icon of its own at all.
pub fn resolve(links: &[Link], page_url: &str) -> Option<String> {
    let (icons, touch) = rank(links, page_url);
    let mut best: Option<String> = None;
    for href in icons.iter().take(MAX_CANDIDATES) {
        let Some(icon) = fetch_icon(href) else { continue };
        let sharpness = side(&icon);
        if sharpness >= MAX_SIDE {
            return Some(icon);
        }
        if best
            .as_ref()
            .is_none_or(|current| side(current) < sharpness)
        {
            best = Some(icon);
        }
    }
    if let Some(icon) = best {
        return Some(icon);
    }
    touch
        .iter()
        .take(2)
        .find_map(|href| fetch_icon(href))
}

/// A site icon for a saved address whose page may not be loaded: the icons its
/// front page declares, else the site's usual `/favicon.ico` and
/// `/apple-touch-icon.png`. Used once for icons cached by an older build.
pub fn site_icon(page_url: &str) -> Option<String> {
    let base = url::Url::parse(page_url).ok()?;
    if !matches!(base.scheme(), "http" | "https") {
        return None;
    }
    if let Some(html) = get(
        &base,
        "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8",
        MAX_HTML,
    ) {
        let links = icon_links(&html);
        if !links.is_empty() {
            if let Some(icon) = resolve(&links, page_url) {
                return Some(icon);
            }
        }
    }
    resolve(&[], page_url)
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let bytes = tag.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        let name = tag[name_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && matches!(bytes[i], b'"' | b'\'') {
                let quote = bytes[i];
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value = tag[start..i].to_string();
                i += 1;
            } else {
                let start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                value = tag[start..i].to_string();
            }
        }
        if !name.is_empty() {
            attrs.push((name, value));
        }
    }
    attrs
}

/// `<link rel="…icon…" href="…" sizes="…">` tags in a page's HTML, read without
/// loading the page in a web view (Zigzag's `iconLinks`).
pub fn icon_links(html: &[u8]) -> Vec<Link> {
    let html = &html[..html.len().min(MAX_HTML)];
    let text = String::from_utf8_lossy(html);
    let lower = text.to_ascii_lowercase();
    let mut links = Vec::new();
    let mut cursor = 0;
    while links.len() < MAX_LINKS {
        let Some(found) = lower[cursor..].find("<link") else {
            break;
        };
        let start = cursor + found;
        let Some(end) = lower[start..].find('>').map(|offset| start + offset) else {
            break;
        };
        if let Some(link) = parse_link(&text[start..end]) {
            links.push(link);
        }
        cursor = end + 1;
    }
    links
}

fn parse_link(tag: &str) -> Option<Link> {
    let mut link = Link::default();
    for (name, value) in attributes(tag) {
        match name.as_str() {
            "rel" => link.rel = value,
            "href" => link.href = value,
            "type" => link.kind = value,
            "sizes" => link.sizes = value,
            _ => {}
        }
    }
    if link.href.trim().is_empty() || link.href.len() > 2048 {
        return None;
    }
    let rel = link.rel.to_ascii_lowercase();
    let icon = rel
        .split_ascii_whitespace()
        .any(|token| token == "icon" || token == "fluid-icon");
    (icon || rel.contains("apple-touch-icon")).then_some(link)
}

/// Sites asked for their icon this session, so each is asked once and a site
/// that has none is left alone for a while (Zigzag's failure backoff).
#[derive(Default)]
pub struct SiteIcons {
    known: HashMap<String, String>,
    in_flight: HashSet<String>,
    failed: HashMap<String, Instant>,
}

impl SiteIcons {
    pub fn known(&self, origin: &str) -> Option<&String> {
        self.known.get(origin)
    }

    /// Whether asking this site for its declared icon is worth a request now.
    pub fn wants(&self, origin: &str) -> bool {
        !self.known.contains_key(origin)
            && !self.in_flight.contains(origin)
            && self
                .failed
                .get(origin)
                .is_none_or(|at| at.elapsed() >= RETRY_AFTER)
    }

    /// Claims a site for one fetch; false when it is already answered or being asked.
    pub fn begin(&mut self, origin: &str) -> bool {
        if !self.wants(origin) {
            return false;
        }
        self.in_flight.insert(origin.into());
        true
    }

    /// Records the answer; returns the icon when the site had one.
    pub fn finish(&mut self, origin: &str, icon: Option<String>) -> Option<String> {
        self.in_flight.remove(origin);
        match icon {
            Some(icon) => {
                self.known.insert(origin.into(), icon.clone());
                Some(icon)
            }
            None => {
                self.failed.insert(origin.into(), Instant::now());
                None
            }
        }
    }
}

impl crate::model::State {
    /// Ignore late callbacks from a previous navigation or an unloaded controller.
    /// The tab shows the icon of the page it reported; saved pages keep a sharper
    /// icon over a smaller capture, so a 16px reread never blurs a saved 64px one.
    pub fn cache_favicon(&mut self, id: &str, page_url: &str, icon: String) -> bool {
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id && t.url == page_url)
        else {
            return false;
        };
        let mut changed = false;
        // A sharper icon already on the tab (a declared SVG fetched above) must
        // not be replaced by WebView2's smaller bitmap on a later read.
        let sharper = tab
            .favicon
            .as_deref()
            .is_none_or(|current| side(&icon) >= side(current));
        if sharper && tab.favicon.as_deref() != Some(icon.as_str()) {
            tab.favicon = Some(icon.clone());
            changed = true;
        }
        for mark in self.pins.iter_mut().chain(&mut self.favorites) {
            if !same_origin(&mark.url, page_url) {
                continue;
            }
            let sharper = mark
                .favicon
                .as_deref()
                .is_none_or(|current| side(&icon) >= side(current));
            if sharper && mark.favicon.as_deref() != Some(icon.as_str()) {
                mark.favicon = Some(icon.clone());
                changed = true;
            }
        }
        changed
    }

    /// Applies the icon a loaded page declares to that page's tab, and to saved
    /// site icons when a front page speaks for its whole site (or one has no icon).
    pub fn cache_page_icon(&mut self, id: &str, page_url: &str, icon: String) -> bool {
        let mut changed = false;
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id && same_origin(&t.url, page_url))
        {
            if tab.favicon.as_deref() != Some(icon.as_str()) {
                tab.favicon = Some(icon.clone());
                changed = true;
            }
        }
        let speaks = speaks_for_site(page_url);
        for mark in self.pins.iter_mut().chain(&mut self.favorites) {
            if !same_origin(&mark.url, page_url) || !(speaks || mark.favicon.is_none()) {
                continue;
            }
            if mark.favicon.as_deref() != Some(icon.as_str()) {
                mark.favicon = Some(icon.clone());
                changed = true;
            }
        }
        changed
    }

    /// Applies a site-level icon (fetched for a saved address, no page loaded)
    /// to every saved page of that site.
    pub fn cache_site_icon(&mut self, page_url: &str, icon: String) -> bool {
        let mut changed = false;
        for tab in &mut self.tabs {
            if same_origin(&tab.url, page_url) && tab.favicon.as_deref() != Some(icon.as_str()) {
                tab.favicon = Some(icon.clone());
                changed = true;
            }
        }
        for mark in self.pins.iter_mut().chain(&mut self.favorites) {
            if same_origin(&mark.url, page_url) && mark.favicon.as_deref() != Some(icon.as_str()) {
                mark.favicon = Some(icon.clone());
                changed = true;
            }
        }
        changed
    }

    /// Whether saved icons come from a build that enlarged them to 64 pixels
    /// and are blurry; the host then re-fetches each saved site once. A session
    /// with no cached icons (a fresh install) needs no fetching.
    pub fn icons_need_upgrade(&self) -> bool {
        self.icon_version < ICON_VERSION
            && (self
                .tabs
                .iter()
                .chain(self.closed.iter())
                .any(|tab| tab.favicon.is_some())
                || self
                    .pins
                    .iter()
                    .chain(self.favorites.iter())
                    .any(|mark| mark.favicon.is_some()))
    }

    /// Records that the old cache was migrated, so later launches skip the
    /// refresh. Only called once the refresh found at least one real icon: an
    /// offline launch keeps the marker so it retries when the network is back.
    pub fn mark_icons_upgraded(&mut self) {
        self.icon_version = ICON_VERSION;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([100, 80, 200, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        png.into_inner()
    }

    fn decode(url: &str) -> (u32, u32) {
        let bytes = STANDARD
            .decode(url.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn a_small_icon_is_never_enlarged_and_a_large_one_is_shrunk() {
        assert_eq!(decode(&icon_url(&png(16, 16)).unwrap()), (16, 16));
        assert_eq!(decode(&icon_url(&png(32, 32)).unwrap()), (32, 32));
        assert_eq!(decode(&icon_url(&png(64, 64)).unwrap()), (64, 64));
        assert_eq!(decode(&icon_url(&png(128, 128)).unwrap()), (64, 64));
        assert_eq!(decode(&icon_url(&png(160, 90)).unwrap()), (64, 36));
        assert!(icon_url(b"invalid").is_none());
    }

    #[test]
    fn svg_icons_are_kept_as_vectors_and_blank_ones_are_passed_over() {
        let vector = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M0 0h16v16H0z"/></svg>"#;
        let url = icon_url(vector).unwrap();
        assert!(url.starts_with("data:image/svg+xml;base64,"));
        assert_eq!(side(&url), u32::MAX);
        let styled = br#"<svg xmlns="http://www.w3.org/2000/svg"><style>.a{fill:red}</style></svg>"#;
        assert!(icon_url(styled).is_none());
    }

    #[test]
    fn ico_icons_decode_to_their_largest_frame() {
        let image = image::RgbaImage::from_pixel(48, 48, image::Rgba([20, 120, 40, 255]));
        let frame = image::codecs::ico::IcoFrame::as_png(
            image.as_raw(),
            48,
            48,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        let mut ico = Vec::new();
        image::codecs::ico::IcoEncoder::new(&mut ico)
            .encode_images(&[frame])
            .unwrap();
        assert_eq!(decode(&icon_url(&ico).unwrap()), (48, 48));
    }

    #[test]
    fn declared_icons_rank_svg_then_large_then_small_and_fall_back() {
        let links = vec![
            Link { rel: "icon".into(), href: "/small.png".into(), kind: "image/png".into(), sizes: "16x16".into() },
            Link { rel: "shortcut icon".into(), href: "/big.png".into(), kind: "image/png".into(), sizes: "96x96".into() },
            Link { rel: "icon".into(), href: "/vector.svg".into(), kind: "image/svg+xml".into(), sizes: "any".into() },
            Link { rel: "apple-touch-icon".into(), href: "/touch.png".into(), kind: "".into(), sizes: "180x180".into() },
            Link { rel: "mask-icon".into(), href: "/mask.svg".into(), kind: "image/svg+xml".into(), sizes: "".into() },
        ];
        let (icons, touch) = rank(&links, "https://example.com/deep/page");
        assert_eq!(
            icons,
            vec![
                "https://example.com/vector.svg",
                "https://example.com/big.png",
                "https://example.com/small.png",
            ]
        );
        assert_eq!(touch, vec!["https://example.com/touch.png"]);
        // A page that declares no icon falls back to the site's usual addresses.
        let (icons, touch) = rank(&[], "https://example.com/deep/page");
        assert_eq!(icons, vec!["https://example.com/favicon.ico"]);
        assert_eq!(touch, vec!["https://example.com/apple-touch-icon.png"]);
    }

    #[test]
    fn html_icon_links_are_found_quoted_or_not() {
        let html = br#"<!doctype html><html><head>
            <link rel="icon" href="/favicon.ico">
            <link rel=icon href=//cdn.example.com/icon.png sizes=64x64 type=image/png>
            <link rel="apple-touch-icon" sizes="180x180" href="/touch.png">
            <link rel="mask-icon" href="/mask.svg">
            <link rel="stylesheet" href="/style.css">
        </head></html>"#;
        let links = icon_links(html);
        assert_eq!(links.len(), 3);
        let (icons, touch) = rank(&links, "https://example.com/page");
        assert_eq!(icons, vec!["https://cdn.example.com/icon.png", "https://example.com/favicon.ico"]);
        assert_eq!(touch, vec!["https://example.com/touch.png"]);
    }

    #[test]
    fn the_script_result_is_parsed_and_bounded() {
        let links = parse_links(r#"[{"rel":"icon","href":"https://example.com/a.svg","type":"image/svg+xml","sizes":""}]"#);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href, "https://example.com/a.svg");
        assert!(parse_links("not json").is_empty());
    }

    #[test]
    fn a_site_is_asked_once_and_failures_back_off() {
        let mut icons = SiteIcons::default();
        assert!(icons.begin("https://example.com"));
        assert!(!icons.begin("https://example.com"));
        assert_eq!(icons.finish("https://example.com", Some("data:image/png;base64,x".into())), Some("data:image/png;base64,x".into()));
        assert!(icons.known("https://example.com").is_some());
        assert!(!icons.begin("https://example.com"));
        assert!(icons.begin("https://other.example"));
        assert_eq!(icons.finish("https://other.example", None), None);
        assert!(!icons.begin("https://other.example"));
    }

    fn state_with_icon(icon: &str) -> crate::model::State {
        let mut state = crate::model::State::default();
        state.favorites[0].favicon = Some(icon.into());
        state
    }

    #[test]
    fn a_small_capture_does_not_blur_a_sharper_saved_icon() {
        let sharp = icon_url(&png(64, 64)).unwrap();
        let small = icon_url(&png(16, 16)).unwrap();
        let mut state = state_with_icon(&sharp);
        // Same origin as the default favorite, so the capture targets it.
        let page_url = state.favorites[0].url.clone();
        state.tabs[0].url = page_url.clone();
        let tab = state.tabs[0].id.clone();
        assert!(state.cache_favicon(&tab, &page_url, small.clone()));
        assert_eq!(state.favorites[0].favicon.as_deref(), Some(sharp.as_str()));
        assert_eq!(state.tabs[0].favicon.as_deref(), Some(small.as_str()));
    }

    #[test]
    fn a_sharper_capture_and_a_declared_page_icon_replace_saved_icons() {
        let small = icon_url(&png(16, 16)).unwrap();
        let sharp = icon_url(&png(64, 64)).unwrap();
        let mut state = state_with_icon(&small);
        let page_url = state.favorites[0].url.clone();
        state.tabs[0].url = page_url.clone();
        let tab = state.tabs[0].id.clone();
        assert!(state.cache_favicon(&tab, &page_url, sharp.clone()));
        assert_eq!(state.favorites[0].favicon.as_deref(), Some(sharp.as_str()));
        let vector = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Ik0wIDBoMTZ2MTZIMHoiLz48L3N2Zz4=";
        assert!(state.cache_page_icon(&tab, &page_url, vector.into()));
        assert_eq!(state.favorites[0].favicon.as_deref(), Some(vector));
    }

    #[test]
    fn a_site_refresh_replaces_every_saved_page_of_that_site_once() {
        let mut state = crate::model::State::default();
        let origin = "https://example.com";
        state.favorites[0].url = origin.into();
        state.tabs[0].url = format!("{origin}/page");
        let icon = icon_url(&png(32, 32)).unwrap();
        assert!(state.cache_site_icon(&format!("{origin}/page"), icon.clone()));
        assert_eq!(state.favorites[0].favicon.as_deref(), Some(icon.as_str()));
        assert_eq!(state.tabs[0].favicon.as_deref(), Some(icon.as_str()));
        assert!(!state.cache_site_icon(origin, icon));
    }

    #[test]
    fn an_older_session_is_upgraded_once() {
        let mut state = crate::model::State::default();
        // A fresh session has no icons to replace, so nothing is fetched.
        assert!(!state.icons_need_upgrade());
        state.mark_icons_upgraded();
        assert_eq!(state.icon_version, ICON_VERSION);

        let mut state = crate::model::State::default();
        state.icon_version = 0;
        state.favorites[0].favicon = Some(icon_url(&png(64, 64)).unwrap());
        assert!(state.icons_need_upgrade());
        // The marker is only written once the refresh found real icons.
        assert_eq!(state.icon_version, 0);
        state.mark_icons_upgraded();
        assert!(!state.icons_need_upgrade());
    }

    #[test]
    fn site_keys_ignore_the_path_and_internal_pages() {
        assert_eq!(origin_of("https://example.com/a?b=1").as_deref(), Some("https://example.com"));
        assert_eq!(origin_of("http://example.com:8080/x").as_deref(), Some("http://example.com:8080"));
        assert!(origin_of("era://newtab").is_none());
        assert!(speaks_for_site("https://example.com/"));
        assert!(speaks_for_site("https://example.com/explore"));
        assert!(!speaks_for_site("https://example.com/customers/cursor"));
    }
}
