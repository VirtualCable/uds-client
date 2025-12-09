mod graphics;  // implements graphics callbacks
mod update;    // implements update callbacks
mod instance;  // implements instance callbacks
mod channels;  // implements channel callbacks

// Clipboard is set on channel connection. Callbacks will be registered then 
// and will invoke us
mod clipboard;  // implements clipboard callbacks

use crate::{
    callbacks::{
        altsec,entrypoint, input, pointer_update, primary, secondary,
        window,
    },
};

use super::{Rdp, RdpMessage};


impl input::InputCallbacks for Rdp {}
impl pointer_update::PointerCallbacks for Rdp {}
impl primary::PrimaryCallbacks for Rdp {}
impl secondary::SecondaryCallbacks for Rdp {}
impl altsec::AltSecCallbacks for Rdp {}
impl window::WindowCallbacks for Rdp {}
impl entrypoint::EntrypointCallbacks for Rdp {}


