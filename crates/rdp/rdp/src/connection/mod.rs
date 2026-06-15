// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;

use freerdp_sys::*;

use crate::utils::log;

use crate::{Rdp, context, messaging::RdpMessage};

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
    fn set_rdp_settings(&self) {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();
        unsafe {
            if let Some(settings) = self.settings() {
                // Set Falses first
                [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathInput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathOutput,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_BitmapCompressionDisabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteConsoleAudio, // So audio is not played on server
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DrawAllowSkipAlpha,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxAVC444v2,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxAVC444,
                ]
                .iter()
                .for_each(|i| {
                    freerdp_settings_set_bool(settings, *i, false.into());
                });
                // Then Trues
                [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxThinClient,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxProgressive,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowCacheWaitingList,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DesktopResize,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DynamicResolutionUpdate,
                    // From proper client settings
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FastPathOutput,
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_FrameMarkerCommandEnabled,
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncUpdate,  // Note: currently works badly
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AsyncChannels,
                    // Compression
                    // FreeRDP_Settings_Keys_Bool_FreeRDP_CompressionEnabled,
                    // Graphics
                    // TODO: Test this settings on all platforms (gfx related and h264)
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxH264,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxProgressiveV2,
                    //FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteFxCodec,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_SupportGraphicsPipeline,
                ]
                .iter()
                .for_each(|i| {
                    // Ignore the result, try with best effort
                    freerdp_settings_set_bool(settings, *i, true.into());
                });

                // Set uint32 values
                [
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
                    // Between 100 and 500
                    (FreeRDP_Settings_Keys_UInt32_FreeRDP_FrameAcknowledge, 0),
                    (
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopScaleFactor,
                        (self.config.settings.options.desktop_scale * 100.0) as u32,
                    ),
                    // 100% device = use desktop scale factor
                    // DeviceScaleFactor only allows 100, 140 y 180.. O.o
                    (FreeRDP_Settings_Keys_UInt32_FreeRDP_DeviceScaleFactor, 100),
                ]
                .iter()
                .for_each(|(i, v)| {
                    freerdp_settings_set_uint32(settings, *i, *v);
                });

                // Audio redirection settings
                fn channels(
                    settings: *mut rdpSettings,
                    name: &str,
                    channel: &str,
                    add_static: bool,
                    add_dynamic: bool,
                ) {
                    // Note: We can use the internal freerdp rdpsnd channel subsystems

                    let cname = std::ffi::CString::new(name).unwrap();
                    let cchannel = std::ffi::CString::new(channel).unwrap();
                    let channels: [*const std::os::raw::c_char; 2] =
                        [cname.as_ptr(), cchannel.as_ptr()];
                    unsafe {
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

                if self.config.settings.redirections.audio {
                    // Sound redirection
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
                        false.into(), // Always false, we want audio on client
                    );
                    let channel = format!("sys:{}", crate::addins::RDPSND_SUBSYSTEM_CUSTOM);
                    channels(settings, "rdpsnd", &channel, true, true);
                    // Default subsystem right now
                    // channels(settings, "rdpsnd", None, true, true);
                }
                // Microphone redirection
                if self.config.settings.redirections.mic {
                    freerdp_settings_set_bool(
                        settings,
                        FreeRDP_Settings_Keys_Bool_FreeRDP_AudioCapture,
                        true.into(),
                    );
                    let channel = format!("sys:{}", crate::addins::AUDIN_SUBSYSTEM_CUSTOM);
                    channels(settings, "audin", &channel, false, true);
                }
                // Webcam redirection
                if let Some(ref webcam) = self.config.settings.redirections.webcam
                    && webcam.enabled
                    && let Some(ref webcam_int) = self.config.integrations.webcam
                {
                    let (cam_w, cam_h) = webcam_int.get_camera_dimensions();
                    if cam_w > 0 && cam_h > 0 {
                        log::info!("Webcam redirection: Camera detected at {}x{}", cam_w, cam_h);

                        // Parse UDSLAUNCHER_LIMITS (width,height,fps,quality)
                        let parse_launcher_limits =
                            || -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
                                if let Ok(val) = std::env::var("UDSLAUNCHER_LIMITS") {
                                    let parts: Vec<&str> =
                                        val.split(',').map(|s| s.trim()).collect();
                                    let get_part = |idx: usize| -> Option<u32> {
                                        parts.get(idx).and_then(|&s| {
                                            if s.is_empty() {
                                                None
                                            } else {
                                                s.parse::<u32>().ok()
                                            }
                                        })
                                    };
                                    (get_part(0), get_part(1), get_part(2), get_part(3))
                                } else {
                                    (None, None, None, None)
                                }
                            };
                        let (env_w, env_h, env_fps, env_q) = parse_launcher_limits();

                        // Base values from settings
                        let mut final_quality = webcam.quality;
                        let mut final_fps = webcam.fps;
                        let (mut max_w, mut max_h) = if let Some((w, h)) = webcam.size_limit {
                            (w, h)
                        } else {
                            (0, 0)
                        };

                        // Apply env limits (only allowing them to decrease values, never increase)
                        if let Some(eq) = env_q {
                            final_quality = final_quality.min(eq);
                        }
                        if let Some(efps) = env_fps {
                            final_fps = final_fps.min(efps);
                        }
                        if let Some(ew) = env_w {
                            max_w = if max_w == 0 { ew } else { max_w.min(ew) };
                        }
                        if let Some(eh) = env_h {
                            max_h = if max_h == 0 { eh } else { max_h.min(eh) };
                        }

                        webcam_int.set_limits(final_quality, final_fps, max_w, max_h);

                        let channel = format!("sys:{}", crate::addins::WEBCAM_SUBSYSTEM_CUSTOM);
                        channels(settings, "rdpecam", &channel, false, true);
                    } else {
                        log::warn!(
                            "Webcam redirection enabled in settings, but no webcam was detected on the system. Webcam redirection will not be enabled."
                        );
                    }
                }

                // Set config settings for clipboard redirection
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectClipboard,
                    self.config.settings.redirections.clipboard.into(),
                );

                if self.config.settings.redirections.printing {
                    freerdp_settings_set_bool(
                        settings,
                        FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectPrinters,
                        true.into(),
                    );
                }

                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_IgnoreCertificate,
                    (!self.config.settings.options.verify_cert).into(),
                );

                // NLA setting
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_NlaSecurity,
                    self.config.settings.options.use_nla.into(),
                );

                let drives_to_redirect = std::ffi::CString::new(
                    self.config
                        .settings
                        .redirections
                        .drives
                        .iter()
                        .map(|s| match s.as_str() {
                            "all" => "*",
                            "DynamicDrives" => "DynamicDrives",
                            other => other,
                        })
                        .collect::<Vec<&str>>()
                        .join(";"),
                )
                .unwrap();

                let len_drives = self.config.settings.redirections.drives.len();
                if len_drives > 0 {
                    log::debug!(
                        "Enabling drive redirection for: {}",
                        self.config.settings.redirections.drives.join(", ")
                    );
                    freerdp_settings_set_bool(
                        settings,
                        FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectDrives,
                        true.into(),
                    );

                    freerdp_settings_set_string(
                        settings,
                        FreeRDP_Settings_Keys_String_FreeRDP_DrivesToRedirect,
                        drives_to_redirect.as_ptr(),
                    );
                }

                log::debug!("Best experience: {}", self.config.settings.best_experience);
                // Best experience settings (enabled an disabled due to Disable && Allow
                [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableWallpaper,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableFullWindowDrag,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableMenuAnims,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableThemes,
                ]
                .iter()
                .for_each(|key| {
                    freerdp_settings_set_bool(
                        settings,
                        *key,
                        (!self.config.settings.best_experience).into(),
                    );
                });
                [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowFontSmoothing,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_AllowDesktopComposition,
                ]
                .iter()
                .for_each(|key| {
                    freerdp_settings_set_bool(
                        settings,
                        *key,
                        self.config.settings.best_experience.into(),
                    );
                });

                if self.config.settings.features.disable_threading {
                    freerdp_settings_set_uint32(
                        settings,
                        FreeRDP_Settings_Keys_UInt32_FreeRDP_ThreadingFlags,
                        THREADING_FLAGS_DISABLE_THREADS,
                    );
                }

                if self.config.settings.features.force_software_gdi {
                    freerdp_settings_set_bool(
                        settings,
                        FreeRDP_Settings_Keys_Bool_FreeRDP_SoftwareGdi,
                        true.into(),
                    );
                }

                // Set perfromance flags from settings
                freerdp_sys::freerdp_performance_flags_make(settings);
                // Finally, set rail settings if needed
                self.set_rail_settings();
            } else {
                log::debug!("Connection not built, cannot optimize settings.");
            }
        }
    }

    fn set_rail_settings(&self) {
        #[cfg(debug_assertions)]
        self.debug_assert_instance();
        unsafe {
            if let Some(settings) = self.settings()
                && let Some(ref rail) = self.config.settings.rail
            {
                log::debug!("Enabling RAIL mode in FreeRDP settings");
                freerdp_settings_set_bool(
                    settings,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_RemoteApplicationMode,
                    true.into(),
                );
                #[allow(clippy::unnecessary_cast)]
                // Windows/linux/mac differ on UINT32 implementation
                freerdp_settings_set_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_RemoteApplicationSupportLevel,
                    (freerdp_sys::RAIL_LEVEL_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_HANDSHAKE_EX_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_SHELL_INTEGRATION_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_LANGUAGE_IME_SYNC_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_SERVER_TO_CLIENT_IME_SYNC_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_HIDE_MINIMIZED_APPS_SUPPORTED
                        | freerdp_sys::RAIL_LEVEL_WINDOW_CLOAKING_SUPPORTED)
                        as u32,
                );
                freerdp_settings_set_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_RemoteApplicationSupportMask,
                    0xFFFFFFFF, // Allow all capabilities negotiated with server
                );

                let capp = std::ffi::CString::new(rail.app.clone()).unwrap();
                freerdp_settings_set_string(
                    settings,
                    FreeRDP_Settings_Keys_String_FreeRDP_RemoteApplicationProgram,
                    capp.as_ptr(),
                );

                if let Some(rail_args) = &rail.args {
                    let cargs = std::ffi::CString::new(rail_args.clone()).unwrap();
                    freerdp_settings_set_string(
                        settings,
                        FreeRDP_Settings_Keys_String_FreeRDP_RemoteApplicationCmdLine,
                        cargs.as_ptr(),
                    );
                }

                if let Some(rail_dir) = &rail.working_dir {
                    let cdir = std::ffi::CString::new(rail_dir.clone()).unwrap();
                    freerdp_settings_set_string(
                        settings,
                        FreeRDP_Settings_Keys_String_FreeRDP_RemoteApplicationWorkingDir,
                        cdir.as_ptr(),
                    );
                }

                for key in [
                    FreeRDP_Settings_Keys_Bool_FreeRDP_Workarea,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableWallpaper,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_DisableFullWindowDrag,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_GfxH264,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_SupportGraphicsPipeline,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_Workarea,
                    // Explicitly enable markers
                    FreeRDP_Settings_Keys_Bool_FreeRDP_FrameMarkerCommandEnabled,
                    FreeRDP_Settings_Keys_Bool_FreeRDP_SurfaceFrameMarkerEnabled,
                    // If enabled this, stops working :)
                    FreeRDP_Settings_Keys_Bool_FreeRDP_HiDefRemoteApp,
                ] {
                    freerdp_settings_set_bool(settings, key, true.into());
                }

                // Allow for now single element, will include more in a future
                #[allow(clippy::single_element_loop)]
                for key in [FreeRDP_Settings_Keys_Bool_FreeRDP_GfxH264] {
                    freerdp_settings_set_bool(settings, key, false.into());
                }

                if self.config.settings.features.force_software_gdi {
                    freerdp_settings_set_bool(
                        settings,
                        FreeRDP_Settings_Keys_Bool_FreeRDP_SoftwareGdi,
                        true.into(),
                    );
                }

                // Enable Frame Acknowledge for GFX flow control
                // Increasing this value reduces the number of ACKs sent, which can lower CPU usage
                freerdp_settings_set_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_FrameAcknowledge,
                    1,
                );
            }
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
        self.set_rdp_settings();

        unsafe {
            if let Some(instance) = self.instance {
                if freerdp_connect(instance.as_mut_ptr()) == 0 {
                    let code = freerdp_get_last_error(instance.context); // Allow panic if context is null
                    // Get error string
                    let error_str = freerdp_get_last_error_string(code);
                    let error_str = if error_str.is_null() {
                        "Unknown error".to_string()
                    } else {
                        std::ffi::CStr::from_ptr(error_str)
                            .to_string_lossy()
                            .into_owned()
                    };
                    return Err(anyhow::anyhow!("{} : {:08X}", error_str, code));
                }
                log::debug!("Connected to RDP server successfully.");
            } else {
                return Err(anyhow::anyhow!("Connection not built"));
            }
        }
        Ok(())
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

            if handle_count > handles.len() - 2 {
                log::error!("Too many event handles from FreeRDP: {}", handle_count);
                tx.send(RdpMessage::Error(
                    "Too many event handles, exiting.".to_string(),
                ))?;
                break;
            }

            // Add our stop event handle and command event handle
            handles[handle_count] = self.stop_event.as_handle();
            handles[handle_count + 1] = self.command_event.as_handle();

            let wait_result = unsafe {
                WaitForMultipleObjects(
                    (handle_count + 2) as u32,
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

            // If our command event is signaled, process commands
            if wait_result == (handle_count as u32 + 1) {
                while let Ok(cmd) = self.command_rx.try_recv() {
                    match cmd {
                        crate::messaging::RdpCommand::Input(ev) => unsafe {
                            if let Some(instance) = self.instance.as_ref() {
                                let instance_ptr = instance.as_mut_ptr();
                                if instance_ptr.is_null() {
                                    continue;
                                }
                                let context = (*instance_ptr).context;
                                if context.is_null() {
                                    continue;
                                }
                                let input = (*context).input;
                                if input.is_null() {
                                    continue;
                                }

                                match ev {
                                    crate::messaging::InputEvent::Keyboard {
                                        scancode,
                                        pressed,
                                        repeat,
                                    } => {
                                        freerdp_input_send_keyboard_event_ex(
                                            input,
                                            if pressed { 1 } else { 0 },
                                            if repeat { 1 } else { 0 },
                                            scancode as u32,
                                        );
                                    }
                                    crate::messaging::InputEvent::Mouse { flags, x, y } => {
                                        freerdp_input_send_mouse_event(input, flags, x, y);
                                    }
                                    crate::messaging::InputEvent::ExtendedMouse { flags, x, y } => {
                                        freerdp_input_send_extended_mouse_event(input, flags, x, y);
                                    }
                                    crate::messaging::InputEvent::Unicode { code } => {
                                        freerdp_input_send_unicode_keyboard_event(input, 0, code);
                                    }
                                }
                            }
                        },
                        crate::messaging::RdpCommand::ViewportMove {
                            window_id,
                            left,
                            top,
                            right,
                            bottom,
                        } => {
                            if let Some(rail) = self.channels.read().unwrap().rail() {
                                rail.send_window_move(window_id, left, top, right, bottom);
                            }
                        }
                        crate::messaging::RdpCommand::LaunchRailApp { app, args, dir } => {
                            if let Some(rail) = self.channels.read().unwrap().rail() {
                                rail.send_execute(&app, &args, &dir);
                            }
                        }
                        crate::messaging::RdpCommand::Close => {
                            unsafe {
                                freerdp_set_last_error_ex(
                                    context as *mut rdpContext,
                                    0, // Success
                                    std::ptr::null(),
                                    std::ptr::null(),
                                    0,
                                );
                            }
                            break;
                        }
                        _ => {}
                    }
                }
                // Continue to wait/process events
                continue;
            }

            if unsafe { freerdp_check_event_handles(context as *mut rdpContext) } == 0 {
                if unsafe { client_auto_reconnect(instance.as_mut_ptr()) } != 0 {
                    log::debug!("Reconnected successfully");
                } else {
                    tx.send(RdpMessage::Error(
                        "Disconnected (could not reconnect)".to_string(),
                    ))?;
                    return Err(anyhow::anyhow!("Disconnected (could not reconnect)"));
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
        // If we have a clipboard native, stop it
        if let Some(ref clipboard) = self.config.integrations.clipboard {
            clipboard.stop();
        }

        log::debug!("* Dropping Rdp instance, cleaning up resources...");
        unsafe {
            if let Some(conn) = self.instance {
                let ctx = conn.context as *mut context::RdpContext;
                if !ctx.is_null() {
                    log::debug!(" **** Clearing owner in context to prevent UAF in callbacks");
                    (*ctx).owner = std::ptr::null_mut();
                }

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
