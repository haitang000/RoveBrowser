//! In-memory snapshots for native Peek's HWND airspace and Desk thumbnails.
//! Never written to session/history/disk or exposed to site scripts.
use base64::{engine::general_purpose::STANDARD, Engine};
use webview2_com::{
    CapturePreviewCompletedHandler,
    Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::Com::{
        IStream, StructuredStorage::CreateStreamOnHGlobal, STATFLAG_NONAME, STATSTG,
        STREAM_SEEK_SET,
    },
};
use wry::{WebView, WebViewExtWindows};

fn bytes(stream: &IStream) -> windows::core::Result<Vec<u8>> {
    let mut stat = STATSTG::default();
    // SAFETY: stream is an owned in-memory COM stream on the WebView's UI thread.
    unsafe {
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        if stat.cbSize == 0 || stat.cbSize > 8 * 1024 * 1024 {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x8007000Eu32 as i32,
            )));
        }
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut bytes = vec![0u8; stat.cbSize as usize];
        let mut read = 0;
        stream
            .Read(
                bytes.as_mut_ptr().cast(),
                bytes.len() as u32,
                Some(&mut read),
            )
            .ok()?;
        bytes.truncate(read as usize);
        Ok(bytes)
    }
}
pub fn background(
    view: &WebView,
    done: impl FnOnce(Option<String>) + 'static,
) -> windows::core::Result<()> {
    // A null HGLOBAL asks COM to allocate and free its own backing buffer.
    let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true)? };
    let output = stream.clone();
    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        done(
            result
                .and_then(|_| bytes(&output))
                .ok()
                .map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes))),
        );
        Ok(())
    }));
    unsafe {
        view.webview().CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        )?;
    }
    Ok(())
}

/// Capture the actual renderer, then resize/encode off the window thread.
/// The UI receives a bounded JPEG rather than full viewport PNGs for every tab.
pub fn thumbnail(
    view: &WebView,
    done: impl FnOnce(Option<String>) + Send + 'static,
) -> windows::core::Result<()> {
    let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true)? };
    let output = stream.clone();
    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        let captured = result.and_then(|_| bytes(&output));
        std::thread::spawn(move || done(captured.ok().and_then(|png| small_jpeg(&png))));
        Ok(())
    }));
    unsafe {
        view.webview().CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        )?;
    }
    Ok(())
}
fn small_jpeg(png: &[u8]) -> Option<String> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.thumbnail(480, 320).to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 75)
        .encode_image(&image)
        .ok()?;
    (jpeg.len() <= 160 * 1024).then(|| format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_is_a_real_bounded_image() {
        let source = image::RgbImage::from_pixel(1600, 900, image::Rgb([222, 32, 40]));
        let mut png = std::io::Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let url = small_jpeg(png.get_ref()).unwrap();
        let bytes = STANDARD
            .decode(url.strip_prefix("data:image/jpeg;base64,").unwrap())
            .unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), (480, 270));
        assert!(url.len() < 220 * 1024);
        assert!(small_jpeg(b"invalid screenshot").is_none());
    }
}
