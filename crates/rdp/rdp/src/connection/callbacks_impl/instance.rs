use shared::log;

use crate::callbacks::instance;

use super::Rdp;

impl instance::InstanceCallbacks for Rdp {
    fn on_post_connect(&mut self) -> bool {
        log::debug!(" **** Connected successfully!");
        true
    }
}
