use super::safe::SafeHandle;
use std::time::Duration;
use windows::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForSingleObject,
};

use crate::log;

#[derive(Clone, Debug)]
pub struct Event {
    handle: SafeHandle,
}

#[allow(dead_code)]
impl Event {
    pub fn new() -> Self {
        unsafe {
            // Manual reset event, initial state non-signaled
            let handle = CreateEventW(
                None,  // default security
                true,  // manual reset
                false, // initial state: not signaled
                None,  // no name
            )
            .expect("Failed to create event");
            Event {
                handle: SafeHandle::new(handle),
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.handle.is_valid()
    }

    /// Blocks until the event is signaled
    pub fn wait(&self) {
        log::debug!("Waiting for event: {:?}", self.handle);
        unsafe {
            let res = WaitForSingleObject(self.handle.get(), INFINITE);
            assert!(res == WAIT_OBJECT_0, "WaitForSingleObject failed");
        }
    }

    /// Blocks until the event is signaled or the timeout expires
    /// Returns true if the event was signaled, false if the timeout expired
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        log::debug!("Waiting for event with timeout: {:?}, handle: {:?}", timeout, self.handle);
        unsafe {
            let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
            let res = WaitForSingleObject(self.handle.get(), ms);
            match res {
                x if x == WAIT_OBJECT_0 => true,
                x if x == WAIT_TIMEOUT => false,
                _ => panic!("WaitForSingleObject failed: {res:?}"),
            }
        }
    }

    /// Signals the event (wakes up all waiters)
    pub fn signal(&self) {
        log::debug!("Signaling event: {:?}", self.handle);
        unsafe {
            let ok = SetEvent(self.handle.get()).is_ok();
            assert!(ok, "SetEvent failed");
        }
    }

    /// Resets the event to non-signaled state (optional)
    pub fn reset(&self) {
        log::debug!("Resetting event: {:?}", self.handle);
        unsafe {
            let ok = ResetEvent(self.handle.get()).is_ok();
            assert!(ok, "ResetEvent failed");
        }
    }

    /// If is set to true, the event is in a signaled state
    pub fn is_set(&self) -> bool {
        unsafe {
            let res = WaitForSingleObject(self.handle.get(), 0);
            res == WAIT_OBJECT_0
        }
    }

    pub fn into_raw(self) -> *mut core::ffi::c_void {
        // Consumes the Event and returns the SafeHandle, without closing it
        self.handle.into_raw()
    }

    pub fn from_raw(handle: *mut core::ffi::c_void) -> Self {
        let handle = SafeHandle::from_raw(handle);
        Event { handle }
    }

    pub fn get(&self) -> SafeHandle {
        // Returns a clone of the SafeHandle
        self.handle.clone()
    }
}

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn event_wait_blocks_until_signal() {
        let event = Event::new();
        let event_clone = event.clone();

        let handle = thread::spawn(move || {
            // Wait 100ms and then signal the event
            thread::sleep(Duration::from_millis(100));
            event_clone.signal();
        });

        let start = Instant::now();
        event.wait();
        let elapsed = start.elapsed();

        // Should have waited at least 100ms
        assert!(elapsed >= Duration::from_millis(100));
        handle.join().unwrap();
    }

    #[test]
    fn event_signal_wakes_all_waiters() {
        let event = Event::new();
        let mut handles = vec![];

        for _ in 0..5 {
            let event_clone = event.clone();
            handles.push(thread::spawn(move || {
                event_clone.wait();
            }));
        }

        // Signal the event and wait for all threads to finish
        thread::sleep(Duration::from_millis(50));
        event.signal();
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn event_reset_blocks_again() {
        let event = Event::new();
        event.signal();
        event.wait(); // Should not block

        event.reset();

        let event_clone = event.clone();
        let handle = thread::spawn(move || {
            // Wait 50ms and then signal the event
            thread::sleep(Duration::from_millis(50));
            event_clone.signal();
        });

        let start = Instant::now();
        event.wait();
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(50));
        handle.join().unwrap();
    }

    #[test]
    fn event_wait_timeout() {
        let event = Event::new();
        let start = Instant::now();
        let result = event.wait_timeout(Duration::from_millis(100));
        let elapsed = start.elapsed();

        // Should return false and wait for less than 100ms
        assert!(!result);
        assert!(elapsed < Duration::from_millis(200));

        // Now signal the event and check that it returns true
        event.signal();
        let result = event.wait_timeout(Duration::from_millis(100));
        assert!(result);
    }

    #[test]
    fn event_is_set() {
        let event = Event::new();
        assert!(!event.is_set());

        event.signal();
        assert!(event.is_set());

        event.reset();
        assert!(!event.is_set());
    }
}
