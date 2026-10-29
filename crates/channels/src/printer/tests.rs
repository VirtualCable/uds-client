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
            ("A".into(), "Driver A".into()),
            ("B".into(), "Driver B".into()),
        ],
        Some("B"),
    );
    let defaults: Vec<_> = printers
        .iter()
        .map(|p| (p.id, p.name.as_str(), p.is_default))
        .collect();
    assert_eq!(defaults, vec![(1, "A", false), (2, "B", true)]);
    assert!(printers.iter().all(|p| p.port_name.is_none()));
}

#[test]
fn the_first_printer_is_default_when_the_os_has_none() {
    let printers = announce(
        vec![
            ("A".into(), "Driver A".into()),
            ("B".into(), "Driver B".into()),
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
        ("Office".into(), "Driver".into()),
        ("Stale".into(), "Driver".into()),
    ];
    let kept = keep_openable(listed, |name| name != "Stale");
    assert_eq!(kept, vec![("Office".to_string(), "Driver".to_string())]);
}

static DELIVERED: Mutex<Vec<(std::path::PathBuf, String)>> = Mutex::new(Vec::new());

fn record_delivery(path: std::path::PathBuf, name: String) {
    DELIVERED.lock().unwrap().push((path, name));
}

#[test]
fn print_to_pdf_is_recognised_by_its_driver() {
    assert!(is_print_to_pdf("Microsoft Print To PDF"));
    assert!(is_print_to_pdf("microsoft print to pdf"));
    assert!(!is_print_to_pdf("Microsoft XPS Document Writer v4"));
}

#[test]
fn a_pdf_job_is_written_to_a_file_and_handed_over_when_finished() {
    let mut job = Box::new(SaveAsPdfJob::create("UDS print job 7", record_delivery).unwrap());
    let path = job.path.clone();
    job.write(b"%PDF-1.7\n").unwrap();
    job.write(b"%%EOF").unwrap();
    job.finish().unwrap();

    let delivered = DELIVERED.lock().unwrap().clone();
    let (file, name) = delivered.iter().find(|(file, _)| *file == path).unwrap();
    assert_eq!(name, "UDS print job 7.pdf");
    assert_eq!(std::fs::read(file).unwrap(), b"%PDF-1.7\n%%EOF");
    std::fs::remove_file(file).unwrap();
}

#[test]
fn an_aborted_pdf_job_leaves_no_file_behind() {
    let mut job = Box::new(SaveAsPdfJob::create("UDS print job 8", record_delivery).unwrap());
    let path = job.path.clone();
    job.write(b"%PDF-1.7").unwrap();
    job.abort();
    assert!(!path.exists());
    assert!(
        DELIVERED
            .lock()
            .unwrap()
            .iter()
            .all(|(file, _)| *file != path)
    );
}

#[cfg(target_os = "linux")]
mod cups_backend {
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::os::unix::fs::PermissionsExt;

    use super::super::cups::{ANNOUNCED_DRIVER, Cups, CupsDest, CupsJob, queues};
    use super::*;

    // (printer, title, bytes of the file when CUPS was called)
    static PRINTED: Mutex<Vec<(String, String, Vec<u8>)>> = Mutex::new(Vec::new());

    unsafe extern "C" fn fake_print_file(
        printer: *const c_char,
        file: *const c_char,
        title: *const c_char,
        _num_options: c_int,
        _options: *const c_void,
    ) -> c_int {
        let printer = unsafe { CStr::from_ptr(printer) }
            .to_string_lossy()
            .into_owned();
        if printer == "Broken" {
            return 0;
        }
        let file = unsafe { CStr::from_ptr(file) }
            .to_string_lossy()
            .into_owned();
        let title = unsafe { CStr::from_ptr(title) }
            .to_string_lossy()
            .into_owned();
        let data = std::fs::read(file).unwrap();
        PRINTED.lock().unwrap().push((printer, title, data));
        42
    }

    unsafe extern "C" fn fake_last_error() -> *const c_char {
        c"client-error-not-found".as_ptr()
    }

    fn cups() -> Arc<Cups> {
        Arc::new(Cups::with_print_file(fake_print_file, fake_last_error))
    }

    fn printed_on(printer: &str) -> Vec<(String, Vec<u8>)> {
        PRINTED
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _, _)| name == printer)
            .map(|(_, title, data)| (title.clone(), data.clone()))
            .collect()
    }

    fn dest(name: &CString, instance: Option<&CString>, is_default: bool) -> CupsDest {
        CupsDest {
            name: name.as_ptr().cast_mut(),
            instance: instance.map_or(std::ptr::null_mut(), |i| i.as_ptr().cast_mut()),
            is_default: is_default as c_int,
            num_options: 0,
            options: std::ptr::null_mut(),
        }
    }

    #[test]
    fn queues_are_announced_with_the_postscript_driver_and_the_cups_default() {
        let (office, pdf, draft) = (c"Office".to_owned(), c"PDF".to_owned(), c"draft".to_owned());
        let dests = [
            dest(&office, None, false),
            dest(&office, Some(&draft), false),
            dest(&pdf, None, true),
        ];
        let (printers, default) = unsafe { queues(&dests) };
        assert_eq!(
            printers,
            vec![
                ("Office".to_string(), ANNOUNCED_DRIVER.to_string()),
                ("PDF".to_string(), ANNOUNCED_DRIVER.to_string()),
            ]
        );
        assert_eq!(default.as_deref(), Some("PDF"));
    }

    #[test]
    fn the_whole_job_reaches_cups_once_it_is_closed() {
        let mut job = Box::new(CupsJob::create(cups(), "Whole", "UDS print job 3").unwrap());
        let path = job.path().to_path_buf();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        job.write(b"%!PS-Adobe-3.0\n").unwrap();
        assert!(printed_on("Whole").is_empty());
        job.write(b"%%EOF\n").unwrap();
        job.finish().unwrap();

        assert_eq!(
            printed_on("Whole"),
            vec![(
                "UDS print job 3".to_string(),
                b"%!PS-Adobe-3.0\n%%EOF\n".to_vec()
            )]
        );
        assert!(!path.exists());
    }

    #[test]
    fn an_aborted_job_never_reaches_cups() {
        let mut job = Box::new(CupsJob::create(cups(), "Aborted", "UDS print job 4").unwrap());
        let path = job.path().to_path_buf();
        job.write(b"partial").unwrap();
        job.abort();
        assert!(printed_on("Aborted").is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn an_empty_job_is_not_sent() {
        let job = Box::new(CupsJob::create(cups(), "Empty", "UDS print job 5").unwrap());
        let path = job.path().to_path_buf();
        assert_eq!(job.finish(), Ok(()));
        assert!(printed_on("Empty").is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn a_job_cups_rejects_fails_and_leaves_no_file() {
        let mut job = Box::new(CupsJob::create(cups(), "Broken", "UDS print job 6").unwrap());
        let path = job.path().to_path_buf();
        job.write(b"data").unwrap();
        assert_eq!(job.finish(), Err(STATUS_UNSUCCESSFUL));
        assert!(!path.exists());
    }
}
