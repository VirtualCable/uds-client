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

use crate::utils;
use shared::log;
use freerdp_sys::*;

#[derive(Clone, Debug)]
pub struct RailChannel {
    #[allow(dead_code)]
    ptr: Option<utils::SafePtr<freerdp_sys::RailClientContext>>,
}

impl RailChannel {
    pub fn new(ptr: *mut freerdp_sys::RailClientContext) -> Self {
        let mut slf = Self {
            ptr: utils::SafePtr::new(ptr),
        };
        slf.init_callbacks();
        slf
    }

    fn init_callbacks(&mut self) {
        if let Some(ptr) = &self.ptr {
            log::debug!("RAIL: Initializing callbacks");
            let context = ptr.as_mut_ptr();
            unsafe {
                (*context).ServerHandshake = Some(server_handshake);
                (*context).ServerHandshakeEx = Some(server_handshake_ex);
                (*context).ServerExecuteResult = Some(server_execute_result);
                (*context).ServerSystemParam = Some(server_system_param);
                (*context).ServerLocalMoveSize = Some(server_local_move_size);
                (*context).ServerMinMaxInfo = Some(server_min_max_info);
                (*context).ServerLanguageBarInfo = Some(server_language_bar_info);
                (*context).ServerGetAppIdResponse = Some(server_get_appid_response);
                (*context).OnOpen = Some(on_open);
            }
        }
    }

    pub fn start(&self) {
        if let Some(ptr) = &self.ptr {
            log::debug!("RAIL: Manually starting RAIL handshake");
            unsafe {
                freerdp_sys::client_rail_server_start_cmd(ptr.as_mut_ptr());
            }
        }
    }
}

extern "C" fn on_open(
    _context: *mut RailClientContext,
    send_handshake: *mut BOOL,
) -> UINT {
    log::debug!("RAIL: Received OnOpen");
    unsafe {
        *send_handshake = 1.into();
    }
    0 // CHANNEL_RC_OK
}

extern "C" fn server_handshake(
    context: *mut RailClientContext,
    handshake: *const RAIL_HANDSHAKE_ORDER,
) -> UINT {
    unsafe {
        log::debug!("RAIL: Received ServerHandshake (build {})", (*handshake).buildNumber);
        freerdp_sys::client_rail_server_start_cmd(context)
    }
}

extern "C" fn server_handshake_ex(
    context: *mut RailClientContext,
    handshake_ex: *const RAIL_HANDSHAKE_EX_ORDER,
) -> UINT {
    unsafe {
        log::debug!("RAIL: Received ServerHandshakeEx (build {}, flags 0x{:X})", (*handshake_ex).buildNumber, (*handshake_ex).railHandshakeFlags);
        freerdp_sys::client_rail_server_start_cmd(context)
    }
}

extern "C" fn server_execute_result(
    _context: *mut RailClientContext,
    exec_result: *const RAIL_EXEC_RESULT_ORDER,
) -> UINT {
    let result = unsafe { (*exec_result).rawResult };
    log::debug!("RAIL: ServerExecuteResult: 0x{:08X}", result);
    0 // CHANNEL_RC_OK
}

extern "C" fn server_system_param(
    _context: *mut RailClientContext,
    _sysparam: *const RAIL_SYSPARAM_ORDER,
) -> UINT {
    log::debug!("RAIL: ServerSystemParam");
    0 // CHANNEL_RC_OK
}

extern "C" fn server_local_move_size(
    _context: *mut RailClientContext,
    _local_move_size: *const RAIL_LOCALMOVESIZE_ORDER,
) -> UINT {
    log::debug!("RAIL: ServerLocalMoveSize");
    0 // CHANNEL_RC_OK
}

extern "C" fn server_min_max_info(
    _context: *mut RailClientContext,
    _minmax_info: *const RAIL_MINMAXINFO_ORDER,
) -> UINT {
    log::debug!("RAIL: ServerMinMaxInfo");
    0 // CHANNEL_RC_OK
}

extern "C" fn server_language_bar_info(
    _context: *mut RailClientContext,
    _langbar_info: *const RAIL_LANGBAR_INFO_ORDER,
) -> UINT {
    log::debug!("RAIL: ServerLanguageBarInfo");
    0 // CHANNEL_RC_OK
}

extern "C" fn server_get_appid_response(
    _context: *mut RailClientContext,
    _get_appid_resp: *const RAIL_GET_APPID_RESP_ORDER,
) -> UINT {
    log::debug!("RAIL: ServerGetAppIdResponse");
    0 // CHANNEL_RC_OK
}
