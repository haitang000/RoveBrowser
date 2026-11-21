//! Fixed Agent operations on the selected browser document.
use crate::agent_bridge::{PageCommand, Response};
use serde_json::{json, Value};
use std::{
    rc::Rc,
    sync::mpsc::SyncSender,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use windows::core::BOOL;
use wry::{WebView, WebViewExtWindows};

const MAX_FILL_BYTES: usize = 4096;
const SCRIPT: &str = include_str!("agent-page.js");

pub fn run(
    view: &WebView,
    url: &str,
    command: PageCommand,
    deadline: Instant,
    reply: SyncSender<Response>,
) {
    let mut params = match parameters(command) {
        Ok(params) => params,
        Err(code) => {
            let _ = reply.try_send(Response::error(code));
            return;
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_add(remaining)
        .as_millis() as u64;
    params["url"] = json!(url);
    params["expires"] = json!(expires);
    let expression = format!("({SCRIPT})({params})");
    let controller = view.controller();
    crate::web_script::evaluate(
        &view.webview(),
        "Rove Agent Page",
        expression,
        move || {
            let mut visible = BOOL::default();
            // SAFETY: the controller and callback belong to the UI thread.
            Instant::now() < deadline
                && unsafe { controller.IsVisible(&mut visible) }.is_ok()
                && visible.as_bool()
        },
        Rc::new(move |result| {
            let response = match result {
                Some(value) => decode_result(value),
                None if Instant::now() >= deadline => Response::error("timeout"),
                None => Response::error("page_unavailable"),
            };
            let _ = reply.try_send(response);
        }),
    );
}

fn parameters(command: PageCommand) -> Result<Value, &'static str> {
    let (snapshot, element, text) = match command {
        PageCommand::Snapshot { offset, .. } => {
            return Ok(
                json!({"op": "snapshot", "snapshot": Uuid::new_v4().to_string(), "offset": offset}),
            );
        }
        PageCommand::Click {
            snapshot, element, ..
        } => (snapshot, element, None),
        PageCommand::Fill {
            snapshot,
            element,
            text,
            ..
        } => {
            if text.len() > MAX_FILL_BYTES {
                return Err("text_too_large");
            }
            (snapshot, element, Some(text))
        }
    };
    let snapshot = Uuid::parse_str(&snapshot).map_err(|_| "invalid_element")?;
    if element == 0 {
        return Err("invalid_element");
    }
    let op = if text.is_some() { "fill" } else { "click" };
    Ok(json!({"op": op, "snapshot": snapshot.to_string(), "element": element, "text": text}))
}

fn decode_result(mut value: Value) -> Response {
    match value.get("error").and_then(Value::as_str) {
        Some("page_changed") => Response::error("page_changed"),
        Some("page_not_ready") => Response::error("page_not_ready"),
        Some("tab_not_visible") => Response::error("tab_not_visible"),
        Some("stale_snapshot") => Response::error("stale_snapshot"),
        Some("stale_element") => Response::error("stale_element"),
        Some("element_not_found") => Response::error("element_not_found"),
        Some("unsupported_element") => Response::error("unsupported_element"),
        Some("timeout") => Response::error("timeout"),
        None if value["result"].is_object() => Response::ok(value["result"].take()),
        _ => Response::error("page_unavailable"),
    }
}
