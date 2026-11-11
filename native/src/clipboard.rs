//! The one clipboard write Rove needs for Zigzag's "Copy Link" menu item.
//! WebView2 does not expose the system clipboard to the trusted shell, so the
//! host owns the Win32 call and the UI only sends the text.
use windows::Win32::{
    Foundation::HANDLE,
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
    },
};

pub const CF_UNICODETEXT: u32 = 13;

pub fn write_text(text: &str) -> windows::core::Result<()> {
    // SAFETY: the clipboard is opened and closed in this call on the UI thread;
    // the global block is owned by the clipboard after SetClipboardData succeeds.
    unsafe {
        OpenClipboard(None)?;
        let result = (|| -> windows::core::Result<()> {
            EmptyClipboard()?;
            let mut wide: Vec<u16> = text.encode_utf16().collect();
            wide.push(0);
            let bytes = wide.len() * std::mem::size_of::<u16>();
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes)?;
            let pointer = GlobalLock(handle);
            if pointer.is_null() {
                return Err(windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), pointer.cast::<u8>(), bytes);
            let _ = GlobalUnlock(handle);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(handle.0)))?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}
