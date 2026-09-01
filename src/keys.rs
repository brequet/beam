//! The key catalogue: every [`Key`] a device can send to the host, with its
//! wire name, display label, and kind, organized into the pads the page
//! renders. This module is the single source of truth for what the page
//! renders and what `press_key` accepts; the OS injection mapping stays with
//! the input backend in `input.rs`.

/// A single discrete keypress a device can send to the host, chosen from
/// the fixed set zappette supports — media keys, letter keys, and typing keys
/// alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Backspace,
    Tab,
    Space,
    Escape,
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

/// One catalogue entry: a [`Key`] and the facts the rest of zappette reads.
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

/// The Keys on the remote pad, in display order. The catalogue owns
/// membership and order; the page owns layout.
pub const REMOTE: &[KeyDef] = &[
    KeyDef {
        key: Key::J,
        wire_name: "j",
        label: "−10s",
        kind: KeyKind::Letter,
    },
    KeyDef {
        key: Key::MediaPlayPause,
        wire_name: "media-play-pause",
        label: "Play/Pause",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::L,
        wire_name: "l",
        label: "+10s",
        kind: KeyKind::Letter,
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
        key: Key::VolumeUp,
        wire_name: "volume-up",
        label: "Vol +",
        kind: KeyKind::Media,
    },
    KeyDef {
        key: Key::F,
        wire_name: "f",
        label: "Fullscreen",
        kind: KeyKind::Letter,
    },
];

/// The Keys on the typing pad, in display order.
pub const TYPING: &[KeyDef] = &[
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
    KeyDef {
        key: Key::Escape,
        wire_name: "esc",
        label: "Esc",
        kind: KeyKind::Typing,
    },
];

/// Every pad, so lookups and tests can iterate the whole catalogue.
const PADS: &[&[KeyDef]] = &[REMOTE, TYPING];

impl Key {
    /// Parses a wire name into its Key.
    ///
    /// Matching is case-insensitive but otherwise exact: the catalogue is
    /// the complete description of what devices can send.
    pub fn from_name(name: &str) -> Option<Self> {
        let lowered = name.to_ascii_lowercase();
        PADS.iter()
            .flat_map(|pad| pad.iter())
            .find(|def| def.wire_name == lowered)
            .map(|def| def.key)
    }

    fn def(self) -> &'static KeyDef {
        PADS.iter()
            .flat_map(|pad| pad.iter())
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

    /// The tying test: every Key variant must appear on exactly one pad.
    /// Adding a variant without a pad entry fails here.
    #[test]
    fn every_key_appears_on_exactly_one_pad() {
        let all = [
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::Space,
            Key::Escape,
            Key::F,
            Key::J,
            Key::L,
            Key::MediaPlayPause,
            Key::VolumeUp,
            Key::VolumeDown,
            Key::VolumeMute,
        ];
        for key in all {
            let entries = PADS
                .iter()
                .flat_map(|pad| pad.iter())
                .filter(|def| def.key == key)
                .count();
            assert_eq!(entries, 1, "key {:?} must appear on exactly one pad", key);
        }
        assert_eq!(
            PADS.iter().map(|pad| pad.len()).sum::<usize>(),
            all.len(),
            "no extra catalogue entries"
        );
    }

    #[test]
    fn wire_names_are_unique_and_non_empty() {
        let names: Vec<&str> = PADS
            .iter()
            .flat_map(|pad| pad.iter())
            .map(|def| def.wire_name)
            .collect();
        for (i, name) in names.iter().enumerate() {
            assert!(!name.is_empty());
            assert!(
                !names[i + 1..].contains(name),
                "duplicate wire name: {name}"
            );
        }
    }

    #[test]
    fn wire_names_round_trip_through_from_name() {
        for def in PADS.iter().flat_map(|pad| pad.iter()) {
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
        for rejected in ["", "f13", "ctrl", "return", "escape", "volume_up"] {
            assert_eq!(Key::from_name(rejected), None, "must reject {rejected:?}");
        }
    }
}
