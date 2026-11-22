use serde::Deserialize;
use std::{ptr, rc::Rc};
use webview2_com::{
    take_pwstr, ExecuteScriptCompletedHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2,
};
use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::{WAIT_FAILED, WAIT_OBJECT_0},
        Globalization::LocaleNameToLCID,
        Media::Speech::{
            ISpObjectToken, ISpObjectTokenCategory, ISpVoice, SpObjectTokenCategory, SpVoice,
            SPCAT_VOICES, SPF_ASYNC, SPF_IS_NOT_XML, SPF_PURGEBEFORESPEAK, SPVOICESTATUS,
        },
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Threading::WaitForSingleObject,
        },
    },
};

pub const RATES: [i32; 5] = [-3, -1, 0, 1, 3];
pub const TEXT_LIMIT: usize = 100_000;
pub const PLAYBACK_ERROR: &str = "无法播放朗读音频。请检查 Windows 语音和音频设置。";
const SPEAK_FLAGS: u32 = (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32;

#[derive(Debug, Deserialize)]
pub struct Text {
    pub text: String,
    pub lang: String,
}

pub fn setting_announcement(enabled: bool, language: &str) -> Text {
    let (lang, on, off) = match language {
        "en" => (
            "en-US",
            "Read aloud is on. Press Control plus Shift plus U to read aloud.",
            "Read aloud is off.",
        ),
        "zh-TW" => (
            "zh-TW",
            "已開啟文字朗讀，按 Control 加 Shift 加 U 鍵即可朗讀",
            "已關閉文字朗讀",
        ),
        _ => (
            "zh-CN",
            "已打开文本朗读，通过 Control 加 Shift 加 U 键以朗读",
            "已关闭文本朗读",
        ),
    };
    Text {
        text: if enabled { on } else { off }.into(),
        lang: lang.into(),
    }
}

pub fn extract(
    core: &ICoreWebView2,
    url: Option<&str>,
    selection: Option<String>,
    report: impl Fn(Option<Text>) + 'static,
) {
    let script = format!(
        "((url, selectionOnly, limit) => {{ {}\n}})({}, {}, {})",
        include_str!("read-aloud.js"),
        serde_json::json!(url),
        selection.is_some(),
        TEXT_LIMIT,
    );
    let reply: crate::web_script::Reply = Rc::new(move |value| {
        let mut text = value.and_then(|value| serde_json::from_value::<Text>(value).ok());
        if let Some(selection) = &selection {
            let lang = text.take().map_or_else(String::new, |text| text.lang);
            text = Some(Text {
                text: selection.clone(),
                lang,
            });
        }
        report(text);
    });
    if url.is_some() {
        crate::web_script::evaluate(core, "Rove Read Aloud", script, || true, reply);
    } else {
        // Reader contains sanitized markup with page scripts disabled. ExecuteScript
        // can still read this host-owned document, like the Reader appearance script.
        let on_result = reply.clone();
        let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
            on_result(
                result
                    .ok()
                    .and_then(|_| serde_json::from_str(&json.to_string()).ok()),
            );
            Ok(())
        }));
        if unsafe { core.ExecuteScript(&HSTRING::from(script), &handler) }.is_err() {
            reply(None);
        }
    }
}

pub struct Speech {
    voice: ISpVoice,
    pub paused: bool,
}

impl Speech {
    pub fn start(text: &Text, rate: i32) -> windows::core::Result<Option<Self>> {
        let text_to_speak: String = text
            .text
            .replace('\0', " ")
            .trim()
            .chars()
            .take(TEXT_LIMIT)
            .collect();
        if text_to_speak.is_empty() {
            return Ok(None);
        }
        let lang = crate::reader::language(&text.lang, &text_to_speak);
        // The host UI thread already owns a COM apartment for WebView2.
        unsafe {
            let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_INPROC_SERVER)?;
            if let Some(token) = matching_voice(&lang) {
                let _ = voice.SetVoice(&token);
            }
            voice.SetRate(rate)?;
            voice.Speak(&HSTRING::from(text_to_speak), SPEAK_FLAGS, None)?;
            Ok(Some(Self {
                voice,
                paused: false,
            }))
        }
    }

    pub fn toggle_pause(&mut self) -> windows::core::Result<()> {
        unsafe {
            if self.paused {
                self.voice.Resume()?;
            } else {
                self.voice.Pause()?;
            }
        }
        self.paused = !self.paused;
        Ok(())
    }

    pub fn finished(&self) -> windows::core::Result<bool> {
        unsafe {
            let wait = WaitForSingleObject(self.voice.SpeakCompleteEvent(), 0);
            if wait == WAIT_FAILED {
                return Err(windows::core::Error::from_thread());
            }
            if wait != WAIT_OBJECT_0 {
                return Ok(false);
            }
            let mut status = SPVOICESTATUS::default();
            self.voice.GetStatus(&mut status, ptr::null_mut())?;
            status.hrLastResult.ok()?;
            Ok(true)
        }
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        unsafe {
            let _ = self.voice.Speak(PCWSTR::null(), SPEAK_FLAGS, None);
            if self.paused {
                let _ = self.voice.Resume();
            }
        }
    }
}

fn matching_voice(lang: &str) -> Option<ISpObjectToken> {
    if lang.is_empty() || lang.len() > 85 {
        return None;
    }
    unsafe {
        let wanted = LocaleNameToLCID(&HSTRING::from(lang), 0);
        if wanted == 0 {
            return None;
        }
        let category: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_INPROC_SERVER).ok()?;
        category.SetId(SPCAT_VOICES, false).ok()?;
        let tokens = category.EnumTokens(PCWSTR::null(), PCWSTR::null()).ok()?;
        let mut fallback = None;
        let mut count = 0;
        tokens.GetCount(&mut count).ok()?;
        for token in (0..count).filter_map(|index| tokens.Item(index).ok()) {
            let Ok(attributes) = token.OpenKey(w!("Attributes")) else {
                continue;
            };
            let Ok(languages) = attributes.GetStringValue(w!("Language")) else {
                continue;
            };
            let languages = take_pwstr(languages);
            for locale in languages
                .split(';')
                .filter_map(|code| u32::from_str_radix(code.trim(), 16).ok())
            {
                if locale == wanted {
                    return Some(token);
                }
                if fallback.is_none() && (locale & 0x3ff) == (wanted & 0x3ff) {
                    fallback = Some(token.clone());
                }
            }
        }
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::setting_announcement;

    #[test]
    fn setting_announcements_use_the_interface_language_and_voice_locale() {
        for (locale, voice, on, off) in [
            (
                "zh-CN",
                "zh-CN",
                "已打开文本朗读，通过 Control 加 Shift 加 U 键以朗读",
                "已关闭文本朗读",
            ),
            (
                "zh-TW",
                "zh-TW",
                "已開啟文字朗讀，按 Control 加 Shift 加 U 鍵即可朗讀",
                "已關閉文字朗讀",
            ),
            (
                "en",
                "en-US",
                "Read aloud is on. Press Control plus Shift plus U to read aloud.",
                "Read aloud is off.",
            ),
        ] {
            for (enabled, message) in [(true, on), (false, off)] {
                let announcement = setting_announcement(enabled, locale);
                assert_eq!(announcement.lang, voice);
                assert_eq!(announcement.text, message);
            }
        }
    }
}
