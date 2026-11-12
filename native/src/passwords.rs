//! Imported logins stay encrypted with the current Windows user's DPAPI key.
//! They never enter browser state, UI IPC, diagnostics or session JSON.
use crate::import::Login;
use windows::{
    core::{w, PWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL, HWND},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
        UI::Controls::Dialogs::{
            CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
            OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
        },
    },
};

fn crypt(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "密码数据过大".to_string())?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if decrypt {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                w!("Rove imported login"),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|_| "Windows 账户无法加密或解密该密码".to_string())?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        // Wipe the OS plaintext buffer before releasing it.
        if decrypt {
            std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        }
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(result)
    }
}
pub fn protect(login: &Login) -> Result<Vec<u8>, String> {
    let mut plain = serde_json::to_vec(login).map_err(|_| "无法编码密码".to_string())?;
    let result = crypt(&plain, false);
    plain.fill(0);
    result
}
pub fn unprotect(bytes: &[u8]) -> Result<Login, String> {
    let mut plain = crypt(bytes, true)?;
    let result = serde_json::from_slice(&plain).map_err(|_| "无法读取已保存的密码".to_string());
    plain.fill(0);
    result
}
pub fn choose_csv(owner: Option<HWND>) -> Result<Option<std::path::PathBuf>, String> {
    #[cfg(debug_assertions)]
    if let Some(base) = std::env::var_os("ERA_IMPORT_FIXTURES") {
        let path = std::path::PathBuf::from(base).join("passwords.csv");
        return Ok(path.is_file().then_some(path));
    }
    let mut file = vec![0u16; 32768];
    let filter: Vec<u16> = "密码 CSV 文件 (*.csv)\0*.csv\0\0".encode_utf16().collect();
    let mut options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner.unwrap_or_default(),
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrTitle: w!("导入浏览器导出的密码 CSV"),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    unsafe {
        if GetOpenFileNameW(&mut options).as_bool() {
            use std::os::windows::ffi::OsStringExt;
            let end = file.iter().position(|c| *c == 0).unwrap_or(file.len());
            Ok(Some(std::ffi::OsString::from_wide(&file[..end]).into()))
        } else if CommDlgExtendedError().0 == 0 {
            Ok(None)
        } else {
            Err("无法打开文件选择窗口，请重试".into())
        }
    }
}
pub fn fill_script(origin: &str, logins: &[Login]) -> String {
    let accounts: Vec<_> = logins
        .iter()
        .take(30)
        .map(|l| serde_json::json!({"username":l.username,"password":l.password}))
        .collect();
    format!(
        "({})({}, {})",
        include_str!("password-fill.js"),
        serde_json::to_string(origin).unwrap(),
        serde_json::to_string(&accounts).unwrap()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrip_keeps_plaintext_out_of_ciphertext() {
        let login = Login {
            username: "用户".into(),
            password: "unique-secret-123".into(),
            notes: "note".into(),
            otp: String::new(),
        };
        let cipher = protect(&login).unwrap();
        assert!(!cipher
            .windows(login.password.len())
            .any(|w| w == login.password.as_bytes()));
        let decoded = unprotect(&cipher).unwrap();
        assert_eq!(decoded.username, login.username);
        assert_eq!(decoded.password, login.password);
        assert!(unprotect(b"broken").is_err());
    }
}
