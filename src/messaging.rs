use std::sync::Arc;
use std::time::Duration;

use async_std::net::TcpStream;
use async_std::sync::Mutex;

use async_tungstenite::{connect_async, MaybeTlsStream};

use futures::channel::mpsc::{Receiver, Sender};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};

use log::*;
use tungstenite::Message;
use url::Url;

use crate::banphrase::{BanphraseAPI, BanphraseResponse};
use crate::cooldown::{CooldownState, CooldownTracker};
use crate::executor::PreparedCommand;
use crate::history::History;
use crate::irc;
use crate::state::BotState;
use crate::util::modify_message;

pub(crate) struct MessagingState {
    pub cooldowns: CooldownTracker<String>,
    pub history: History<String>,
    pub banphrase_api: BanphraseAPI,
}

impl MessagingState {
    pub fn new(
        channels: &Vec<String>,
        initial_cooldown: Duration,
        history_ttl: Duration,
        banphrase_api_url: String,
    ) -> MessagingState {
        MessagingState {
            cooldowns: CooldownTracker::new(channels.iter().map(|c| (c.to_string(), initial_cooldown)).collect()),
            history: History::new(channels.iter().map(|c| c.to_string()).collect(), history_ttl),
            banphrase_api: BanphraseAPI::new(banphrase_api_url),
        }
    }
}

#[derive(Debug)]
pub enum Action {
    ExecuteCommand(PreparedCommand),
    SendMessage(String),
    None,
}

#[derive(Debug, Clone)]
pub struct PreparedMessage {
    pub channel: String,
    pub message: String,
}

type WebSocketStreamSink = async_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

type WebSocketSharedSink = Arc<Mutex<SplitSink<WebSocketStreamSink, Message>>>;

type WebSocketStream = SplitStream<WebSocketStreamSink>;

/// This function initializes messaging stream.
pub(crate) async fn initialize(
    url: Url,
    username: &str,
    password: &str,
    channels: impl Iterator<Item = &String>,
) -> WebSocketStreamSink {
    info!("Connecting to {}...", url);
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect to socket");

    info!(
        "Authenticating with user name '{}', oauth token '{}'",
        username, "*".repeat(password.len())
    );

    // login to twitch IRC
    ws_stream
        .send(Message::Text(format!("PASS oauth:{}", password)))
        .await
        .expect("Failed to send WS message");
    ws_stream
        .send(Message::Text(format!("NICK {}", username)))
        .await
        .expect("Failed to send WS message");
    ws_stream
        .send(Message::Text(
            "CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership".to_owned(),
        ))
        .await
        .expect("Failed to send WS message");

    // join channels
    for channel in channels {
        info!("Joining channel: {}", channel);
        ws_stream
            .send(Message::Text(format!("JOIN #{}", channel)))
            .await
            .expect("Failed to send WS message");
    }

    ws_stream
}

/// This function acts as event loop for reading messages from socket.
pub(crate) async fn receiver_event_loop<T: 'static + Send + Sync>(
    rx_socket: WebSocketStream,
    tx_socket: WebSocketSharedSink,
    tx_command: Sender<PreparedCommand>,
    state: Arc<BotState<T>>,
    messaging_state: Arc<MessagingState>,
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
                                    if let Some(command) = state.try_convert_to_command(&message) {
                                        Action::ExecuteCommand(PreparedCommand {
                                            message: raw_message.to_string(),
                                            command,
                                        })
                                    } else {
                                        info!("{}", message);
                                        Action::None
                                    }
                                }
                                "PING" => {
                                    info!("Responding to PING...");
                                    Action::SendMessage(
                                        irc::MessageBuilder::new("PONG")
                                            .with_trailing(message.trailing.unwrap_or(""))
                                            .string(),
                                    )
                                }
                                "USERSTATE" => {
                                    const MODERATOR_CD: Duration = Duration::from_millis(100);

                                    let channel = message.first_arg_as_channel_name().unwrap().to_string();
                                    info!("Received USERSTATE: {}", raw_message);

                                    for badge in message.tag_value("badges").unwrap_or("").split_terminator(',') {
                                        if badge.starts_with("moderator") {
                                            info!(
                                                "Updated cooldown to {:?} for channel {} because of moderator status",
                                                MODERATOR_CD, channel
                                            );
                                            messaging_state.cooldowns.update(&channel, MODERATOR_CD);
                                        }
                                    }
                                    Action::None
                                }
                                cmd => {
                                    info!("No handler for command {} / {}", cmd, message);
                                    Action::None
                                }
                            };

                            match action {
                                Action::ExecuteCommand(command) => {
                                    tx_command.send(command).await.expect("Failed to submit command")
                                }
                                Action::SendMessage(message) => tx_socket
                                    .lock()
                                    .await
                                    .send(Message::text(message))
                                    .await
                                    .expect("Failed to send message"),
                                Action::None => trace!("No action taken"),
                            }
                        }
                        Err(err) => error!("Error parsing message: {} (message = {})", err, message),
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
        .for_each_concurrent(
            concurrency,
            async move |PreparedMessage {
                            mut message,
                            mut channel,
                        }| {
                // consult cooldown tracker and/or banphrase API
                let banphrase_future = get_state().banphrase_api.check(message.clone());
                let response = match get_state().cooldowns.access_raw(&channel) {
                    Some(read_lock) => {
                        // let's simply check for cooldown first
                        match read_lock.cooldown() {
                            CooldownState::Ready => {
                                // if this is ready, we don't really care -- we need to check banphrase
                                // api first.
                                banphrase_future.await
                            }
                            CooldownState::NotReady(how_long) => {
                                // if this is not ready, we can align banphrase api request and waiting
                                // time.
                                futures::future::join(tokio::timer::delay_for(how_long), banphrase_future)
                                    .await
                                    .1
                            }
                        }
                    }
                    None => {
                        error!("No such channel: {}", channel);
                        return;
                    }
                };

                // now that we've got response from banphrase api, lets check it
                match response {
                    Ok(r) => match r.json::<BanphraseResponse>().await {
                        Ok(r) => {
                            if r.banned {
                                info!("Banphrase API says that message is banned -- not sending ({})", message);
                                return;
                            }
                        }
                        Err(e) => {
                            error!("Weird response from banphrase API: {:?}", e);
                            return;
                        }
                    },
                    Err(e) => {
                        error!("Failed to consult banphrase API: {:?}", e);
                        return;
                    }
                }

                // ok, so message is not a banphrase. now we should consult history to find out
                // whether do we need to modify it
                // TODO what if modification results in a message becoming banphrase?
                let mut should_add_to_history = false;
                match get_state().history.contains(&channel, &message).await {
                    Some(0) => should_add_to_history = true,
                    Some(n) => modify_message(&mut message, n - 1),
                    None => {
                        error!("No such channel: {}", channel);
                        return;
                    }
                }

                if should_add_to_history {
                    get_state().history.push(&channel, message.clone()).await;
                }

                // bu-u-ut here we need to consult cooldown tracker again to find out whether we can
                // send this message
                match get_state().cooldowns.access_raw(&channel) {
                    Some(read_lock) => {
                        if let CooldownState::NotReady(how_long) = read_lock.try_reset() {
                            tokio::timer::delay_for(how_long).await;
                        }

                        channel.insert(0, '#');

                        let text = irc::MessageBuilder::new("PRIVMSG")
                            .with_arg(&channel)
                            .with_trailing(&message)
                            .string();

                        info!("Sending message: {:?}", text);

                        get_tx_socket()
                            .lock()
                            .await
                            .send(Message::text(text))
                            .await
                            .expect("Failed to send message");
                    }
                    None => {
                        error!("No such channel: {}", channel);
                        return;
                    }
                }
            },
        )
        .await;
}
