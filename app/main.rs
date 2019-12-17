use log::*;

use async_trait::async_trait;
use modelflat_bot::core::bot::{
    ExecutableCommand, ExecutionOutcome, ShareableBotState, ShareableExecutableCommand,
};
use modelflat_bot::core::model::PreparedMessage;
use modelflat_bot::irc;
use std::collections::HashMap;
use url::Url;

pub struct MyState;

impl MyState {
    fn new() -> MyState {
        MyState {}
    }
}

struct Bot;

#[async_trait]
impl ExecutableCommand<MyState> for Bot {
    async fn execute<'a>(
        &self,
        message: irc::Message<'a>,
        _: &ShareableBotState<MyState>,
    ) -> ExecutionOutcome {
        ExecutionOutcome::Success(PreparedMessage {
            channel: message.first_arg_as_channel_name().unwrap().to_string(),
            message: "FeelsDankMan I'm a bot by @modelflat. \
            Prefix: '>>'. \
            Language: Rust (nightly). \
            See (help) for commands. \
            Source code at github: modelflat/twitchbot"
                .to_string(),
        })
    }
}

struct Lua;

#[async_trait]
impl ExecutableCommand<MyState> for Echo {
    async fn execute<'a>(
        &self,
        message: irc::Message<'a>,
        _: &ShareableBotState<MyState>,
    ) -> ExecutionOutcome {
        message.trailing.unwrap_or("").splitn(2, ' ');
        ExecutionOutcome::Success(PreparedMessage {
            channel: message.first_arg_as_channel_name().unwrap().to_string(),
            message: ,
        })
    }
}

struct Echo;

#[async_trait]
impl ExecutableCommand<MyState> for Lua {
    async fn execute<'a>(
        &self,
        message: irc::Message<'a>,
        _: &ShareableBotState<MyState>,
    ) -> ExecutionOutcome {
        use modelflat_bot::core::lua::run_untrusted_lua_code;

        let mut args = message.trailing.unwrap_or("").splitn(2, ' ');

        if let Some(code) = args.nth(2) {
            let user = message
                .tag_value("display-name")
                .unwrap_or("<no-display-name>");

            info!("{} is executing Lua: {}", user, code);

            let result = run_untrusted_lua_code(code.to_string());

            ExecutionOutcome::Success(PreparedMessage {
                channel: message.first_arg_as_channel_name().unwrap().to_string(),
                message: match result {
                    Ok(result) => format!("@{}, result = {}", user, result),
                    Err(err) => format!("@{}, error! {}", user, err),
                },
            })
        } else {
            info!("lua: not enough arguments");
            ExecutionOutcome::Error("lua: not enough arguments".to_string())
        }
    }
}

fn main() {
    env_logger::try_init().expect("Failed to initialize logger");

    let url = Url::parse("wss://irc-ws.chat.twitch.tv:443").unwrap();

    let username = std::env::var("TWITCH_USERNAME").expect("twitch username");

    let password = std::env::var("TWITCH_OAUTH_TOKEN").expect("twitch oauth token");

    let channels = std::env::var("TWITCH_CHANNELS_TO_JOIN").expect("twitch channels to join");

    let data = MyState::new();

    let mut commands: HashMap<String, ShareableExecutableCommand<MyState>> = HashMap::new();
    commands.insert("bot".to_string(), Box::new(Bot {}));
    commands.insert("echo".to_string(), Box::new(Echo {}));
    commands.insert("lua".to_string(), Box::new(Lua {}));

    modelflat_bot::core::run(
        url,
        username,
        password,
        channels
            .split_terminator(',')
            .map(|s| s.to_string())
            .collect(),
        data,
        commands,
    );
}
