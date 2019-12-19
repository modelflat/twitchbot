use futures::channel::mpsc::{Receiver, Sender};
use futures::{SinkExt, StreamExt};

use tungstenite::Message;

use crate::irc;
use crate::util::modify_message;

use super::cooldown::{CooldownState, CooldownTracker};
use super::history::History;
use super::model::*;

use super::bot::{CommandRegistry, RawCommand};

pub struct MessagingState {
    pub cooldowns: CooldownTracker<String>,
    pub history: History<String>,
}

impl MessagingState {
    pub fn new(
        channels: &Vec<String>,
        initial_cooldown: Duration,
        history_ttl: Duration,
    ) -> MessagingState {
        MessagingState {
            cooldowns: CooldownTracker::new(
                channels
                    .iter()
                    .map(|c| (c.to_string(), initial_cooldown))
                    .collect(),
            ),
            history: History::new(
                channels.iter().map(|c| c.to_string()).collect(),
                history_ttl,
            ),
        }
    }
}

/// This function acts as event loop for reading messages from socket.
pub(crate) async fn receiver_event_loop<T: 'static + std::marker::Send + std::marker::Sync>(
    rx_socket: WebSocketStream,
    tx_socket: WebSocketSharedSink,
    tx_command: Sender<RawCommand>,
    command_registry: Arc<CommandRegistry<T>>,
) {
    let mut rx_socket = rx_socket;
    let mut tx_command = tx_command;

    while let Some(message) = rx_socket.next().await {
        match message {
            Ok(Message::Text(message)) => {
                for raw_message in message.split_terminator("\r\n") {
                    match irc::Message::parse(raw_message) {
                        Ok(message) => {
                            let action = match message.command.name {
                                "PRIVMSG" => {
                                    if command_registry.is_command(&message) {
                                        Action::ExecuteCommand(raw_message.to_string())
                                    } else {
                                        info!("{}", message);
                                        Action::None
                                    }
                                }
                                "PING" => {
                                    info!("responding to PING...");
                                    Action::SendMessage(
                                        irc::MessageBuilder::new("PONG")
                                            .with_trailing(message.trailing.unwrap_or(""))
                                            .string(),
                                    )
                                }
                                cmd => {
                                    info!("no handler for command {} / {}", cmd, message);
                                    Action::None
                                }
                            };

                            match action {
                                Action::ExecuteCommand(command) => tx_command
                                    .send(command)
                                    .await
                                    .expect("Failed to submit command"),
                                Action::SendMessage(message) => tx_socket
                                    .lock()
                                    .await
                                    .send(Message::text(message))
                                    .await
                                    .expect("Failed to send message"),
                                Action::None => trace!("No action taken"),
                            }
                        }
                        Err(err) => {
                            error!("Error parsing message: {} (message = {})", err, message)
                        }
                    }
                }
            }
            Ok(message) => error!("Unsupported message: {:?}", message),
            Err(err) => error!("Received error: {:?}", err),
        }
    }
}

/// This function acts as event loop for sending messages to socket.
pub(crate) async fn sender_event_loop(
    rx_message: Receiver<PreparedMessage>,
    tx_socket: WebSocketSharedSink,
    state: Arc<MessagingState>,
    concurrency: usize,
) {
    let get_tx_socket = || tx_socket.clone();
    let get_state = || state.clone();

    rx_message
        .for_each_concurrent(concurrency, async move |mut message| {
            // TODO revise this -- maybe bad in terms of performance
            // 1. consult cooldown tracker
            match get_state().cooldowns.access(&message.channel) {
                Some(CooldownState::NotReady(how_long)) => tokio::timer::delay_for(how_long).await,
                Some(CooldownState::Ready) => {} // ready to send
                None => {
                    error!("No such channel: {}", message.channel);
                    return;
                }
            }
            // 2. consult message history
            let mut should_add_to_history = false;
            match get_state()
                .history
                .contains(&message.channel, &message.message)
                .await
            {
                Some(0) => should_add_to_history = true,
                Some(n) => modify_message(&mut message.message, n - 1),
                None => {
                    error!("No such channel: {}", message.channel);
                    return;
                }
            }
            if should_add_to_history {
                get_state()
                    .history
                    .push(&message.channel, message.message.clone())
                    .await;
            }
            // 3. prepare message
            message.channel.insert(0, '#');
            let text = irc::MessageBuilder::new("PRIVMSG")
                .with_arg(&message.channel)
                .with_trailing(&message.message)
                .string();

            // 4. send message
            info!("Sending message: {:?}", text);
            get_tx_socket()
                .lock()
                .await
                .send(Message::text(text))
                .await
                .expect("Failed to send message");
        })
        .await;
}
