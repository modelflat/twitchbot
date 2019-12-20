#![feature(test)]
#![feature(async_closure)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_std::sync::Mutex;

use futures::channel::mpsc::channel;
use futures::StreamExt;
use url::Url;

pub mod irc;
pub mod lua;
pub mod permissions;
pub mod prelude;
pub mod state;

mod cooldown;
mod executor;
mod history;
mod messaging;
mod util;

use executor::ShareableExecutableCommand;
use messaging::MessagingState;
use permissions::PermissionList;
use state::BotState;

pub fn run<T: 'static + Send + Sync>(
    url: Url,
    username: String,
    password: String,
    channels: Vec<String>,
    data: T,
    commands: HashMap<String, ShareableExecutableCommand<T>>,
    permissions: PermissionList,
) {
    let runtime = tokio::runtime::Builder::new()
        .build()
        .expect("Failed to create runtime");

    // initialize client
    let ws_stream = runtime.block_on(messaging::initialize(url, &username, &password, channels.iter()));

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

    let bot_state = Arc::new(BotState::new(
        username,
        ">>".to_string(),
        channels,
        commands,
        permissions,
        data,
    ));

    // Message sending loop
    runtime.spawn(messaging::sender_event_loop(
        rx_message,
        tx_socket.clone(),
        messaging_state,
        concurrency,
    ));

    // Command handling loop
    runtime.spawn(executor::event_loop(
        rx_command,
        tx_message,
        bot_state.clone(),
        concurrency,
    ));

    // Main loop
    runtime.block_on(messaging::receiver_event_loop(
        rx_socket,
        tx_socket,
        tx_command,
        bot_state.clone(),
    ));
}
