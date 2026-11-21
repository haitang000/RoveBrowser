use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    ffi::OsString,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    rc::Rc,
};
use uuid::Uuid;
use webview2_com::{
    take_pwstr, BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2BrowserExtension, ICoreWebView2Profile7, ICoreWebView2_13,
    },
    ProfileAddBrowserExtensionCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler,
};
use windows::{
    core::{w, Interface, HSTRING, PWSTR},
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT},
        System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
        UI::Shell::{
            FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
            FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
        },
    },
};
use wry::{WebView, WebViewExtWindows};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
const MAX_MANIFEST: u64 = 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    manifest_version: u32,
    name: String,
    version: String,
    key: Option<String>,
    default_locale: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    optional_permissions: Vec<String>,
    #[serde(default)]
    host_permissions: Vec<String>,
    #[serde(default)]
    optional_host_permissions: Vec<String>,
    #[serde(default)]
    content_scripts: Vec<ContentScript>,
}

#[derive(Deserialize)]
struct ContentScript {
    matches: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    ticket: String,
    name: String,
    version: String,
    permissions: BTreeSet<String>,
    hosts: BTreeSet<String>,
}

#[derive(Debug)]
pub struct Prepared {
    stage: PathBuf,
    package: PathBuf,
    id: String,
    pub review: Review,
}
impl Prepared {
    pub fn matches(&self, ticket: &str) -> bool {
        self.review.ticket == ticket
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        // Stage names are generated beneath the managed root and never come from IPC.
        let _ = fs::remove_dir_all(&self.stage);
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    id: String,
    version: String,
}

#[derive(Serialize)]
pub struct Item {
    id: String,
    name: String,
    version: String,
    enabled: bool,
}

pub fn choose_folder(owner: HWND) -> Result<Option<PathBuf>> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("ERA_EXTENSION_FIXTURE") {
        return Ok(Some(path.into()));
    }
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR,
        )?;
        dialog.SetTitle(w!("选择包含 manifest.json 的扩展目录"))?;
        match dialog.Show(Some(owner)) {
            Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => return Ok(None),
            result => result?,
        }
        let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let value = OsString::from_wide(path.as_wide());
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(value.into()))
    }
}

fn reparse(metadata: &Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
}

fn regular_file(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || reparse(&metadata) {
        return Err("扩展文件必须是普通文件".into());
    }
    Ok(file)
}

fn json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let mut bytes = Vec::new();
    regular_file(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("清单文件过大".into());
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    Ok(serde_json::from_slice(bytes)?)
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    depth: usize,
    budget: &mut (u64, usize),
) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_dir() || reparse(&metadata) {
        return Err("扩展目录包含链接或不可读取的目录".into());
    }
    if depth > 32 {
        return Err("扩展目录层级过深".into());
    }
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path)?;
        if budget.1 == 0 {
            return Err("扩展目录的文件数量过多".into());
        }
        budget.1 -= 1;
        if reparse(&metadata) {
            return Err("扩展目录包含符号链接或目录联接".into());
        }
        if metadata.is_dir() {
            copy_directory(&path, &target, depth + 1, budget)?;
        } else if metadata.is_file() && metadata.len() <= budget.0 {
            let mut source = regular_file(&path)?.take(budget.0 + 1);
            let mut target = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            let copied = io::copy(&mut source, &mut target)?;
            budget.0 = budget.0.checked_sub(copied).ok_or("扩展目录超过 256 MB")?;
        } else {
            return Err("扩展目录包含特殊文件或超过 256 MB".into());
        }
    }
    Ok(())
}

fn localized_name(manifest: &Manifest, payload: &Path) -> String {
    let safe_component = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    };
    let Some(key) = manifest
        .name
        .strip_prefix("__MSG_")
        .and_then(|name| name.strip_suffix("__"))
        .filter(|key| safe_component(key))
    else {
        return manifest.name.clone();
    };
    let Some(locale) = manifest
        .default_locale
        .as_deref()
        .filter(|locale| safe_component(locale))
    else {
        return manifest.name.clone();
    };
    let messages = payload.join("_locales").join(locale).join("messages.json");
    json_file::<HashMap<String, serde_json::Value>>(&messages)
        .ok()
        .and_then(|entries| {
            entries
                .into_iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
        })
        .and_then(|(_, value)| value.get("message")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| manifest.name.clone())
}

fn id_for(payload: &Path, key: Option<&str>) -> Result<String> {
    // Chromium uses the public key when present, otherwise the Windows path as
    // UTF-16LE with an uppercase drive letter. Check the ID before Add can replace
    // another extension, and verify it against the runtime result afterwards.
    let bytes = if let Some(key) = key {
        let body = key.trim();
        let body = if body.starts_with("-----BEGIN ") {
            body.split_once("-----\n")
                .or_else(|| body.split_once("-----\r\n"))
                .and_then(|(_, rest)| rest.split_once("-----END "))
                .map(|(body, _)| body)
                .ok_or("扩展公钥格式无效")?
        } else {
            body
        };
        let bytes = STANDARD.decode(body.replace(|c: char| c.is_ascii_whitespace(), ""))?;
        if bytes.is_empty() {
            return Err("扩展公钥为空".into());
        }
        bytes
    } else {
        let mut wide: Vec<_> = payload.as_os_str().encode_wide().collect();
        if wide.get(1) == Some(&(b':' as u16)) && (b'a' as u16..=b'z' as u16).contains(&wide[0]) {
            wide[0] -= (b'a' - b'A') as u16;
        }
        wide.into_iter().flat_map(u16::to_le_bytes).collect()
    };
    Ok(Sha256::digest(bytes)[..16]
        .iter()
        .flat_map(|byte| {
            [
                char::from(b'a' + (byte >> 4)),
                char::from(b'a' + (byte & 0x0f)),
            ]
        })
        .collect())
}

pub fn prepare(root: &Path, source: &Path) -> Result<Prepared> {
    if reparse(&fs::symlink_metadata(source)?) {
        return Err("请选择实际的扩展目录".into());
    }
    let source = fs::canonicalize(source)?;
    if !source.join("manifest.json").is_file() {
        return Err("所选目录缺少 manifest.json".into());
    }
    fs::create_dir_all(root)?;
    let canonical_root = fs::canonicalize(root)?;
    if source.starts_with(&canonical_root) || canonical_root.starts_with(&source) {
        return Err("请选择浏览资料目录之外的扩展目录".into());
    }
    let source_path: Vec<_> = source
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect();
    let source_key = Sha256::digest(source_path);
    let package = root.join(format!("{source_key:x}"));
    let ticket = Uuid::new_v4().to_string();
    let stage = root.join(format!("staging-{ticket}"));
    fs::create_dir(&stage)?;
    let mut prepared = Prepared {
        stage,
        package,
        id: String::new(),
        review: Review {
            ticket,
            name: String::new(),
            version: String::new(),
            permissions: BTreeSet::new(),
            hosts: BTreeSet::new(),
        },
    };
    let payload = prepared.stage.join("payload");
    copy_directory(&source, &payload, 0, &mut (MAX_BYTES, MAX_FILES))?;
    let manifest: Manifest = json_file(&payload.join("manifest.json"))
        .map_err(|error| format!("无法读取扩展清单：{error}"))?;
    if manifest.manifest_version != 3
        || manifest.name.trim().is_empty()
        || manifest.version.trim().is_empty()
    {
        return Err("请选择有名称和版本号的 Manifest V3 扩展".into());
    }
    prepared.id = id_for(&prepared.package.join("payload"), manifest.key.as_deref())?;
    prepared.review.name = localized_name(&manifest, &payload);
    prepared.review.version = manifest.version;
    prepared.review.permissions = manifest
        .permissions
        .into_iter()
        .chain(manifest.optional_permissions)
        .collect();
    prepared.review.hosts = manifest
        .host_permissions
        .into_iter()
        .chain(manifest.optional_host_permissions)
        .chain(
            manifest
                .content_scripts
                .into_iter()
                .flat_map(|script| script.matches),
        )
        .collect();
    let record = Record {
        id: prepared.id.clone(),
        version: prepared.review.version.clone(),
    };
    serde_json::to_writer(File::create(prepared.stage.join("record.json"))?, &record)?;
    Ok(prepared)
}

fn valid_package_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn managed_packages(root: &Path) -> Result<Vec<(PathBuf, Record)>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    Ok(entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            if !valid_package_name(name.to_str()?)
                || !entry.file_type().ok()?.is_dir()
                || reparse(&fs::symlink_metadata(entry.path()).ok()?)
            {
                return None;
            }
            let record = json_file(&entry.path().join("record.json")).ok()?;
            Some((entry.path(), record))
        })
        .collect())
}

fn remove_package(root: &Path, package: &Path) -> Result<()> {
    if package.parent() != Some(root)
        || !package
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(valid_package_name)
    {
        return Err("扩展目录不在受管理范围内".into());
    }
    let root = fs::canonicalize(root)?;
    let target = fs::canonicalize(package)?;
    if target.parent() != Some(root.as_path()) || reparse(&fs::symlink_metadata(package)?) {
        return Err("扩展目录指向其他位置".into());
    }
    fs::remove_dir_all(target)?;
    Ok(())
}

fn extension_id(extension: &ICoreWebView2BrowserExtension) -> windows::core::Result<String> {
    unsafe {
        let mut value = PWSTR::null();
        extension.Id(&mut value)?;
        Ok(take_pwstr(value))
    }
}

pub struct Profile(ICoreWebView2Profile7);
impl Profile {
    pub fn new(view: &WebView) -> Result<Self> {
        let profile = unsafe { view.webview().cast::<ICoreWebView2_13>()?.Profile()? };
        Ok(Self(profile.cast().map_err(|_| {
            "当前 WebView2 Runtime 不支持扩展，请更新后重试"
        })?))
    }

    fn installed(&self) -> Result<HashMap<String, ICoreWebView2BrowserExtension>> {
        let output = Rc::new(RefCell::new(HashMap::new()));
        let list = output.clone();
        let profile = self.0.clone();
        ProfileGetBrowserExtensionsCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe { Ok(profile.GetBrowserExtensions(&handler)?) }),
            Box::new(move |status, extensions| {
                status?;
                if let Some(extensions) = extensions {
                    let mut count = 0;
                    unsafe {
                        extensions.Count(&mut count)?;
                    }
                    for index in 0..count {
                        let extension = unsafe { extensions.GetValueAtIndex(index)? };
                        list.borrow_mut()
                            .insert(extension_id(&extension)?, extension);
                    }
                }
                Ok(())
            }),
        )?;
        Ok(output.take())
    }

    pub fn list(&self, root: &Path) -> Result<Vec<Item>> {
        let installed = self.installed()?;
        let mut items = Vec::new();
        for (_, record) in managed_packages(root)? {
            let Some(extension) = installed.get(&record.id) else {
                continue;
            };
            unsafe {
                let mut name = PWSTR::null();
                let mut enabled = windows::core::BOOL::default();
                extension.IsEnabled(&mut enabled)?;
                extension.Name(&mut name)?;
                items.push(Item {
                    id: record.id,
                    name: take_pwstr(name),
                    version: record.version,
                    enabled: enabled.as_bool(),
                });
            }
        }
        items.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(items)
    }

    pub fn install(&self, root: &Path, prepared: Prepared) -> Result<()> {
        let existing = self.installed()?;
        if existing.contains_key(&prepared.id) {
            return Err("该扩展已安装。重新安装前请先卸载".into());
        }
        if prepared.package.try_exists()? {
            let record: Record = json_file(&prepared.package.join("record.json"))?;
            if existing.contains_key(&record.id) {
                return Err("该目录的扩展已安装。重新安装前请先卸载".into());
            }
            remove_package(root, &prepared.package)?;
        }
        fs::rename(&prepared.stage, &prepared.package)?;
        let path = HSTRING::from(prepared.package.join("payload").as_path());
        let profile = self.0.clone();
        let added = Rc::new(RefCell::new(None));
        let result = added.clone();
        let install = ProfileAddBrowserExtensionCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe { Ok(profile.AddBrowserExtension(&path, &handler)?) }),
            Box::new(move |status, extension| {
                status?;
                *result.borrow_mut() = extension;
                Ok(())
            }),
        );
        match (install, added.take()) {
            (Ok(()), Some(extension)) => {
                let id = extension_id(&extension)?;
                if id == prepared.id {
                    return Ok(());
                }
                if existing.contains_key(&id) {
                    return Err("WebView2 返回了与现有扩展冲突的标识".into());
                }
                Self::uninstall(extension)?;
                remove_package(root, &prepared.package)?;
                Err("WebView2 返回了意外的扩展标识，安装已撤销".into())
            }
            (result, _) => {
                remove_package(root, &prepared.package)?;
                result?;
                Err("WebView2 未返回已安装的扩展".into())
            }
        }
    }

    fn managed(&self, root: &Path, id: &str) -> Result<(PathBuf, ICoreWebView2BrowserExtension)> {
        let (path, _) = managed_packages(root)?
            .into_iter()
            .find(|(_, record)| record.id == id)
            .ok_or("找不到该扩展")?;
        let extension = self
            .installed()?
            .remove(id)
            .ok_or("该扩展已从浏览资料中移除")?;
        Ok((path, extension))
    }

    pub fn enable(&self, root: &Path, id: &str, enabled: bool) -> Result<()> {
        let (_, extension) = self.managed(root, id)?;
        BrowserExtensionEnableCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe { Ok(extension.Enable(enabled, &handler)?) }),
            Box::new(|status| status),
        )?;
        Ok(())
    }

    fn uninstall(extension: ICoreWebView2BrowserExtension) -> Result<()> {
        BrowserExtensionRemoveCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe { Ok(extension.Remove(&handler)?) }),
            Box::new(|status| status),
        )?;
        Ok(())
    }

    pub fn remove(&self, root: &Path, id: &str) -> Result<()> {
        let (path, extension) = self.managed(root, id)?;
        Self::uninstall(extension)?;
        remove_package(root, &path)
            .map_err(|error| format!("扩展已从浏览资料移除，无法删除本地副本：{error}").into())
    }
}
