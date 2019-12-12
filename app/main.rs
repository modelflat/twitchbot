use std::env;

use ws::connect;
use modelflat_bot::core::Bot;


const TWITCH_IRC_WS: &str = "wss://irc-ws.chat.twitch.tv:443";


fn run(username: &str, password: &str, channels: Vec<String>) -> ws::Result<()> {
    connect(TWITCH_IRC_WS, |out| {
        Bot::new(out, username, password, channels.clone()).unwrap()
    })
}


fn main() {
    let username = env::var("TWITCH_USERNAME")
        .expect("TWITCH_USERNAME should be set!");

    let password = env::var("TWITCH_OAUTH_TOKEN")
        .expect("TWITCH_OAUTH_TOKEN should be set!");

    let channels = env::var("TWITCH_CHANNELS_TO_JOIN")
        .expect("TWITCH_CHANNELS_TO_JOIN should be set!")
        .split(",")
        .map(|x| x.to_lowercase())
        .collect();

    run(&username, &password, channels).unwrap();
}
