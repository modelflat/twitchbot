use url::Url;
use structopt::StructOpt;

mod commands;
use commands::{commands, permissions, state};

#[derive(StructOpt)]
#[structopt(about = "primitive twitch bot")]
struct Opt {

    /// Which channels should the bot join upon startup, comma-separated
    #[structopt(long)]
    channels: String,

}

fn main() {
    let opt: Opt = Opt::from_args();

    env_logger::try_init().expect("Failed to initialize logger");

    let url = Url::parse("wss://irc-ws.chat.twitch.tv:443").unwrap();

    let username = std::env::var("TWITCH_USERNAME").expect("twitch username");

    let password = std::env::var("TWITCH_OAUTH_TOKEN").expect("twitch oauth token");

    bot::run(
        url,
        username,
        password,
        opt.channels.split_terminator(',').map(|s| s.to_string()).collect(),
        state(),
        commands(),
        permissions(),
    );
}
