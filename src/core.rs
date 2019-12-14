use std::collections::HashMap;
use std::error::Error;
use std::time::{Instant, Duration};

use ws;
use ws::util::Token;

use phf::phf_map;

use crate::irc;
use crate::event;
use crate::event::{MultichannelEventQueue, Event};
use crate::util::modify_message;
use crate::history::History;


// TODO all of these should be configurable

const BOT_PREFIX: &str = ">>";

const BOT_MESSAGE_TTL: Duration = Duration::from_secs(20);

const BOT_MESSAGE_HISTORY_TTL: Duration = Duration::from_secs(30);

const BOT_CHANNEL_TIMEOUT: Duration = Duration::from_millis(1500);


#[derive(Clone)]
pub enum Command {
    PrivMsg,
    Ping,
}

static COMMANDS: phf::Map<&'static str, Command> = phf_map! {
    "PRIVMSG" => Command::PrivMsg,
    "PING" => Command::Ping,
};

// TODO base trait for bots?


pub struct Bot {
    socket: ws::Sender,
    username: String,
    channels: Vec<String>,

    // TODO is there a better way to keep those
    channel_to_token: HashMap<String, Token>,
    token_to_channel: HashMap<Token, String>,

    message_queue: MultichannelEventQueue<Token, String>,
    message_history: History<Token, String>,
}


impl Bot {

    pub fn new(socket: ws::Sender, username: &str, password: &str, channels: Vec<String>)
        -> ws::Result<Bot> {

        // TODO this seems non-optimal
        let channel_to_token: HashMap<String, Token> = channels.iter()
            .enumerate()
            .map(|(i, ch)| (ch.to_owned(), Token(i)))
            .collect();
        let token_to_channel: HashMap<Token, String> = channels.iter()
            .enumerate()
            .map(|(i, ch)| (Token(i), ch.to_owned()))
            .collect();

        let channel_tokens = channel_to_token.values().cloned().collect();

        let channels_and_default_timeouts: HashMap<Token, Duration> = channels.iter()
            .enumerate()
            .map(|(i, _)| (Token(i), Duration::from_secs(1)))
            .collect();

        let mut client = Bot {
            socket,
            username: username.to_string(),
            channels,
            channel_to_token,
            token_to_channel,
            message_queue: MultichannelEventQueue::new(&channels_and_default_timeouts),
            message_history: History::new(channel_tokens, BOT_MESSAGE_HISTORY_TTL),
        };

        client.login(username, password)?;
        client.join()?;
        client.initialize_channel_timers()?;

        return Ok(client)
    }

    fn initialize_channel_timers(&mut self) -> ws::Result<()> {
        for channel in self.channel_to_token.values() {
            self.socket.timeout(BOT_CHANNEL_TIMEOUT.as_millis() as u64, *channel)?;
        }
        Ok(())
    }

    fn handle_message<'a>(&mut self, msg: irc::Message<'a>) -> Result<(), Box<dyn Error>> {
        if let Some(command) = COMMANDS.get(msg.command.name) {
            // this should compile to a jump table
            match command {
                Command::PrivMsg => {
                    let channel = msg.command.args.first()
                        .ok_or("PRIVMSG: not enough arguments")?
                        .trim_start_matches('#');

                    let timestamp: u64 = msg.tag_value("tmi-sent-ts")
                        .ok_or("no timestamp on message")?
                        .parse()?;

                    let username = msg.tag_value("display-name")
                        .ok_or("no display name set")?;

                    let message = msg.trailing.unwrap_or("");

                    if self.is_bot_command(message) {
                        println!("COMMAND! {}", msg);
                        self.send(channel, format!("echo! {}", message));
                    } else {
                        let bytes: Vec<u8> = message.bytes().collect();
                        println!("[{}] [{}] {}: {:?}", timestamp, channel, username, bytes);
                    }
                },
                Command::Ping => {
                    println!("[system] Replying to PING...");
                    self.socket.send("PONG :tmi.twitch.tv")?;
                }
            }
        } else {
            println!("(unknown command) {}", msg);
        }
        Ok(())
    }

    /// Performs log-in into twitch IRC.
    fn login(&mut self, username: &str, password: &str) -> ws::Result<()> {
        self.socket.send(format!("PASS oauth:{}", password))?;
        self.socket.send(format!("NICK {}", username))?;
        self.socket.send("CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership")
    }

    fn join(&mut self) -> ws::Result<()> {
        for channel in &self.channels {
            self.socket.send(format!("JOIN #{}", channel))?;
        }
        Ok(())
    }

    fn send(&mut self, channel: &str, text: String) {
        let channel = *self.channel_to_token.get(channel).expect("channel not registered");
        self.message_queue.submit(channel, BOT_MESSAGE_TTL, text);
    }

    fn is_bot_command(&self, msg: &str) -> bool {
        msg.starts_with(BOT_PREFIX) || msg.starts_with(&format!("@{}", self.username))
    }

}


impl ws::Handler for Bot {

    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        match msg {
            ws::Message::Text(msg) => {
                for part in msg.split_terminator("\r\n") {
                    match irc::Message::parse(part) {
                        Ok(message) => if let Err(err) = self.handle_message(message) {
                            eprintln!("Handling error: {:?}", err);
                        },
                        Err(err) => eprintln!("Parsing error ('{}'): {:?}", part, err)
                    }
                }
            },
            ws::Message::Binary(_) => unimplemented!("Handling raw bytes is not implemented!")
        };

        Ok(())
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        if event.0 < self.channel_to_token.len() {
            use event::NextEvent;
            match self.message_queue.next(event) {
                NextEvent::Ready(Event { mut data, ..}) => {
                    let timeout = self.message_queue.get_min_delay(event);
                    let times_sent = self.message_history.contains(event, &data);

                    if times_sent > 0 {
                        // modify message so it can be sent
                        modify_message(&mut data, times_sent - 1)
                    }

                    let channel = self.token_to_channel.get(&event).expect("No such channel");

                    let message = irc::MessageBuilder::new("PRIVMSG")
                        .with_arg(&format!("#{}", channel))
                        .with_trailing(&data)
                        .string();

                    println!("sending message: {}", message);

                    self.socket.send(message)?;
                    self.socket.timeout(timeout.as_millis() as u64, event)?;
                    self.message_history.push(event, data);
                },
                NextEvent::NotReady(ready_at) => {
                    let timeout = (Instant::now() - ready_at).as_millis() as u64;
                    println!("channel {:?} is not ready, retrying in: {:?} ms", event, timeout);
                    self.socket.timeout(timeout, event)?;
                },
                NextEvent::ChannelIsEmpty => {
                    let timeout = self.message_queue.get_min_delay(event);
                    // println!("channel {:?} is empty, scheduling next poll in: {:?} ms", event, timeout);
                    self.socket.timeout(timeout.as_millis() as u64, event)?;
                },
            }
            return Ok(())
        }

        println!("[system] no handler for token {:?}", event);
        Ok(())
    }

}
