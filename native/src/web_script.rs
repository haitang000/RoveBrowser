//! Evaluate app-owned scripts in a named WebView2 world outside the page's JavaScript.
use serde_json::{json, Value};
use std::rc::Rc;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2,
};
use windows::core::HSTRING;

pub type Reply = Rc<dyn Fn(Option<Value>)>;
pub type Guard = Rc<dyn Fn() -> bool>;

fn protocol(core: &ICoreWebView2, method: &str, parameters: Value, reply: Reply) {
    let failed = reply.clone();
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |error, result| {
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

pub fn evaluate(
    core: &ICoreWebView2,
    world_name: &'static str,
    expression: String,
    can_execute: impl Fn() -> bool + 'static,
    reply: Reply,
) {
    let frame_core = core.clone();
    let can_execute = Rc::new(can_execute);
    protocol(
        core,
        "Page.getFrameTree",
        json!({}),
        Rc::new(move |tree| {
            let Some(frame) = tree
                .as_ref()
                .and_then(|v| v["frameTree"]["frame"]["id"].as_str())
                .filter(|_| can_execute())
            else {
                reply(None);
                return;
            };
            let eval_core = frame_core.clone();
            let expression = expression.clone();
            let can_execute = can_execute.clone();
            let reply = reply.clone();
            protocol(
                &frame_core,
                "Page.createIsolatedWorld",
                json!({"frameId": frame, "worldName": world_name}),
                Rc::new(move |world| {
                    let Some(context) = world
                        .as_ref()
                        .and_then(|v| v["executionContextId"].as_u64())
                        .filter(|_| can_execute())
                    else {
                        reply(None);
                        return;
                    };
                    let reply = reply.clone();
                    protocol(
                        &eval_core,
                        "Runtime.evaluate",
                        json!({
                            "contextId": context,
                            "expression": expression,
                            "returnByValue": true,
                            "timeout": 3000,
                        }),
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
