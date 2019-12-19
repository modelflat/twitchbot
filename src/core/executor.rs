use log::*;

use std::sync::Arc;

use futures::channel::mpsc::{Receiver, Sender};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};

use crate::irc;
use crate::core::bot::{CommandRegistry, ExecutionOutcome, RawCommand, ShareableBotState};
use crate::core::model::PreparedMessage;
use crate::core::permissions::PermissionList;
use crate::core::cooldown::{CooldownState, CooldownTracker};


type GlobalCooldownTracker = CooldownTracker<String>;

type UserCooldownTracker = CooldownTracker<(String, String)>;


async fn execute<T: 'static + std::marker::Send + std::marker::Sync>(
    raw_message: String,
    tx_message: &Mutex<Sender<PreparedMessage>>,
    commands: &CommandRegistry<T>,
    permissions: &PermissionList,
    global_cooldowns: &GlobalCooldownTracker,
    user_cooldowns: &UserCooldownTracker,
    state: &ShareableBotState<T>,
) {
    const PREFIX: &str = ">>";

    let message = irc::Message::parse(&raw_message).unwrap();

    let (command_name, command_body) = {
        let mut command_split = (&message.trailing.unwrap()[PREFIX.len()..]).splitn(2, ' ');
        (command_split.next().unwrap().to_string(), command_split.next().unwrap_or(""))
    };

    let user = message.tag_value("display-name").unwrap_or("");

    let outcome = match commands.commands.get(&command_name) {
        Some(executable) => {
            // 1. consult user permissions
            if !permissions.get(user).permits(executable.level()) {
                info!("user {} lacks permissions to execute '{}'", user, command_name);
                return;
            }

            let (command_cooldown, user_cooldown) = executable.cooldown();

            let command_user_pair = (command_name.to_string(), user.to_string());

            if let Some(cooldown) = user_cooldown {
                if !user_cooldowns.contains(&command_user_pair) {
                    user_cooldowns.add_channel(command_user_pair.clone(), cooldown, true);
                }
            }

            match (command_cooldown, user_cooldown) {
                (None, Some(_)) => {
                    match user_cooldowns.access(&command_user_pair) {
                        Some(CooldownState::Ready) => {
                            trace!("user cooldown is satisfied");
                        },
                        Some(CooldownState::NotReady(remaining)) => {
                            info!("{} -> '{}' is on cooldown ({} s remaining)",
                                  user, command_name, remaining.as_secs_f64());
                            return;
                        },
                        None => {
                            error!("'{}' is not found in cooldown tracker", command_name);
                            return;
                        }
                    }
                },
                (Some(_), None) => {
                    match global_cooldowns.access(&command_name) {
                        Some(CooldownState::Ready) => {
                            trace!("command cooldown is satisfied");
                        },
                        Some(CooldownState::NotReady(remaining)) => {
                            info!("'{}' is on cooldown ({} s remaining)",
                                  command_name, remaining.as_secs_f64());
                            return;
                        },
                        None => {
                            error!("'{}' is not found in cooldown tracker", command_name);
                            return;
                        }
                    }
                },
                (Some(_), Some(_)) => {
                    if let Some(mut user_write_lock) = user_cooldowns.access_raw(&command_user_pair) {
                        if user_write_lock.is_cooldown() {
                            info!("{} -> '{}' is on cooldown", user, command_name);
                            return;
                        } else {
                            match global_cooldowns.access(&command_name) {
                                Some(CooldownState::Ready) => {
                                    match user_write_lock.try_reset() {
                                        CooldownState::Ready => {
                                            trace!("user and command cooldowns are satisfied");
                                        },
                                        CooldownState::NotReady(_) => {
                                            unreachable!("lock guarantees broken")
                                        }
                                    }
                                },
                                Some(CooldownState::NotReady(remaining)) => {
                                    info!("'{}' is on cooldown ({} s remaining)",
                                          command_name, remaining.as_secs_f64());
                                    return;
                                },
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
                },
                (None, None) => {
                    // TODO check this at setup time
                    error!("command '{}' has no cooldowns, skipping...", command_name);
                    return;
                }
            }

            info!("executing command: {}", command_name);
            executable.execute(command_body, message, &state, commands).await
        },
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
        },
        ExecutionOutcome::SilentSuccess => {
            info!("Successfully executed command: {:?}", raw_message);
        },
        ExecutionOutcome::Error(error) => {
            error!(
                "Error executing command: {:?} / command = {:?}", error, raw_message
            );
        }
    };
}

/// An event loop for executing commands.
pub(crate) async fn event_loop<T: 'static + std::marker::Send + std::marker::Sync>(
    rx_command: Receiver<RawCommand>,
    tx_message: Sender<PreparedMessage>,
    command_registry: Arc<CommandRegistry<T>>,
    permission_list: Arc<PermissionList>,
    global_cooldowns: Arc<GlobalCooldownTracker>,
    user_cooldowns: Arc<UserCooldownTracker>,
    state: Arc<ShareableBotState<T>>,
    concurrency: usize,
) {
    let tx_message = Arc::new(Mutex::new(tx_message));
    let get_tx_message = || tx_message.clone();
    let get_commands = || command_registry.clone();
    let get_permissions = || permission_list.clone();
    let get_global_cooldowns = || global_cooldowns.clone();
    let get_user_cooldowns = || user_cooldowns.clone();
    let get_state = || state.clone();

    rx_command.for_each_concurrent(concurrency, async move |raw_message| {
        execute(
            raw_message,
            &*get_tx_message(),
            &*get_commands(),
            &*get_permissions(),
            &*get_global_cooldowns(),
            &*get_user_cooldowns(),
            &*get_state(),
        ).await;
    }).await;
}
