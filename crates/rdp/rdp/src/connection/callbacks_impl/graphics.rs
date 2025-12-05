use shared::log;

use crate::callbacks::graphics;

use super::{Rdp, RdpMessage};

impl graphics::GraphicsCallbacks for Rdp {
    unsafe fn on_pointer_set(&self, pointer: *mut freerdp_sys::rdpPointer) -> bool {
        log::debug!(" **** Custom Pointer Set called.");

        let pointer = unsafe { &*pointer };
        let gdi = match self.gdi() {
            Some(gdi) => gdi,
            None => {
                log::error!(" **** GDI context not available.");
                return false;
            }
        };
        let size = 4 * pointer.width * pointer.height;
        let data = vec![0u8; size as usize];
        // Create the custom pointer image from the pointer data
        unsafe {
            freerdp_sys::freerdp_image_copy_from_pointer_data(
                data.as_ptr() as *mut freerdp_sys::BYTE,
                (*gdi).dstFormat,
                0,
                0,
                0,
                pointer.width,
                pointer.height,
                pointer.xorMaskData,
                pointer.lengthXorMask,
                pointer.andMaskData,
                pointer.lengthAndMask,
                pointer.xorBpp,
                &(*gdi).palette,
            )
        };
        // Send the custom pointer data to the UI or handle it as needed
        if let Some(tx) = &self.update_tx {
            log::debug!(" **** Sending custom pointer data to UI.");
            if let Err(e) = tx.try_send(RdpMessage::SetCursorIcon(
                data,
                pointer.xPos,
                pointer.yPos,
                pointer.width,
                pointer.height,
            )) {
                log::error!(" **** Failed to send custom pointer data: {}", e);
            }
        }
        true
    }

    unsafe fn on_pointer_free(&self, pointer: *mut freerdp_sys::rdpPointer) {
        log::debug!(" **** Custom Pointer Free called: {:?}", unsafe { *pointer });
        // We do not need special handling for freeing the pointer in this implementation.
        // Because the cursor data was sent to the UI.
    }

    unsafe fn on_pointer_new(&self, pointer: *mut freerdp_sys::rdpPointer) -> bool {
        log::debug!(" **** Custom Pointer New called: {:?}", unsafe { *pointer });
        // We do not need special handling for new pointers in this implementation.
        // Because the cursor data will be sent to the UI when set.
        true
    }

    fn on_pointer_position(&self, x: u32, y: u32) -> bool {
        log::debug!(" **** Custom Pointer Position called: x={}, y={}", x, y);
        // We do not need special handling for pointer position in this implementation.
        // Because the cursor position will be handled by the UI.
        true
    }

    fn on_pointer_set_null(&self) -> bool {
        log::debug!(" **** Custom Pointer SetNull called.");
        if let Some(tx) = &self.update_tx {
            log::debug!(" **** Sending null pointer to UI.");
            if let Err(e) = tx.try_send(RdpMessage::SetCursorIcon(vec![0u8; 4], 0, 0, 1, 1)) {
                log::error!(" **** Failed to send null pointer data: {}", e);
            }
        }
        true
    }
}