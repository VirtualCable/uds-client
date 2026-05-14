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

use crate::context::OwnerFromCtx;
use crate::utils;
use freerdp_sys::*;
use shared::log;

#[derive(Clone, Debug)]
pub struct RailChannel {
    ptr: Option<utils::SafePtr<freerdp_sys::RailClientContext>>,
}

unsafe impl Send for RailChannel {}
unsafe impl Sync for RailChannel {}

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
            log::debug!("RAIL: Initializing callbacks (Simplified)");
            let context = ptr.as_mut_ptr();
            unsafe {
                (*context).ServerHandshake = Some(server_handshake);
                (*context).ServerHandshakeEx = Some(server_handshake_ex);
                (*context).ServerExecuteResult = Some(server_execute_result);
                // Standard plugins often set OnOpen to send the handshake to the server
                (*context).OnOpen = Some(on_open);

                // We leave other callbacks as NULL for now,
                // allowing FreeRDP standard handlers or ignorance for better compatibility.
                (*context).ServerSystemParam = None;
                (*context).ServerLocalMoveSize = None;
                (*context).ServerMinMaxInfo = None;
                (*context).ServerLanguageBarInfo = None;
                (*context).ServerGetAppIdResponse = None;
                (*context).ServerTaskBarInfo = None;
                (*context).ServerZOrderSync = None;
                (*context).ServerCloak = None;
            }
        }
    }

    pub fn send_system_command(&self, window_id: u32, command: u16) {
        if let Some(ptr) = &self.ptr {
            let context = ptr.as_mut_ptr();
            let syscmd = RAIL_SYSCOMMAND_ORDER {
                windowId: window_id,
                command,
            };
            unsafe {
                if let Some(client_sys_cmd) = (*context).ClientSystemCommand {
                    log::debug!(
                        "RAIL: Sending System Command {} for window {}",
                        command,
                        window_id
                    );
                    client_sys_cmd(context, &syscmd);
                }
            }
        }
    }

    pub fn send_window_move(&self, window_id: u32, left: i16, top: i16, right: i16, bottom: i16) {
        if let Some(ptr) = &self.ptr {
            let context = ptr.as_mut_ptr();
            let movecmd = RAIL_WINDOW_MOVE_ORDER {
                windowId: window_id,
                left,
                top,
                right,
                bottom,
            };
            unsafe {
                if let Some(client_win_move) = (*context).ClientWindowMove {
                    log::trace!("RAIL: Sending Window Move for window {}", window_id);
                    client_win_move(context, &movecmd);
                }
            }
        }
    }

    /// Send a ClientActivate PDU to the server for the given window.
    /// Setting `enabled = true` tells Windows this window is now focused/active,
    /// which causes it to force a full redraw — useful after SC_RESTORE.
    pub fn send_activate(&self, window_id: u32, enabled: bool) {
        if let Some(ptr) = &self.ptr {
            let context = ptr.as_mut_ptr();
            let activate = RAIL_ACTIVATE_ORDER {
                windowId: window_id,
                enabled: if enabled { 1 } else { 0 },
            };
            unsafe {
                if let Some(client_activate) = (*context).ClientActivate {
                    log::trace!(
                        "RAIL: Sending ClientActivate(enabled={}) for window {}",
                        enabled,
                        window_id
                    );
                    client_activate(context, &activate);
                }
            }
        }
    }

    /// Send a ClientExecute PDU to launch a new RemoteApp on an already-running session.
    pub fn send_execute(&self, app: &str, args: &str, dir: &str) {
        if let Some(ptr) = &self.ptr {
            let context = ptr.as_mut_ptr();
            let capp = std::ffi::CString::new(app).unwrap();
            let cdir = std::ffi::CString::new(
                if dir.is_empty() { "C:\\" } else { dir }
            ).unwrap();
            let cargs = std::ffi::CString::new(args).unwrap();
            let exec = RAIL_EXEC_ORDER {
                flags: RAIL_EXEC_FLAG_EXPAND_ARGUMENTS as u16,
                RemoteApplicationProgram: capp.as_ptr(),
                RemoteApplicationWorkingDir: cdir.as_ptr(),
                RemoteApplicationArguments: cargs.as_ptr(),
            };
            unsafe {
                if let Some(execute_fn) = (*context).ClientExecute {
                    log::info!("RAIL: Sending ClientExecute for {}", app);
                    execute_fn(context, &exec);
                }
            }
        }
    }
}

fn complete_handshake(context: *mut RailClientContext) -> UINT {
    unsafe {
        let rdp_context = (*context).custom as *mut rdpContext;
        if let Some(rdp) = rdp_context.owner() {
            log::info!("RAIL: Completing handshake PDU sequence...");

            // 1. Client Information/Status
            if let Some(client_info_fn) = (*context).ClientInformation {
                let status = RAIL_CLIENT_STATUS_ORDER {
                    // ALLOWLOCALMOVESIZE | ZORDER_SYNC | WINDOW_RESIZE_MARGIN_SUPPORTED | APPBAR_REMOTING_SUPPORTED
                    flags: 0x01 | 0x04 | 0x08 | 0x40,
                };
                log::debug!("RAIL: Sending ClientInformation");
                client_info_fn(context, &status);
            }

            // 2. Client System Param (Work Area)
            if let Some(sys_param_fn) = (*context).ClientSystemParam {
                let mut sysparam: RAIL_SYSPARAM_ORDER = std::mem::zeroed();
                sysparam.params = SPI_SET_WORK_AREA;
                sysparam.workArea.left = 0;
                sysparam.workArea.top = 0;
                sysparam.workArea.right = rdp.config.settings.screen_size.width() as u16;
                sysparam.workArea.bottom = rdp.config.settings.screen_size.height() as u16;

                log::debug!("RAIL: Sending ClientSystemParam (WorkArea)");
                sys_param_fn(context, &sysparam);
            }

            // 3. Client Execute (Launch RemoteApp)
            if let Some(app) = &rdp.config.settings.rail_app
                && let Some(execute_fn) = (*context).ClientExecute
            {
                let capp = std::ffi::CString::new(app.clone()).unwrap();
                let cdir = std::ffi::CString::new(
                    rdp.config
                        .settings
                        .rail_working_dir
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("C:\\"),
                )
                .unwrap();
                let cargs =
                    std::ffi::CString::new(rdp.config.settings.rail_args.as_deref().unwrap_or(""))
                        .unwrap();
                let exec = RAIL_EXEC_ORDER {
                    flags: RAIL_EXEC_FLAG_EXPAND_ARGUMENTS as u16,
                    RemoteApplicationProgram: capp.as_ptr(),
                    RemoteApplicationWorkingDir: cdir.as_ptr(),
                    RemoteApplicationArguments: cargs.as_ptr(),
                };
                log::info!(
                    "RAIL: Sending ClientExecute for RemoteApp: {} (args={:?}, dir={:?})",
                    app,
                    rdp.config.settings.rail_args,
                    rdp.config.settings.rail_working_dir
                );
                execute_fn(context, &exec);
            }
        }
    }
    0 // CHANNEL_RC_OK
}

extern "C" fn on_open(_context: *mut RailClientContext, send_handshake: *mut BOOL) -> UINT {
    log::debug!("RAIL: Received OnOpen");
    unsafe {
        *send_handshake = true.into();
    }
    0 // CHANNEL_RC_OK
}

extern "C" fn server_handshake(
    context: *mut RailClientContext,
    handshake: *const RAIL_HANDSHAKE_ORDER,
) -> UINT {
    unsafe {
        log::debug!(
            "RAIL: Received ServerHandshake (build {})",
            (*handshake).buildNumber
        );
        complete_handshake(context);
        0
    }
}

extern "C" fn server_handshake_ex(
    context: *mut RailClientContext,
    handshake_ex: *const RAIL_HANDSHAKE_EX_ORDER,
) -> UINT {
    unsafe {
        log::debug!(
            "RAIL: Received ServerHandshakeEx (build {}, flags 0x{:X})",
            (*handshake_ex).buildNumber,
            (*handshake_ex).railHandshakeFlags
        );
        complete_handshake(context);
        0
    }
}

extern "C" fn server_execute_result(
    _context: *mut RailClientContext,
    exec_result: *const RAIL_EXEC_RESULT_ORDER,
) -> UINT {
    let result = unsafe { (*exec_result).execResult };
    log::info!("RAIL: RemoteApp execution result: 0x{:08X}", result as u32);
    0 // CHANNEL_RC_OK
}
