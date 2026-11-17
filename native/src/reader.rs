//! Behavioral port of Zigzag Web/Reader.swift (79437afc), Zigao Wang, Apache-2.0.
//! Extract in a named isolated world; render only sanitized markup, with scripts off.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, rc::Rc};
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2,
};
use windows::core::HSTRING;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub size: u8,
    pub face: String,
    pub theme: String,
    pub width: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            size: 20,
            face: "serif".into(),
            theme: "auto".into(),
            width: "medium".into(),
        }
    }
}
impl Preferences {
    pub fn valid(&self) -> bool {
        (14..=32).contains(&self.size)
            && matches!(self.face.as_str(), "serif" | "sans")
            && matches!(self.theme.as_str(), "auto" | "light" | "sepia" | "dark")
            && matches!(self.width.as_str(), "narrow" | "medium" | "wide")
    }
    fn measure(&self) -> &str {
        match self.width.as_str() {
            "narrow" => "29em",
            "wide" => "40em",
            _ => "34em",
        }
    }
    pub fn script(&self) -> String {
        format!("document.documentElement.dataset.theme={};document.documentElement.dataset.face={};document.documentElement.style.setProperty('--size','{}px');document.documentElement.style.setProperty('--measure','{}');",
            json!(self.theme), json!(self.face), self.size, self.measure())
    }
    pub fn step(&mut self, action: &str) {
        self.size = match action {
            "reset" => 20,
            "out" => self.size.saturating_sub(2).max(14),
            _ => self.size.saturating_add(2).min(32),
        };
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub title: String,
    pub byline: String,
    pub site: String,
    #[serde(default)]
    pub site_title: String,
    pub content: String,
    pub length: usize,
    pub lang: String,
    #[serde(default)]
    pub sample: String,
    #[serde(default)]
    pub published: String,
    pub url: String,
}

type Reply = Rc<dyn Fn(Option<Value>)>;
fn protocol(core: &ICoreWebView2, method: &str, parameters: Value, reply: Reply) {
    let failed = reply.clone();
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |error, result| {
            // Bound renderer-owned data before parsing it. Nothing returned is a host command.
            let value = if error.is_ok() {
                let text = result.to_string();
                if text.len() <= 8 * 1024 * 1024 {
                    serde_json::from_str(&text).ok()
                } else {
                    None
                }
            } else {
                None
            };
            reply(value);
            Ok(())
        }));
    // SAFETY: WebView2 and its callbacks stay on the owning UI thread.
    if unsafe {
        core.CallDevToolsProtocolMethod(
            &HSTRING::from(method),
            &HSTRING::from(parameters.to_string()),
            &handler,
        )
    }
    .is_err()
    {
        failed(None);
    }
}

/// A separate JS world, as WebKit's .defaultClient: sites cannot replace our
/// parser or cache. No remote debugger or page message bridge is required.
pub fn evaluate(core: &ICoreWebView2, expression: String, reply: Reply) {
    let core = core.clone();
    let frame_core = core.clone();
    protocol(
        &core,
        "Page.getFrameTree",
        json!({}),
        Rc::new(move |tree| {
            let Some(frame) = tree
                .as_ref()
                .and_then(|v| v["frameTree"]["frame"]["id"].as_str())
            else {
                reply(None);
                return;
            };
            let eval_core = frame_core.clone();
            let expression = expression.clone();
            let reply = reply.clone();
            protocol(
                &frame_core,
                "Page.createIsolatedWorld",
                json!({"frameId": frame, "worldName": "Rove Reader"}),
                Rc::new(move |world| {
                    let Some(context) = world
                        .as_ref()
                        .and_then(|v| v["executionContextId"].as_u64())
                    else {
                        reply(None);
                        return;
                    };
                    let reply = reply.clone();
                    protocol(
                        &eval_core,
                        "Runtime.evaluate",
                        json!({"contextId": context, "expression": expression, "returnByValue": true, "timeout": 3000}),
                        Rc::new(move |result| {
                            reply(result.and_then(|v| {
                                if v.get("exceptionDetails").is_none() {
                                    v["result"].get("value").cloned()
                                } else {
                                    None
                                }
                            }));
                        }),
                    );
                }),
            );
        }),
    );
}

pub fn probe(core: &ICoreWebView2, url: &str, report: impl Fn(bool) + 'static) {
    let script = format!("(()=>{{if(location.href!=={})return false;{};const available=isProbablyReaderable(document,{{minScore:10,minContentLength:100}});if(available){{if(typeof window.__roveReader?.take!=='function'){{{}\n{}\n}}window.__roveReader.prepare();}}return available;}})()", json!(url), include_str!("reader/Readability-readerable.js"), include_str!("reader/Readability.js"), include_str!("reader/extract.js"));
    evaluate(
        core,
        script,
        Rc::new(move |value| report(value == Some(Value::Bool(true)))),
    );
}

pub fn extract(core: &ICoreWebView2, url: &str, report: impl Fn(Option<Article>) + 'static) {
    let script = format!("(()=>{{if(location.href!=={})return null;if(typeof window.__roveReader?.take!=='function'){{{}\n{}\n}}return window.__roveReader.take();}})()", json!(url), include_str!("reader/Readability.js"), include_str!("reader/extract.js"));
    evaluate(
        core,
        script,
        Rc::new(move |value| {
            report(
                value
                    .and_then(|v| {
                        v.as_str()
                            .and_then(|s| serde_json::from_str::<Article>(s).ok())
                    })
                    .filter(|a| a.length >= 100 && !a.content.is_empty()),
            )
        }),
    );
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn author(byline: &str, site: &str) -> String {
    let name = byline.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = if name.to_lowercase().starts_with("by ") {
        &name[3..]
    } else {
        name.strip_prefix("作者").unwrap_or(&name)
    };
    let name = name.trim_matches(|c: char| c.is_whitespace() || "|-–—·•,:：/".contains(c));
    if name.chars().count() > 60
        || name.eq_ignore_ascii_case(site)
        || name.to_lowercase().contains("contributors")
        || name.to_lowercase().contains("http")
        || name.contains(['@', '/'])
    {
        String::new()
    } else {
        name.into()
    }
}
fn title(article: &Article, author: &str) -> String {
    let title = article.title.trim();
    for separator in [" - ", " | ", " – ", " — ", " · ", " :: ", "_"] {
        if let Some((head, tail)) = title.rsplit_once(separator) {
            let tail = tail.trim().to_lowercase();
            let site = [&article.site, &article.site_title].iter().any(|s| {
                let s = s.trim().to_lowercase();
                !s.is_empty() && (tail == s || s.chars().count() >= 4 && tail.contains(&s))
            });
            if head.trim().chars().count() >= 2
                && (site
                    || author.chars().count() >= 2
                        && tail.contains(&author.to_lowercase())
                        && tail.chars().count() > author.chars().count())
            {
                return head.trim().into();
            }
        }
    }
    title.into()
}
fn language(declared: &str, sample: &str) -> String {
    let (mut letters, mut han, mut kana, mut hangul) = (0, 0, 0, 0);
    for c in sample.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        match c as u32 {
            0x3040..=0x30ff => kana += 1,
            0xac00..=0xd7af | 0x1100..=0x11ff => hangul += 1,
            0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff => han += 1,
            _ => {}
        }
    }
    if letters >= 20 {
        if kana * 10 > letters {
            return "ja".into();
        }
        if hangul * 3 > letters {
            return "ko".into();
        }
        if han * 3 > letters && !declared.starts_with("zh") && !declared.starts_with("ja") {
            return "zh".into();
        }
    }
    declared.into()
}

pub fn html(article: &Article, preferences: &Preferences) -> String {
    // Readability is an extractor, not a sanitizer. Strip executable content,
    // event attributes, forms and embedded documents before crossing origins.
    let content = ammonia::Builder::default()
        .url_schemes(HashSet::from(["http", "https"]))
        .url_relative(ammonia::UrlRelative::RewriteWithBase(
            url::Url::parse(&article.url).expect("validated article URL"),
        ))
        .add_tags([
            "figure",
            "figcaption",
            "picture",
            "source",
            "details",
            "summary",
        ])
        .add_tag_attributes("img", ["class", "width", "height"])
        .clean(&article.content)
        .to_string();
    let author = author(&article.byline, &article.site);
    let title = escape(&title(article, &author));
    let lang = escape(&language(&article.lang, &article.sample));
    let date = escape(&article.published);
    let byline = if author.is_empty() && date.is_empty() {
        String::new()
    } else {
        format!(
            "<p class=\"byline\"><span class=\"author\">{}</span><time>{date}</time></p>",
            escape(&author)
        )
    };
    format!(
        r#"<!doctype html><html lang="{lang}" data-reader-phase="loading" data-theme="{}" data-face="{}" style="--size:{}px;--measure:{}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src http: https: data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><meta name="referrer" content="no-referrer"><title>{title}</title><style>{}</style></head><body><main><header><p class="site">{}</p><h1 class="title">{title}</h1>{byline}</header><article>{content}</article><footer class="end"><a href="{}">在 {} 阅读原文</a></footer></main></body></html>"#,
        preferences.theme,
        preferences.face,
        preferences.size,
        preferences.measure(),
        include_str!("reader/reader.css"),
        escape(&article.site),
        escape(&article.url),
        escape(&article.site)
    )
}

/// Trusted host code only: the document still has scripts disabled and no IPC.
pub fn motion_script(opening: bool, reduce_motion: bool) -> String {
    format!("(()=>{{const opening={opening};const reduceMotion={reduce_motion};{}\n}})()", include_str!("reader/motion.js"))
}
pub fn resources_script(cleanup: bool) -> String {
    format!("(()=>{{const cleanup={cleanup};{}\n}})()", include_str!("reader/resources.js"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanitizes_active_content_and_resolves_relative_links() {
        let article = Article { title: "<unsafe> - Example".into(), byline: "By Alice".into(), site: "Example".into(), site_title: String::new(),
            content: r#"<script>alert(1)</script><iframe src="https://evil.test"></iframe><p onclick="alert(1)">正文 <a href="/next">next</a><a href="javascript:alert(1)">bad</a><img src="/photo.png" onerror="alert(1)"></p>"#.into(),
            length: 100, lang: "en".into(), sample: String::new(), published: String::new(), url: "https://example.test/article".into() };
        let html = html(&article, &Preferences::default());
        assert!(!html.contains("<script"));
        assert!(!html.contains("<iframe"));
        assert!(!html.contains("onclick"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("javascript:"));
        assert!(html.contains("https://example.test/next"));
        assert!(html.contains("https://example.test/photo.png"));
        assert!(html.contains("&lt;unsafe&gt;</h1>"));
        assert!(html.contains(">Alice</span>"));
    }
    #[test]
    fn preferences_are_bounded_and_legacy_sessions_have_defaults() {
        let mut p: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Preferences::default());
        for _ in 0..30 {
            p.step("in");
        }
        assert_eq!(p.size, 32);
        for _ in 0..30 {
            p.step("out");
        }
        assert_eq!(p.size, 14);
        p.step("reset");
        assert_eq!(p.size, 20);
        p.theme = "';alert(1)//".into();
        assert!(!p.valid());
        assert_eq!(language("en", &"这是一篇中文文章".repeat(5)), "zh");
    }
}
