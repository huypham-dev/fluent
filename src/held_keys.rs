use std::collections::HashSet;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Tracks which modifier keys are physically held, from global key events.
/// The hook thread updates it and a worker thread waits on it.
#[derive(Default)]
pub struct HeldKeys {
    held: Mutex<HashSet<u32>>,
    released: Condvar,
}

impl HeldKeys {
    pub fn key_down(&self, key: u32) {
        self.held.lock().unwrap().insert(key);
    }

    pub fn key_up(&self, key: u32) {
        let mut held = self.held.lock().unwrap();
        held.remove(&key);
        if held.is_empty() {
            self.released.notify_all();
        }
    }

    /// Returns false if a key is still held at the timeout.
    pub fn wait_released(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut held = self.held.lock().unwrap();
        while !held.is_empty() {
            let now = Instant::now();
            if now >= deadline {
                // A release can go unseen, for example during secure input, so don't keep waiting on it.
                held.clear();
                return false;
            }
            held = self.released.wait_timeout(held, deadline - now).unwrap().0;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    const CTRL: u32 = 1;
    const ALT: u32 = 2;

    #[test]
    fn returns_true_at_once_when_nothing_is_held() {
        assert!(HeldKeys::default().wait_released(Duration::from_secs(1)));
    }

    #[test]
    fn waits_until_every_key_is_released() {
        let keys = Arc::new(HeldKeys::default());
        keys.key_down(CTRL);
        keys.key_down(ALT);
        let waiter = {
            let keys = keys.clone();
            thread::spawn(move || keys.wait_released(Duration::from_secs(5)))
        };
        keys.key_up(CTRL);
        thread::sleep(Duration::from_millis(30));
        assert!(!waiter.is_finished());
        keys.key_up(ALT);
        assert!(waiter.join().unwrap());
    }

    #[test]
    fn returns_false_when_a_key_is_held_at_the_timeout() {
        let keys = HeldKeys::default();
        keys.key_down(CTRL);
        assert!(!keys.wait_released(Duration::from_millis(30)));
    }

    #[test]
    fn forgets_held_keys_after_a_timeout() {
        let keys = HeldKeys::default();
        keys.key_down(CTRL);
        keys.wait_released(Duration::from_millis(10));
        assert!(keys.wait_released(Duration::from_millis(10)));
    }
}
