// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use crate::Rdp;

pub mod altsec;
pub mod altsec_c;

pub mod input;
pub mod input_c;

pub mod graphics;
pub mod graphics_c;

pub mod pointer_update;
pub mod pointer_update_c;

pub mod primary;
pub mod primary_c;

pub mod instance;
pub mod instance_c;

pub mod secondary;
pub mod secondary_c;

pub mod update;
pub mod update_c;

pub mod window;
pub mod window_c;

pub mod entrypoint;
pub mod entrypoint_c;

pub mod channels_c;
pub mod channels;

#[derive(Debug)]
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

impl Rdp {
    #[allow(dead_code)]
    pub fn set_update_callbacks(&mut self, callbacks: Vec<update_c::Callbacks>) {
        self.config.callbacks.update = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_window_callbacks(&mut self, callbacks: Vec<window_c::Callbacks>) {
        self.config.callbacks.window = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_primary_callbacks(&mut self, callbacks: Vec<primary_c::Callbacks>) {
        self.config.callbacks.primary = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_secondary_callbacks(&mut self, callbacks: Vec<secondary_c::Callbacks>) {
        self.config.callbacks.secondary = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_altsec_callbacks(&mut self, callbacks: Vec<altsec_c::Callbacks>) {
        self.config.callbacks.altsec = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_pointer_callbacks(&mut self, callbacks: Vec<pointer_update_c::Callbacks>) {
        self.config.callbacks.pointer = callbacks;
    }

    #[allow(dead_code)]
    pub fn set_input_callbacks(&mut self, callbacks: Vec<input_c::Callbacks>) {
        self.config.callbacks.input = callbacks;
    }

    pub fn get_callbacks(&self) -> &Callbacks {
        &self.config.callbacks
    }
}