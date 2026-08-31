use std::sync::Arc;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    runtime::{Event, procedure},
    view::view,
};

use crate::browsers::{
    BrowserDef, BrowserService, OnboardingAction, browser_line, onboarding_action,
};
use crate::context::{ContextService, focus_line};
use crate::input::{InputError, InputService};
use crate::keys::{Key, KeyKind, REMOTE, TYPING};

/// Shareable host identity, registered as app context and rendered on the page.
pub struct HostInfo {
    pub hostname: String,
    pub url: String,
    /// This build's procedure-id hash; the page's connection-health script
    /// compares it against `/healthz` to notice a swapped binary.
    pub build: String,
}

/// The small hint under a key button, derived from the Key's kind: media
/// keys show "media key", letter keys show their wire name, typing keys
/// show nothing.
fn button_hint(key: Key) -> Option<&'static str> {
    match key.kind() {
        KeyKind::Media => Some("media key"),
        KeyKind::Letter => Some(key.wire_name()),
        KeyKind::Typing => None,
    }
}

#[page("/")]
pub async fn home(cx: &Cx) -> Result {
    // Runtime handlers repeat one canonical outcome line — `status.set(if
    // outcome.is_ok() { outcome.unwrap() } else { outcome.err().unwrap() })` —
    // because runtime expressions must compile to JS: no `match`, no helper
    // calls. The message content itself lives server-side, in the procedures.
    let info: &HostInfo = app_context(cx);
    let hostname = info.hostname.clone();
    let url = info.url.clone();
    let build = info.build.clone();
    let reconnecting = format!("reconnecting to {hostname}…");

    let context: &Arc<dyn ContextService> = app_context(cx);
    let focus = focus_line(context.focused_window());

    let browsers: &Arc<dyn BrowserService> = app_context(cx);
    let detected = browsers.detect();
    let action = onboarding_action(&detected);
    let browser = browser_line(detected);

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
                <meta name="beam-build" content=(build)>
                <title>"beam"</title>
                <link rel="icon" href="/icon-192.png">
                <link rel="manifest" href="/manifest.webmanifest">
                <link rel="apple-touch-icon" href="/icon-192.png">
                <link rel="stylesheet" href="/beam.css">
                topcoat::runtime::script()
                <script src="/beam.js" defer="defer"></script>
            </head>
            <body>
                <div class="banner" role="status">(reconnecting)</div>
                <header class="topbar">
                    <span class="host">
                        <b>(hostname)</b>
                        <span class="dot" title="connection"></span>
                    </span>
                    <span class="mono">(url)</span>
                </header>
                <p class="focus-line">
                    <b>"FOCUS"</b>
                    <span id="focus-text">(focus)</span>
                </p>
                <p class="focus-line">
                    <b>"BROWSER"</b>
                    <span id="browser-text">(browser)</span>
                </p>
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
                        <span class="tag">"02 — Remote"</span>
                        <div class="remote">
                            for def in REMOTE {
                                let wire = def.wire_name;
                                <button
                                    class=(if def.key == Key::F { Some("wide") } else { None })
                                    @click=$(async move |_e| {
                                        let outcome = press_key(wire.to_owned()).await;
                                        status.set(if outcome.is_ok() {
                                            outcome.unwrap()
                                        } else {
                                            outcome.err().unwrap()
                                        });
                                    })
                                >
                                    (def.label)
                                    match button_hint(def.key) {
                                        Some(hint) => <small>(hint)</small>,
                                        None => {}
                                    }
                                </button>
                            }
                        </div>
                    </section>
                    <section>
                        <span class="tag">"03 — Keys"</span>
                        <div class="keys">
                            for def in TYPING {
                                let wire = def.wire_name;
                                <button
                                    @click=$(async move |_e| {
                                        let outcome = press_key(wire.to_owned()).await;
                                        status.set(if outcome.is_ok() {
                                            outcome.unwrap()
                                        } else {
                                            outcome.err().unwrap()
                                        });
                                    })
                                >(def.label)</button>
                            }
                        </div>
                    </section>
                    <section>
                        <span class="tag">"04 — Open URL"</span>
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
                    <section>
                        <span class="tag">"05 — Browser"</span>
                        match action {
                            OnboardingAction::Start { process } => <button
                                class="browser-action"
                                @click=$(async move |_e| {
                                    let outcome = start_browser(process.to_owned()).await;
                                    status.set(if outcome.is_ok() {
                                        outcome.unwrap()
                                    } else {
                                        outcome.err().unwrap()
                                    });
                                })
                            >"Start with remote control"</button>,
                            OnboardingAction::Restart { process, warning } => <button
                                class="browser-action"
                                @click=$(async move |_e| {
                                    let outcome = restart_browser(process.to_owned()).await;
                                    status.set(if outcome.is_ok() {
                                        outcome.unwrap()
                                    } else {
                                        outcome.err().unwrap()
                                    });
                                })
                            >
                                "Restart with remote control"
                                <small>(warning)</small>
                            </button>,
                            OnboardingAction::Active => <p class="browser-note">"Remote control is active."</p>,
                            OnboardingAction::Unavailable => <p class="browser-note">"Browser control is unavailable."</p>,
                        }
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

/// Rejects a procedure call before it reaches the input backend; the
/// reason travels to the browser as data.
fn rejected(message: String) -> Result<Result<String, String>> {
    Ok(Err(message))
}

/// One home for the outcome rule: a backend call becomes either its success
/// message or the error string, both travelling to the browser as data
/// (`Result<String, String>`), never as exceptions.
fn backend_outcome(
    message: String,
    result: Result<(), InputError>,
) -> Result<Result<String, String>> {
    Ok(match result {
        Ok(()) => Ok(message),
        Err(error) => Err(error.to_string()),
    })
}

/// Sends the given text block into the focused window on the host.
///
/// Errors are returned as data (`Err(String)`) so the browser can show them.
#[procedure]
pub async fn send_text(cx: &Cx, text: String) -> Result<Result<String, String>> {
    if text.is_empty() {
        return rejected("nothing to send".to_owned());
    }

    let input: &Arc<dyn InputService> = app_context(cx);
    backend_outcome("Sent text to host.".to_owned(), input.send_text(&text))
}

/// Presses one special key on the host.
///
/// The key catalogue is the complete description of what devices can send;
/// anything else is rejected here.
#[procedure]
pub async fn press_key(cx: &Cx, name: String) -> Result<Result<String, String>> {
    let Some(key) = Key::from_name(&name) else {
        return rejected(format!("unsupported key: {name}"));
    };

    let input: &Arc<dyn InputService> = app_context(cx);
    backend_outcome(format!("Sent {}.", key.label()), input.press_key(key))
}

/// Opens an http(s) URL in the host's default browser.
///
/// The scheme allow-list runs before the service call, so a tampered or
/// spoofed procedure call can never launch anything but a web link.
#[procedure]
pub async fn open_url(cx: &Cx, raw: String) -> Result<Result<String, String>> {
    let url = match validate_open_url(&raw) {
        Ok(url) => url,
        Err(message) => return rejected(message),
    };

    let input: &Arc<dyn InputService> = app_context(cx);
    backend_outcome(format!("Opened {url} on the host."), input.open_url(&url))
}

/// Cold-starts a known browser with beam's remote-control port.
///
/// The catalogue is the complete description of what can be started;
/// anything else is rejected here. The button is the consent, and a stale
/// click gets an honest refusal instead of a surprise launch.
#[procedure]
pub async fn start_browser(cx: &Cx, name: String) -> Result<Result<String, String>> {
    let Some(def) = BrowserDef::by_process(&name) else {
        return rejected(format!("unsupported browser: {name}"));
    };

    let browsers: &Arc<dyn BrowserService> = app_context(cx);
    Ok(match browsers.start(def) {
        Ok(info) => Ok(info.display()),
        Err(error) => Err(error.to_string()),
    })
}

/// Restarts a known browser with beam's remote-control port: graceful
/// close, force-stop of leftovers, cold launch, verified endpoint.
///
/// Same catalogue rule as [`start_browser`]; the server-side state check
/// makes a stale restart button safe (no surprise close of a working
/// remote-control browser).
#[procedure]
pub async fn restart_browser(cx: &Cx, name: String) -> Result<Result<String, String>> {
    let Some(def) = BrowserDef::by_process(&name) else {
        return rejected(format!("unsupported browser: {name}"));
    };

    let browsers: &Arc<dyn BrowserService> = app_context(cx);
    Ok(match browsers.restart(def) {
        Ok(info) => Ok(info.display()),
        Err(error) => Err(error.to_string()),
    })
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
    fn backend_results_become_outcome_data() {
        assert_eq!(
            backend_outcome("done".to_owned(), Ok(())).unwrap(),
            Ok("done".to_owned())
        );
        let failure = InputError::Inject("boom".to_owned());
        assert_eq!(
            backend_outcome("done".to_owned(), Err(failure)).unwrap(),
            Err("could not inject input into the host OS: boom".to_owned())
        );
    }

    #[test]
    fn button_hints_derive_from_kind() {
        assert_eq!(button_hint(Key::VolumeUp), Some("media key"));
        assert_eq!(button_hint(Key::MediaPlayPause), Some("media key"));
        assert_eq!(button_hint(Key::J), Some("j"));
        assert_eq!(button_hint(Key::F), Some("f"));
        assert_eq!(button_hint(Key::Enter), None);
        assert_eq!(button_hint(Key::Tab), None);
    }

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
