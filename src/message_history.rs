use std::collections::{HashMap, VecDeque};
use std::time::{Instant, Duration};
use std::hash::Hash;


pub struct HistoryEntry {
    ts: Instant,
    msg: String,
    times_found: usize,
}

// TODO improve this struct
// this is a prototype that is far from optimal
// ideally we don't need to store actual messages -- can just check
// hashes or something like this
pub struct LastMessages<Token> {
    messages: HashMap<Token, VecDeque<HistoryEntry>>,
    ttl: Duration,
}

impl <Token> LastMessages <Token>
where Token: Hash + Eq
{

    pub fn new(channel_tokens: Vec<Token>, ttl: Duration) -> LastMessages<Token> {
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
