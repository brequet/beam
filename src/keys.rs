//! The key catalogue: every [`Key`] a device can send to the host, with its
//! wire name, display label, and kind. This module is the single source of
//! truth for what the page renders and what `press_key` accepts; the OS
//! injection mapping stays with the input backend in `input.rs`.

/// A single discrete keypress a device can send to the host, chosen from
/// the fixed set beam supports — media keys, letter keys, and typing keys
/// alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Backspace,
    Tab,
    Space,
    F,
    J,
    L,
    MediaPlayPause,
    VolumeUp,
    VolumeDown,
    VolumeMute,
}

/// How a [`Key`] reaches the host.
///
/// Media keys apply globally regardless of which window has focus; letter
/// keys land in the focused window (app shortcuts); typing keys are
/// ordinary text-entry keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Media,
    Letter,
    Typing,
}

/// One catalogue entry: a [`Key`] and the facts the rest of beam reads.
#[derive(Debug)]
pub struct KeyDef {
    pub key: Key,
    /// The exact string a device sends to name this Key. One Key, one wire
    /// name — no synonyms.
    pub wire_name: &'static str,
    /// Display label, shown on the button and in the status line.
    pub label: &'static str,
    pub kind: KeyKind,
}

/// The key catalogue. One entry per Key — enforced by the tests below.
pub const CATALOGUE: &[KeyDef] = &[
    KeyDef {
        key: Key::MediaPlayPause,
        wire_name: "media-play-pause",
        label: "Play/Pause",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::VolumeUp,
        wire_name: "volume-up",
        label: "Vol +",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::VolumeDown,
        wire_name: "volume-down",
        label: "Vol −",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::VolumeMute,
        wire_name: "volume-mute",
        label: "Mute",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::F,
        wire_name: "f",
        label: "Fullscreen",
        kind: KeyKind::Letter,
    },
    KeyDef {
        key: Key::J,
        wire_name: "j",
        label: "−10s",
        kind: KeyKind::Letter,
    },
    KeyDef {
        key: Key::L,
        wire_name: "l",
        label: "+10s",
        kind: KeyKind::Letter,
    },
    KeyDef {
        key: Key::Enter,
        wire_name: "enter",
        label: "Enter",
        kind: KeyKind::Typing,
    },
    KeyDef {
        key: Key::Space,
        wire_name: "space",
        label: "Space",
        kind: KeyKind::Typing,
    },
    KeyDef {
        key: Key::Backspace,
        wire_name: "backspace",
        label: "Bksp",
        kind: KeyKind::Typing,
    },
    KeyDef {
        key: Key::Tab,
        wire_name: "tab",
        label: "Tab",
        kind: KeyKind::Typing,
    },
];

impl Key {
    /// Parses a wire name into its Key.
    ///
    /// Matching is case-insensitive but otherwise exact: the catalogue is
    /// the complete description of what devices can send.
    pub fn from_name(name: &str) -> Option<Self> {
        let lowered = name.to_ascii_lowercase();
        CATALOGUE
            .iter()
            .find(|def| def.wire_name == lowered)
            .map(|def| def.key)
    }

    fn def(self) -> &'static KeyDef {
        CATALOGUE
            .iter()
            .find(|def| def.key == self)
            .expect("every Key has a catalogue entry")
    }

    pub fn wire_name(self) -> &'static str {
        self.def().wire_name
    }

    pub fn label(self) -> &'static str {
        self.def().label
    }

    pub fn kind(self) -> KeyKind {
        self.def().kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tying test: every Key variant must appear in the catalogue
    /// exactly once. Adding a variant without a catalogue entry fails here.
    #[test]
    fn every_key_appears_in_the_catalogue_exactly_once() {
        let all = [
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::Space,
            Key::F,
            Key::J,
            Key::L,
            Key::MediaPlayPause,
            Key::VolumeUp,
            Key::VolumeDown,
            Key::VolumeMute,
        ];
        for key in all {
            let entries = CATALOGUE.iter().filter(|def| def.key == key).count();
            assert_eq!(entries, 1, "key {:?} must have exactly one entry", key);
        }
        assert_eq!(CATALOGUE.len(), all.len(), "no extra catalogue entries");
    }

    #[test]
    fn wire_names_are_unique_and_non_empty() {
        for (i, def) in CATALOGUE.iter().enumerate() {
            assert!(!def.wire_name.is_empty());
            assert_eq!(
                CATALOGUE[i + 1..]
                    .iter()
                    .find(|other| other.wire_name == def.wire_name)
                    .map(|dup| dup.wire_name),
                None,
                "duplicate wire name: {}",
                def.wire_name
            );
        }
    }

    #[test]
    fn wire_names_round_trip_through_from_name() {
        for def in CATALOGUE {
            assert_eq!(Key::from_name(def.wire_name), Some(def.key));
            assert_eq!(def.key.wire_name(), def.wire_name);
            assert_eq!(def.key.label(), def.label);
        }
    }

    #[test]
    fn from_name_is_case_insensitive_but_has_no_synonyms() {
        assert_eq!(Key::from_name("ENTER"), Some(Key::Enter));
        assert_eq!(
            Key::from_name("Media-Play-Pause"),
            Some(Key::MediaPlayPause)
        );
        for rejected in ["", "f13", "ctrl", "return", "esc", "volume_up"] {
            assert_eq!(Key::from_name(rejected), None, "must reject {rejected:?}");
        }
    }
}
