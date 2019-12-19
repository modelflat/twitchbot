use url::Url;

mod commands;
use commands::{state, commands, permissions};


fn main() {
    env_logger::try_init().expect("Failed to initialize logger");

    let url = Url::parse("wss://irc-ws.chat.twitch.tv:443").unwrap();

    let username = std::env::var("TWITCH_USERNAME").expect("twitch username");

    let password = std::env::var("TWITCH_OAUTH_TOKEN").expect("twitch oauth token");

    let channels = std::env::var("TWITCH_CHANNELS_TO_JOIN").expect("twitch channels to join");

    modelflat_bot::core::run(
        url,
        username,
        password,
        channels
            .split_terminator(',')
            .map(|s| s.to_string())
            .collect(),
        state(),
        commands(),
        permissions(),
    );
}
