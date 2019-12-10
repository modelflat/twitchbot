#![feature(test)]

use std::env;
use ws;

extern crate pest;
#[macro_use]
extern crate pest_derive;

mod tmi;
mod irc;

const TWITCH_IRC_WS: &str = "wss://irc-ws.chat.twitch.tv:443";

fn run(username: &str, password: &str, channels: Vec<String>) -> ws::Result<()> {
    ws::connect(TWITCH_IRC_WS, |out| {
        tmi::Client::new(out, username, password, channels.clone()).unwrap()
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
