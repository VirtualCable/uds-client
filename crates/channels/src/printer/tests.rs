use std::sync::{Arc, Mutex};

use super::*;

#[derive(Debug, Default)]
struct Spool {
    started: Vec<(String, String)>,
    written: Vec<Vec<u8>>,
    finished: usize,
    aborted: usize,
}

#[derive(Debug, Default)]
struct FakeBackend {
    spool: Arc<Mutex<Spool>>,
    fail_start: bool,
    fail_write: bool,
    // When set, writes on the job opened for this printer wait until it is released.
    blocked: Option<(String, Arc<std::sync::Barrier>)>,
}

struct FakeJob {
    spool: Arc<Mutex<Spool>>,
    fail_write: bool,
    blocked: Option<Arc<std::sync::Barrier>>,
}

impl BackendJob for FakeJob {
    fn write(&mut self, data: &[u8]) -> Result<usize, u32> {
        if let Some(barrier) = &self.blocked {
            barrier.wait();
            barrier.wait();
        }
        if self.fail_write {
            return Err(STATUS_UNSUCCESSFUL);
        }
        self.spool.lock().unwrap().written.push(data.to_vec());
        Ok(data.len())
    }

    fn finish(self: Box<Self>) -> Result<(), u32> {
        self.spool.lock().unwrap().finished += 1;
        Ok(())
    }

    fn abort(self: Box<Self>) {
        self.spool.lock().unwrap().aborted += 1;
    }
}

impl PrinterBackend for FakeBackend {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        vec![PrinterDeviceInfo {
            id: 1,
            name: "Office".into(),
            driver_name: "Office Driver".into(),
            is_default: true,
            port_name: None,
        }]
    }

    fn start_job(&self, printer_name: &str, job_name: &str) -> Result<Box<dyn BackendJob>, u32> {
        if self.fail_start {
            return Err(STATUS_UNSUCCESSFUL);
        }
        self.spool
            .lock()
            .unwrap()
            .started
            .push((printer_name.into(), job_name.into()));
        Ok(Box::new(FakeJob {
            spool: self.spool.clone(),
            fail_write: self.fail_write,
            blocked: self
                .blocked
                .as_ref()
                .filter(|(name, _)| name == printer_name)
                .map(|(_, barrier)| barrier.clone()),
        }))
    }
}

fn handle() -> (PrinterHandle, Arc<Mutex<Spool>>) {
    let backend = FakeBackend::default();
    let spool = backend.spool.clone();
    (PrinterHandle::with_backend(Some(Box::new(backend))), spool)
}

#[test]
fn without_backend_nothing_is_announced() {
    let handle = PrinterHandle::with_backend(None);
    assert!(!handle.is_available());
    assert!(handle.list_printers().is_empty());
    assert_eq!(handle.create_job("Office"), Err(STATUS_UNSUCCESSFUL));
}

#[test]
fn a_job_goes_to_the_backend_in_order() {
    let (handle, spool) = handle();
    assert!(handle.is_available());
    assert_eq!(handle.list_printers()[0].name, "Office");

    let job_id = handle.create_job("Office").unwrap();
    assert_eq!(handle.write_data(job_id, b"%!PS").unwrap(), 4);
    assert_eq!(handle.write_data(job_id, b" showpage").unwrap(), 9);
    handle.close_job(job_id).unwrap();

    let spool = spool.lock().unwrap();
    assert_eq!(
        spool.started,
        vec![("Office".to_string(), format!("UDS print job {job_id}"))]
    );
    assert_eq!(spool.written.concat(), b"%!PS showpage");
    assert_eq!((spool.finished, spool.aborted), (1, 0));
    assert_eq!(handle.open_jobs(), 0);
}

#[test]
fn every_job_gets_its_own_id() {
    let (handle, _) = handle();
    let first = handle.create_job("Office").unwrap();
    let second = handle.create_job("Office").unwrap();
    assert_ne!(first, second);
    assert_eq!(handle.open_jobs(), 2);
}

#[test]
fn unknown_or_closed_jobs_are_rejected() {
    let (handle, _) = handle();
    assert_eq!(handle.write_data(42, b"x"), Err(STATUS_INVALID_HANDLE));
    assert_eq!(handle.close_job(42), Err(STATUS_INVALID_HANDLE));

    let job_id = handle.create_job("Office").unwrap();
    handle.close_job(job_id).unwrap();
    assert_eq!(handle.write_data(job_id, b"x"), Err(STATUS_INVALID_HANDLE));
    assert_eq!(handle.close_job(job_id), Err(STATUS_INVALID_HANDLE));
}

#[test]
fn a_failed_start_leaves_no_open_job() {
    let backend = FakeBackend {
        fail_start: true,
        ..Default::default()
    };
    let handle = PrinterHandle::with_backend(Some(Box::new(backend)));
    assert_eq!(handle.create_job("Office"), Err(STATUS_UNSUCCESSFUL));
    assert_eq!(handle.open_jobs(), 0);
}

#[test]
fn abort_job_cancels_it_in_the_spooler() {
    let (handle, spool) = handle();
    let job_id = handle.create_job("Office").unwrap();
    handle.abort_job(job_id);
    handle.abort_job(job_id);
    let spool = spool.lock().unwrap();
    assert_eq!((spool.finished, spool.aborted), (0, 1));
}

#[test]
fn jobs_left_open_are_aborted_when_the_session_ends() {
    let (handle, spool) = handle();
    let open = handle.create_job("Office").unwrap();
    let closed = handle.create_job("Office").unwrap();
    handle.write_data(open, b"half a page").unwrap();
    handle.close_job(closed).unwrap();
    drop(handle);
    let spool = spool.lock().unwrap();
    assert_eq!((spool.finished, spool.aborted), (1, 1));
}

#[test]
fn a_failed_write_aborts_the_job_instead_of_printing_it_incomplete() {
    let backend = FakeBackend {
        fail_write: true,
        ..Default::default()
    };
    let spool = backend.spool.clone();
    let handle = PrinterHandle::with_backend(Some(Box::new(backend)));
    let job_id = handle.create_job("Office").unwrap();

    assert_eq!(handle.write_data(job_id, b"page"), Err(STATUS_UNSUCCESSFUL));
    assert_eq!(handle.open_jobs(), 0);
    assert_eq!(handle.close_job(job_id), Err(STATUS_INVALID_HANDLE));
    let spool = spool.lock().unwrap();
    assert_eq!((spool.finished, spool.aborted), (0, 1));
}

#[test]
fn a_slow_printer_does_not_block_the_others() {
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let backend = FakeBackend {
        blocked: Some(("Slow".into(), barrier.clone())),
        ..Default::default()
    };
    let spool = backend.spool.clone();
    let handle = Arc::new(PrinterHandle::with_backend(Some(Box::new(backend))));
    let slow = handle.create_job("Slow").unwrap();
    let fast = handle.create_job("Office").unwrap();

    let writer = {
        let handle = handle.clone();
        std::thread::spawn(move || handle.write_data(slow, b"slow"))
    };
    barrier.wait();
    // If the slow write blocked every printer, this write would never return
    // and the slow one would wait forever at the second barrier: run it on its
    // own thread and fail after a timeout instead of hanging the suite.
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    {
        let handle = handle.clone();
        std::thread::spawn(move || {
            let result = handle.write_data(fast, b"fast");
            handle.close_job(fast).unwrap();
            done_tx.send(result).unwrap();
        });
    }
    let fast_result = done_rx.recv_timeout(std::time::Duration::from_secs(5));
    barrier.wait();
    assert_eq!(
        fast_result,
        Ok(Ok(4)),
        "the fast printer was blocked by the slow one"
    );
    assert_eq!(writer.join().unwrap(), Ok(4));
    assert_eq!(
        spool.lock().unwrap().written,
        vec![b"fast".to_vec(), b"slow".to_vec()]
    );
}

#[test]
fn a_close_racing_a_failed_write_never_prints_the_job() {
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let backend = FakeBackend {
        fail_write: true,
        blocked: Some(("Office".into(), barrier.clone())),
        ..Default::default()
    };
    let spool = backend.spool.clone();
    let handle = Arc::new(PrinterHandle::with_backend(Some(Box::new(backend))));
    let job_id = handle.create_job("Office").unwrap();

    let writer = {
        let handle = handle.clone();
        std::thread::spawn(move || handle.write_data(job_id, b"page"))
    };
    barrier.wait();
    let closer = {
        let handle = handle.clone();
        std::thread::spawn(move || handle.close_job(job_id))
    };
    // close_job removes the entry from the map before it waits on the job
    // lock that the failing write still holds.
    while handle.open_jobs() != 0 {
        std::thread::yield_now();
    }
    barrier.wait();

    assert_eq!(writer.join().unwrap(), Err(STATUS_UNSUCCESSFUL));
    assert_eq!(closer.join().unwrap(), Err(STATUS_INVALID_HANDLE));
    let spool = spool.lock().unwrap();
    assert_eq!((spool.finished, spool.aborted), (0, 1));
}

#[test]
fn write_all_retries_short_writes_until_the_chunk_is_done() {
    let mut calls = Vec::new();
    let result = write_all(b"abcdef", |pending| {
        calls.push(pending.to_vec());
        Ok(pending.len().min(4))
    });
    assert_eq!(result, Ok(6));
    assert_eq!(calls, vec![b"abcdef".to_vec(), b"ef".to_vec()]);
}

#[test]
fn write_all_fails_when_the_spooler_makes_no_progress() {
    assert_eq!(write_all(b"abc", |_| Ok(0)), Err(STATUS_UNSUCCESSFUL));
    assert_eq!(write_all(b"abc", |_| Err(0xC000_0022)), Err(0xC000_0022));
    assert_eq!(write_all(b"abc", |_| Ok(9)), Err(STATUS_UNSUCCESSFUL));
}

#[test]
fn the_os_default_printer_is_announced_as_default() {
    let printers = announce(
        vec![
            ("A".into(), "Driver A".into(), None),
            ("B".into(), "Driver B".into(), Some("USB001".into())),
        ],
        Some("B"),
    );
    let defaults: Vec<_> = printers
        .iter()
        .map(|p| (p.id, p.name.as_str(), p.is_default))
        .collect();
    assert_eq!(defaults, vec![(1, "A", false), (2, "B", true)]);
    assert_eq!(printers[1].port_name.as_deref(), Some("USB001"));
}

#[test]
fn the_first_printer_is_default_when_the_os_has_none() {
    let printers = announce(
        vec![
            ("A".into(), "Driver A".into(), None),
            ("B".into(), "Driver B".into(), None),
        ],
        None,
    );
    assert!(printers[0].is_default && !printers[1].is_default);
    assert!(announce(Vec::new(), None).is_empty());
}

#[test]
fn a_wrapped_counter_never_reuses_the_id_of_an_open_job() {
    let (handle, spool) = handle();
    let open = handle.create_job("Office").unwrap();
    handle.next_job_id.store(open, Ordering::Relaxed);

    let next = handle.create_job("Office").unwrap();
    assert_ne!(next, open);
    handle.write_data(open, b"first").unwrap();
    handle.close_job(open).unwrap();
    handle.close_job(next).unwrap();
    assert_eq!(spool.lock().unwrap().finished, 2);
}

#[test]
fn a_late_cleanup_does_not_remove_a_job_that_reused_the_id() {
    let (handle, _) = handle();
    let job_id = handle.create_job("Office").unwrap();
    let old_slot = handle.find_job(job_id).unwrap();
    handle.abort_job(job_id);
    handle.next_job_id.store(job_id, Ordering::Relaxed);
    assert_eq!(handle.create_job("Office").unwrap(), job_id);

    handle.remove_slot(job_id, &old_slot);
    assert_eq!(handle.open_jobs(), 1);
    assert_eq!(handle.write_data(job_id, b"x"), Ok(1));
}

#[test]
fn queues_that_cannot_be_opened_are_not_announced() {
    let listed = vec![
        ("Office".into(), "Driver".into(), None),
        ("Stale".into(), "Driver".into(), None),
    ];
    let kept = keep_openable(listed, |name| name != "Stale");
    assert_eq!(
        kept,
        vec![("Office".to_string(), "Driver".to_string(), None)]
    );
}
