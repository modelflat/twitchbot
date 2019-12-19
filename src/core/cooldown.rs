use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_std::sync::RwLock;
use std::hash::Hash;
use chashmap::WriteGuard;

pub enum CooldownState {
    Ready,
    NotReady(Duration),
}

pub struct CooldownData {
    value: Duration,
    last_accessed: Instant,
}

impl CooldownData {

    pub fn new(cooldown: Duration, reset: bool) -> CooldownData {
        CooldownData {
            value: cooldown, last_accessed: if reset {
                Instant::now() - cooldown
            } else {
                Instant::now()
            }
        }
    }

    /// Tries to reset this cooldown.
    pub fn try_reset(&mut self) -> CooldownState {
        let now = Instant::now();
        if self.last_accessed + self.value >= now {
            CooldownState::NotReady(self.last_accessed + self.value - now)
        } else {
            self.last_accessed = now;
            CooldownState::Ready
        }
    }

    #[inline]
    pub fn cooldown(&self) -> bool {
        self.last_accessed + self.value >= Instant::now()
    }

}

/// Simple cooldown tracker.
pub struct CooldownTracker {
    // TODO can we get rid of locks/optimize them?
    // there are two locks in here: external and internal
    // external is needed because we support adding/removing channels
    // internal is needed because of the main functionality of CooldownTracker
    //
    // internal lock is locked ONLY through .write() (consider this when optimising)
    //
    // external lock can be locked for both writes and reads, but writes are expected to occur
    // much more rarely than reads (therefore RwLock seems like a perfect fit for this task)
    cooldown_map: RwLock<HashMap<String, RwLock<CooldownData>>>,
}

impl CooldownTracker {
    pub fn new(init: HashMap<String, Duration>) -> CooldownTracker {
        CooldownTracker {
            cooldown_map: RwLock::new(
                init.into_iter()
                    .map(|(channel, cooldown)| (
                        channel,
                        RwLock::new(CooldownData::new(cooldown, true)),
                    ))
                    .collect(),
            ),
        }
    }

    pub async fn access(&self, channel: &str) -> Option<CooldownState> {
        if let Some(state) = self.cooldown_map.read().await.get(channel) {
            Some(state.write().await.try_reset())
        } else {
            None
        }
    }

    pub async fn update(&self, channel: &str, new_value: Duration) {
        if let Some(state) = self.cooldown_map.read().await.get(channel) {
            let mut value = state.write().await;
            value.value = new_value;
        }
    }

    pub async fn add_channel(
        &self,
        channel: String,
        cooldown: Duration,
        reset: bool,
    ) -> bool {
        self.cooldown_map
            .write()
            .await
            .insert(
                channel,
                RwLock::new(CooldownData::new(cooldown, reset)),
            )
            .is_some()
    }

    pub async fn remove_channel(&self, channel: &str) {
        let _ = self.cooldown_map.write().await.remove(channel);
    }
}

pub struct CooldownTrackerV2<K>
    where K: Hash + PartialEq
{
    // TODO figure out:
    // do locks in this map affect asynchronous model of execution?
    cooldown_map: chashmap::CHashMap<K, CooldownData>,
}

impl <K> CooldownTrackerV2 <K>
    where K: Hash + PartialEq
{

    pub fn new(init: HashMap<K, Duration>) -> CooldownTrackerV2<K> {
        CooldownTrackerV2 {
            cooldown_map: init
                .into_iter()
                .map(|(channel, cooldown)| (
                    channel,
                    CooldownData::new(cooldown, true),
                ))
                .collect(),
        }
    }

    /// Accesses cooldown state.
    ///
    /// If no cooldown happens right now, CooldownState::Ready is returned, and the
    /// state is reset (i.e. cooldown is triggered).
    /// If there is a cooldown, CooldownState::NotReady is returned.
    pub fn access(&self, channel: &K) -> Option<CooldownState> {
        self.cooldown_map.get_mut(channel).map(|mut state| state.try_reset())
    }

    pub fn access_raw(&self, channel: &K) -> Option<WriteGuard<K, CooldownData>> {
        self.cooldown_map.get_mut(channel)
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