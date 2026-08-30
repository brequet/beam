# beam

A single-binary local-network input bridge: devices on the Wi-Fi open a page,
and beam injects what they send as keyboard input into the focused window on
the host. No auth, no database, by design.

## Language

**Key**:
A single discrete keypress a device can send to the host, chosen from the
fixed set beam supports — media keys, letter keys, and typing keys alike.
_Avoid_: Special key, hotkey, quick key

**Media key**:
A Key the host OS applies globally (play/pause, volume) regardless of which
window has focus. Letter keys are the opposite: they land in the focused
window only.
_Avoid_: Global key

**Wire name**:
The exact string a device sends to name a Key. One Key, one wire name — no
synonyms; the catalogue is the complete description of what devices can send.
_Avoid_: Alias, key code, key name

**Key catalogue**:
The fixed, exhaustive description of every Key beam supports: its Wire name,
label, kind, and the Pad it renders on, in display order. What devices may
send, what the page renders, and what `press_key` accepts are all read from
it — no second list exists.
_Avoid_: Key list, key set, button set

**Pad**:
A named block of Keys the page renders together, in catalogue order — the
remote pad and the typing pad today. The catalogue owns membership and
order; the page owns layout only.
_Avoid_: Section, key group, view
