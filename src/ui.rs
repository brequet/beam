use std::sync::Arc;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    runtime::{Event, procedure},
    view::view,
};

use crate::input::{InputService, KeyName};

/// Shareable host address, registered as app context and rendered on the page.
pub struct HostInfo {
    pub url: String,
}

#[page("/")]
pub async fn home(cx: &Cx) -> Result {
    let info: &HostInfo = app_context(cx);
    let url = info.url.clone();

    view! {
        signal text = String::new();
        signal status = "Ready.".to_owned();

        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"beam"</title>
                topcoat::runtime::script()
                <style>"body { margin: 0; font-family: system-ui, sans-serif; background: #0f172a; color: #e2e8f0; display: flex; justify-content: center; padding: 24px 16px; min-height: 100vh; } main { width: 100%; max-width: 560px; display: flex; flex-direction: column; gap: 12px; } h1 { margin: 0; font-size: 1.6rem; letter-spacing: 0.02em; } .muted { margin: 0; color: #94a3b8; font-size: 0.9rem; } .url { margin: 0; font-size: 0.9rem; color: #94a3b8; } .url code { color: #7dd3fc; background: #1e293b; padding: 2px 8px; border-radius: 6px; } textarea { width: 100%; min-height: 160px; resize: vertical; border: 1px solid #334155; border-radius: 10px; padding: 12px; font: inherit; background: #1e293b; color: #e2e8f0; } textarea:focus { outline: 2px solid #38bdf8; outline-offset: 1px; border-color: transparent; } .row { display: flex; gap: 8px; flex-wrap: wrap; } button { border: 1px solid #334155; background: #1e293b; color: #e2e8f0; border-radius: 10px; padding: 10px 16px; font: inherit; cursor: pointer; } button:hover { background: #334155; } button:active { transform: translateY(1px); } .primary { flex: 1; background: #0ea5e9; border-color: #0ea5e9; color: #082f49; font-weight: 600; } .primary:hover { background: #38bdf8; } .status { margin: 4px 0 0; min-height: 1.2em; color: #94a3b8; font-size: 0.9rem; }"</style>
            </head>
            <body>
                <main>
                    <h1>"beam"</h1>
                    <p class="muted">"Type below and send it to the focused window on the host."</p>
                    <p class="url">"Open from any device on the Wi-Fi: " <code>(url)</code></p>

                    <textarea
                        rows="6"
                        placeholder="Type or paste text, then press Send Text."
                        :value=$(text.get())
                        @input=$(|e: Event| text.set(e.target.value))
                    ></textarea>

                    <div class="row">
                        <button
                            class="primary"
                            @click=$(async |_e| {
                                let outcome = send_text(text.get()).await;
                                if outcome.is_ok() {
                                    text.set("".to_owned());
                                }
                                status.set(if outcome.is_ok() {
                                    outcome.unwrap()
                                } else {
                                    outcome.err().unwrap()
                                });
                            })
                        >"Send Text"</button>
                    </div>

                    <div class="row">
                        <button @click=$(async |_e| {
                            let outcome = press_key("enter".to_owned()).await;
                            status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                        })>"Enter"</button>
                        <button @click=$(async |_e| {
                            let outcome = press_key("backspace".to_owned()).await;
                            status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                        })>"Backspace"</button>
                        <button @click=$(async |_e| {
                            let outcome = press_key("tab".to_owned()).await;
                            status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                        })>"Tab"</button>
                        <button @click=$(async |_e| {
                            let outcome = press_key("space".to_owned()).await;
                            status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                        })>"Space"</button>
                    </div>

                    <p class="status">$(status.get())</p>
                </main>
            </body>
        </html>
    }
}

/// Sends the given text block into the focused window on the host.
///
/// Errors are returned as data (`Err(String)`) so the browser can show them.
#[procedure]
pub async fn send_text(cx: &Cx, text: String) -> Result<Result<String, String>> {
    if text.is_empty() {
        return Ok(Err("nothing to send".to_owned()));
    }

    let input: &Arc<dyn InputService> = app_context(cx);
    match input.send_text(&text) {
        Ok(()) => Ok(Ok("Sent text to host.".to_owned())),
        Err(error) => Ok(Err(error.to_string())),
    }
}

/// Presses one special key on the host.
#[procedure]
pub async fn press_key(cx: &Cx, name: String) -> Result<Result<String, String>> {
    let Some(key) = KeyName::from_name(&name) else {
        return Ok(Err(format!("unsupported key: {name}")));
    };

    let input: &Arc<dyn InputService> = app_context(cx);
    match input.press_key(key) {
        Ok(()) => Ok(Ok(format!("Sent {}.", key.label()))),
        Err(error) => Ok(Err(error.to_string())),
    }
}
