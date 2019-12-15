use std::sync::Arc;

use async_std::sync::RwLock;

use futures::channel::mpsc::{Receiver, Sender};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};

use super::model::*;
use super::bot::BotState;
use crate::core::bot::{CommandRegistry, ExecutionOutcome, CommandInstance};

/// An event loop for executing commands.
pub(crate) async fn event_loop(
    rx_command: Receiver<CommandInstance>,
    tx_message: Sender<PreparedMessage>,
    concurrency: usize,
    command_registry: Arc<CommandRegistry>,
    bot_state: Arc<RwLock<BotState>>,
) {
    let tx_message = Arc::new(Mutex::new(tx_message));
    let tx_message_factory = || tx_message.clone();
    let bot_state_factory = || bot_state.clone();
    let command_registry_factory = || command_registry.clone();

    rx_command
        .for_each_concurrent(concurrency, async move |command| {
            let outcome = {
                command_registry_factory().execute(command.clone(), &*bot_state_factory()).await
            };

            match outcome {
                ExecutionOutcome::Success(message) => {
                    tx_message_factory()
                        .lock()
                        .await
                        .send(message)
                        .await
                        .expect("Failed to submit message to message queue");
                },
                ExecutionOutcome::SilentSuccess => {
                    info!("Successfully executed command: {:?}", command);
                },
                ExecutionOutcome::Error(error) => {
                    error!("Error executing command: {:?} / command = {:?}", error, command);
                }
            }
        })
        .await;
}
