use std::sync::{Arc, RwLock};

use anyhow::Result;

use freerdp_sys::*;

use crossbeam::channel::Sender;

use crate::{
    callbacks::{altsec_c, input_c, pointer_update_c, primary_c, secondary_c, update_c, window_c},
    connection::context::RdpContext,
    geom::Rect,
    settings::RdpSettings,
    utils::{SafeHandle, SafePtr, ToStringLossy},
};

use shared::log;
pub mod builder;
pub mod context;
pub mod impl_callbacks;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RdpMessage {
    UpdateRects(Vec<Rect>),
    Disconnect,
    FocusRequired,
    Error(String),
    Resize(u32, u32),
}

#[derive(Debug, Clone)]
pub struct Callbacks {
    pub update: Vec<update_c::Callbacks>,
    pub window: Vec<window_c::Callbacks>,
    pub secondary: Vec<secondary_c::Callbacks>,
    pub primary: Vec<primary_c::Callbacks>,
    pub pointer: Vec<pointer_update_c::Callbacks>,
    pub input: Vec<input_c::Callbacks>,
    pub altsec: Vec<altsec_c::Callbacks>,
}

impl Default for Callbacks {
    fn default() -> Self {
        Callbacks {
            update: vec![
                update_c::Callbacks::BeginPaint,
                update_c::Callbacks::EndPaint,
                update_c::Callbacks::DesktopResize,
            ],
            window: vec![],
            secondary: vec![],
            primary: vec![],
            pointer: vec![],
            input: vec![],
            altsec: vec![],
        }
    }
}

#[derive(Debug, Default)]
pub struct Config {
    settings: RdpSettings,
    callbacks: Callbacks,
}

#[derive(Debug)]
pub struct Rdp {
    config: Config,
    instance: Option<SafePtr<freerdp>>,
    update_tx: Option<Sender<RdpMessage>>,
    // Flags the stop request
    stop_event: Option<SafeHandle>,
    // GDI lock for thread safety
    gdi_lock: Arc<RwLock<()>>,
    _pin: std::marker::PhantomPinned, // Do not allow moving
}

#[allow(dead_code)]
impl Rdp {
    #[cfg(debug_assertions)]
    pub fn debug_assert_instance(&self) {
        assert!(self.instance.is_some(), "RDP instance is not initialized");
        // Context intsance
        unsafe {
            let instance = self.instance.as_ref().unwrap();
            assert!(
                !instance.context.is_null(),
                "RDP context is not initialized"
            );
            // owner should point to self
            let ctx = instance.context as *mut RdpContext;
            assert!(
                !(*ctx).owner.is_null(),
                "RDP context owner is not initialized"
            );
            let owner = &*(*ctx).owner;
            let self_ptr: *const Rdp = self as *const Rdp;
            assert_eq!(
                owner as *const Rdp, self_ptr,
                "RDP context owner does not match self"
            );
        }
    }

    pub fn optimize(&self) {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();

        if let Some(conn) = self.instance.as_deref() {
            unsafe {
                let ctx = conn.context;
                if ctx.is_null() {
                    log::debug!("RDP context is null, cannot optimize settings.");
                    return;
                }
                let settings = (*ctx).settings;
                if settings.is_null() {
                    log::debug!("RDP settings is null, cannot optimize settings.");
                    return;
                }

                // Set Falses first
                for i in [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathInput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathOutput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCompressionDisabled,
                ]
                .iter()
                {
                    freerdp_settings_set_bool(settings, *i, false.into());
                }
                // Then Trues
                for i in [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxThinClient,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxProgressive,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_SupportGraphicsPipeline,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxH264,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCacheEnabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCacheV3Enabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowFontSmoothing,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowDesktopComposition,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowCacheWaitingList,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DesktopResize,
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncUpdate,
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncChannels,
                ]
                .iter()
                {
                    freerdp_settings_set_bool(settings, *i, true.into());
                }

                // Set uint32 values
                for (i, v) in [
                    (FreeRDP_Settings_Keys_UInt32_FreeRDP_ColorDepth, 32),
                    (
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopWidth,
                        self.config.settings.screen_size.width(),
                    ),
                    (
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopHeight,
                        self.config.settings.screen_size.height(),
                    ),
                    (
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_OffscreenSupportLevel,
                        1,
                    ),
                    (FreeRDP_Settings_Keys_UInt32_FreeRDP_FrameAcknowledge, 0),
                ]
                .iter()
                {
                    freerdp_settings_set_uint32(settings, *i, *v);
                }
            }
        } else {
            log::debug!("Connection not built, cannot optimize settings.");
        }
    }

    pub fn connect(&self) -> Result<()> {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();

        unsafe {
            if let Some(instance) = self.instance {
                if freerdp_connect(instance.as_mut_ptr()) == 0 {
                    let code = freerdp_error_info(instance.as_mut_ptr());
                    let name = freerdp_get_error_info_name(code);
                    let msg = freerdp_get_error_info_string(code);
                    return Err(anyhow::anyhow!(
                        "Failed to connect to RDP server: {} ({})",
                        name.to_string_lossy(),
                        msg.to_string_lossy()
                    ));
                }
                log::debug!("Connected to RDP server successfully.");
            } else {
                return Err(anyhow::anyhow!("Connection not built"));
            }
        }
        Ok(())
    }

    pub fn input(&self) -> Option<*mut rdpInput> {
        if let Some(context) = self.context() {
            let input = context.context().input;
            if input.is_null() { None } else { Some(input) }
        } else {
            None
        }
    }

    pub fn gdi(&self) -> Option<*mut rdpGdi> {
        if let Some(context) = self.context() {
            let gdi = context.context().gdi;
            if gdi.is_null() { None } else { Some(gdi) }
        } else {
            None
        }
    }

    pub fn gdi_lock(&self) -> Arc<RwLock<()>> {
        self.gdi_lock.clone()
    }

    pub fn width(&self) -> i32 {
        if let Some(gdi) = self.gdi() {
            unsafe { (*gdi).width }
        } else {
            0
        }
    }

    pub fn height(&self) -> i32 {
        if let Some(gdi) = self.gdi() {
            unsafe { (*gdi).height }
        } else {
            0
        }
    }

    pub fn get_stop_event(&self) -> Option<HANDLE> {
        self.stop_event.as_ref().map(|h| h.as_handle())
    }

    // Executes the RDP connection until end or stop is requested
    pub fn run(&self) -> Result<()> {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();

        let instance = self
            .instance
            .ok_or_else(|| anyhow::anyhow!("Connection not built"))?;

        let context = instance.context as *mut RdpContext;

        let tx = if let Some(tx) = &self.update_tx {
            tx
        } else {
            return Err(anyhow::anyhow!("No update sender provided"));
        };

        if context.is_null() {
            return Err(anyhow::anyhow!("RDP context is null"));
        }

        let mut handles = vec![HANDLE::default(); 64];

        while unsafe { freerdp_shall_disconnect_context(context as *mut rdpContext) == 0 } {
            if unsafe { freerdp_focus_required(instance.as_mut_ptr()) } != 0 {
                log::debug!("RDP focus required");
                //tx.send(RdpMessage::FocusRequired)?;
            }

            let handle_count: usize = unsafe {
                freerdp_get_event_handles(
                    context as *mut rdpContext,
                    handles.as_mut_ptr(),
                    handles.len() as u32,
                )
            }
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid handle count"))?;

            if handle_count == 0 {
                log::error!("No handles to wait on, exiting.");
                tx.send(RdpMessage::Error(
                    "No handles to wait on, exiting.".to_string(),
                ))?;
                break;
            }
            // Add our stop event handle
            handles[handle_count] = self
                .stop_event
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Stop event handle not initialized"))?
                .as_handle();

            let wait_result = unsafe {
                WaitForMultipleObjects(
                    (handle_count + 1) as u32,
                    handles.as_ptr(),
                    0,        // wait for any
                    INFINITE, // wait indefinitely
                )
            };
            if wait_result == 0xFFFFFFFF {
                // WAIT_FAILED
                tx.send(RdpMessage::Error(
                    "WaitForMultipleObjects failed".to_string(),
                ))?;
                return Err(anyhow::anyhow!("WaitForMultipleObjects failed"));
            }
            // If our stop event is signaled, break
            if wait_result == (handle_count as u32) {
                log::debug!("Stop event signaled, disconnecting...");
                break;
            }

            if unsafe { freerdp_check_event_handles(context as *mut rdpContext) } == 0 {
                if unsafe { client_auto_reconnect(instance.as_mut_ptr()) } != 0 {
                    log::debug!("Reconnected successfully");
                } else {
                    tx.send(RdpMessage::Error(
                        "freerdp_check_event_handles failed".to_string(),
                    ))?;
                    return Err(anyhow::anyhow!("freerdp_check_event_handles failed"));
                }
            }
        }

        log::debug!("RDP session ended, disconnecting...");
        tx.send(RdpMessage::Disconnect)?;

        // Ensure we wait a bit for the disconnect to process
        // Will know with the stop_event, that will be set on main before joining
        unsafe {
            WaitForSingleObject(
                self.stop_event
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Stop event handle not initialized"))?
                    .as_handle(),
                2000,
            )
        };

        Ok(())
    }

    pub fn get_rdp_version(&self) -> Result<String> {
        unsafe {
            if let Some(conn) = self.instance {
                let settings = (*conn.context).settings;
                let rdp_version = freerdp_settings_get_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_RdpVersion,
                );
                let rdpversion_str =
                    std::ffi::CStr::from_ptr(freerdp_rdp_version_string(rdp_version));
                Ok(rdpversion_str.to_string_lossy().into_owned())
            } else {
                Err(anyhow::anyhow!("Connection not built"))
            }
        }
    }

    #[allow(dead_code)]
    pub fn dump_log_settings(&self) {
        unsafe {
            if let Some(conn) = self.instance {
                let settings = (*conn.context).settings;
                super::wlog::dump_freerdp_settings(settings);
            }
        }
    }
}

impl Drop for Rdp {
    fn drop(&mut self) {
        log::debug!(" 🧪 **** Dropping RDP");

        log::debug!("* Dropping Rdp instance, cleaning up resources...");
        unsafe {
            if let Some(conn) = self.instance {
                freerdp_disconnect(conn.as_mut_ptr());
                freerdp_context_free(conn.as_mut_ptr());
                freerdp_free(conn.as_mut_ptr());
                self.instance = None;
            }
            if let Some(stop_event) = &self.stop_event {
                CloseHandle(stop_event.as_handle());
                self.stop_event = None;
            }
        }
    }
}
