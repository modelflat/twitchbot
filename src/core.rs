use std::collections::{HashMap, VecDeque};
use std::error::Error;

use ws;
use ws::util::Token;

use phf::phf_map;

use super::irc;
use super::event;

use std::time::{Instant, Duration};

use std::sync::atomic::{AtomicUsize, Ordering};
use crate::event::{MultichannelEventQueue, Event};
use pest::error::ErrorVariant;


const BOT_PREFIX: &str = ">>";

const BOT_MESSAGE_TTL: Duration = Duration::from_secs(20);

const BOT_MESSAGE_HISTORY_TTL: Duration = Duration::from_secs(30);

const BOT_CHANNEL_TIMEOUT: Duration = Duration::from_millis(2000);


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

struct HistoryEntry {
    ts: Instant,
    msg: String,
    times_found: usize,
}

// TODO improve this struct and extract into separate module
// this is a prototype that is far from optimal
// ideally we don't need to store actual messages -- can just check
// hashes or something like this
struct LastMessages {
    messages: HashMap<Token, VecDeque<HistoryEntry>>,
    ttl: Duration,
}

impl LastMessages {

    fn new(channel_tokens: Vec<Token>, ttl: Duration) -> LastMessages {
        LastMessages {
            messages: channel_tokens.into_iter().map(|c| (c, VecDeque::new())).collect(),
            ttl
        }
    }

    /// Adds message to a channel's queue.
    pub fn push(&mut self, channel: Token, message: String) -> Option<()> {
        self.messages.get_mut(&channel).map(|queue| queue.push_back(
            HistoryEntry { ts: Instant::now(), msg: message, times_found: 0 }
        ))
    }

    /// Checks if a given message is present in the history.
    /// All messages that are too old are removed from the queue.
    ///
    /// The number of items this message was searched for and found is returned.
    pub fn has_message(&mut self, channel: Token, message: &str) -> Option<usize> {
        let ttl = self.ttl;
        self.messages.get_mut(&channel).map(|queue| {
            let now = Instant::now();
            while let Some(HistoryEntry { ts, .. }) = queue.front() {
                if *ts + ttl < now {
                    let _ = queue.pop_front().unwrap();
                } else {
                    break;
                }
            }

            queue.iter_mut()
                .find(|msg| msg.msg == message)
                .map(|msg| {
                    msg.times_found += 1;
                    msg.times_found
                })
                .unwrap_or(0)
        })
    }

}

// TODO move to util module
fn modify_message(message: &mut String, n: usize) {
    const SUFFIX: [char; 4] = ['\u{e0000}', '\u{e0002}', '\u{e0003}', '\u{e0004}'];

    if n < SUFFIX.len() {
        message.push(SUFFIX[n]);
    } else {
        // in this case, we could use the power of combinatorics to append several
        // chars to message. 4^4 possible combinations should have us covered.
    }
}


pub struct Bot {
    socket: ws::Sender,
    username: String,
    channels: Vec<String>,

    // TODO is there a better way to keep those
    channel_to_token: HashMap<String, Token>,
    token_to_channel: HashMap<Token, String>,

    message_queue: MultichannelEventQueue<Token, String>,
    message_history: LastMessages,
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
            message_history: LastMessages::new(channel_tokens, BOT_MESSAGE_HISTORY_TTL),
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
                        self.send(channel, &format!("echo! {}", message));
                        self.send(channel, &format!("echo! {}", message));
                        self.send(channel, &format!("echo! {}", message));
                        self.send(channel, &format!("echo! {}", message));
                        self.send(channel, &format!("echo! {}", message));
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

    fn send(&mut self, channel: &str, text: &str) {
        self.message_queue.submit(
            *self.channel_to_token.get(channel).expect("channel not registered"),
            BOT_MESSAGE_TTL,
            text.to_owned()
        );
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
                    match irc::Message::parse_fast(part) {
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
                    let timeout = self.message_queue.get_min_delay(event).unwrap();
                    let times = self.message_history.has_message(event, &data)
                        .expect("no history for channel");

                    if times > 0 {
                        // modify message so it can be sent
                        modify_message(&mut data, times - 1)
                    }

                    let channel = self.token_to_channel.get(&event)
                        .expect("no such channel"); // TODO there are many checks like this one, simplify?

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
                    let timeout = self.message_queue.get_min_delay(event).unwrap();
                    // println!("channel {:?} is empty, scheduling next poll in: {:?} ms", event, timeout);
                    self.socket.timeout(timeout.as_millis() as u64, event)?;
                },
                NextEvent::ChannelNotFound => {
                    panic!("channel for token {:?} is not registered in message queue", event)
                },
            }
            return Ok(())
        }

        println!("[system] no handler for token {:?}", event);
        Ok(())
    }

}
