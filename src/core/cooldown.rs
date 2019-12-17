use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_std::sync::RwLock;

pub enum CooldownState {
    Ready,
    NotReady(Duration),
}

struct CooldownData {
    value: Duration,
    last_accessed: Instant,
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
                    .map(|(c, v)| {
                        (
                            c,
                            RwLock::new(CooldownData {
                                value: v,
                                last_accessed: Instant::now() - v,
                            }),
                        )
                    })
                    .collect(),
            ),
        }
    }

    pub async fn access(&self, channel: &str) -> Option<CooldownState> {
        if let Some(state) = self.cooldown_map.read().await.get(channel) {
            let now = Instant::now();
            let mut value = state.write().await;

            if value.last_accessed + value.value < now {
                value.last_accessed = now;
                return Some(CooldownState::Ready);
            } else {
                return Some(CooldownState::NotReady(
                    value.last_accessed + value.value - now,
                ));
            }
        }
        None
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
        reset_access: bool,
    ) -> bool {
        self.cooldown_map
            .write()
            .await
            .insert(
                channel,
                RwLock::new(CooldownData {
                    value: cooldown,
                    last_accessed: if reset_access {
                        Instant::now() - cooldown
                    } else {
                        Instant::now()
                    },
                }),
            )
            .is_some()
    }

    pub async fn remove_channel(&self, channel: &str) {
        let _ = self.cooldown_map.write().await.remove(channel);
    }
}
