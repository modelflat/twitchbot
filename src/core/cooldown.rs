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
    cooldown_map: HashMap<String, RwLock<CooldownData>>,
}

impl CooldownTracker {
    pub fn new(init: HashMap<String, Duration>) -> CooldownTracker {
        CooldownTracker {
            cooldown_map: init
                .into_iter()
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
        }
    }

    pub async fn access(&self, channel: &str) -> Option<CooldownState> {
        if let Some(state) = self.cooldown_map.get(channel) {
            // todo find out
            // is it cheaper to first check if the channel is ready (.read()) and then lock
            // using write(), or lock using write right away?
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
        if let Some(state) = self.cooldown_map.get(channel) {
            let mut value = state.write().await;
            value.value = new_value;
        }
    }
}
