use anyhow::Result;

use freerdp_sys::*;

use shared::log;

use crate::{Rdp, context, messaging::RdpMessage, utils::ToStringLossy};

pub mod builder;
pub mod callbacks_impl;

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
            let ctx = instance.context as *mut context::RdpContext;
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

    fn settings(&self) -> Option<*mut rdpSettings> {
        unsafe {
            if let Some(conn) = self.instance.as_deref() {
                let ctx = conn.context;
                if ctx.is_null() {
                    None
                } else {
                    let settings = (*ctx).settings;
                    if settings.is_null() {
                        None
                    } else {
                        Some(settings)
                    }
                }
            } else {
                None
            }
        }
    }

    /// Optimize the RDP settings for better performance
    /// This function modifies the FreeRDP settings to enable various performance
    /// optimizations such as enabling bitmap caching, graphics pipeline support,
    /// and disabling unnecessary features.
    fn set_connections_parameters(&self) {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();

        if let Some(settings) = self.settings() {
            unsafe {
                // Set Falses first
                for i in [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathInput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathOutput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCompressionDisabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteConsoleAudio, // So audio is not played on server
                ]
                .iter()
                {
                    freerdp_settings_set_bool(settings, *i, false.into());
                }
                // Then Trues
                for i in [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxThinClient,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxProgressive,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCacheEnabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCacheV3Enabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowFontSmoothing,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowDesktopComposition,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowCacheWaitingList,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DesktopResize,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DynamicResolutionUpdate,
                    // From proper client settings
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathOutput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FrameMarkerCommandEnabled,
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncUpdate,  // Note: currently works badly
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncChannels,
                    // Compression
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_CompressionEnabled,
                    // Graphics
                    // TODO: Test this settings on all platforms (gfx related and h264)
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxAVC444v2,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxAVC444,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxH264,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteFxCodec,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_SupportGraphicsPipeline,
                ]
                .iter()
                {
                    // Ignore the result, try with best effort
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

            fn channels(
                settings: *mut rdpSettings,
                name: &str,
                add_static: bool,
                add_dynamic: bool,
            ) {
                unsafe {
                    let channel = if cfg!(target_os = "windows") {
                        "sys:winmm"
                    } else if cfg!(target_os = "linux") {
                        "sys:pulse"  // add support for alsa and oss
                    } else if cfg!(target_os = "macos") {
                        "sys:mac"
                    } else {
                        "sys:fake"
                    };

                    let cname = std::ffi::CString::new(name).unwrap();
                    let cchannel = std::ffi::CString::new(channel).unwrap();
                    let channels: [*const std::os::raw::c_char; 2] =
                        [cname.as_ptr(), cchannel.as_ptr()];
                    if add_static {
                        freerdp_client_add_static_channel(
                            settings,
                            channels.len(),
                            channels.as_ptr(),
                        );
                    }
                    if add_dynamic {
                        freerdp_client_add_dynamic_channel(
                            settings,
                            channels.len(),
                            channels.as_ptr(),
                        );
                    }
                }
            }

            // Sound redirection
            unsafe {
                // true-false = play on client
                // false-true = play on server
                // false-false = no audio
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AudioPlayback,
                    true.into(),
                );
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteConsoleAudio,
                    false.into(),
                );
                channels(settings, "rdpsnd", true, true);
            }
            // Microphone redirection
            unsafe {
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AudioCapture,
                    true.into(),
                );
                channels(settings, "audin", false, true);
            }

            // Set config settings for clipboard redirection
            unsafe {
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectClipboard,
                    self.config.settings.clipboard_redirection.into(),
                );
            }

            // Set perfromance flags from settings
            unsafe { freerdp_sys::freerdp_performance_flags_make(settings) };
        } else {
            log::debug!("Connection not built, cannot optimize settings.");
        }
    }

    // Notes about connect on FreeRDP:
    // we can use "|" as hostname and pass in an fd as port to connect
    // this allows us to connect over an existing socket, for proxying or tunneling scenarios.
    // Also allows an unix socket fd on unix systems with "/...socket"
    // Also, we must set all options on the fd before using it, as freerdp won't change anything
    // on a pre-existing socket.
    // The close will be responsibility of freerdp (that is, we send the fd and freerdp takes ownership)
    // * The hostname after "|" is ignored
    // * The fd must be already connected
    // * Freerdp will close the fd on disconnect (it takes ownership)

    /// Connects to the RDP server using the current settings
    pub fn connect(&self) -> Result<()> {
        self.set_connections_parameters();

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

    pub fn send_resize(self, width: u32, height: u32) {
        // TODO:: implement this
        // We need the disp channel to send the resize request, not alredy implemented in our code
        // Note: avoid too fast resizing, as it may cause issues
        // with the server or client. (simply, implement a delay or debounce mechanism os 200ms or so)
        log::debug!("send_resize not implemented yet: {}x{}", width, height);
        if let Some(settings) = self.settings() {
            let _dcml = unsafe {
                DISPLAY_CONTROL_MONITOR_LAYOUT {
                    Flags: DISPLAY_CONTROL_MONITOR_PRIMARY,
                    Left: 0,
                    Top: 0,
                    Width: width,
                    Height: height,
                    Orientation: freerdp_settings_get_uint16(
                        settings,
                        FreeRDP_Settings_Keys_UInt16_FreeRDP_DesktopOrientation,
                    ) as UINT32,
                    DesktopScaleFactor: freerdp_settings_get_uint32(
                        settings,
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopScaleFactor,
                    ),
                    DeviceScaleFactor: freerdp_settings_get_uint32(
                        settings,
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_DeviceScaleFactor,
                    ),
                    PhysicalWidth: width,
                    PhysicalHeight: height,
                }
            };
            unsafe {
                freerdp_settings_set_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_SmartSizingWidth,
                    width,
                );
                freerdp_settings_set_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_SmartSizingHeight,
                    height,
                );
            }
        };
    }

    // Executes the RDP connection until end or stop is requested
    pub fn run(&self) -> Result<()> {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();

        let instance = self
            .instance
            .ok_or_else(|| anyhow::anyhow!("Connection not built"))?;

        let context = instance.context as *mut context::RdpContext;

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
            handles[handle_count] = self.stop_event.as_handle();

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
        unsafe { WaitForSingleObject(self.stop_event.as_handle(), 2000) };

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
        log::debug!(" **** Dropping RDP");

        log::debug!("* Dropping Rdp instance, cleaning up resources...");
        unsafe {
            if let Some(conn) = self.instance {
                freerdp_disconnect(conn.as_mut_ptr());
                freerdp_context_free(conn.as_mut_ptr());
                freerdp_free(conn.as_mut_ptr());
                self.instance = None;
                // Destroy the stop event
                CloseHandle(self.stop_event.as_handle());
            }
        }
    }
}
