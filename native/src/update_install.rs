//! Stage the replacement before quitting. A copy of this executable waits for
//! the browser to release its files, applies the update, then reopens Rove.
use crate::updates::sha256_file;
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};
use uuid::Uuid;
use windows::{
    core::{w, HSTRING},
    Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR},
};
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
    System::Threading::{OpenProcess, WaitForSingleObject, CREATE_NO_WINDOW, PROCESS_SYNCHRONIZE},
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
    RegKey,
};

const APP_EXE: &str = "Rove.exe";
const HELPER_EXE: &str = "rove-update-helper.exe";
const UPDATE_PREFIX: &str = "rove-update-";
const MODE: &str = "--apply-update";
const CLEANUP_ENV: &str = "ROVE_UPDATE_CLEANUP";
const RESULT_ENV: &str = "ROVE_UPDATE_RESULT";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{76F5D0C8-2824-42C5-A964-CAD38178E0BA}_is1";
const REPARSE_POINT: u32 = 0x400;
const DELETE_ON_CLOSE: u32 = 0x04000000;
const EXTRACT_LIMIT: u64 = 256 * 1024 * 1024;
const ENTRY_LIMIT: usize = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageKind {
    Setup,
    Portable,
}

impl PackageKind {
    pub fn for_executable(exe: &Path) -> Self {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let installed = user
                .open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | view)
                .and_then(|key| key.get_value::<OsString, _>("InstallLocation"))
                .and_then(|path| fs::canonicalize(PathBuf::from(path).join(APP_EXE)));
            if let Ok(installed) = installed {
                if fs::canonicalize(exe).is_ok_and(|path| path == installed) {
                    return Self::Setup;
                }
            }
        }
        Self::Portable
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::Setup => "-windows-x64-setup.exe",
            Self::Portable => "-windows-x64.zip",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Self::Setup => "setup.exe",
            Self::Portable => "package.zip",
        }
    }
}

#[derive(Debug)]
pub struct PreparedUpdate {
    id: Uuid,
    target: PathBuf,
    kind: PackageKind,
    original_hash: String,
    handed_off: bool,
}

fn workspace(id: Uuid) -> PathBuf {
    std::env::temp_dir().join(format!("{UPDATE_PREFIX}{id}"))
}

fn portable_stage(dir: &Path, id: Uuid) -> PathBuf {
    dir.join(format!(".{UPDATE_PREFIX}{id}"))
}

impl PreparedUpdate {
    pub fn new(target: PathBuf, kind: PackageKind) -> Result<Self, String> {
        let original_hash = sha256_file(&target)?;
        let id = Uuid::new_v4();
        fs::create_dir(workspace(id)).map_err(|e| format!("无法创建更新目录：{e}"))?;
        Ok(Self {
            id,
            target,
            kind,
            original_hash,
            handed_off: false,
        })
    }

    pub fn package(&self) -> PathBuf {
        workspace(self.id).join(self.kind.file())
    }

    pub fn stage(&self) -> Result<(), String> {
        let package = self.package();
        match self.kind {
            PackageKind::Setup => {
                if !has_executable_header(&package) {
                    return Err("下载的更新包不是 Windows 安装程序。".into());
                }
            }
            PackageKind::Portable => {
                let stage = portable_stage(self.target.parent().unwrap(), self.id);
                fs::create_dir(&stage).map_err(|e| format!("无法在程序目录准备更新：{e}"))?;
                extract(&package, &stage.join("files"))
                    .map_err(|e| format!("无法解压更新包：{e}"))?;
            }
        }
        fs::copy(&self.target, workspace(self.id).join(HELPER_EXE))
            .map_err(|e| format!("无法准备更新程序：{e}"))?;
        Ok(())
    }

    pub fn start(mut self) -> Result<(), String> {
        Command::new(workspace(self.id).join(HELPER_EXE))
            .arg(MODE)
            .arg(std::process::id().to_string())
            .arg(&self.target)
            .arg(self.kind.file())
            .arg(&self.original_hash)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("无法启动更新程序：{e}"))?;
        self.handed_off = true;
        Ok(())
    }
}

impl Drop for PreparedUpdate {
    fn drop(&mut self) {
        if self.handed_off {
            return;
        }
        let _ = fs::remove_dir_all(workspace(self.id));
        if self.kind == PackageKind::Portable {
            let _ = fs::remove_dir_all(portable_stage(self.target.parent().unwrap(), self.id));
        }
    }
}

fn has_executable_header(path: &Path) -> bool {
    let mut magic = [0; 2];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && magic == *b"MZ"
}

fn invalid_package() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "ZIP 中包含无效的更新文件")
}

fn bundle_path(path: &Path) -> bool {
    let mut parts = path.components();
    let Some(Component::Normal(first)) = parts.next() else {
        return false;
    };
    if ["docs", "licenses"].iter().any(|dir| first == *dir) {
        return parts.all(|part| {
            let Component::Normal(name) = part else {
                return false;
            };
            name.to_str().is_some_and(|name| {
                !name.contains([':', '<', '>', '"', '|', '?', '*'])
                    && !name.ends_with([' ', '.'])
                    && !name.chars().any(char::is_control)
            })
        });
    }
    parts.next().is_none()
        && [
            APP_EXE,
            "LICENSE",
            "README.md",
            "THIRD_PARTY_NOTICES.md",
            "运行说明.txt",
        ]
        .iter()
        .any(|file| first == *file)
}

fn extract(zip: &Path, files: &Path) -> io::Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip)?)?;
    if archive.len() > ENTRY_LIMIT {
        return Err(invalid_package());
    }
    fs::create_dir(files)?;
    let files = fs::canonicalize(files)?;
    let mut remaining = EXTRACT_LIMIT;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let path = entry
            .enclosed_name()
            .filter(|path| bundle_path(path))
            .ok_or_else(invalid_package)?;
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink() || entry.size() > remaining {
            return Err(invalid_package());
        }
        let to = files.join(path);
        fs::create_dir_all(to.parent().unwrap())?;
        let mut output = File::create_new(to)?;
        let count = io::copy(&mut entry.by_ref().take(remaining + 1), &mut output)?;
        if count != entry.size() {
            return Err(invalid_package());
        }
        remaining -= count;
    }
    if !has_executable_header(&files.join(APP_EXE)) {
        return Err(invalid_package());
    }
    Ok(())
}

/// Only the staged copy can enter helper mode. Normal shell launches still
/// accept only web URLs.
pub fn run_helper() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(MODE.as_ref()) {
        return false;
    }
    let Ok(helper) = std::env::current_exe() else {
        return true;
    };
    let Some(id) = helper
        .parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(UPDATE_PREFIX))
        .and_then(|text| Uuid::parse_str(text).ok())
    else {
        return true;
    };
    if helper != workspace(id).join(HELPER_EXE) {
        return true;
    }
    let Some(parent) = args
        .next()
        .and_then(|arg| arg.to_str()?.parse::<u32>().ok())
    else {
        return true;
    };
    let Some(target) = args
        .next()
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    else {
        return true;
    };
    let kind = match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("setup.exe") => PackageKind::Setup,
        Some("package.zip") => PackageKind::Portable,
        _ => return true,
    };
    let Some(hash) = args.next().and_then(|arg| arg.into_string().ok()) else {
        return true;
    };
    run_update(id, parent, &target, kind, &hash);
    true
}

fn wait_for_exit(pid: u32) -> io::Result<()> {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        // The browser can finish shutdown before the helper opens the handle.
        return if pid != 0 && error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(())
        } else {
            Err(error)
        };
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    if unsafe { WaitForSingleObject(handle.as_raw_handle(), 45_000) } == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "等待 Rove 退出超时",
        ))
    }
}

fn run_update(id: Uuid, parent: u32, target: &Path, kind: PackageKind, hash: &str) {
    let directory = target.parent().unwrap();
    let stage = portable_stage(directory, id);
    let downloaded = workspace(id).join(kind.file());
    let result = (|| -> Result<(), String> {
        wait_for_exit(parent).map_err(|e| e.to_string())?;
        // Different profiles can run the same executable. Serialize mutations
        // and reject a prepared update if another process has already applied one.
        let _lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .custom_flags(DELETE_ON_CLOSE)
            .open(directory.join(".rove-update.lock"))
            .map_err(|e| format!("无法锁定程序目录：{e}"))?;
        if sha256_file(target)? != hash {
            return Err("程序文件已变化，请重新检查更新。".into());
        }
        match kind {
            PackageKind::Setup => install_setup(&downloaded, directory),
            PackageKind::Portable => install_portable(target, &stage),
        }
    })();
    let _ = fs::remove_file(downloaded);
    if kind == PackageKind::Portable && !stage.join("keep-backups").exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    let message = match result {
        Ok(()) => "Rove 已完成更新。".into(),
        Err(error) => format!("Rove 更新失败：{error}"),
    };
    if let Err(error) = Command::new(target)
        .env(CLEANUP_ENV, id.to_string())
        .env(RESULT_ENV, &message)
        .spawn()
    {
        let body = HSTRING::from(format!("{message}\n无法重新启动 Rove：{error}"));
        unsafe { MessageBoxW(None, &body, w!("Rove 更新"), MB_ICONERROR) };
    }
}

fn install_setup(setup: &Path, directory: &Path) -> Result<(), String> {
    let mut directory_arg = OsString::from("/DIR=");
    directory_arg.push(directory);
    let status = Command::new(setup)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/SP-",
            "/NORESTART",
            "/NOCLOSEAPPLICATIONS",
            "/NORESTARTAPPLICATIONS",
            "/RESTARTEXITCODE=100",
        ])
        .arg(directory_arg)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("无法运行安装程序：{e}"))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(100) => Err("安装程序需要重启 Windows 才能完成更新。".into()),
        _ => Err(format!("安装程序未完成（{status}）。")),
    }
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else {
            files.push(entry.path().strip_prefix(root).unwrap().to_owned());
        }
    }
    Ok(())
}

fn checked_destination(directory: &Path, relative: &Path) -> io::Result<bool> {
    let mut destination = directory.to_owned();
    let mut parts = relative.iter().peekable();
    while let Some(part) = parts.next() {
        destination.push(part);
        let last = parts.peek().is_none();
        match fs::symlink_metadata(&destination) {
            Ok(meta)
                if meta.file_attributes() & REPARSE_POINT == 0
                    && if last { meta.is_file() } else { meta.is_dir() } =>
            {
                if last {
                    return Ok(true);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "更新路径被非普通文件或目录占用",
                ))
            }
        }
    }
    Ok(false)
}

fn install_portable(target: &Path, stage: &Path) -> Result<(), String> {
    let directory = target.parent().unwrap();
    // Keep staging on the program's drive, so each rename replaces a complete
    // file and never exposes an executable that was only partially copied.
    let files = stage.join("files");
    let backups = stage.join("backups");
    let mut paths = Vec::new();
    collect_files(&files, &files, &mut paths).map_err(|e| e.to_string())?;
    paths.retain(|path| path != Path::new(APP_EXE));
    paths.push(PathBuf::from(APP_EXE));

    let mut applied = Vec::<(PathBuf, Option<PathBuf>)>::new();
    let result = (|| -> io::Result<()> {
        for source in paths {
            let relative = if source == Path::new(APP_EXE) {
                Path::new(target.file_name().unwrap())
            } else {
                &source
            };
            let existed = checked_destination(directory, relative)?;
            let destination = directory.join(relative);
            let backup = if existed {
                let backup = backups.join(relative);
                fs::create_dir_all(backup.parent().unwrap())?;
                fs::copy(&destination, &backup)?;
                Some(backup)
            } else {
                fs::create_dir_all(destination.parent().unwrap())?;
                None
            };
            fs::rename(files.join(source), &destination)?;
            applied.push((destination, backup));
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_failed = false;
        for (destination, backup) in applied.into_iter().rev() {
            let restored = match backup {
                Some(backup) => fs::rename(backup, destination),
                None => fs::remove_file(destination),
            };
            rollback_failed |= restored.is_err();
        }
        if rollback_failed {
            let _ = File::create(stage.join("keep-backups"));
            return Err(format!("{error}。恢复文件保存在 {}", backups.display()));
        }
        return Err(error.to_string());
    }
    Ok(())
}

pub fn startup_message() -> Option<String> {
    let cleanup = std::env::var(CLEANUP_ENV).ok();
    let message = std::env::var(RESULT_ENV).ok();
    std::env::remove_var(CLEANUP_ENV);
    std::env::remove_var(RESULT_ENV);
    if let Some(id) = cleanup.as_deref().and_then(|id| Uuid::parse_str(id).ok()) {
        std::thread::spawn(move || {
            let dir = workspace(id);
            for _ in 0..30 {
                match fs::remove_file(dir.join(HELPER_EXE)) {
                    Ok(()) => break,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            let _ = fs::remove_dir(dir);
        });
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    fn fixture() -> PreparedUpdate {
        let id = Uuid::new_v4();
        let directory = workspace(id);
        fs::create_dir(&directory).unwrap();
        let target = directory.join("我的 Rove.exe");
        fs::write(&target, b"MZ old").unwrap();
        // Keep both the package and the mock installation in the test workspace.
        PreparedUpdate {
            id,
            target,
            kind: PackageKind::Portable,
            original_hash: String::new(),
            handed_off: false,
        }
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, body) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn update_extracts_and_replaces_portable_files_in_place() {
        let update = fixture();
        let directory = update.target.parent().unwrap();
        fs::write(directory.join("保留.txt"), "user file").unwrap();
        write_zip(
            &update.package(),
            &[(APP_EXE, b"MZ new"), ("docs/说明.md", b"new docs")],
        );
        update.stage().unwrap();
        install_portable(&update.target, &portable_stage(directory, update.id)).unwrap();
        assert_eq!(fs::read(&update.target).unwrap(), b"MZ new");
        assert_eq!(
            fs::read(directory.join("docs/说明.md")).unwrap(),
            b"new docs"
        );
        assert_eq!(
            fs::read_to_string(directory.join("保留.txt")).unwrap(),
            "user file"
        );
    }

    #[test]
    fn update_rolls_back_portable_files_if_the_target_cannot_be_replaced() {
        let update = fixture();
        let directory = update.target.parent().unwrap();
        let readme = directory.join("README.md");
        fs::write(&readme, "old docs").unwrap();
        write_zip(
            &update.package(),
            &[(APP_EXE, b"MZ new"), ("README.md", b"new docs")],
        );
        update.stage().unwrap();
        let locked = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&update.target)
            .unwrap();
        assert!(install_portable(&update.target, &portable_stage(directory, update.id)).is_err());
        assert_eq!(fs::read_to_string(readme).unwrap(), "old docs");
        assert_eq!(fs::read(&update.target).unwrap(), b"MZ old");
        drop(locked);
    }

    #[test]
    fn update_requires_the_bundle_executable_and_safe_entry_paths() {
        let update = fixture();
        write_zip(&update.package(), &[("README.md", b"docs only")]);
        assert!(update.stage().is_err());
        assert!(!bundle_path(Path::new("docs/../Rove.exe")));
        assert!(!bundle_path(Path::new("docs/report.txt:alternate")));
        assert!(!bundle_path(Path::new("user-profile/era.sqlite")));
    }
}
