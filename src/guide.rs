use arboard::Clipboard;
use promkit_widgets::{
    core::{
        crossterm::style::{Color, ContentStyle},
        render::SharedRenderer,
        Widget,
    },
    text::{self, Text},
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::context::Index;

/// Represent a message to be shown in the guide.
/// This is used to decouple the logic of generating messages from the logic of rendering them.
pub enum GuideMessage {
    CopiedToClipboard,
    FailedToCopyToClipboard(String),
    FailedToSetupClipboard(String),
    FailedToCopyWhileRenderingInProgress,
    FailedToSwitchModeWhileRenderingInProgress,
    LoadedAllSuggestions(usize),
    LoadedPartiallySuggestions(usize),
    NoSuggestionFound(String),
    JqReturnedNull(String),
    JqFailed(String),
}

/// Represent an action to be performed on the guide.
pub enum GuideAction {
    Clear,
    Show(GuideMessage),
}

fn message_to_state(message: GuideMessage) -> text::State {
    let (message, color) = match message {
        GuideMessage::CopiedToClipboard => ("Copied to clipboard".to_owned(), Color::Green),
        GuideMessage::FailedToCopyToClipboard(e) => {
            (format!("Failed to copy to clipboard: {e}"), Color::Red)
        }
        GuideMessage::FailedToSetupClipboard(e) => {
            (format!("Failed to setup clipboard: {e}"), Color::Red)
        }
        GuideMessage::FailedToCopyWhileRenderingInProgress => (
            "Failed to copy while rendering is in progress.".to_owned(),
            Color::Yellow,
        ),
        GuideMessage::FailedToSwitchModeWhileRenderingInProgress => (
            "Failed to switch mode while rendering is in progress.".to_owned(),
            Color::Yellow,
        ),
        GuideMessage::LoadedAllSuggestions(count) => {
            (format!("Loaded all ({count}) suggestions"), Color::Green)
        }
        GuideMessage::LoadedPartiallySuggestions(count) => (
            format!("Loaded partially ({count}) suggestions"),
            Color::Green,
        ),
        GuideMessage::NoSuggestionFound(prefix) => {
            (format!("No suggestion found for '{prefix}'"), Color::Yellow)
        }
        GuideMessage::JqReturnedNull(input) => (
            format!("jq returned 'null', which may indicate a typo or incorrect filter: `{input}`"),
            Color::Yellow,
        ),
        GuideMessage::JqFailed(e) => (format!("jq failed: `{e}`"), Color::Red),
    };
    text::State {
        text: Text::from(message),
        config: text::Config {
            style: Some(ContentStyle {
                foreground_color: Some(color),
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

/// Copy the given content to the clipboard and return a message indicating the result.
pub fn copy_to_clipboard_message(content: &str) -> GuideMessage {
    match Clipboard::new() {
        Ok(mut clipboard) => match clipboard.set_text(content) {
            Ok(_) => GuideMessage::CopiedToClipboard,
            Err(e) => GuideMessage::FailedToCopyToClipboard(e.to_string()),
        },
        Err(e) => GuideMessage::FailedToSetupClipboard(e.to_string()),
    }
}

/// Spawn a task that listens for guide actions and updates the guide view accordingly.
pub fn start_guide_task(
    mut action_rx: mpsc::Receiver<GuideAction>,
    shared_renderer: SharedRenderer<Index>,
    no_hint: bool,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let view = if no_hint {
                        Default::default()
                    } else {
                        match action {
                            GuideAction::Clear => text::State::default().create_graphemes(),
                            GuideAction::Show(message) => message_to_state(message).create_graphemes(),
                        }
                    };
                    shared_renderer.update([(Index::Guide, view)]).render().await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
