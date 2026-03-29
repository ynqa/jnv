use arboard::Clipboard;
use promkit_widgets::{
    core::{render::SharedRenderer, Widget},
    status::{self, Severity},
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::context::{Index, SharedContext};

pub enum GuideMessage {
    CopiedToClipboard,
    FailedToCopyToClipboard(String),
    FailedToSetupClipboard(String),
    FailedToCopyWhileRenderingInProgress,
    FailedToSwitchPaneWhileRenderingInProgress,
    LoadedAllSuggestions(usize),
    LoadedPartiallySuggestions(usize),
    NoSuggestionFound(String),
    JqReturnedNull(String),
    JqFailed(String),
}

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
        GuideMessage::FailedToSwitchPaneWhileRenderingInProgress => status::State::new(
            "Failed to switch pane while rendering is in progress.",
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

pub fn copy_to_clipboard_message(content: &str) -> GuideMessage {
    match Clipboard::new() {
        Ok(mut clipboard) => match clipboard.set_text(content) {
            Ok(_) => GuideMessage::CopiedToClipboard,
            Err(e) => GuideMessage::FailedToCopyToClipboard(e.to_string()),
        },
        Err(e) => GuideMessage::FailedToSetupClipboard(e.to_string()),
    }
}

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
                    let pane = if no_hint {
                        Default::default()
                    } else {
                        match action {
                            GuideAction::Clear => status::State::default().create_graphemes(area.0, area.1),
                            GuideAction::Show(message) => message_to_state(message).create_graphemes(area.0, area.1),
                        }
                    };
                    shared_renderer.update([(Index::Guide, pane)]).render().await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
