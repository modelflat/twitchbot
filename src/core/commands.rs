use std::sync::Arc;

use futures::channel::mpsc::{Receiver, Sender};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};

use super::model::*;

/// An event loop for executing commands.
pub(crate) async fn event_loop(
    rx_command: Receiver<Command>,
    tx_message: Sender<PreparedMessage>,
    concurrency: usize,
) {
    let tx_message = Arc::new(Mutex::new(tx_message));
    let tx_message_factory = || tx_message.clone();

    rx_command
        .for_each_concurrent(concurrency, async move |command| {
            tx_message_factory()
                .lock()
                .await
                .send(PreparedMessage {
                    channel: command.channel,
                    message: "123 test".to_string(),
                })
                .await
                .expect("Failed to submit message to message queue");
        })
        .await;
}
