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
use shared::log;

// RDP needs WinSock to be initialized befere, at least, open connection
fn init_socks() {
    log::debug!("Initializing WinSock...");

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Networking::WinSock::{WSADATA, WSAStartup};

        let mut wsa_data = std::mem::zeroed::<WSADATA>();
        let version: u16 = 0x0202;

        // 0x101 = MAKEWORD(1, 1), MAKEWORD(2, 2) for WinSock 2.2
        let ret = WSAStartup(version, &mut wsa_data);
        if ret != 0 {
            panic!("WSAStartup failed: {}", ret);
        }
    }
}

fn uninit_socks() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::Networking::WinSock::WSACleanup();
    }
}

fn init_callbacks() {
    log::debug!("Initializing RDP Callbacks...");
    // Ensure that the callback is set to our wrapper function
    // We will have only that function with varargs disabled
    use super::callbacks::instance_c::get_access_token_no_varargs;

    unsafe { freerdp_sys::set_rust_get_access_token_cb(get_access_token_no_varargs) };
}

pub fn initialize() {
    init_socks();
    init_callbacks();
}

pub fn uninitialize() {
    // Currently, we do not need any special handling here.
    uninit_socks();
}
