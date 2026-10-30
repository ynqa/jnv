use std::collections::HashSet;

use promkit_widgets::{core::crossterm::event::Event, jsonstream, listbox, text_editor};
use serde::{Deserialize, Serialize};
use termcfg::crossterm_config::event_set_serde;
use tokio::time::Duration;

mod duration;
use duration::duration_serde;

#[derive(Serialize, Deserialize)]
pub struct EditorConfig {
    pub on_focus: text_editor::Config,
    pub on_defocus: text_editor::Config,
    /// vi-style modal editing settings. Optional so existing configuration
    /// files written before this feature continue to load.
    #[serde(default)]
    pub vi: ViConfig,
}

/// Settings for vi-style modal editing in the query editor.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViConfig {
    /// When `true`, the query editor starts in NORMAL mode and interprets keys
    /// as vi motions, operators, and edit commands. Press `i`/`a` to insert and
    /// <kbd>Esc</kbd> to return to NORMAL mode.
    pub enable: bool,
    /// Prefix shown while in NORMAL mode, so the current mode is visible. The
    /// configured `on_focus.prefix` is shown while in INSERT mode.
    pub normal_prefix: String,
}

impl Default for ViConfig {
    fn default() -> Self {
        Self {
            enable: false,
            normal_prefix: "❮❮ ".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct JsonConfig {
    pub max_streams: Option<usize>,
    pub stream: jsonstream::Config,
}

#[derive(Serialize, Deserialize)]
pub struct CompletionConfig {
    pub listbox: listbox::Config,
    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,
}

// TODO: remove Clone derive
#[derive(Clone, Serialize, Deserialize)]
pub struct Keybinds {
    #[serde(with = "event_set_serde")]
    pub exit: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub copy_query: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub copy_result: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub switch_mode: HashSet<Event>,
    pub on_editor: EditorKeybinds,
    pub on_json_viewer: JsonViewerKeybinds,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorKeybinds {
    #[serde(with = "event_set_serde")]
    pub backward: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub forward: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_head: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_tail: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_previous_nearest: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_next_nearest: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub erase: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub erase_all: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub erase_to_previous_nearest: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub erase_to_next_nearest: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub completion: HashSet<Event>,
    pub on_completion: CompletionKeybinds,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CompletionKeybinds {
    #[serde(with = "event_set_serde")]
    pub up: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub down: HashSet<Event>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct JsonViewerKeybinds {
    #[serde(with = "event_set_serde")]
    pub up: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub down: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_head: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub move_to_tail: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub toggle: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub expand: HashSet<Event>,
    #[serde(with = "event_set_serde")]
    pub collapse: HashSet<Event>,
}

#[derive(Serialize, Deserialize)]
pub struct ReactivityControl {
    #[serde(with = "duration_serde")]
    pub query_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    pub resize_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    pub spin_duration: Duration,
}

pub static DEFAULT_CONFIG: &str = include_str!("../default.toml");

/// Note that the config struct and the `.toml` configuration file are
/// managed separately because the current toml crate
/// does not readily support the following features:
///
/// - Preserve docstrings as comments in the `.toml` file
///   - https://github.com/toml-rs/toml/issues/376
/// - Output inline tables
///   - https://github.com/toml-rs/toml/issues/592
///
/// Also difficult to patch `Config` using only the items specified in the configuration file
/// (Premise: To address the complexity of configurations,
/// it assumes using a macro to avoid managing Option-wrapped structures on our side).s
///
/// The main challenge is that, for nested structs,
/// it is not able to wrap every leaf field with Option<>.
/// https://github.com/colin-kiegel/rust-derive-builder/issues/254
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub no_hint: bool,
    pub reactivity_control: ReactivityControl,
    pub editor: EditorConfig,
    pub json: JsonConfig,
    pub completion: CompletionConfig,
    pub keybinds: Keybinds,
}

impl Config {
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let config = Config::load_from(DEFAULT_CONFIG).expect("default config must parse");
        // vi editing ships disabled so existing keybindings are unchanged.
        assert!(!config.editor.vi.enable);
        assert_eq!(config.editor.vi.normal_prefix, "❮❮ ");
    }

    #[test]
    fn vi_section_is_optional_for_backward_compatibility() {
        // A config written before the vi feature has no `[editor.vi]` table; it
        // must still load, falling back to the vi defaults.
        let without_vi = r#"
no_hint = false

[reactivity_control]
query_debounce_duration = "600ms"
resize_debounce_duration = "200ms"
spin_duration = "300ms"

[editor.on_focus]
[editor.on_defocus]

[json]
[json.stream]

[completion]
search_result_chunk_size = 100
search_load_chunk_size = 50000
[completion.listbox]

[keybinds]
exit = ["Ctrl+C"]
copy_query = ["Ctrl+Q"]
copy_result = ["Ctrl+O"]
switch_mode = ["Shift+Down"]

[keybinds.on_editor]
backward = ["Left"]
forward = ["Right"]
move_to_head = ["Ctrl+A"]
move_to_tail = ["Ctrl+E"]
move_to_previous_nearest = ["Alt+B"]
move_to_next_nearest = ["Alt+F"]
erase = ["Backspace"]
erase_all = ["Ctrl+U"]
erase_to_previous_nearest = ["Ctrl+W"]
erase_to_next_nearest = ["Alt+D"]
completion = ["Tab"]
on_completion.up = ["Up"]
on_completion.down = ["Down"]

[keybinds.on_json_viewer]
up = ["Up"]
down = ["Down"]
move_to_head = ["Ctrl+L"]
move_to_tail = ["Ctrl+H"]
toggle = ["Enter"]
expand = ["Ctrl+P"]
collapse = ["Ctrl+N"]
"#;
        let config = Config::load_from(without_vi).expect("config without [editor.vi] must parse");
        assert!(!config.editor.vi.enable);
    }
}
