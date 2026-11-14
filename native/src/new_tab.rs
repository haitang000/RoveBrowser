use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetKind { Clock, Github, Calendar, Notes }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetSize { Small, Wide }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Widget {
    pub id: String,
    pub kind: WidgetKind,
    pub size: WidgetSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTab {
    pub display_name: String,
    pub widgets: Vec<Widget>,
}
impl Default for NewTab {
    fn default() -> Self {
        Self {
            display_name: std::env::var("USERNAME").ok().filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "Explorer".into()).chars().take(40).collect(),
            widgets: vec![Widget { id: "clock".into(), kind: WidgetKind::Clock, size: WidgetSize::Wide, text: None, username: None }],
        }
    }
}
impl NewTab {
    pub fn validate(&mut self) -> Result<(), String> {
        self.display_name = self.display_name.trim().to_string();
        if self.display_name.is_empty() || self.display_name.chars().count() > 40 { return Err("称呼应为 1–40 个字符".into()); }
        if self.widgets.len() > 8 { return Err("最多添加 8 个小组件".into()); }
        let mut ids = HashSet::new();
        for widget in &mut self.widgets {
            if widget.id.is_empty() || widget.id.len() > 80 || !ids.insert(widget.id.clone()) { return Err("小组件标识无效".into()); }
            if widget.text.as_ref().is_some_and(|text| text.encode_utf16().count() > 10_000) { return Err("便签最多保存 10000 个字符".into()); }
            if let Some(username) = &mut widget.username {
                *username = username.trim().to_string();
                let bytes = username.as_bytes();
                if bytes.is_empty() || bytes.len() > 39 || !bytes[0].is_ascii_alphanumeric() || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
                    || bytes.windows(2).any(|pair| pair == b"--") || !bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-') {
                    return Err("请输入有效的 GitHub 用户名".into());
                }
            }
        }
        Ok(())
    }
}
