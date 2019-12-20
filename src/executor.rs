use log::*;

use async_trait::async_trait;
use std::marker::{Send, Sync};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::{Receiver, Sender};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};

use crate::cooldown::{CooldownState, CooldownTracker};
use crate::irc;
use crate::messaging::PreparedMessage;
use crate::permissions::PermissionLevel;
use crate::state::BotState;

type GlobalCooldownTracker = CooldownTracker<String>;

type UserCooldownTracker = CooldownTracker<(String, String)>;

#[derive(Debug)]
pub struct CommandCooldown {
    pub command: Option<Duration>,
    pub user: Option<Duration>,
}

#[async_trait]
pub trait ExecutableCommand<T: 'static + Send + Sync> {
    async fn execute<'a>(&self, command: &'a str, message: irc::Message<'a>, state: &BotState<T>) -> ExecutionOutcome;

    fn help(&self) -> String;

    fn cooldown(&self) -> CommandCooldown;

    fn level(&self) -> PermissionLevel;
}

pub type ShareableExecutableCommand<T> = Box<dyn ExecutableCommand<T> + 'static + Send + Sync>;

#[derive(Debug, Clone)]
pub struct PreparedCommand {
    pub message: String,
    pub command: String,
}

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

async fn execute<T: 'static + std::marker::Send + std::marker::Sync>(
    command: PreparedCommand,
    state: &BotState<T>,
    tx_message: &Mutex<Sender<PreparedMessage>>,
    global_cooldowns: &GlobalCooldownTracker,
    user_cooldowns: &UserCooldownTracker,
) {
    let message = irc::Message::parse(&command.message).unwrap();

    let (command_name, command_body) = {
        let mut command_split = command.command.splitn(2, ' ');
        (
            command_split.next().unwrap().to_string(),
            command_split.next().unwrap_or(""),
        )
    };

    let user = message.tag_value("display-name").unwrap_or("");

    let outcome = match state.commands.get(&command_name) {
        Some(executable) => {
            // 1. consult user permissions
            if !state.permissions.get(user).permits(executable.level()) {
                info!("user {} lacks permissions to execute '{}'", user, command_name);
                return;
            }

            let cooldown = executable.cooldown();

            let command_user_pair = (command_name.to_string(), user.to_string());

            if let Some(cooldown) = cooldown.user {
                if !user_cooldowns.contains(&command_user_pair) {
                    user_cooldowns.add_channel(command_user_pair.clone(), cooldown, true);
                }
            }

            match (cooldown.command, cooldown.user) {
                (None, Some(_)) => match user_cooldowns.access(&command_user_pair) {
                    Some(CooldownState::Ready) => {
                        trace!("user cooldown is satisfied");
                    }
                    Some(CooldownState::NotReady(remaining)) => {
                        info!(
                            "{} -> '{}' is on cooldown ({} s remaining)",
                            user,
                            command_name,
                            remaining.as_secs_f64()
                        );
                        return;
                    }
                    None => {
                        error!("'{}' is not found in cooldown tracker", command_name);
                        return;
                    }
                },
                (Some(_), None) => match global_cooldowns.access(&command_name) {
                    Some(CooldownState::Ready) => {
                        trace!("command cooldown is satisfied");
                    }
                    Some(CooldownState::NotReady(remaining)) => {
                        info!(
                            "'{}' is on cooldown ({} s remaining)",
                            command_name,
                            remaining.as_secs_f64()
                        );
                        return;
                    }
                    None => {
                        error!("'{}' is not found in cooldown tracker", command_name);
                        return;
                    }
                },
                (Some(_), Some(_)) => {
                    if let Some(user_read_lock) = user_cooldowns.access_raw(&command_user_pair) {
                        if user_read_lock.is_cooldown() {
                            info!("{} -> '{}' is on cooldown", user, command_name);
                            return;
                        } else {
                            match global_cooldowns.access(&command_name) {
                                Some(CooldownState::Ready) => match user_read_lock.try_reset() {
                                    CooldownState::Ready => {
                                        trace!("user and command cooldowns are satisfied");
                                    }
                                    CooldownState::NotReady(remaining) => {
                                        info!(
                                            "'{}' is on cooldown ({} s remaining)",
                                            command_name,
                                            remaining.as_secs_f64()
                                        );
                                        return;
                                    }
                                },
                                Some(CooldownState::NotReady(remaining)) => {
                                    info!(
                                        "'{}' is on cooldown ({} s remaining)",
                                        command_name,
                                        remaining.as_secs_f64()
                                    );
                                    return;
                                }
                                None => {
                                    error!("'{}' is not found in cooldown tracker", command_name);
                                    return;
                                }
                            }
                        }
                    } else {
                        error!("user '{}' was suddenly removed from cooldown tracker", user);
                        return;
                    }
                }
                (None, None) => {
                    // TODO check this at setup time
                    error!("command '{}' has no cooldowns, skipping...", command_name);
                    return;
                }
            }

            info!("executing command: {}", command_name);
            executable.execute(command_body, message, &state).await
        }
        None => {
            info!("no such command: {}", command_name);
            ExecutionOutcome::SilentSuccess
        }
    };

    match outcome {
        ExecutionOutcome::Success(message) => {
            tx_message
                .lock()
                .await
                .send(message)
                .await
                .expect("Failed to submit message to message queue");
        }
        ExecutionOutcome::SilentSuccess => {
            info!("Successfully executed command: {:?}", command.command);
        }
        ExecutionOutcome::Error(error) => {
            error!("Error executing command: {:?} / command = {:?}", error, command.command);
        }
    };
}

/// An event loop for executing commands.
pub(crate) async fn event_loop<T: 'static + Send + Sync>(
    rx_command: Receiver<PreparedCommand>,
    tx_message: Sender<PreparedMessage>,
    state: Arc<BotState<T>>,
    concurrency: usize,
) {
    let tx_message = Arc::new(Mutex::new(tx_message));
    let get_tx_message = || tx_message.clone();

    let global_cooldowns = Arc::new(GlobalCooldownTracker::new(
        state
            .commands
            .iter()
            .filter_map(|(name, cmd)| {
                let CommandCooldown { command, .. } = cmd.cooldown();
                command.map(|cd| (name.to_string(), cd))
            })
            .collect(),
    ));
    let get_global_cooldowns = || global_cooldowns.clone();

    let user_cooldowns = Arc::new(UserCooldownTracker::new(Default::default()));
    let get_user_cooldowns = || user_cooldowns.clone();

    let get_state = || state.clone();

    rx_command
        .for_each_concurrent(concurrency, async move |command| {
            execute(
                command,
                &*get_state(),
                &*get_tx_message(),
                &*get_global_cooldowns(),
                &*get_user_cooldowns(),
            )
            .await;
        })
        .await;
}
