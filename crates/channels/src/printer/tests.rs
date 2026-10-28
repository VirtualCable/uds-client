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
}

struct FakeJob {
    spool: Arc<Mutex<Spool>>,
}

impl BackendJob for FakeJob {
    fn write(&mut self, data: &[u8]) -> Result<usize, u32> {
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
