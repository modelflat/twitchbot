use log::*;

use std::collections::BTreeSet;
use async_std::sync::RwLock;

use crate::irc;

use super::model::PreparedMessage;
use crate::core::bot::ExecutionOutcome::{Success, SilentSuccess, Error};

#[derive(Debug, Clone)]
pub struct CommandInstance {
    pub user: String,
    pub user_id: String,
    pub channel: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    Success(PreparedMessage),
    SilentSuccess,
    Error(String),
}

#[derive(Debug)]
pub struct BotState {
    username: String,
    channels: BTreeSet<String>,
}

impl BotState {

    pub fn new(username: String, channels: Vec<String>) -> BotState {
        BotState {
            username,
            channels: channels.into_iter().map(|s| s.to_string()).collect(),
        }
    }

}

pub struct CommandRegistry;

impl CommandRegistry {
    pub fn convert_to_command<'a>(&self, msg: &irc::Message<'a>) -> Option<CommandInstance> {
        const PREFIX: &str = ">>";

        let message = msg.trailing.unwrap_or("");
        if message.starts_with(PREFIX) {
            Some(CommandInstance {
                user: msg.tag_value("display-name")?.to_string(),
                user_id: msg.tag_value("user-id")?.to_string(),
                channel: msg.command.args.first()?.trim_start_matches('#').to_string(),
                message: message[PREFIX.len()..].to_string(),
            })
        } else {
            None
        }
    }

    pub async fn execute(&self, command: CommandInstance, state: &RwLock<BotState>)
        -> ExecutionOutcome {
        let mut args = command.message.trim_start_matches(' ').splitn(2, ' ');
        let command_name = args.next().expect("Failed to split by space");

        // TODO this should be made dynamic. leaving as static for now though
        match command_name {
            "bot" => {
                Success(PreparedMessage {
                    channel: command.channel,
                    message: "FeelsDankMan I'm a bot by @modelflat. \
                    Prefix: '>>'. \
                    Language: Rust (nightly). \
                    See (help) for commands. \
                    Source code at github: modelflat/twitchbot".to_string()
                })
            },
            "help" => {
                Success(PreparedMessage {
                    channel: command.channel,
                    message: "FeelsDankMan \
                    I can do only the two safest things in the world: \
                    (echo) echo your message back without checking the banphrase API and \
                    (lua) execute untrusted lua code in a sandbox (640kb, ~1000 instructions max). \
                    And all that with no timeouts or permissions! (for now)".to_string()
                })
            },
            "echo" => {
                Success(PreparedMessage {
                    channel: command.channel, message: match args.next() {
                        Some(message) => message.to_string(),
                        None => "echo!".to_string(),
                    }
                })
            },
            "lua" => {
                use super::lua::run_untrusted_lua_code;
                if let Some(message) = args.next() {
                    info!("executing Lua: {}", message);

                    let result = run_untrusted_lua_code(message.to_string());

                    Success(PreparedMessage {
                        channel: command.channel,
                        message: match result {
                            Ok(result) => format!("@{}, result = {}", command.user, result),
                            Err(err) => format!("@{}, error! {}", command.user, err),
                        }
                    })
                } else {
                    info!("lua: not enough arguments");
                    SilentSuccess
                }
            }
            _ => {
                info!("unknown command: {}", command_name);
                SilentSuccess
            }
        }

    }
}
