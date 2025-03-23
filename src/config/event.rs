use std::collections::HashSet;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use serde::{Deserialize, Serialize};

pub trait Matcher<T> {
    fn matches(&self, other: &T) -> bool;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EventDefSet(HashSet<EventDef>);

impl Matcher<Event> for EventDefSet {
    fn matches(&self, other: &Event) -> bool {
        self.0.iter().any(|event_def| event_def.matches(other))
    }
}

impl From<KeyEventDef> for EventDefSet {
    fn from(key_event_def: KeyEventDef) -> Self {
        EventDefSet(HashSet::from_iter([EventDef::Key(key_event_def)]))
    }
}

impl From<MouseEventDef> for EventDefSet {
    fn from(mouse_event_def: MouseEventDef) -> Self {
        EventDefSet(HashSet::from_iter([EventDef::Mouse(mouse_event_def)]))
    }
}

/// A part of `crossterm::event::Event`.
/// It is used for parsing from a config file or
/// for comparison with crossterm::event::Event.
/// https://docs.rs/crossterm/0.28.1/crossterm/event/enum.Event.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventDef {
    Key(KeyEventDef),
    Mouse(MouseEventDef),
}

impl Matcher<Event> for EventDef {
    fn matches(&self, other: &Event) -> bool {
        match (self, other) {
            (EventDef::Key(key_def), Event::Key(key_event)) => key_def.matches(key_event),
            (EventDef::Mouse(mouse_def), Event::Mouse(mouse_event)) => {
                mouse_def.matches(mouse_event)
            }
            _ => false,
        }
    }
}

/// A part of `crossterm::event::KeyEvent`.
/// It is used for parsing from a config file or
/// for comparison with crossterm::event::KeyEvent.
/// https://docs.rs/crossterm/0.28.1/crossterm/event/struct.KeyEvent.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct KeyEventDef {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyEventDef {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        KeyEventDef { code, modifiers }
    }
}

impl Matcher<KeyEvent> for KeyEventDef {
    fn matches(&self, other: &KeyEvent) -> bool {
        self.code == other.code && self.modifiers == other.modifiers
    }
}

/// A part of `crossterm::event::MouseEvent`.
/// It is used for parsing from a config file or
/// for comparison with crossterm::event::MouseEvent.
/// https://docs.rs/crossterm/0.28.1/crossterm/event/struct.MouseEvent.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MouseEventDef {
    kind: MouseEventKind,
    modifiers: KeyModifiers,
}

impl MouseEventDef {
    pub fn new(kind: MouseEventKind, modifiers: KeyModifiers) -> Self {
        MouseEventDef { kind, modifiers }
    }
}

impl Matcher<MouseEvent> for MouseEventDef {
    fn matches(&self, other: &MouseEvent) -> bool {
        self.kind == other.kind && self.modifiers == other.modifiers
    }
}
