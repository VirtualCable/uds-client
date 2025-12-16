// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::{Arc, RwLock};

pub mod callbacks;

mod addins;
pub mod connection;
mod init;
pub mod utils;

pub mod events;
pub mod wlog;

pub mod geom;
pub mod keymap;
pub mod settings;

pub mod context;
pub mod messaging;

// Re-export sys module
pub mod sys;

pub mod channels;

#[derive(Debug)]
pub struct Config {
    settings: settings::RdpSettings,
    callbacks: callbacks::Callbacks,
    use_rgba: bool, // Whether to use RGBA pixel format or BGRA
}

#[derive(Debug)]
pub struct Rdp {
    config: Config,
    instance: Option<utils::SafePtr<freerdp_sys::freerdp>>,
    update_tx: Option<messaging::Sender>,
    // GDI lock for thread safety
    gdi_lock: Arc<RwLock<()>>,
    // Note: We need a clonable safe struct for channels
    // because they are initialized after connection is created, on a later step
    channels: Arc<RwLock<channels::RdpChannels>>,
    stop_event: utils::SafeHandle,
    _pin: std::marker::PhantomPinned, // Do not allow moving
}

#[allow(dead_code)]
impl Rdp {
    pub fn new(
        settings: settings::RdpSettings,
        update_tx: messaging::Sender,
        use_rgba: bool,
    ) -> Self {
        let stop_event: freerdp_sys::HANDLE =
            unsafe { freerdp_sys::CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };

        let stop_event = utils::SafeHandle::new(stop_event).unwrap();
        Rdp {
            config: Config {
                settings,
                use_rgba,
                callbacks: callbacks::Callbacks::default(),
            },
            instance: None,
            update_tx: Some(update_tx),
            gdi_lock: Arc::new(RwLock::new(())),
            channels: Arc::new(RwLock::new(channels::RdpChannels::new())),
            stop_event,
            _pin: std::marker::PhantomPinned,
        }
    }

    pub fn context(&self) -> Option<&context::RdpContext> {
        unsafe {
            if let Some(instance) = self.instance {
                let ctx = instance.context as *mut context::RdpContext;
                if ctx.is_null() { None } else { Some(&*ctx) }
            } else {
                None
            }
        }
    }

    pub fn get_stop_event(&self) -> crate::utils::SafeHandle {
        crate::utils::SafeHandle::new(self.stop_event.as_handle()).unwrap_or_else(|| {
            panic!("Failed to clone stop event handle");
        })
    }

    // Note: For conveinence only, does not has "self"
    pub fn set_stop_event(stop_event: &crate::utils::SafeHandle) {
        unsafe {
            freerdp_sys::SetEvent(stop_event.as_handle());
        }
    }

    pub fn input(&self) -> Option<*mut freerdp_sys::rdpInput> {
        if let Some(context) = self.context() {
            let input = context.context().input;
            if input.is_null() { None } else { Some(input) }
        } else {
            None
        }
    }

    pub fn channels(&self) -> Arc<RwLock<channels::RdpChannels>> {
        self.channels.clone()
    }

    pub fn gdi(&self) -> Option<*mut freerdp_sys::rdpGdi> {
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

    // Note: Rdp does not knows if it is fullscree or not
    // Always returns the current size unless there is no GDI
    // then returns 0x0 (But not Full)
    pub fn screen_size(&self) -> geom::ScreenSize {
        if let Some(gdi) = self.gdi() {
            let width = unsafe { (*gdi).width as u32 };
            let height = unsafe { (*gdi).height as u32 };
            geom::ScreenSize::Fixed(width, height)
        } else {
            geom::ScreenSize::Fixed(0, 0)
        }
    }

    pub fn use_rgba(&self) -> bool {
        self.config.use_rgba
    }
}
