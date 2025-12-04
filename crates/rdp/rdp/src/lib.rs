use std::sync::{Arc, RwLock};

pub mod callbacks;

pub mod connection;
mod init;
pub mod utils;

pub mod events;
pub mod wlog;

pub mod geom;
pub mod keymap;
pub mod settings;

// Re-export sys module
pub mod messaging;
pub mod sys;

#[derive(Debug, Default)]
pub struct Config {
    settings: settings::RdpSettings,
    callbacks: callbacks::Callbacks,
}

#[derive(Debug)]
pub struct Rdp {
    config: Config,
    instance: Option<utils::SafePtr<freerdp_sys::freerdp>>,
    update_tx: Option<messaging::Sender>,
    // GDI lock for thread safety
    gdi_lock: Arc<RwLock<()>>,
    // TODO: implement display context
    disp: Option<utils::SafePtr<freerdp_sys::DispClientContext>>,
    stop_event: utils::SafeHandle,
    _pin: std::marker::PhantomPinned, // Do not allow moving
}

#[allow(dead_code)]
impl Rdp {
    pub fn new(settings: settings::RdpSettings, update_tx: messaging::Sender) -> Self {
        let stop_event: freerdp_sys::HANDLE =
            unsafe { freerdp_sys::CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };

        let stop_event = utils::SafeHandle::new(stop_event).unwrap();
        Rdp {
            config: Config {
                settings,
                ..Config::default()
            },
            instance: None,
            update_tx: Some(update_tx),
            gdi_lock: Arc::new(RwLock::new(())),
            disp: None,
            stop_event,
            _pin: std::marker::PhantomPinned,
        }
    }
}
