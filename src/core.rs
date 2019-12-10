use std::collections::HashMap;
use std::error::Error;

use ws;
use ws::util::Token;

use phf::phf_map;

use super::irc;
use std::time::{Instant, Duration};

use std::sync::atomic::{AtomicUsize, Ordering};


const BOT_PREFIX: &str = "<<";

const MESSAGE_RANGE_START: usize = 1 << 0;
const MESSAGE_RANGE_END: usize = 1 << 32;


// TODO this module should be refactored as I get insight into async model of ws/mio/rust


#[derive(Clone)]
pub enum Command {
    PrivMsg,
    Ping,
}

static COMMANDS: phf::Map<&'static str, Command> = phf_map! {
    "PRIVMSG" => Command::PrivMsg,
    "PING" => Command::Ping,
};

#[derive(Debug)]
struct ChannelTimeouts {
    last_message_sent: Instant,
    min_delay: Duration,
}


impl ChannelTimeouts {

    pub fn update(&mut self) {
        self.last_message_sent = Instant::now();
    }

    pub fn min_delay_as_of_now(&self) -> u64 {
        let diff = (Instant::now() - self.last_message_sent);
        println!("{:?}", self.last_message_sent);
        if diff > self.min_delay {
            0
        } else {
            (self.min_delay.as_millis() - diff.as_millis()) as u64
        }
    }

}


impl Default for ChannelTimeouts {
    fn default() -> Self {
        ChannelTimeouts {
            last_message_sent: Instant::now() - Duration::from_secs(1),
            min_delay: Duration::from_secs(1),
        }
    }
}


pub struct Client {
    socket: ws::Sender,
    username: String,
    channels: Vec<String>,
    timeouts: HashMap<String, ChannelTimeouts>,
    messages_to_send: HashMap<Token, (String, String)>,
    message_token_provider: AtomicUsize,
}


impl Client {

    pub fn new(socket: ws::Sender, username: &str, password: &str, channels: Vec<String>) -> ws::Result<Client> {
        let timeouts = channels.iter().map(|ch| (ch.clone(), ChannelTimeouts::default())).collect();

        let mut client = Client {
            socket,
            username: username.to_string(),
            channels,
            timeouts,
            messages_to_send: HashMap::new(),
            message_token_provider: AtomicUsize::new(MESSAGE_RANGE_START)
        };

        client.login(username, password)?;
        client.join()?;

        return Ok(client)
    }

    fn handle_message<'a>(&mut self, msg: irc::Message<'a>) -> Result<(), Box<dyn Error>> {
        if let Some(command) = COMMANDS.get(msg.command.name) {
            // this should compile to a jump table
            match command {
                Command::PrivMsg => {
                    let channel = msg.command.args.first().ok_or("PRIVMSG: not enough arguments")?;

                    let timestamp: u64 = msg.tag_value("tmi-sent-ts")
                        .ok_or("no timestamp on message")?
                        .parse()?;

                    let username = msg.tag_value("display-name")
                        .ok_or("no display name set")?;

                    let message = msg.trailing.unwrap_or("");

                    if self.is_bot_command(message) {
                        println!("COMMAND! {}", msg);
                        self.send(channel, &format!("echo! {}", message))?;
                    } else {
                        println!("[{}] [{}] {}: {}", timestamp, channel, username, message);
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

    fn send(&mut self, channel: &str, text: &str) -> ws::Result<()> {
        let message = irc::MessageBuilder::new("PRIVMSG")
            .with_arg(channel)
            .with_trailing(text)
            .string();
        let token = self.allocate_message_token();

        let channel = channel.trim_start_matches('#');

        self.messages_to_send.insert(token, (channel.to_owned(), message));

        let timeout = self.timeouts
            .get(channel)
            .unwrap_or(&ChannelTimeouts::default())
            .min_delay_as_of_now();

        println!("[system] scheduled message {:?} to be sent in {} ms", token, timeout);

        self.socket.timeout(timeout, token)
    }

    fn join(&mut self) -> ws::Result<()> {
        for channel in &self.channels {
            self.socket.send(format!("JOIN #{}", channel))?;
        }
        Ok(())
    }

    fn is_bot_command(&self, msg: &str) -> bool {
        msg.starts_with(BOT_PREFIX) || msg.starts_with(&format!("@{}", self.username))
    }

    fn allocate_message_token(&self) -> Token {
        Token(self.message_token_provider.fetch_add(1, Ordering::SeqCst))
    }

    fn is_message_event(&self, token: Token) -> bool {
        MESSAGE_RANGE_START <= token.0 && token.0 < MESSAGE_RANGE_END
    }

}


impl ws::Handler for Client {

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
        match event {
            message if self.is_message_event(event) => {
                match self.messages_to_send.remove(&message) {
                    Some((channel, message)) => {
                        let mut should_reschedule = false;

                        self.timeouts.entry(channel)
                            .and_modify(|time| {
                                time.update();
                            })
                            .or_default();

                        self.socket.send(message)?;

                        println!("[system] {:?}", &self.timeouts);
                        println!("[system] message with token {:?} was sent", event);
                    },
                    None => println!("[system] message with token {:?} not found", event),
                }
            }
            _ => {
                println!("[system] unknown event {:?}", event);
            }
        };
        Ok(())
    }

}
