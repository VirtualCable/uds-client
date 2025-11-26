use freerdp_sys::{
    POINTER_CACHED_UPDATE, POINTER_COLOR_UPDATE, POINTER_LARGE_UPDATE, POINTER_NEW_UPDATE, POINTER_POSITION_UPDATE, POINTER_SYSTEM_UPDATE
};

use shared::log;

pub trait PointerCallbacks {
    fn on_pointer_position(&self, pointer_position: *const POINTER_POSITION_UPDATE) -> bool {
        log::debug!("Pointer position event: pointer_position={:?}", pointer_position);
        true
    }

    fn on_pointer_system(&self, pointer_system: *const POINTER_SYSTEM_UPDATE) -> bool {
        log::debug!("Pointer system event: pointer_system={:?}", pointer_system);
        true
    }

    fn on_pointer_color(&self, pointer_color: *const POINTER_COLOR_UPDATE) -> bool {
        log::debug!("Pointer color event: pointer_color={:?}", pointer_color);
        true
    }

    fn on_pointer_new(&self, pointer_new: *const POINTER_NEW_UPDATE) -> bool {
        log::debug!("Pointer new event: pointer_new={:?}", pointer_new);
        true
    }

    fn on_pointer_cached(&self, pointer_cached: *const POINTER_CACHED_UPDATE) -> bool {
        log::debug!("Pointer cached event: pointer_cached={:?}", pointer_cached);
        true
    }

    fn on_pointer_large(&self, pointer_large: *const POINTER_LARGE_UPDATE) -> bool {
        log::debug!("Pointer large event: pointer_large={:?}", pointer_large);
        true
    }
}
