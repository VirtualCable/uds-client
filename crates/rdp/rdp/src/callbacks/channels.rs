pub trait ChannelsCallbacks {
    fn on_channel_connected(
        &mut self,
        _size: usize,
        _sender: &str,
        _name: &str,
        _p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        false // Defaults to false, let freerdp handle it.
    }

    fn on_channel_disconnected(
        &mut self,
        _size: usize,
        _sender: &str,
        _name: &str,
        _p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        false // Defaults to false, let freerdp handle it.
    }
}
