use std::collections::{HashMap, BinaryHeap, VecDeque};
use std::time::{Instant, Duration};
use std::cmp::{Ordering, min};
use std::hash::Hash;

#[derive(Debug)]
pub struct Event<T> {
    pub timestamp: Instant,
    pub ttl: Duration,
    pub data: T,
}

impl <T> PartialEq for Event<T> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp && self.ttl == other.ttl
    }
}

impl <T> Eq for Event<T> {
}

impl <T> PartialOrd for Event<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl <T> Ord for Event<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // note the reversed ordering -- we want the earliest timestamp
        // to be on top of the max heap
        self.timestamp.cmp(&other.timestamp).reverse()
    }
}

/// Represents the result of submitting an event into event queue.
#[derive(Debug)]
pub enum NewEvent {
    Created,
    ChannelNotFound,
}

/// Represents the result of retrieving an event from event queue.
#[derive(Debug)]
pub enum NextEvent<T> {
    /// An event is ready to be processed.
    Ready(Event<T>),
    /// Event is not ready. The earliest it will be ready is provided by argument.
    NotReady(Instant),
    /// There are no events in the channel.
    ChannelIsEmpty,
    /// There is no channel with provided token.
    ChannelNotFound,
}

/// An event channel (or "queue").
struct Channel<Data> {
    queue: BinaryHeap<Event<Data>>,
    min_delay: Duration,
    last_event_ts: Instant,
}

impl <Data> Channel <Data> {

    pub fn new(min_delay: Duration) -> Channel<Data> {
        Channel {
            queue: BinaryHeap::with_capacity(16),
            min_delay,
            last_event_ts: Instant::now() - min_delay
        }
    }

    /// Try to retrieve the next event which is not expired.
    pub fn get_first_non_expired(&mut self) -> NextEvent<Data> {
        match self.queue.peek() {
            Some(event) => {
                let now = Instant::now();
                if event.timestamp + event.ttl < now {
                    // this event has expired, drop it and look for another
                    let _ = self.queue.pop().unwrap();
                    self.get_first_non_expired()
                } else {
                    let next_ready = self.last_event_ts + self.min_delay;
                    if next_ready < now {
                        self.last_event_ts = now;
                        NextEvent::Ready(self.queue.pop().unwrap())
                    } else {
                        NextEvent::NotReady(next_ready)
                    }
                }
            },
            None => NextEvent::ChannelIsEmpty,
        }
    }

}

/// A very simple multi-channel event queue. Not thread-safe.
pub struct MultichannelEventQueue<Token, Data> {
    channels: HashMap<Token, Channel<Data>>,
}

impl<Token, Data> MultichannelEventQueue<Token, Data>
    where
        Token: Hash + Eq + Copy
{
    pub fn new(channels: &HashMap<Token, Duration>) -> MultichannelEventQueue<Token, Data> {
        let now = Instant::now();
        MultichannelEventQueue {
            channels: channels.iter()
                .map(|(tok, conf)| { (*tok, Channel::new(*conf)) })
                .collect(),
        }
    }

    pub fn submit(&mut self, channel: Token, ttl: Duration, data: Data) -> NewEvent {
        match self.channels.get_mut(&channel) {
            Some(channel) => {
                let timestamp = Instant::now();
                channel.queue.push(Event { timestamp, ttl, data });
                NewEvent::Created
            }
            None => NewEvent::ChannelNotFound,
        }
    }

    pub fn next(&mut self, channel: Token) -> NextEvent<Data> {
        match self.channels.get_mut(&channel) {
            Some(channel) => channel.get_first_non_expired(),
            None => NextEvent::ChannelNotFound,
        }
    }

    pub fn get_min_delay(&self, channel: Token) -> Option<Duration> {
        self.channels.get(&channel).map(|ch| ch.min_delay)
    }

    pub fn set_min_delay(&mut self, channel: Token, min_delay: Duration) -> Option<()> {
        self.channels.get_mut(&channel).and_then(|ch| {
            ch.min_delay = min_delay;
            Some(())
        })
    }
}

#[cfg(test)]
mod tests {
    // Unfortunately, there seems to be no easy way of mocking `Instant`, aside from
    // introducing some kind of `TimeProvider` entity to `MultichannelEventQueue`.
    //
    // However, this is not done for now, as it seems possible to test main functionality
    // using the real-world time flow - due to coarse granularity and relaxed requirements
    // on this component
    //
    // TODO introduce TimeProvider or find a way to mock `Instant`

    use super::*;
    use std::thread::sleep;
    use std::ops::Add;

    type Token = u64;
    type Data = &'static str;

    fn make_simple_queue() -> MultichannelEventQueue<Token, Data> {
        let mut channels = HashMap::new();
        channels.insert(1, Duration::from_millis(10));
        MultichannelEventQueue::new(&channels)
    }

    const DEFAULT_TTL: Duration = Duration::from_secs(10);

    #[test]
    pub fn test_execution_order() {
        let mut queue = make_simple_queue();

        queue.submit(1, DEFAULT_TTL, "first");
        queue.submit(1, DEFAULT_TTL, "second");

        match queue.next(1) {
            NextEvent::Ready(evt) => assert_eq!(evt.data, "first"),
            _ => assert!(false, "first event should be Ready")
        };
        sleep(Duration::from_millis(15));
        match queue.next(1) {
            NextEvent::Ready(evt) => assert_eq!(evt.data, "second"),
            _ => assert!(false, "second event should be Ready after 10+ ms")
        };
    }

    #[test]
    pub fn test_early_request_fails() {
        let mut queue = make_simple_queue();

        queue.submit(1, DEFAULT_TTL, "first");
        queue.submit(1, DEFAULT_TTL, "second");

        match queue.next(1) {
            NextEvent::Ready(evt) => assert_eq!(evt.data, "first"),
            _ => assert!(false, "first event should be ready")
        };
        sleep(Duration::from_millis(5));
        match queue.next(1) {
            NextEvent::NotReady(_) => assert!(true),
            _ => assert!(false, "second event should not be ready after 5 ms")
        };
        sleep(Duration::from_millis(5));
        match queue.next(1) {
            NextEvent::Ready(evt) => assert_eq!(evt.data, "second"),
            other => assert!(false, format!("second event should be ready after 10 ms, but got {:?}", other))
        };
    }

    #[test]
    pub fn test_events_expire() {
        let mut queue = make_simple_queue();

        queue.submit(1, Duration::from_millis(10), "first");
        queue.submit(1, Duration::from_millis(10), "second");
        queue.submit(1, Duration::from_millis(10), "third");

        sleep(Duration::from_millis(10).add(Duration::from_nanos(10)));

        match queue.next(1) {
            NextEvent::ChannelIsEmpty => assert!(true),
            _ => assert!(false, "channel should be empty after all events have expired")
        };
    }

}
