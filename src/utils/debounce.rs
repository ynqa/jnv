use std::time::Duration;

use tokio::{sync::mpsc, task::JoinHandle};

/// Initializes the debouncer input/output channels and task handle as a set.
///
/// Returns:
/// - `debounce_tx`: Input channel for values that should be debounced.
///   - Send a value here whenever the source value changes.
///   - If multiple values arrive within a short period, only the latest one is kept as a candidate.
/// - `last_rx`: Output channel for debounced values.
///   - On each `duration` tick, one value is emitted if a latest candidate exists.
///   - Consumers can process only the "settled" latest value by reading from this channel.
/// - `debouncer`: Join handle of the background task running the debounce loop.
pub fn setup_debouncer<T: Send + 'static>(
    duration: Duration,
) -> (mpsc::Sender<T>, mpsc::Receiver<T>, JoinHandle<()>) {
    let (last_tx, last_rx) = mpsc::channel(1);
    let (debounce_tx, debounce_rx) = mpsc::channel(1);
    let debouncer = spawn_debouncer(debounce_rx, last_tx, duration);
    (debounce_tx, last_rx, debouncer)
}

fn spawn_debouncer<T: Send + 'static>(
    mut debounce_rx: mpsc::Receiver<T>,
    last_tx: mpsc::Sender<T>,
    duration: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_query = None;
        let mut delay = tokio::time::interval(duration);
        loop {
            tokio::select! {
                maybe_query = debounce_rx.recv() => {
                    if let Some(query) = maybe_query {
                        last_query = Some(query);
                    } else {
                        break;
                    }
                },
                _ = delay.tick() => {
                    if let Some(text) = last_query.take() {
                        let _ = last_tx.send(text).await;
                    }
                },
            }
        }
    })
}
