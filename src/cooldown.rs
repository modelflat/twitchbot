use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use chashmap::ReadGuard;

pub enum CooldownState {
    Ready,
    NotReady(Duration),
}

pub struct CooldownData {
    value: Duration,
    last_accessed: RwLock<Instant>,
}

impl CooldownData {
    pub fn new(cooldown: Duration, reset: bool) -> CooldownData {
        CooldownData {
            value: cooldown,
            last_accessed: RwLock::new(if reset {
                Instant::now() - cooldown
            } else {
                Instant::now()
            }),
        }
    }

    /// Tries to reset this cooldown.
    pub fn try_reset(&self) -> CooldownState {
        let now = Instant::now();
        let mut last_accessed = self
            .last_accessed
            .write()
            .expect("lock is poisoned, but this shouldn't have happened");

        let when_reset = *last_accessed + self.value;

        if when_reset >= now {
            return CooldownState::NotReady(when_reset - now);
        }

        *last_accessed = now;

        CooldownState::Ready
    }

    pub fn cooldown(&self) -> CooldownState {
        let now = Instant::now();
        let last_accessed = self
            .last_accessed
            .read()
            .expect("lock is poisoned, but this shouldn't have happened");

        let when_reset = *last_accessed + self.value;

        if when_reset >= now {
            return CooldownState::NotReady(when_reset - now);
        }

        CooldownState::Ready
    }

    pub fn is_cooldown(&self) -> bool {
        match self.cooldown() {
            CooldownState::Ready => false,
            CooldownState::NotReady(_) => true,
        }
    }
}

pub struct CooldownTracker<K>
where
    K: Hash + PartialEq,
{
    // TODO figure out:
    // do locks in this map affect asynchronous model of execution?
    cooldown_map: chashmap::CHashMap<K, CooldownData>,
}

impl<K> CooldownTracker<K>
where
    K: Hash + PartialEq,
{
    pub fn new(init: HashMap<K, Duration>) -> CooldownTracker<K> {
        CooldownTracker {
            cooldown_map: init
                .into_iter()
                .map(|(channel, cooldown)| (channel, CooldownData::new(cooldown, true)))
                .collect(),
        }
    }

    /// Accesses cooldown state.
    ///
    /// If no cooldown happens right now, CooldownState::Ready is returned, and the
    /// state is reset (i.e. cooldown is triggered).
    /// If there is a cooldown, CooldownState::NotReady is returned.
    pub fn access(&self, channel: &K) -> Option<CooldownState> {
        self.cooldown_map.get(channel).map(|state| match state.cooldown() {
            CooldownState::Ready => state.try_reset(),
            not_ready => not_ready,
        })
    }

    pub fn access_raw(&self, channel: &K) -> Option<ReadGuard<K, CooldownData>> {
        self.cooldown_map.get(channel)
    }

    pub fn contains(&self, channel: &K) -> bool {
        self.cooldown_map.contains_key(channel)
    }

    /// Updates channel cooldown to a new value.
    pub fn update(&self, channel: &K, new_cooldown: Duration) {
        if let Some(mut state) = self.cooldown_map.get_mut(channel) {
            state.value = new_cooldown;
        }
    }

    /// Adds a new channel to tracker.
    pub fn add_channel(&self, channel: K, cooldown: Duration, reset: bool) {
        self.cooldown_map.insert(channel, CooldownData::new(cooldown, reset));
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_cooldown_is_tracked() {
        let channel = 1;

        let tracker = CooldownTracker::<i32>::new({
            let mut map = HashMap::new();
            map.insert(channel, Duration::from_millis(10));
            map
        });

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(true),
            Some(CooldownState::NotReady(_)) => assert!(false, "untouched channel should not be on cooldown"),
            None => assert!(false, "channel was not added to tracker"),
        }

        std::thread::sleep(Duration::from_millis(5));

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(false, "cooldown shouldn't have passed yet"),
            Some(CooldownState::NotReady(duration)) => assert!(
                duration <= Duration::from_millis(5),
                "at least 5 ms should have already passed"
            ),
            None => assert!(false, "channel was not added to tracker"),
        }

        std::thread::sleep(Duration::from_millis(5));

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(true),
            Some(CooldownState::NotReady(_)) => assert!(false, "cooldown should have already passed"),
            None => assert!(false, "channel was not added to tracker"),
        }
    }

    #[test]
    fn test_cooldown_can_be_updated() {
        let channel = 1;

        let tracker = CooldownTracker::<i32>::new({
            let mut map = HashMap::new();
            map.insert(channel, Duration::from_millis(10));
            map
        });

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(true),
            _ => assert!(false),
        }

        std::thread::sleep(Duration::from_millis(5));

        match tracker.access(&channel) {
            Some(CooldownState::NotReady(duration)) => assert!(
                duration <= Duration::from_millis(5),
                "at least 5 ms should have already passed"
            ),
            _ => assert!(false),
        }

        std::thread::sleep(Duration::from_millis(5));

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(true),
            _ => assert!(false),
        }

        std::thread::sleep(Duration::from_millis(15));

        tracker.update(&channel, Duration::from_millis(20));

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(
                false,
                "readiness of channel should be affected immediately after update"
            ),
            Some(CooldownState::NotReady(_)) => assert!(true),
            None => assert!(false, "channel was lost"),
        }

        std::thread::sleep(Duration::from_millis(5));

        match tracker.access(&channel) {
            Some(CooldownState::Ready) => assert!(true),
            Some(CooldownState::NotReady(_)) => assert!(false, "channel should be ready by this time"),
            None => assert!(false, "channel was lost"),
        }
    }
}
