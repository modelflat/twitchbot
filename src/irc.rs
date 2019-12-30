use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::pin::Pin;

type Tags<'a> = BTreeMap<&'a str, Option<&'a str>>;

/// Prefix part of an IRC message. Roughly corresponds to what is meant by "prefix"
/// in RFC1459 (see `Message` description for more info)
#[derive(Debug)]
pub enum Prefix<'a> {
    Full {
        nick: &'a str,
        user: &'a str,
        host: &'a str,
    },
    UserHost {
        user: &'a str,
        host: &'a str,
    },
    Host(&'a str),
    None,
}

impl Default for Prefix<'_> {
    fn default() -> Self {
        Prefix::None
    }
}

impl Display for Prefix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Prefix::Full { nick, user, host } => f.write_fmt(format_args!(":{}!{}@{}", nick, user, host)),
            Prefix::UserHost { user, host } => f.write_fmt(format_args!(":{}@{}", user, host)),
            Prefix::Host(host) => f.write_fmt(format_args!(":{}", host)),
            Prefix::None => Ok(()),
        }
    }
}

/// Command part of an IRC message. Includes command itself and all the arguments,
/// except trailing. Roughly corresponds to what is meant by "command" and "params"
/// in RFC1459 (see `Message` description for more info)
#[derive(Debug)]
pub struct Command<'a> {
    pub name: &'a str,
    pub args: Vec<&'a str>,
}

impl Default for Command<'_> {
    fn default() -> Self {
        Command {
            name: "",
            args: Vec::default(),
        }
    }
}

impl Display for Command<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.args.is_empty() {
            f.write_str(self.name)
        } else {
            f.write_fmt(format_args!("{} {}", self.name, self.args.join(" ")))
        }
    }
}

/// Structure representing a message in IRC chat.
///
/// Note, that this structure and the parser code do not precisely implement all of the
/// corresponding IRC RFCs. For example, nothing special is done wrt vendor-specific tags,
/// i.e. they are handled the same way as all the other keys. Some of the restrictions RFC
/// places on entities such as usernames, tag keys/values, prefixes, hostnames, etc.
/// may or may not not enforced.
#[derive(Debug)]
pub struct Message<'a> {
    pub tags: Tags<'a>,
    pub prefix: Prefix<'a>,
    pub command: Command<'a>,
    pub trailing: Option<&'a str>,
}

impl Message<'_> {
    /// Parses a string into Twitch IRC message.
    pub fn parse<'a>(raw: &'a str) -> Result<Message<'a>, Box<dyn Error>> {
        fn parse_tags(raw_tags: &str) -> Tags {
            // tags are conveniently separated by a semicolon
            let mut tags = Tags::new();
            for pair in raw_tags.split(';') {
                if !pair.is_empty() {
                    let mut iter = pair.splitn(2, '=');
                    let key = iter.next().unwrap();
                    let val = iter.next();
                    tags.insert(key, val);
                }
            }
            tags
        }

        fn parse_prefix(prefix: &str) -> Prefix {
            // we support three types of prefix: Full, UserHost, and Host
            // full is a prefix of form <nick>!<user>@<host>
            // user-host is a prefix of form <user>@<host>
            // host is simply a <host>
            let mut iter = prefix.rsplitn(2, '@');
            let host = iter.next().unwrap();
            match iter.next() {
                Some(nick_and_user) => {
                    let mut iter = nick_and_user.rsplitn(2, '!');
                    let user = iter.next().unwrap();
                    match iter.next() {
                        Some(nick) => Prefix::Full { nick, user, host },
                        None => Prefix::UserHost { user, host },
                    }
                }
                None => Prefix::Host(host),
            }
        }

        let mut message = Message::default();
        let mut raw = raw;

        if raw.chars().next().ok_or("Unexpected end of input")? == '@' {
            // the next space will designate end of IRCv3 tags
            let tag_end = raw.find(' ').ok_or("Unexpected end of input")?;

            message.tags = parse_tags(&raw[1..tag_end]);

            raw = &raw[tag_end + 1..];
        }

        if raw.chars().next().ok_or("Unexpected end of input")? == ':' {
            // the next space will designate end of IRC prefix
            let prefix_end = raw.find(' ').ok_or("Unexpected end of input")?;

            message.prefix = parse_prefix(&raw[1..prefix_end]);

            raw = &raw[prefix_end + 1..];
        }

        let mut command_and_params = match raw.find(" :") {
            Some(idx) => {
                // we found the trailing part
                let (raw, trailing) = raw.split_at(idx);
                message.trailing = Some(&trailing[2..]);
                raw
            }
            None => {
                // no trailing part in message
                raw
            }
        }
        .split(' ');

        message.command.name = command_and_params.next().ok_or("Command expected")?;
        message.command.args = command_and_params.filter(|x| !x.is_empty()).collect();

        Ok(message)
    }

    pub fn tag_value(&self, key: &str) -> Option<&str> {
        *self.tags.get(key)?
    }

    pub fn first_arg_as_channel_name(&self) -> Option<&str> {
        self.command.args.first().map(|s| s.trim_start_matches('#'))
    }

    /// Splits the trailing into two parts - before the first space character and after.
    pub fn arg_split(&self) -> (&str, Option<&str>) {
        match self.trailing {
            Some(s) => match s.find(' ') {
                Some(n) => {
                    let (l, r) = s.split_at(n);
                    (l, Some(r))
                }
                None => (s, None),
            },
            None => ("", None),
        }
    }
}

impl Default for Message<'_> {
    fn default() -> Self {
        Message {
            tags: Tags::default(),
            prefix: Prefix::default(),
            command: Command::default(),
            trailing: None,
        }
    }
}

impl Display for Message<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if !self.tags.is_empty() {
            f.write_str("@")?;

            for (i, (k, v)) in self.tags.iter().enumerate() {
                match v {
                    Some(val) => f.write_fmt(format_args!("{}={}", k, val))?,
                    None => f.write_str(k)?,
                };
                if i < self.tags.len() - 1 {
                    f.write_str(";")?;
                }
            }

            f.write_str(" ")?;
        }

        match &self.prefix {
            Prefix::None => {}
            prefix => {
                prefix.fmt(f)?;
                f.write_str(" ")?;
            }
        }

        self.command.fmt(f)?;

        if let Some(trailing) = self.trailing {
            f.write_fmt(format_args!(" :{}", trailing))?;
        }

        Ok(())
    }
}

pub struct MessageBuilder<'a> {
    message: Message<'a>,
}

impl<'a> MessageBuilder<'a> {
    pub fn new(command_name: &'a str) -> MessageBuilder {
        MessageBuilder {
            message: {
                let mut msg = Message::default();
                msg.command.name = command_name;
                msg
            },
        }
    }

    pub fn with_arg(&'a mut self, arg: &'a str) -> &'a mut MessageBuilder {
        self.message.command.args.push(arg);
        self
    }

    pub fn with_tag(&'a mut self, key: &'a str, value: Option<&'a str>) -> &'a mut MessageBuilder {
        self.message.tags.insert(key, value);
        self
    }

    pub fn with_prefix(&'a mut self, prefix: Prefix<'a>) -> &'a mut MessageBuilder {
        self.message.prefix = prefix;
        self
    }

    pub fn with_trailing(&'a mut self, trailing: &'a str) -> &'a mut MessageBuilder {
        self.message.trailing = Some(trailing);
        self
    }

    pub fn string(&mut self) -> String {
        format!("{}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate test;
    use test::Bencher;

    #[test]
    fn test_msg_parse() {
        let parsed = Message::parse("CAP LS").expect("Failed to parse message");
        assert_eq!(parsed.command.name, "CAP");
        assert_eq!(parsed.command.args.len(), 1);
        assert_eq!(parsed.command.args.first().unwrap(), &"LS");
    }

    #[test]
    fn test_msg_parse_with_host_prefix() {
        let parsed = Message::parse(":host.com CAP LS").expect("Failed to parse message");
        match parsed.prefix {
            Prefix::Host(host) => {
                assert_eq!(host, "host.com");
            }
            _ => assert!(false),
        };
    }

    #[test]
    fn test_msg_parse_with_full_prefix() {
        let parsed = Message::parse(":nick!user@host.com CAP LS").expect("Failed to parse message");
        match parsed.prefix {
            Prefix::Full { nick, user, host } => {
                assert_eq!(nick, "nick");
                assert_eq!(user, "user");
                assert_eq!(host, "host.com");
            }
            _ => assert!(false),
        };
    }

    #[test]
    fn test_msg_parse_single_tag() {
        let parsed = Message::parse("@aaa=a_value :host.com CAP LS").expect("Failed to parse message");
        assert!(!parsed.tags.is_empty());
        assert_eq!(
            parsed.tags.get("aaa").expect("Expected key is not present").unwrap(),
            "a_value"
        );
    }

    #[test]
    fn test_msg_parse_multiple_tags() {
        let parsed = Message::parse("@a=a_value;b;c=c_value :host.com CAP LS").expect("Failed to parse message");
        assert!(!parsed.tags.is_empty());
        assert_eq!(
            parsed.tags.get("a").expect("Expected key is not present").unwrap(),
            "a_value"
        );
        assert!(parsed.tags.get("b").expect("Expected key is not present").is_none());
        assert_eq!(
            parsed.tags.get("c").expect("Expected key is not present").unwrap(),
            "c_value"
        );
    }

    #[test]
    fn test_msg_parse_trailing() {
        let parsed = Message::parse(":host.com CAP LS :trailing").expect("Failed to parse message");
        assert_eq!(parsed.trailing.expect("Trailing should not be None"), "trailing");
    }

    #[test]
    fn test_msg_build_simple() {
        let message = MessageBuilder::new("CAP")
            .with_arg("arg1")
            .with_arg("arg2")
            .with_trailing("message")
            .with_prefix(Prefix::Host("tmi.twitch.tv"))
            .with_tag("color", Some("blue"))
            .string();

        assert_eq!(message, "@color=blue :tmi.twitch.tv CAP arg1 arg2 :message");
    }

    #[test]
    fn test_msg_build_tags_are_properly_constructed() {
        let message = MessageBuilder::new("CAP")
            .with_trailing("message")
            .with_prefix(Prefix::Host("tmi.twitch.tv"))
            .with_tag("ak", Some("av"))
            .with_tag("bk", Some("bv"))
            .with_tag("ck", None)
            .string();

        let tags = Message::parse(&message).expect("message is unparseable").tags;

        assert_eq!(tags.get("ak").expect("no key ak").expect("should have value"), "av");
        assert_eq!(tags.get("bk").expect("no key bk").expect("should have value"), "bv");
        assert!(tags.get("ck").expect("no key ck").is_none());
    }

    #[bench]
    fn bench_msg_parse_simple(b: &mut Bencher) {
        b.iter(|| Message::parse("CAP LS").expect("Failed to parse message"));
    }

    #[bench]
    fn bench_msg_parse_complex(b: &mut Bencher) {
        let message = "@color=;user-id=123123123;badge-info=;emotes=;display-name=adasdasdasdaaaaa;\
        room-id=123123123;subscriber=0;turbo=0;badges=;flags=;user-type=;wow-such-a-tag;+asdasdada;\
        room-id=123123123;subscriber=0;turbo=0;badges=;flags=;user-type=;wow-such-a-tag;+asdasdada;\
        id=XXXXXXXX-XXXX-XXXX-XXXX-23123123;mod=0;tmi-sent-ts=219319231;vendor.com/key=21931923123 \
        :user!useruserus@user.very.very.very.very.very.very.very.very.very.very.very.long.hostname \
        PRIVMSG #channel argument argument argument argument argument argument argument argument11 \
        :asdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdas dasdasdasdadasdasd\
        asdasdasdasdasdasd asdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdadasdasda\
        asdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasdasda sdasdasdassdasdasda";

        b.iter(|| Message::parse(message).expect("Failed to parse message"));
    }
}
