use std::sync::{Arc, Condvar, Mutex};

use tokio::sync::Notify;

#[derive(Clone, Debug)]
pub struct Trigger {
    state: Arc<(Mutex<bool>, Condvar, Notify)>,
}

impl Trigger {
    pub fn new() -> Self {
        Trigger {
            state: Arc::new((Mutex::new(false), Condvar::new(), Notify::new())),
        }
    }

    pub fn set(&self) {
        let (lock, cvar, notify) = &*self.state;
        let mut guard = lock.lock().unwrap();
        *guard = true;
        cvar.notify_all();
        notify.notify_waiters();
    }

    pub fn is_set(&self) -> bool {
        let (lock, _, _) = &*self.state;
        *lock.lock().unwrap()
    }

    pub fn wait(&self) {
        let (lock, cvar, _) = &*self.state;
        let mut guard = lock.lock().unwrap();
        while !*guard {
            guard = cvar.wait(guard).unwrap();
        }
    }

    pub fn wait_timeout(&self, timeout: std::time::Duration) -> bool {
        let (lock, cvar, _) = &*self.state;
        let triggered = lock.lock().unwrap();
        let (guard, _result) = cvar
            .wait_timeout_while(triggered, timeout, |t| !*t)
            .unwrap();
        *guard
    }

    pub async fn async_wait(&self) {
        let (lock, _, notify) = &*self.state;
        {
            let guard = lock.lock().unwrap();
            if *guard {
                return;
            }
        }
        notify.notified().await;
    }

    pub async fn async_wait_timeout(&self, timeout: std::time::Duration) -> bool {
        let (lock, _, notify) = &*self.state;
        {
            let guard = lock.lock().unwrap();
            if *guard {
                return true;
            }
        }
        tokio::select! {
            _ = notify.notified() => true,
            _ = tokio::time::sleep(timeout) => false,
        }
        
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn trigger_wait_blocks_until_set() {
        let trigger = Trigger::new();
        let trigger_clone = trigger.clone();
        let handle = thread::spawn(move || {
            // Wait 100ms and then set the trigger
            thread::sleep(Duration::from_millis(100));
            trigger_clone.set();
        });
        handle.join().unwrap();
        trigger.wait();
    }

    #[test]
    fn trigger_wait_timeout() {
        let trigger = Trigger::new();
        let result = trigger.wait_timeout(Duration::from_millis(100));
        assert!(!result);
    }

    #[test]
    fn trigger_is_set() {
        let trigger = Trigger::new();
        assert!(!trigger.is_set());
        trigger.set();
        assert!(trigger.is_set());
        trigger.wait();  // Should return immediately
        assert!(trigger.is_set());
    }
}
