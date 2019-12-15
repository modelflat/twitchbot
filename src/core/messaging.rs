
use futures::{SinkExt, StreamExt};
use futures::channel::mpsc::{Sender, Receiver};

use tungstenite::Message;

use crate::irc;
use crate::util::modify_message;

use super::model::*;
use super::history::History;
use super::cooldown::{CooldownState, CooldownTracker};

pub struct MessagingState {
    pub cooldowns: CooldownTracker,
    pub history: History<String>,
}

impl MessagingState {

    pub fn new(channels: &Vec<String>, initial_cooldown: Duration, history_ttl: Duration)
        -> MessagingState {
        MessagingState {
            cooldowns: CooldownTracker::new(
                channels.iter().map(|c| (c.to_string(), initial_cooldown)).collect()
            ),
            history: History::new(
                channels.iter().map(|c| c.to_string()).collect(), history_ttl
            ),
        }
    }

}


fn is_command(msg: &str) -> bool {
    msg.starts_with(">>")
}

fn match_irc_command<'a>(message: &irc::Message<'a>) -> Result<Action, Box<dyn std::error::Error>> {
    let message_text = message.trailing.unwrap_or("");
    match message.command.name {
        "PRIVMSG" if is_command(message_text) => {
            Ok(Action::ExecuteCommand(Command {
                user: message.tag_value("display-name")
                    .ok_or("no display-name tag!")?
                    .to_string(),
                user_id: message.tag_value("user-id")
                    .ok_or("no user-id tag!")?
                    .to_string(),
                channel: message.command.args
                    .first()
                    .ok_or("no argument to PRIVMSG")?
                    .trim_start_matches('#')
                    .to_string(),
                message: message_text.to_string(),
            }))
        },
        "PRIVMSG" => {
            info!("{}", message);
            Ok(Action::None)
        },
        "PING" => {
            info!("responding to PING...");
            Ok(Action::SendMessage(
                irc::MessageBuilder::new("PONG")
                    .with_trailing(message.trailing.unwrap_or(""))
                    .string()
            ))
        },
        cmd => {
            info!("no handler for command {} / {}", cmd, message);
            Ok(Action::None)
        }
    }
}

/// This function acts as event loop for reading messages from socket.
pub(crate) async fn receiver_event_loop(
    rx_socket: WebSocketStream,
    tx_socket: WebSocketSharedSink,
    tx_command: Sender<Command>,
) {
    let mut rx_socket = rx_socket;
    let mut tx_command = tx_command;
    while let Some(message) = rx_socket.next().await {
        match message {
            Ok(Message::Text(message)) => for message in message.split_terminator("\r\n") {
                match irc::Message::parse(message) {
                    Ok(message) => match match_irc_command(&message) {
                        Ok(action) => match action {
                            Action::ExecuteCommand(command) =>
                                tx_command.send(command).await
                                    .expect("Failed to submit command"),
                            Action::SendMessage(message) =>
                                tx_socket.lock().await
                                    .send(Message::text(message)).await
                                    .expect("Failed to send message"),
                            Action::None =>
                                trace!("No action taken"),
                        },
                        Err(err) =>
                            error!("Error handling message: {} (message = {})", err, message),
                    }
                    Err(err) =>
                        error!("Error parsing message: {} (message = {})", err, message),
                }
            },
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
    concurrency: usize
) {
    let tx_socket_factory = || tx_socket.clone();
    let cooldown_factory = || state.clone();

    rx_message.for_each_concurrent(concurrency, async move |mut message| {
        // TODO revise this -- maybe bad in terms of performance
        // 1. consult cooldown tracker
        match cooldown_factory().cooldowns.access(&message.channel).await {
            Some(CooldownState::NotReady(how_long)) => tokio::timer::delay_for(how_long).await,
            Some(CooldownState::Ready) => {}, // ready to send
            None => {
                error!("No such channel: {}", message.channel);
                return;
            }
        }
        // 2. consult message history
        let mut should_add_to_history = false;
        match cooldown_factory().history.contains(&message.channel, &message.message).await {
            Some(0) => should_add_to_history = true,
            Some(n) => modify_message(&mut message.message, n - 1),
            None => {
                error!("No such channel: {}", message.channel);
                return;
            }
        }
        if should_add_to_history {
            cooldown_factory().history.push(&message.channel, message.message.clone()).await;
        }
        // 3. prepare message
        message.channel.insert(0, '#');
        let text = irc::MessageBuilder::new("PRIVMSG")
            .with_arg(&message.channel)
            .with_trailing(&message.message)
            .string();

        // 4. send message
        info!("Sending message: {:?}", text);
        tx_socket_factory()
            .lock().await
            .send(Message::text(text)).await
            .expect("Failed to send message");
    }).await;
}