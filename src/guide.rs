use arboard::Clipboard;
use promkit_widgets::{
    core::{render::SharedRenderer, Widget},
    status::{self, Severity},
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::context::{Index, SharedContext};

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

fn message_to_state(message: GuideMessage) -> status::State {
    match message {
        GuideMessage::CopiedToClipboard => {
            status::State::new("Copied to clipboard", Severity::Success)
        }
        GuideMessage::FailedToCopyToClipboard(e) => {
            status::State::new(format!("Failed to copy to clipboard: {e}"), Severity::Error)
        }
        GuideMessage::FailedToSetupClipboard(e) => {
            status::State::new(format!("Failed to setup clipboard: {e}"), Severity::Error)
        }
        GuideMessage::FailedToCopyWhileRenderingInProgress => status::State::new(
            "Failed to copy while rendering is in progress.",
            Severity::Warning,
        ),
        GuideMessage::FailedToSwitchModeWhileRenderingInProgress => status::State::new(
            "Failed to switch mode while rendering is in progress.",
            Severity::Warning,
        ),
        GuideMessage::LoadedAllSuggestions(count) => status::State::new(
            format!("Loaded all ({count}) suggestions"),
            Severity::Success,
        ),
        GuideMessage::LoadedPartiallySuggestions(count) => status::State::new(
            format!("Loaded partially ({count}) suggestions"),
            Severity::Success,
        ),
        GuideMessage::NoSuggestionFound(prefix) => status::State::new(
            format!("No suggestion found for '{prefix}'"),
            Severity::Warning,
        ),
        GuideMessage::JqReturnedNull(input) => status::State::new(
            format!("jq returned 'null', which may indicate a typo or incorrect filter: `{input}`"),
            Severity::Warning,
        ),
        GuideMessage::JqFailed(e) => {
            status::State::new(format!("jq failed: `{e}`"), Severity::Error)
        }
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

/// A guide state that occupies exactly one (blank) row.
///
/// Used to reserve the guide line when there is no message to show, so the
/// pane never collapses to zero rows and the panes below it stay put.
fn blank_line() -> status::State {
    status::State::new(" ", Severity::Success)
}

/// Graphemes for the guide pane's initial state.
///
/// With hints enabled this reserves one blank row so the guide line is present
/// from the first frame — otherwise the first message would shift the panes
/// below it. With `--no-hint` the guide is permanently empty.
pub fn initial_graphemes(
    no_hint: bool,
    width: u16,
    height: u16,
) -> promkit_widgets::core::grapheme::StyledGraphemes {
    if no_hint {
        Default::default()
    } else {
        blank_line().create_graphemes(width, height)
    }
}

/// Spawn a task that listens for guide actions and updates the guide view accordingly.
pub fn start_guide_task(
    mut action_rx: mpsc::Receiver<GuideAction>,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    no_hint: bool,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let area = shared_ctx.area().await;
                    let view = if no_hint {
                        Default::default()
                    } else {
                        match action {
                            // Render a single blank line rather than an empty
                            // state. An empty state produces zero rows, which the
                            // renderer drops entirely (terminal.rs filters out
                            // empty panes), collapsing the guide line and shifting
                            // the JSON viewer up — and back down when a message
                            // reappears. Reserving one row keeps the layout stable.
                            GuideAction::Clear => blank_line().create_graphemes(area.0, area.1),
                            GuideAction::Show(message) => message_to_state(message).create_graphemes(area.0, area.1),
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
