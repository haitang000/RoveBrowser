//! Per-user browser capabilities. Windows owns UserChoice; we never write it.
use serde::Serialize;
use std::path::Path;
use windows::{
    core::{w, HSTRING, PWSTR},
    Win32::UI::{
        Shell::{
            AssocQueryStringW, SHChangeNotify, ShellExecuteW, ASSOCF_IS_PROTOCOL, ASSOCSTR_PROGID,
            SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
        },
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const CLIENT: &str = "Software\\Clients\\StartMenuInternet\\Rove";
const PROG_ID: &str = "RoveURL";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub http: bool,
    pub https: bool,
}

// Debug integration uses a private HKCU subtree; tests never replace real defaults.
fn root() -> std::io::Result<RegKey> {
    let user = RegKey::predef(HKEY_CURRENT_USER);
    #[cfg(debug_assertions)]
    if let Ok(path) = std::env::var("ERA_ASSOCIATIONS_FIXTURES") {
        if !path.starts_with("Software\\Rove\\Tests\\") || path.contains("..") {
            return Err(std::io::Error::other("无效的关联测试路径"));
        }
        return Ok(user.create_subkey(path)?.0);
    }
    Ok(user)
}

fn set(root: &RegKey, path: &str, name: &str, value: &str) -> std::io::Result<()> {
    root.create_subkey(path)?.0.set_value(name, &value)
}

pub fn register_at(root: &RegKey, executable: &Path) -> std::io::Result<()> {
    let path = executable
        .to_str()
        .filter(|s| !s.contains(['"', '\0']))
        .ok_or_else(|| std::io::Error::other("无效的应用路径"))?;
    let command = format!("\"{path}\" -- \"%1\"");
    let icon = format!("\"{path}\",0");
    set(
        root,
        &format!("Software\\Classes\\{PROG_ID}"),
        "",
        "Rove 网页链接",
    )?;
    set(
        root,
        &format!("Software\\Classes\\{PROG_ID}"),
        "URL Protocol",
        "",
    )?;
    set(
        root,
        &format!("Software\\Classes\\{PROG_ID}\\DefaultIcon"),
        "",
        &icon,
    )?;
    set(
        root,
        &format!("Software\\Classes\\{PROG_ID}\\shell\\open\\command"),
        "",
        &command,
    )?;
    set(root, CLIENT, "", "Rove")?;
    set(root, &format!("{CLIENT}\\DefaultIcon"), "", &icon)?;
    set(
        root,
        &format!("{CLIENT}\\shell\\open\\command"),
        "",
        &format!("\"{path}\""),
    )?;
    let capabilities = format!("{CLIENT}\\Capabilities");
    set(root, &capabilities, "ApplicationName", "Rove")?;
    set(
        root,
        &capabilities,
        "ApplicationDescription",
        "Rove 网页浏览器",
    )?;
    set(root, &capabilities, "ApplicationIcon", &icon)?;
    for protocol in ["http", "https"] {
        set(
            root,
            &format!("{capabilities}\\URLAssociations"),
            protocol,
            PROG_ID,
        )?;
    }
    set(
        root,
        "Software\\RegisteredApplications",
        "Rove",
        &capabilities,
    )
}

fn handler(protocol: &str) -> Option<String> {
    let mut buffer = [0u16; 512];
    let mut size = buffer.len() as u32;
    unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL,
            ASSOCSTR_PROGID,
            &HSTRING::from(protocol),
            None,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut size,
        )
        .ok()
        .ok()?;
    }
    let end = buffer.iter().position(|c| *c == 0)?;
    String::from_utf16(&buffer[..end]).ok()
}
pub fn status() -> Status {
    Status {
        http: handler("http").is_some_and(|s| s == PROG_ID),
        https: handler("https").is_some_and(|s| s == PROG_ID),
    }
}
pub fn open_settings() -> Result<(), String> {
    register_at(
        &root().map_err(|_| "无法注册当前用户的浏览器关联")?,
        &std::env::current_exe().map_err(|_| "无法定位 Rove 程序")?,
    )
    .map_err(|_| "无法注册 Rove，请重新安装后重试")?;
    #[cfg(debug_assertions)]
    if std::env::var_os("ERA_ASSOCIATIONS_FIXTURES").is_some() {
        return Ok(());
    }
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        // Windows 11 opens the Rove page. Older Windows versions fall back to
        // Default Apps, where the user selects Rove as the web browser.
        let result = ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:defaultapps?registeredAppUser=Rove"),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if result.0 as isize > 32 {
            return Ok(());
        }
        let result = ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:defaultapps"),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err("无法打开 Windows 默认应用设置".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capabilities_quote_paths_and_preserve_user_choice() {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!("Software\\Rove\\Tests\\{}", uuid::Uuid::new_v4());
        {
            let root = user.create_subkey(&path).unwrap().0;
            set(&root,"Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\https\\UserChoice", "ProgId", "ExistingBrowser").unwrap();
            let exe = Path::new("C:\\中文 路径\\Rove.exe");
            register_at(&root, exe).unwrap();
            register_at(&root, exe).unwrap();
            let command: String = root
                .open_subkey("Software\\Classes\\RoveURL\\shell\\open\\command")
                .unwrap()
                .get_value("")
                .unwrap();
            assert_eq!(command, "\"C:\\中文 路径\\Rove.exe\" -- \"%1\"");
            let app: String = root
                .open_subkey("Software\\RegisteredApplications")
                .unwrap()
                .get_value("Rove")
                .unwrap();
            for scheme in ["http", "https"] {
                assert_eq!(
                    root.open_subkey(format!("{app}\\URLAssociations"))
                        .unwrap()
                        .get_value::<String, _>(scheme)
                        .unwrap(),
                    PROG_ID
                );
            }
            assert_eq!(root.open_subkey("Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\https\\UserChoice").unwrap().get_value::<String,_>("ProgId").unwrap(),"ExistingBrowser");
            assert!(!root.open_subkey("Software\\Classes\\http").is_ok());
            assert!(register_at(&root, Path::new("C:\\bad\"path.exe")).is_err());
        }
        user.delete_subkey_all(path).unwrap();
    }
}
