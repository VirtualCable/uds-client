use shared::log;

pub trait EntrypointCallbacks {
    fn client_start(&mut self) -> bool {
        println!(" 🏁 **** Client started");
        true
    }

    fn client_stop(&mut self) -> bool {
        log::debug!(" 🏁 **** Client stopped");
        true
    }
}
