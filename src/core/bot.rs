use async_std::sync::RwLock;
use async_trait::async_trait;
use std::collections::{BTreeSet, HashMap};

use crate::irc;

use super::model::PreparedMessage;
use crate::core::permissions::PermissionLevel;
use std::time::Duration;

// TODO make this generic
const PREFIX: &str = ">>";

#[async_trait]
pub trait ExecutableCommand<T: 'static + std::marker::Send + std::marker::Sync> {
    async fn execute<'a>(
        &self,
        command: &'a str,
        message: irc::Message<'a>,
        state: &ShareableBotState<T>,
        read_only_state: &ReadonlyState<T>,
    ) -> ExecutionOutcome;

    fn help(&self) -> String;

    fn cooldown(&self) -> (Option<Duration>, Option<Duration>);

    fn level(&self) -> PermissionLevel;
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

impl ExecutionOutcome {
    pub fn success(channel: String, message: String) -> ExecutionOutcome {
        ExecutionOutcome::Success(PreparedMessage { channel, message })
    }
}

#[derive(Debug)]
pub struct BotState<T> {
    pub username: String,
    pub channels: BTreeSet<String>,
    pub data: T,
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
    pub commands: HashMap<String, ShareableExecutableCommand<T>>,
}

impl<T: 'static + std::marker::Send + std::marker::Sync> CommandRegistry<T> {
    pub fn new(commands: HashMap<String, ShareableExecutableCommand<T>>) -> CommandRegistry<T> {
        CommandRegistry { commands }
    }

    pub fn is_command<'a>(&self, msg: &irc::Message<'a>) -> bool {
        msg.trailing.unwrap_or("").starts_with(PREFIX)
    }
}

pub type ReadonlyState<T> = CommandRegistry<T>;
