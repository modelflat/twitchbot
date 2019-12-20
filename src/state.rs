use async_std::sync::RwLock;
use std::collections::{BTreeSet, HashMap};

use crate::executor::ShareableExecutableCommand;
use crate::irc;
use crate::permissions::PermissionList;

pub type Commands<T> = HashMap<String, ShareableExecutableCommand<T>>;

pub struct BotState<T: 'static + Send + Sync> {
    pub username: String,
    pub username_with_at: String,
    pub prefix: String,
    pub channels: BTreeSet<String>,
    pub commands: Commands<T>,
    pub permissions: PermissionList,
    pub data: RwLock<T>,
}

impl<T: 'static + Send + Sync> BotState<T> {
    pub fn new(
        username: String,
        prefix: String,
        channels: Vec<String>,
        commands: Commands<T>,
        permissions: PermissionList,
        data: T,
    ) -> BotState<T> {
        BotState {
            username_with_at: format!("@{}", username),
            username,
            prefix,
            channels: channels.into_iter().map(|s| s.to_string()).collect(),
            commands,
            permissions,
            data: RwLock::new(data),
        }
    }

    pub fn try_convert_to_command(&self, message: &irc::Message) -> Option<String> {
        if let Some(s) = message.trailing {
            if s.starts_with(&self.prefix) {
                return Some((&s[self.prefix.len()..]).trim_start().to_string());
            }
            if s.starts_with(&self.username_with_at) {
                return Some((&s[self.username_with_at.len()..]).trim_start().to_string());
            }
        }
        None
    }
}
