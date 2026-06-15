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

pub mod cliprdr;
pub mod disp;
pub mod gfx;
pub mod rail;

#[derive(Clone, Debug)]
pub struct RdpChannels {
    disp: Option<disp::DispChannel>,
    cliprdr: Option<cliprdr::RdpClipboard>,
    rail: Option<rail::RailChannel>,
    gfx: Option<gfx::GfxChannel>,
}

impl RdpChannels {
    pub fn new() -> Self {
        RdpChannels {
            disp: None,
            cliprdr: None,
            rail: None,
            gfx: None,
        }
    }

    pub fn set_disp_ptr(&mut self, disp: *mut freerdp_sys::DispClientContext) {
        self.disp = Some(disp::DispChannel::new(disp));
    }

    pub fn clear_disp(&mut self) {
        self.disp = None;
    }

    pub fn disp(&self) -> Option<disp::DispChannel> {
        self.disp.clone()
    }

    pub fn set_cliprdr_ptr(&mut self, cliprdr: *mut freerdp_sys::CliprdrClientContext) {
        let clipboard = cliprdr::RdpClipboard::new(cliprdr);
        self.cliprdr = Some(clipboard);
    }

    pub fn clear_cliprdr(&mut self) {
        self.cliprdr = None;
    }

    pub fn cliprdr(&self) -> Option<cliprdr::RdpClipboard> {
        self.cliprdr.clone()
    }

    pub fn set_rail_ptr(&mut self, rail: *mut freerdp_sys::RailClientContext) {
        self.rail = Some(rail::RailChannel::new(rail));
    }

    pub fn clear_rail(&mut self) {
        self.rail = None;
    }

    pub fn rail(&self) -> Option<rail::RailChannel> {
        self.rail.clone()
    }

    pub fn set_gfx_ptr(&mut self, gfx: *mut freerdp_sys::RdpgfxClientContext) {
        self.gfx = Some(gfx::GfxChannel::new(gfx));
    }

    pub fn clear_gfx(&mut self) {
        self.gfx = None;
    }

    pub fn gfx(&self) -> Option<gfx::GfxChannel> {
        self.gfx.clone()
    }
}

impl Default for RdpChannels {
    fn default() -> Self {
        Self::new()
    }
}
