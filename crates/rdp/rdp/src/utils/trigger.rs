// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Debug)]
pub struct Trigger {
    state: Arc<(Mutex<bool>, Condvar)>,
}

impl Trigger {
    pub fn new() -> Self {
        Trigger {
            state: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn trigger(&self) {
        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap();
        *guard = true;
        cvar.notify_all();
    }

    pub fn is_triggered(&self) -> bool {
        let (lock, _) = &*self.state;
        *lock.lock().unwrap()
    }

    pub fn wait(&self) {
        let (lock, cvar) = &*self.state;
        let mut guard = lock.lock().unwrap();
        while !*guard {
            guard = cvar.wait(guard).unwrap();
        }
    }

    pub fn wait_timeout(&self, timeout: std::time::Duration) -> Result<(), std::io::Error> {
        let (lock, cvar) = &*self.state;
        let triggered = lock.lock().unwrap();
        let (guard, _result) = cvar
            .wait_timeout_while(triggered, timeout, |t| !*t)
            .unwrap();
        if *guard {
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
        }
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::new()
    }
}
