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
