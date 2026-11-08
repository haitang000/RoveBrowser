//! Persist real WebView2 favicon pixels, rather than a URL that needs another request.
use base64::{engine::general_purpose::STANDARD, Engine};
use webview2_com::{
    GetFaviconCompletedHandler,
    Microsoft::Web::WebView2::Win32::{ICoreWebView2_15, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG},
};
use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};

pub fn same_origin(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => matches!(a.scheme(), "http" | "https") && a.origin() == b.origin(),
        _ => false,
    }
}

fn png_data(png: &[u8]) -> Option<String> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(8 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.thumbnail(64, 64);
    let mut output = std::io::Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(output.into_inner())
    ))
}

fn read_icon(stream: &IStream) -> Option<String> {
    let mut stat = STATSTG::default();
    // WebView2 owns this stream on the UI thread; cap input and decoded pixels.
    unsafe {
        stream.Stat(&mut stat, STATFLAG_NONAME).ok()?;
        if stat.cbSize == 0 || stat.cbSize > 256 * 1024 {
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
        png_data(&bytes)
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

impl crate::model::State {
    /// Ignore late callbacks from a previous navigation or an unloaded controller.
    pub fn cache_favicon(&mut self, id: &str, page_url: &str, icon: String) -> bool {
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id && t.url == page_url)
        else {
            return false;
        };
        tab.favicon = Some(icon.clone());
        let mut changed = false;
        for mark in self.pins.iter_mut().chain(&mut self.favorites) {
            if same_origin(&mark.url, page_url) && mark.favicon.as_ref() != Some(&icon) {
                mark.favicon = Some(icon.clone());
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_icons_survive_unloading_and_restart_and_ignore_stale_callbacks() {
        use crate::{
            model::{Command, State},
            storage::Storage,
        };
        let path = std::env::temp_dir().join(format!("rove-icons-{}", uuid::Uuid::new_v4()));
        let mut state = State::default();
        let live = state.tabs[0].id.clone();
        let page_url = state.tabs[0].url.clone();
        let icon = "data:image/png;base64,cached".to_string();
        assert!(!state.cache_favicon(&live, "https://stale.example/", icon.clone()));
        assert!(state.cache_favicon(&live, &page_url, icon.clone()));
        state.close(&live);
        assert!(!state.cache_favicon(&live, &page_url, "late".into()));
        {
            let storage = Storage::open(&path).unwrap();
            storage.save(&state).unwrap();
        }
        {
            let storage = Storage::open(&path).unwrap();
            let mut restored = storage.load().unwrap();
            let mark = restored.favorites[0].id.clone();
            assert_eq!(restored.favorites[0].favicon.as_ref(), Some(&icon));
            restored.open_bookmark(&mark).unwrap();
            assert_eq!(restored.active().unwrap().favicon.as_ref(), Some(&icon));
            restored
                .apply(&Command::UpdateBookmark {
                    id: mark.clone(),
                    title: "Explore".into(),
                    url: "https://github.com/explore".into(),
                })
                .unwrap();
            assert_eq!(
                restored.bookmark(&mark).unwrap().favicon.as_ref(),
                Some(&icon)
            );
            restored
                .apply(&Command::UpdateBookmark {
                    id: mark.clone(),
                    title: "Other".into(),
                    url: "https://example.com".into(),
                })
                .unwrap();
            assert!(restored.bookmark(&mark).unwrap().favicon.is_none());
        }
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn cache_contains_bounded_png_pixels() {
        let image = image::RgbaImage::from_pixel(128, 128, image::Rgba([100, 80, 200, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let data = png_data(png.get_ref()).unwrap();
        let pixels = STANDARD
            .decode(data.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&pixels).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 64));
        assert!(png_data(b"invalid").is_none());
    }
}
