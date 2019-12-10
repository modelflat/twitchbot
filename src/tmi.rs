use std::collections::HashMap;
use std::error::Error;

use ws;

use super::irc;

use phf::phf_map;


#[derive(Clone)]
pub enum Command {
    PrivMsg,
    RoomState,
}

static COMMANDS: phf::Map<&'static str, Command> = phf_map! {
    "PRIVMSG" => Command::PrivMsg,
    "ROOMSTATE" => Command::RoomState,
};


pub struct Client {
    socket: ws::Sender,
    channels: Vec<String>,
}


impl Client {

    pub fn new(socket: ws::Sender, username: &str, password: &str, channels: Vec<String>)
        -> ws::Result<Client> {
        socket.send(format!("PASS oauth:{}", password))?;
        socket.send(format!("NICK {}", username))?;

        socket.send("CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership")?;

        for channel in &channels {
            socket.send(format!(":{user}!{user}@{user}.tmi.twitch.tv JOIN #{channel}",
                                user = username, channel = channel))?;
        }

        return Ok(Client { socket, channels })
    }

    fn handle_message<'a>(&mut self, msg: irc::Message<'a>) -> Result<(), Box<dyn Error>> {
        if let Some(command) = COMMANDS.get(msg.command.name) {
            match command {
                Command::PrivMsg => {
                    let channel = msg.command.args.first().ok_or("PRIVMSG: not enough arguments")?;
                    println!("[{}] {}: {}",
                             channel,
                             msg.display_name().unwrap_or(msg.user_id().unwrap_or("")),
                             msg.trailing.unwrap_or(""));
                },
                Command::RoomState => {
                    let channel = msg.command.args.first().ok_or("ROOMSTATE: not enough arguments")?;
                    println!("Got ROOMSTATE for channel {}: {:?}", channel, msg.tags);
                }
            }
        } else {
            println!("(unknown command) {}", msg);
        }

        Ok(())
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

}
