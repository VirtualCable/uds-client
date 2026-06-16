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

use freerdp_sys::*;

use crate::utils::log;

use crate::{
    callbacks::channels,
    channels::{cliprdr::register_cliprdr_callbacks, disp::register_disp_callbacks},
};

use super::Rdp;

// Simple macro for, from a channel string, that has a \0 at the end, returns the value without this
macro_rules! channel_name {
    ($name:expr) => {
        &$name[..$name.len() - 1]
    };
}

impl channels::ChannelsCallbacks for Rdp {
    fn on_channel_connected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        let context_ptr = if let Some(context) = self.context() {
            context as *const _ as *mut std::os::raw::c_void
        } else {
            log::error!("**** No context found for channel connection.");
            return false;
        };
        match name.as_bytes() {
            name if name == channel_name!(freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME) => {
                if !self.config.settings.redirections.clipboard {
                    log::debug!("**** CLIPRDR channel connection rejected (disabled in settings).");
                    return false;
                }
                let interface = p_interface as *mut CliprdrClientContext;
                unsafe {
                    (*interface).custom = context_ptr;
                    register_cliprdr_callbacks(&mut *interface);
                }

                log::debug!("**** CLIPRDR channel connection accepted.");
                self.channels.write().unwrap().set_cliprdr_ptr(interface);
                true
            }
            name if name == channel_name!(freerdp_sys::RAIL_SVC_CHANNEL_NAME) => {
                log::debug!("**** RAIL channel connection accepted.");
                let interface = p_interface as *mut RailClientContext;
                unsafe {
                    (*interface).custom = context_ptr;
                }
                {
                    let mut channels = self.channels.write().unwrap();
                    channels.set_rail_ptr(interface);
                }
                true
            }
            name if name == channel_name!(freerdp_sys::DISP_DVC_CHANNEL_NAME) => {
                let interface = p_interface as *mut DispClientContext;
                unsafe { register_disp_callbacks(interface) };
                self.channels.write().unwrap().set_disp_ptr(interface);
                true
            }
            name if name == channel_name!(freerdp_sys::AUDIN_DVC_CHANNEL_NAME) => {
                log::debug!("**** AUDIO_INPUT channel connection accepted.");
                true
            }
            name if name == channel_name!(freerdp_sys::RDPSND_DVC_CHANNEL_NAME) => {
                log::debug!("**** AUDIO_PLAYBACK_DVC channel connection accepted.");
                true
            }
            name if name == channel_name!(freerdp_sys::RDPGFX_DVC_CHANNEL_NAME) => {
                log::debug!("**** GFX channel connection accepted.");
                let interface = p_interface as *mut RdpgfxClientContext;
                unsafe {
                    (*interface).custom = context_ptr;
                    let gdi = (*self.instance.as_deref().unwrap().context).gdi;
                    let mut channels = self.channels.write().unwrap();
                    channels.set_gfx_ptr(interface);
                    if let Some(gfx) = channels.gfx() {
                        // Hook GDI to enable internal FreeRDP blitting from GFX surfaces to GDI buffer
                        // Use modern FreeRDP 3 initialization
                        gfx.hook_gdi(gdi);
                        // gdi_graphics_pipeline_init overwrites our EndPaint/BeginPaint with
                        // its own internal versions. The GFX compositing (H264 decode → surface
                        // → primary buffer) happens in GFX surface callbacks, NOT in EndPaint.
                        // EndPaint just checks the invalidation region the compositing left behind.
                        // So we MUST re-register our callbacks to get notified of updates.
                        log::debug!("Re-registering EndPaint/BeginPaint after GFX pipeline init");
                        let context = self.instance.as_deref().unwrap().context;
                        crate::callbacks::update_c::set_callbacks(
                            context,
                            &[
                                crate::callbacks::update_c::Callbacks::BeginPaint,
                                crate::callbacks::update_c::Callbacks::EndPaint,
                            ],
                        );
                    }
                }
                true
            }
            _ => false, // Defaults to false
        }
    }

    fn on_channel_disconnected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        match name.as_bytes() {
            name if name == channel_name!(freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME) => {
                log::debug!("**** CLIPRDR channel disconnected.");
                if let Some(ref clipboard_integration) = self.config.integrations.clipboard {
                    clipboard_integration.stop();
                }
                self.channels.write().unwrap().clear_cliprdr();
                let interface = p_interface as *mut CliprdrClientContext;
                unsafe {
                    (*interface).custom = std::ptr::null_mut();
                }
                true
            }
            name if name == channel_name!(freerdp_sys::RAIL_SVC_CHANNEL_NAME) => {
                log::debug!("**** RAIL channel disconnected.");
                self.channels.write().unwrap().clear_rail();
                let interface = p_interface as *mut RailClientContext;
                unsafe {
                    (*interface).custom = std::ptr::null_mut();
                }
                true
            }
            name if name == channel_name!(freerdp_sys::DISP_DVC_CHANNEL_NAME) => {
                log::debug!("**** DISP channel disconnected.");
                self.channels.write().unwrap().clear_disp();
                true
            }
            name if name == channel_name!(freerdp_sys::AUDIN_DVC_CHANNEL_NAME) => {
                log::debug!("**** AUDIO_INPUT channel disconnected.");
                if let Some(ref audio_input) = self.config.integrations.audio_input {
                    audio_input.stop();
                }
                true
            }
            name if name == channel_name!(freerdp_sys::RDPSND_DVC_CHANNEL_NAME) => {
                log::debug!("**** AUDIO_PLAYBACK_DVC channel disconnected.");
                true
            }
            name if name == channel_name!(freerdp_sys::RDPGFX_DVC_CHANNEL_NAME) => {
                log::debug!("**** GFX channel disconnected.");
                let interface = p_interface as *mut RdpgfxClientContext;
                unsafe {
                    if let Some(instance) = self.instance.as_deref() {
                        let gdi = (*instance.context).gdi;
                        if !gdi.is_null() {
                            gdi_graphics_pipeline_uninit(gdi, interface);
                        }
                    }
                }
                self.channels.write().unwrap().clear_gfx();
                true
            }
            _ => false, // Defaults to false
        }
    }
}
