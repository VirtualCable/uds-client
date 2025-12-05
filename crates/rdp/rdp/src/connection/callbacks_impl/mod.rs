mod graphics;
mod update;
mod instance;
mod channels;

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
