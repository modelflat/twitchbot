use log::*;

use std::sync::Arc;

use futures::channel::mpsc::{Receiver, Sender};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};

use super::bot::{CommandRegistry, ExecutionOutcome, RawCommand, ShareableBotState};
use super::model::PreparedMessage;

/// An event loop for executing commands.
pub(crate) async fn event_loop<T: 'static + std::marker::Send + std::marker::Sync>(
    rx_command: Receiver<RawCommand>,
    tx_message: Sender<PreparedMessage>,
    concurrency: usize,
    command_registry: Arc<CommandRegistry<T>>,
    state: Arc<ShareableBotState<T>>,
) {
    let tx_message = Arc::new(Mutex::new(tx_message));
    let get_tx_message = || tx_message.clone();
    let get_command_registry = || command_registry.clone();
    let get_state = || state.clone();

    rx_command
        .for_each_concurrent(concurrency, async move |command| {
            let outcome = get_command_registry()
                .execute(command.clone(), &*get_state())
                .await;

            match outcome {
                ExecutionOutcome::Success(message) => {
                    get_tx_message()
                        .lock()
                        .await
                        .send(message)
                        .await
                        .expect("Failed to submit message to message queue");
                }
                ExecutionOutcome::SilentSuccess => {
                    info!("Successfully executed command: {:?}", command);
                }
                ExecutionOutcome::Error(error) => {
                    error!(
                        "Error executing command: {:?} / command = {:?}",
                        error, command
                    );
                }
            }
        })
        .await;
}
