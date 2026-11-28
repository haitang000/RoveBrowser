use crate::https;
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{self, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    ptr::null_mut,
    time::{Duration, Instant},
};
use url::Url;
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::*,
    },
};

const DOWNLOAD: &str = "https://clients2.google.com/service/update2/crx";
const MAX_HEADER: usize = 256 * 1024;
// SHA-256 of the production Chrome Web Store publisher SPKI pinned by Chromium.
// https://chromium.googlesource.com/chromium/src/+/main/components/crx_file/crx_verifier.cc
const PUBLISHER_KEY: [u8; 32] = [
    0x61, 0xf7, 0xf2, 0xa6, 0xbf, 0xcf, 0x74, 0xcd, 0x0b, 0xc1, 0xfe, 0x24, 0x97, 0xcc, 0x9b, 0x04,
    0x25, 0x4c, 0x65, 0x8f, 0x79, 0xf2, 0x14, 0x53, 0x92, 0x86, 0x7e, 0xa8, 0x36, 0x63, 0x67, 0xcf,
];

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn id_bytes(id: &str) -> Option<[u8; 16]> {
    let text = id.as_bytes();
    if text.len() != 32 || !text.iter().all(|c| (b'a'..=b'p').contains(c)) {
        return None;
    }
    let mut bytes = [0; 16];
    for (byte, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
        *byte = ((pair[0] - b'a') << 4) | (pair[1] - b'a');
    }
    Some(bytes)
}

pub fn detail_id(url: &Url) -> Option<&str> {
    if url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let prefix = match url.host_str()? {
        "chromewebstore.google.com" => "/detail/",
        "chrome.google.com" => "/webstore/detail/",
        _ => return None,
    };
    let tail = url.path().strip_prefix(prefix)?.trim_end_matches('/');
    let id = match tail.split_once('/') {
        Some((slug, id)) if !slug.is_empty() => id,
        Some(_) => return None,
        None => tail,
    };
    id_bytes(id).map(|_| id)
}

pub fn input_id(input: &str) -> Option<String> {
    let input = input.trim();
    if id_bytes(input).is_some() {
        return Some(input.into());
    }
    detail_id(&Url::parse(input).ok()?).map(str::to_owned)
}

fn download_url(id: &str, runtime: &str) -> Result<Url> {
    let major = runtime
        .split('.')
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .filter(|major| *major >= 120)
        .ok_or("当前 WebView2 Runtime 版本无法下载商店扩展，请先更新 Runtime")?;
    let mut url = Url::parse(DOWNLOAD)?;
    url.query_pairs_mut()
        .append_pair("response", "redirect")
        .append_pair("prodversion", &format!("{major}.0.0.0"))
        .append_pair("acceptformat", "crx3")
        .append_pair("x", &format!("id={id}&installsource=ondemand&uc"));
    Ok(url)
}

fn download_host(url: &Url) -> bool {
    match url.host_str() {
        Some("clients2.google.com") => url.path() == "/service/update2/crx",
        Some("clients2.googleusercontent.com") => url.path().starts_with("/crx/blobs/"),
        Some("chrome.google.com") => url.path().starts_with("/webstore/download/"),
        _ => false,
    }
}

pub fn download(
    id: &str,
    runtime: &str,
    max: u64,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Vec<u8>> {
    debug_assert!(id_bytes(id).is_some());
    let url = download_url(id, runtime)?;
    let start = Instant::now();
    let mut bytes = Vec::new();
    https::fetch_with_progress(
        &url,
        download_host,
        "application/x-chrome-extension, application/octet-stream",
        max,
        [8_000, 8_000, 15_000, 20_000],
        &mut |chunk| {
            if start.elapsed() >= Duration::from_secs(90) {
                return Err("商店扩展下载超时，请重试".into());
            }
            bytes.extend_from_slice(chunk);
            Ok(())
        },
        progress,
    )
    .map_err(|error| match error {
        https::Error::Status(204 | 404 | 410) => "商店没有为当前版本提供这个扩展".into(),
        https::Error::Status(401 | 403) => "商店拒绝了下载请求，请确认该扩展可公开下载".into(),
        https::Error::Status(status) => format!("商店下载失败（HTTP {status}）"),
        https::Error::Transport => "无法连接 Chrome 网上应用店，请检查网络后重试".into(),
        https::Error::TooLarge => "商店扩展的安装包超过 256 MB".into(),
        https::Error::InvalidUrl | https::Error::RedirectLimit => {
            "商店返回了无法验证的下载地址".into()
        }
        https::Error::Write(message) => message,
    })?;
    Ok(bytes)
}

fn invalid() -> &'static str {
    "无法读取商店安装包的 CRX3 签名"
}

// CRX3 uses Protocol Buffers for the header. Read only the bounded header and
// skip unknown fields using their wire type.
fn varint(input: &mut &[u8]) -> Option<u64> {
    let mut result = 0;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = input.split_first()?;
        *input = rest;
        if shift == 63 && byte > 1 {
            return None;
        }
        result |= u64::from(byte & 0x7f) << shift;
        if byte < 128 {
            return Some(result);
        }
    }
    None
}

fn field<'a>(input: &mut &'a [u8]) -> Option<(u64, Option<&'a [u8]>)> {
    let tag = varint(input)?;
    let number = tag >> 3;
    if !(1..(1 << 29)).contains(&number) {
        return None;
    }
    let wire = tag & 7;
    let size = match wire {
        0 => {
            varint(input)?;
            return Some((number, None));
        }
        1 => 8,
        2 => usize::try_from(varint(input)?).ok()?,
        5 => 4,
        _ => return None,
    };
    let (bytes, rest) = input.split_at_checked(size)?;
    *input = rest;
    Some((number, (wire == 2).then_some(bytes)))
}

fn one_field(mut input: &[u8], wanted: u64) -> Option<&[u8]> {
    let mut found = None;
    while !input.is_empty() {
        let (number, bytes) = field(&mut input)?;
        if number == wanted {
            if found.is_some() {
                return None;
            }
            found = Some(bytes?);
        }
    }
    found
}

#[derive(Clone, Copy)]
enum Algorithm {
    Rsa,
    Ecdsa,
}

struct Decoded<T>(*mut T);
impl<T> Decoded<T> {
    fn new(blob: &[u8], structure: PCSTR) -> windows::core::Result<Self> {
        let mut decoded = Self(null_mut());
        unsafe {
            CryptDecodeObjectEx(
                X509_ASN_ENCODING,
                structure,
                blob,
                CRYPT_DECODE_ALLOC_FLAG,
                None,
                Some((&mut decoded.0 as *mut *mut T).cast()),
                &mut 0,
            )?;
        }
        Ok(decoded)
    }
}
impl<T> Drop for Decoded<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let _ = unsafe { LocalFree(Some(HLOCAL(self.0.cast()))) };
        }
    }
}
struct Key(BCRYPT_KEY_HANDLE);
impl Drop for Key {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            let _ = unsafe { BCryptDestroyKey(self.0) };
        }
    }
}

fn verify_signature(
    algorithm: Algorithm,
    public: &[u8],
    signature: &[u8],
    digest: &[u8],
) -> Result<()> {
    let info = Decoded::<CERT_PUBLIC_KEY_INFO>::new(public, X509_PUBLIC_KEY_INFO)?;
    let mut key = Key(BCRYPT_KEY_HANDLE::default());
    unsafe {
        CryptImportPublicKeyInfoEx2(
            X509_ASN_ENCODING,
            info.0,
            CRYPT_IMPORT_PUBLIC_KEY_FLAGS::default(),
            None,
            &mut key.0,
        )?;
    }
    let status = match algorithm {
        Algorithm::Rsa => {
            let padding = BCRYPT_PKCS1_PADDING_INFO {
                pszAlgId: BCRYPT_SHA256_ALGORITHM,
            };
            unsafe {
                BCryptVerifySignature(
                    key.0,
                    Some((&padding as *const BCRYPT_PKCS1_PADDING_INFO).cast::<c_void>()),
                    digest,
                    signature,
                    BCRYPT_PAD_PKCS1,
                )
            }
        }
        Algorithm::Ecdsa => {
            let decoded = Decoded::<CERT_ECC_SIGNATURE>::new(signature, X509_ECC_SIGNATURE)?;
            let parts = unsafe { &*decoded.0 };
            let mut raw = [0u8; 64];
            for (part, output) in [parts.r, parts.s].into_iter().zip(raw.chunks_exact_mut(32)) {
                let len = part.cbData as usize;
                if len == 0 || len > output.len() || part.pbData.is_null() {
                    return Err(invalid().into());
                }
                let little_endian = unsafe { std::slice::from_raw_parts(part.pbData, len) };
                for (byte, dest) in little_endian.iter().zip(output.iter_mut().rev()) {
                    *dest = *byte;
                }
            }
            unsafe { BCryptVerifySignature(key.0, None, digest, &raw, BCRYPT_FLAGS::default()) }
        }
    };
    status.ok()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    fn signed_fixture() -> (String, [u8; 32], Vec<u8>) {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../testdata/chrome_store_crx3.json")).unwrap();
        let id = fixture["id"].as_str().unwrap().to_owned();
        let publisher = STANDARD
            .decode(fixture["publisherHash"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let package = STANDARD
            .decode(fixture["package"].as_str().unwrap())
            .unwrap();
        (id, publisher, package)
    }

    #[test]
    fn recognizes_only_store_details_or_a_valid_id() {
        let id = "ddkjiahejlhfcafbddmgiahcphecmpfh";
        for url in [
            id.to_owned(),
            format!("https://chromewebstore.google.com/detail/example/{id}?hl=zh-CN"),
            format!("https://chrome.google.com/webstore/detail/{id}/"),
        ] {
            assert_eq!(input_id(&url).as_deref(), Some(id));
        }
        for url in [
            format!("http://chromewebstore.google.com/detail/example/{id}"),
            format!("https://chromewebstore.google.com.invalid/detail/example/{id}"),
            format!("https://example@chromewebstore.google.com/detail/example/{id}"),
            format!("https://chromewebstore.google.com/search/{id}"),
            format!("https://chromewebstore.google.com/detail/example/{id}/extra"),
        ] {
            assert!(input_id(&url).is_none(), "{url}");
            assert!(!download_host(&Url::parse(&url).unwrap()));
        }
    }

    #[test]
    fn requires_the_correct_id_publisher_and_signed_archive() {
        let (id, publisher, mut bytes) = signed_fixture();
        let verified = verify_with_publisher(&bytes, &id, &publisher).unwrap();
        assert_eq!(
            Sha256::digest(verified.developer_key)[..16],
            id_bytes(&id).unwrap()
        );
        assert!(verify(&bytes, &id).is_err());
        assert!(
            verify_with_publisher(&bytes, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &publisher).is_err()
        );
        assert!(verify_with_publisher(&bytes[..24], &id, &publisher).is_err());
        *bytes.last_mut().unwrap() ^= 1;
        assert!(verify_with_publisher(&bytes, &id, &publisher).is_err());
    }

    #[test]
    fn only_accepts_safe_windows_archive_paths() {
        for name in [
            "/manifest.json",
            "../manifest.json",
            "nested/../file",
            "C:/file",
            "dir\\file",
            "file:stream",
            "NUL.txt",
            "COM¹",
            "folder./child",
            "nested//file",
        ] {
            assert!(package_path(name).is_none(), "{name}");
        }
        assert_eq!(
            package_path("_locales/en/messages.json").unwrap(),
            PathBuf::from("_locales/en/messages.json")
        );
    }
}

pub struct Verified<'a> {
    pub archive: &'a [u8],
    pub developer_key: &'a [u8],
}

pub fn verify<'a>(bytes: &'a [u8], id: &str) -> Result<Verified<'a>> {
    verify_with_publisher(bytes, id, &PUBLISHER_KEY)
}

fn verify_with_publisher<'a>(
    bytes: &'a [u8],
    id: &str,
    publisher_key: &[u8; 32],
) -> Result<Verified<'a>> {
    let declared_id = id_bytes(id).ok_or("商店扩展 ID 无效")?;
    let header = bytes
        .strip_prefix(b"Cr24\x03\x00\x00\x00")
        .ok_or(invalid())?;
    let (size, payload) = header.split_at_checked(4).ok_or(invalid())?;
    let size = u32::from_le_bytes(size.try_into()?) as usize;
    let (header, archive) = payload
        .split_at_checked(size)
        .filter(|(_, archive)| size <= MAX_HEADER && archive.starts_with(b"PK\x03\x04"))
        .ok_or(invalid())?;
    let signed = one_field(header, 10000).ok_or(invalid())?;
    let signed_id = one_field(signed, 1).ok_or(invalid())?;
    if signed_id != declared_id {
        return Err("商店安装包与请求的扩展 ID 不一致".into());
    }
    let mut digest = Sha256::new();
    digest.update(b"CRX3 SignedData\0");
    digest.update((signed.len() as u32).to_le_bytes());
    digest.update(signed);
    digest.update(archive);
    let digest = digest.finalize();

    let mut input = header;
    let mut developer_key = None;
    let mut publisher_verified = false;
    while !input.is_empty() {
        let (tag, data) = field(&mut input).ok_or(invalid())?;
        let algorithm = match tag {
            2 => Algorithm::Rsa,
            3 => Algorithm::Ecdsa,
            _ => continue,
        };
        let data = data.ok_or(invalid())?;
        let public = one_field(data, 1).ok_or(invalid())?;
        let signature = one_field(data, 2).ok_or(invalid())?;
        let key_hash: [u8; 32] = Sha256::digest(public).into();
        verify_signature(algorithm, public, signature, &digest)
            .map_err(|_| "商店安装包签名校验失败")?;
        if key_hash[..16] == declared_id {
            developer_key = Some(public);
        }
        publisher_verified |= &key_hash == publisher_key;
    }
    if !publisher_verified {
        return Err("安装包缺少 Chrome 网上应用店的有效签名".into());
    }
    Ok(Verified {
        archive,
        developer_key: developer_key.ok_or("安装包缺少匹配扩展 ID 的开发者签名")?,
    })
}

pub(crate) fn package_path(name: &str) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for component in name.trim_end_matches('/').split('/') {
        if component.is_empty()
            || component.ends_with([' ', '.'])
            || component.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
            || component.chars().any(char::is_control)
        {
            return None;
        }
        let stem = component.split('.').next()?.trim_end().to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        {
            return None;
        }
        relative.push(component);
    }
    Some(relative)
}

pub fn unpack(
    archive: &[u8],
    payload: &Path,
    mut remaining: u64,
    max_entries: usize,
    mut progress: impl FnMut(usize, usize),
) -> Result<()> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
    if zip.len() > max_entries {
        return Err("商店扩展目录的文件数量过多".into());
    }
    fs::create_dir(payload)?;
    let total = zip.len();
    for i in 0..total {
        progress(i, total);
        let mut entry = zip.by_index(i)?;
        let name = std::str::from_utf8(entry.name_raw())?;
        let relative = package_path(name).ok_or("商店安装包包含无效路径")?;
        if entry.is_symlink() {
            return Err("商店安装包包含符号链接".into());
        }
        // Store bookkeeping is not part of the unpacked extension.
        if relative.starts_with("_metadata") {
            continue;
        }
        let target = payload.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
            continue;
        }
        if entry.size() > remaining {
            return Err("解包后的商店扩展超过 256 MB".into());
        }
        fs::create_dir_all(target.parent().unwrap())?;
        let mut file = File::create_new(target)?;
        let written = io::copy(&mut entry.by_ref().take(remaining + 1), &mut file)?;
        if written != entry.size() {
            return Err("商店安装包的文件大小无效".into());
        }
        remaining -= written;
    }
    Ok(())
}
