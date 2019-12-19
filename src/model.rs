pub use log::*;

pub use async_std::net::TcpStream;
pub use std::collections::HashMap;
pub use std::sync::Arc;
pub use std::time::Duration;

pub use async_tungstenite::MaybeTlsStream;
pub use futures::lock::Mutex;
pub use futures::stream::{SplitSink, SplitStream};
pub use tungstenite::Message;

use crate::bot::RawCommand;

pub type WebSocketStreamSink = async_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub type WebSocketStream = SplitStream<WebSocketStreamSink>;

pub type WebSocketSharedSink = Arc<Mutex<SplitSink<WebSocketStreamSink, Message>>>;

#[derive(Debug, Clone)]
pub struct PreparedMessage {
    pub channel: String,
    pub message: String,
}

#[derive(Debug)]
pub enum Action {
    ExecuteCommand(RawCommand),
    SendMessage(String),
    None,
}

#[derive(Debug)]
pub struct CoreState {
    username: String,
    channels: Vec<String>,
}
