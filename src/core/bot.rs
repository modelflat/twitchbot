use log::*;

use async_std::sync::RwLock;
use async_trait::async_trait;
use std::collections::{BTreeSet, HashMap};

use crate::irc;

use super::model::PreparedMessage;

// TODO make this generic
const PREFIX: &str = ">>";

#[async_trait]
pub trait ExecutableCommand<T: 'static + std::marker::Send + std::marker::Sync> {
    async fn execute<'a>(
        &self,
        message: irc::Message<'a>,
        state: &ShareableBotState<T>,
    ) -> ExecutionOutcome;
}

pub type ShareableExecutableCommand<T> =
    Box<dyn ExecutableCommand<T> + 'static + std::marker::Send + std::marker::Sync>;

#[derive(Debug, Clone)]
pub struct CommandInstance {
    pub user: String,
    pub user_id: String,
    pub channel: String,
    pub message: String,
}

pub type RawCommand = String;

#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    Success(PreparedMessage),
    SilentSuccess,
    Error(String),
}

#[derive(Debug)]
pub struct BotState<T> {
    username: String,
    channels: BTreeSet<String>,
    data: T,
}

impl<T> BotState<T> {
    pub fn new(username: String, channels: Vec<String>, data: T) -> BotState<T> {
        BotState {
            username,
            channels: channels.into_iter().map(|s| s.to_string()).collect(),
            data,
        }
    }
}

pub type ShareableBotState<T> = RwLock<BotState<T>>;

pub struct CommandRegistry<T>
where
    T: 'static + std::marker::Send + std::marker::Sync,
{
    commands: HashMap<String, ShareableExecutableCommand<T>>,
}

impl<T: 'static + std::marker::Send + std::marker::Sync> CommandRegistry<T> {
    pub fn new(commands: HashMap<String, ShareableExecutableCommand<T>>) -> CommandRegistry<T> {
        CommandRegistry { commands }
    }

    pub fn is_command<'a>(&self, msg: &irc::Message<'a>) -> bool {
        msg.trailing.unwrap_or("").starts_with(PREFIX)
    }

    pub async fn execute(&self, message: String, state: &ShareableBotState<T>) -> ExecutionOutcome {
        let message = irc::Message::parse(&message).unwrap();

        let mut command_and_trailing = message
            .trailing
            .unwrap_or("")
            .trim_start_matches(' ')
            .splitn(2, ' ');

        let command_name = &command_and_trailing
            .next()
            .expect("Failed to split by space")[PREFIX.len()..];

        match self.commands.get(command_name) {
            Some(command) => {
                info!("executing command: {}", command_name);
                command.execute(message, state).await
            }
            None => {
                info!("no such command: {}", command_name);
                ExecutionOutcome::SilentSuccess
            }
        }
    }
}
