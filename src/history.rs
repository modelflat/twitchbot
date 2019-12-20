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

#[cfg(test)]
mod tests {

    use super::*;
    use futures::task::SpawnExt;

    macro_rules! async_test {
        ($b:block) => {
            let mut pool = futures::executor::LocalPool::new();
            pool.spawner().spawn((async move || $b)()).unwrap();
            pool.run();
        };
    }

    #[test]
    fn test_missing_item() {
        async_test!({
            let channel = "test".to_string();
            let history = History::new(vec![channel.clone()], Duration::from_secs(1));

            match history.contains(&channel, &"message".to_string()).await {
                Some(0) => assert!(true),
                Some(_) => assert!(false, "message is found, but was never inserted"),
                None => assert!(false, "channel was lost"),
            }
        });
    }

    #[test]
    fn test_item_can_be_found() {
        async_test!({
            let channel = "test".to_string();
            let history = History::new(vec![channel.clone()], Duration::from_secs(1));

            history.push(&channel, "message".to_string()).await;

            match history.contains(&channel, &"message".to_string()).await {
                Some(0) => assert!(false, "message was inserted, but cannot be found"),
                Some(_) => assert!(true),
                None => assert!(false, "channel was lost"),
            }
        });
    }

    #[test]
    fn test_number_of_times_item_was_found_is_tracked() {
        async_test!({
            let channel = "test".to_string();
            let history = History::new(vec![channel.clone()], Duration::from_secs(1));

            history.push(&channel, "message".to_string()).await;

            match history.contains(&channel, &"message".to_string()).await {
                Some(1) => assert!(true),
                Some(n) => assert!(
                    false,
                    "item was searched for for the first time, \
                    but history says it was found {} times",
                    n
                ),
                None => assert!(false, "message was lost in history"),
            }

            match history.contains(&channel, &"message".to_string()).await {
                Some(2) => assert!(true),
                Some(n) => assert!(
                    false,
                    "item was searched for for the second time, \
                    but history says it was found {} times",
                    n
                ),
                None => assert!(false, "message was lost in history"),
            }
        });
    }

    #[test]
    fn test_items_expire_according_to_ttl() {
        async_test!({
            let channel = "test".to_string();
            let history = History::new(vec![channel.clone()], Duration::from_millis(10));

            history.push(&channel, "message".to_string()).await;

            std::thread::sleep(Duration::from_millis(10));

            match history.contains(&channel, &"message".to_string()).await {
                Some(0) => assert!(true),
                Some(_) => assert!(false, "item should have already expired"),
                None => assert!(true),
            }
        });
    }
}
