use std::sync::Mutex;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;

/// A special key the UI can send as a discrete keypress.
///
/// Kept separate from `enigo::Key` so the domain layer stays independent
/// of the OS input backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyName {
    Enter,
    Backspace,
    Tab,
    Space,
}

impl KeyName {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "enter" | "return" => Some(Self::Enter),
            "backspace" => Some(Self::Backspace),
            "tab" => Some(Self::Tab),
            "space" => Some(Self::Space),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Enter => "Enter",
            Self::Backspace => "Backspace",
            Self::Tab => "Tab",
            Self::Space => "Space",
        }
    }

    fn to_enigo(self) -> Key {
        match self {
            Self::Enter => Key::Return,
            Self::Backspace => Key::Backspace,
            Self::Tab => Key::Tab,
            Self::Space => Key::Space,
        }
    }
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("could not initialize the OS input backend: {0}")]
    Init(String),
    #[error("could not inject input into the host OS: {0}")]
    Inject(String),
}

/// Abstraction over host-side input generation.
///
/// The web layer only knows this trait, so development (`--mock`) and tests
/// never trigger real OS keystrokes.
pub trait InputService: Send + Sync {
    /// Types a block of text into the focused window.
    fn send_text(&self, text: &str) -> Result<(), InputError>;

    /// Presses a single special key.
    fn press_key(&self, key: KeyName) -> Result<(), InputError>;
}

/// Real backend backed by [`enigo`], injecting into the focused window.
pub struct OsInput {
    enigo: Mutex<Enigo>,
}

impl OsInput {
    pub fn new() -> Result<Self, InputError> {
        let enigo =
            Enigo::new(&Settings::default()).map_err(|error| InputError::Init(error.to_string()))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }
}

impl InputService for OsInput {
    fn send_text(&self, text: &str) -> Result<(), InputError> {
        self.enigo
            .lock()
            .expect("input backend mutex poisoned")
            .text(text)
            .map_err(|error| InputError::Inject(error.to_string()))
    }

    fn press_key(&self, key: KeyName) -> Result<(), InputError> {
        self.enigo
            .lock()
            .expect("input backend mutex poisoned")
            .key(key.to_enigo(), Direction::Click)
            .map_err(|error| InputError::Inject(error.to_string()))
    }
}

/// Dev/test backend that records events instead of touching the host OS.
#[derive(Default)]
pub struct MockInput {
    pub events: Mutex<Vec<String>>,
}

impl MockInput {
    fn record(&self, event: String) {
        println!("[mock input] {event}");
        self.events
            .lock()
            .expect("mock input mutex poisoned")
            .push(event);
    }
}

impl InputService for MockInput {
    fn send_text(&self, text: &str) -> Result<(), InputError> {
        self.record(format!("text {text:?}"));
        Ok(())
    }

    fn press_key(&self, key: KeyName) -> Result<(), InputError> {
        self.record(format!("key {}", key.label()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_map_exactly() {
        assert_eq!(KeyName::from_name("enter"), Some(KeyName::Enter));
        assert_eq!(KeyName::from_name("return"), Some(KeyName::Enter));
        assert_eq!(KeyName::from_name("Backspace"), Some(KeyName::Backspace));
        assert_eq!(KeyName::from_name("tab"), Some(KeyName::Tab));
        assert_eq!(KeyName::from_name("space"), Some(KeyName::Space));
        assert_eq!(KeyName::from_name("f13"), None);
        assert_eq!(KeyName::from_name(""), None);
    }

    #[test]
    fn mock_records_events_without_touching_the_os() {
        let input = MockInput::default();
        input.press_key(KeyName::Enter).unwrap();
        input.send_text("hello").unwrap();
        assert_eq!(
            *input.events.lock().unwrap(),
            vec!["key Enter".to_owned(), "text \"hello\"".to_owned()]
        );
    }
}
