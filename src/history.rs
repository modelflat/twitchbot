use std::collections::{HashMap, VecDeque};
use std::time::{Instant, Duration};
use std::hash::Hash;


pub struct HistoryEntry<Data> {
    timestamp: Instant,
    data: Data,
    times_found: usize,
}

// TODO improve this struct
// this is a prototype that is far from optimal
// ideally we don't need to store actual messages -- can just check
// hashes or something like this
pub struct History<Token, Data> {
    channels: HashMap<Token, VecDeque<HistoryEntry<Data>>>,
    ttl: Duration,
}

impl <Token, Data> History<Token, Data>
where
    Token: Hash + Eq,
    Data: Eq,
{

    pub fn new(channel_tokens: Vec<Token>, ttl: Duration) -> History<Token, Data> {
        History {
            channels: channel_tokens.into_iter().map(|c| (c, VecDeque::new())).collect(),
            ttl
        }
    }

    /// Adds item to a channel's queue.
    pub fn push(&mut self, channel: Token, data: Data) -> Option<()> {
        self.channels.get_mut(&channel).map(|queue| queue.push_back(
            HistoryEntry { timestamp: Instant::now(), data, times_found: 0 }
        ))
    }

    /// Checks if a given message is present in the history.
    /// All messages that are too old are removed from the queue.
    ///
    /// The number of items this message was searched for and found is returned.
    pub fn contains(&mut self, channel: Token, data: &Data) -> Option<usize> {
        let ttl = self.ttl;
        self.channels.get_mut(&channel).map(|queue| {
            let now = Instant::now();
            while let Some(HistoryEntry { timestamp, .. }) = queue.front() {
                if *timestamp + ttl < now {
                    let _ = queue.pop_front().unwrap();
                } else {
                    break;
                }
            }

            queue.iter_mut()
                .find(|d| d.data == *data)
                .map(|data| {
                    data.times_found += 1;
                    data.times_found
                })
                .unwrap_or(0)
        })
    }

}

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_message_can_be_discovered() {
        let mut history = History::new(vec![1], Duration::from_millis(10));

        history.push(1, "message".to_string()).unwrap();

        assert_eq!(history.contains(1, &"message".to_string()).unwrap(), 1);
    }

    #[test]
    fn test_non_existant_message() {
        let mut history = History::new(vec![1], Duration::from_millis(10));

        assert_eq!(history.contains(1, &"message".to_string()).unwrap(), 0);
    }

    #[test]
    fn test_message_expires() {
        let mut history = History::new(vec![1], Duration::from_millis(10));

        history.push(1, "message".to_string()).unwrap();

        sleep(Duration::from_millis(10));

        assert_eq!(history.contains(1, &"message".to_string()).unwrap(), 0);
    }

}
