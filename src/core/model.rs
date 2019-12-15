pub use log::*;

pub use std::sync::Arc;
pub use std::time::Duration;
pub use std::collections::HashMap;
pub use async_std::net::TcpStream;

pub use futures::stream::{SplitStream, SplitSink};
pub use futures::lock::Mutex;
pub use tungstenite::Message;
pub use async_tungstenite::MaybeTlsStream;

use crate::history::History;
use crate::cooldown::CooldownTracker;

pub type WebSocketStreamSink = async_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub type WebSocketStream = SplitStream<WebSocketStreamSink>;

pub type WebSocketSharedSink = Arc<Mutex<SplitSink<WebSocketStreamSink, Message>>>;

#[derive(Debug)]
pub struct PreparedMessage {
    pub channel: String,
    pub message: String,
}

#[derive(Debug)]
pub struct Command {
    pub user: String,
    pub user_id: String,
    pub channel: String,
    pub message: String,
}

#[derive(Debug)]
pub enum Action {
    ExecuteCommand(Command),
    SendMessage(String),
    None,
}


#[derive(Debug)]
pub struct CoreState {
    username: String,
    channels: Vec<String>,
}

pub struct MessagingState {
    pub cooldowns: CooldownTracker,
    pub history: History<String>,
}
