use std::collections::HashMap;

use bot::prelude::*;

mod help;
use help::Help;

mod bot_description;
use bot_description::BotDescription;

mod echo;
use echo::Echo;

mod lua;
use lua::Lua;

pub struct MyState;

impl MyState {
    pub fn new() -> MyState {
        MyState {}
    }
}

pub fn state() -> MyState {
    MyState::new()
}

pub fn commands() -> HashMap<String, ShareableExecutableCommand<MyState>> {
    let mut map: HashMap<String, ShareableExecutableCommand<MyState>> = HashMap::new();
    map.insert("bot".to_string(), Box::new(BotDescription {}));
    map.insert("echo".to_string(), Box::new(Echo {}));
    map.insert("lua".to_string(), Box::new(Lua {}));
    map.insert("help".to_string(), Box::new(Help {}));
    map
}

pub fn permissions() -> PermissionList {
    let mut map = HashMap::new();
    map.insert("modelflat".to_string(), PermissionLevel::Admin);
    PermissionList::new(map)
}
