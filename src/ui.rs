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
        signal url_text = String::new();
        signal status = "Ready.".to_owned();

        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="theme-color" content="#fffdf5">
                <title>"beam"</title>
                <link rel="icon" href="/icon-192.png">
                <link rel="manifest" href="/manifest.webmanifest">
                <link rel="apple-touch-icon" href="/icon-192.png">
                <link rel="stylesheet" href="/beam.css">
                topcoat::runtime::script()
            </head>
            <body>
                <header class="topbar">
                    <b>"BEAM / REMOTE INPUT"</b>
                    <span class="mono">(url)</span>
                </header>
                <div class="blocks">
                    <section class="b-text">
                        <span class="tag">"01 — Text"</span>
                        <textarea
                            placeholder="Type or paste text…"
                            :value=$(text.get())
                            @input=$(|e: Event| text.set(e.target.value))
                        ></textarea>
                        <button
                            class="send"
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
                        >"Send text"</button>
                    </section>
                    <section>
                        <span class="tag">"02 — Keys"</span>
                        <div class="keys">
                            <button @click=$(async |_e| {
                                let outcome = press_key("enter".to_owned()).await;
                                status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                            })>"Enter"</button>
                            <button @click=$(async |_e| {
                                let outcome = press_key("space".to_owned()).await;
                                status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                            })>"Space"</button>
                            <button @click=$(async |_e| {
                                let outcome = press_key("backspace".to_owned()).await;
                                status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                            })>"Bksp"</button>
                            <button @click=$(async |_e| {
                                let outcome = press_key("tab".to_owned()).await;
                                status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                            })>"Tab"</button>
                        </div>
                    </section>
                    <section>
                        <span class="tag">"03 — Open URL"</span>
                        <div class="urlrow">
                            <input
                                type="url"
                                placeholder="https://…"
                                :value=$(url_text.get())
                                @input=$(|e: Event| url_text.set(e.target.value))
                            >
                            <button @click=$(async |_e| {
                                let outcome = open_url(url_text.get()).await;
                                if outcome.is_ok() {
                                    url_text.set("".to_owned());
                                }
                                status.set(if outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() });
                            })>"Open"</button>
                        </div>
                    </section>
                    <section class="status">
                        <span>"STATUS"</span>
                        <span>$(status.get())</span>
                    </section>
                </div>
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

/// Opens an http(s) URL in the host's default browser.
///
/// The scheme allow-list runs before the service call, so a tampered or
/// spoofed procedure call can never launch anything but a web link.
#[procedure]
pub async fn open_url(cx: &Cx, raw: String) -> Result<Result<String, String>> {
    let url = match validate_open_url(&raw) {
        Ok(url) => url,
        Err(message) => return Ok(Err(message)),
    };

    let input: &Arc<dyn InputService> = app_context(cx);
    match input.open_url(&url) {
        Ok(()) => Ok(Ok(format!("Opened {url} on the host."))),
        Err(error) => Ok(Err(error.to_string())),
    }
}

/// Only http/https may be opened, with no embedded whitespace or control
/// characters (defense in depth against launcher argument games).
fn validate_open_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("no URL to open".to_owned());
    }

    let lowered = url.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return Err("only http and https URLs can be opened".to_owned());
    }

    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL contains invalid characters".to_owned());
    }

    Ok(url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_accepts_trimmed_http_and_https() {
        assert_eq!(
            validate_open_url("  https://example.com "),
            Ok("https://example.com".to_owned())
        );
        assert_eq!(
            validate_open_url("HTTP://example.com"),
            Ok("HTTP://example.com".to_owned())
        );
    }

    #[test]
    fn open_url_rejects_everything_but_web_links() {
        for raw in [
            "",
            "   ",
            "example.com",
            "ftp://example.com/file",
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "https://example.com/a b",
            "http://example.com/\nfile://x",
        ] {
            assert!(validate_open_url(raw).is_err(), "must reject {raw:?}");
        }
    }
}
