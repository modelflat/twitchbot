use async_std::sync::RwLock;

use async_tungstenite::connect_async;
use futures::channel::mpsc::channel;
use futures::{SinkExt, StreamExt};
use url::Url;

pub mod model;

pub mod executor;
pub mod messaging;

pub mod bot;

pub mod lua;

mod cooldown;
mod history;

use messaging::MessagingState;
use model::*;
use crate::core::bot::{BotState, CommandRegistry};

async fn initialize(
    url: Url,
    username: &str,
    password: &str,
    channels: impl Iterator<Item = &String>,
) -> WebSocketStreamSink {
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // login to twitch IRC
    ws_stream
        .send(Message::Text(format!("PASS oauth:{}", password)))
        .await
        .unwrap();
    ws_stream
        .send(Message::Text(format!("NICK {}", username)))
        .await
        .unwrap();
    ws_stream
        .send(Message::Text(
            "CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership".to_owned(),
        ))
        .await
        .unwrap();

    // join channels
    for channel in channels {
        ws_stream
            .send(Message::Text(format!("JOIN #{}", channel)))
            .await
            .unwrap();
    }

    ws_stream
}

pub fn run(
    url: Url, username: String, password: String, channels: Vec<String>,
) {
    let runtime = tokio::runtime::Builder::new()
        .build()
        .expect("Failed to create runtime");

    // initialize client
    let ws_stream = runtime.block_on(initialize(url, &username, &password, channels.iter()));

    let (tx_socket, rx_socket) = ws_stream.split();
    let tx_socket = Arc::new(Mutex::new(tx_socket));

    let (tx_command, rx_command) = channel(1024);
    let (tx_message, rx_message) = channel(1024);

    let concurrency = 64;

    let messaging_state = Arc::new(MessagingState::new(
        &channels,
        Duration::from_secs(1),
        Duration::from_secs(30),
    ));

    // Message sending loop
    runtime.spawn(messaging::sender_event_loop(
        rx_message,
        tx_socket.clone(),
        messaging_state,
        concurrency,
    ));

    let bot_state = Arc::new(RwLock::new(BotState::new(username, channels)));
    let command_registry = Arc::new(CommandRegistry {});

    // Command handling loop
    runtime.spawn(executor::event_loop(
        rx_command, tx_message, concurrency, command_registry.clone(), bot_state.clone()
    ));

    // Main loop
    runtime.block_on(messaging::receiver_event_loop(
        rx_socket, tx_socket, tx_command, command_registry
    ));
}
