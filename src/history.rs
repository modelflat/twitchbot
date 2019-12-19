use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use async_std::sync::RwLock;

pub struct HistoryEntry<Data> {
    timestamp: Instant,
    data: Data,
    times_found: usize,
}

// TODO improve this struct
// this is a prototype that is far from optimal
// ideally we don't need to store actual messages -- can just check
// hashes or something like this
pub struct History<Data> {
    channels: HashMap<String, RwLock<VecDeque<HistoryEntry<Data>>>>,
    ttl: Duration,
}

impl<Data> History<Data>
where
    Data: Eq,
{
    pub fn new(channels: Vec<String>, ttl: Duration) -> History<Data> {
        History {
            channels: channels
                .into_iter()
                .map(|c| (c, RwLock::new(VecDeque::new())))
                .collect(),
            ttl,
        }
    }

    /// Adds item to a channel's queue.
    pub async fn push(&self, channel: &str, data: Data) {
        if let Some(lock) = self.channels.get(channel) {
            let mut queue = lock.write().await;
            queue.push_back(HistoryEntry {
                timestamp: Instant::now(),
                data,
                times_found: 0,
            });
        }
    }

    /// Checks if a given message is present in the history.
    /// All messages that are too old are removed from the queue.
    ///
    /// The number of items this message was searched for and found is returned.
    pub async fn contains(&self, channel: &str, data: &Data) -> Option<usize> {
        let ttl = self.ttl;
        if let Some(lock) = self.channels.get(channel) {
            let now = Instant::now();

            let mut queue = lock.write().await;

            while let Some(HistoryEntry { timestamp, .. }) = queue.front() {
                if *timestamp + ttl < now {
                    let _ = queue.pop_front().unwrap();
                } else {
                    break;
                }
            }

            return queue
                .iter_mut()
                .find(|d| d.data == *data)
                .map(|data| {
                    data.times_found += 1;
                    data.times_found
                })
                .or(Some(0));
        }
        None
    }
}
