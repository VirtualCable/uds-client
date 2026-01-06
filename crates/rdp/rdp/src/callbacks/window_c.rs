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
use freerdp_sys::{
    BOOL, MONITORED_DESKTOP_ORDER, NOTIFY_ICON_STATE_ORDER, WINDOW_CACHED_ICON_ORDER,
    WINDOW_ICON_ORDER, WINDOW_ORDER_INFO, WINDOW_STATE_ORDER, rdpContext,
};

use super::{super::context::OwnerFromCtx, window::WindowCallbacks};

use shared::log::debug;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    Create,
    Update,
    Icon,
    CachedIcon,
    Delete,
    NotifyIconCreate,
    NotifyIconUpdate,
    NotifyIconDelete,
    MonitoredDesktop,
    NonMonitoredDesktop,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::Create,
            Callbacks::Update,
            Callbacks::Icon,
            Callbacks::CachedIcon,
            Callbacks::Delete,
            Callbacks::NotifyIconCreate,
            Callbacks::NotifyIconUpdate,
            Callbacks::NotifyIconDelete,
            Callbacks::MonitoredDesktop,
            Callbacks::NonMonitoredDesktop,
        ]
    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        let window = (*update).window;
        if update.is_null() || window.is_null() {
            debug!(" **** Window not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::Create => {
                    (*window).WindowCreate = Some(window_create);
                }
                Callbacks::Update => {
                    (*window).WindowUpdate = Some(window_update);
                }
                Callbacks::Icon => {
                    (*window).WindowIcon = Some(window_icon);
                }
                Callbacks::CachedIcon => {
                    (*window).WindowCachedIcon = Some(window_cached_icon);
                }
                Callbacks::Delete => {
                    (*window).WindowDelete = Some(window_delete);
                }
                Callbacks::NotifyIconCreate => {
                    (*window).NotifyIconCreate = Some(notify_icon_create);
                }
                Callbacks::NotifyIconUpdate => {
                    (*window).NotifyIconUpdate = Some(notify_icon_update);
                }
                Callbacks::NotifyIconDelete => {
                    (*window).NotifyIconDelete = Some(notify_icon_delete);
                }
                Callbacks::MonitoredDesktop => {
                    (*window).MonitoredDesktop = Some(monitored_desktop);
                }
                Callbacks::NonMonitoredDesktop => {
                    (*window).NonMonitoredDesktop = Some(non_monitored_desktop);
                }
            }
        }
    }
}

pub extern "C" fn window_create(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    window_state: *const WINDOW_STATE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_window_create(order_info, window_state).into()
    } else {
        true.into()
    }
}
pub extern "C" fn window_update(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    window_state: *const WINDOW_STATE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_window_update(order_info, window_state).into()
    } else {
        true.into()
    }
}

pub extern "C" fn window_icon(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    window_icon: *const WINDOW_ICON_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_window_icon(order_info, window_icon).into()
    } else {
        true.into()
    }
}

pub extern "C" fn window_cached_icon(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    window_cached_icon: *const WINDOW_CACHED_ICON_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_window_cached_icon(order_info, window_cached_icon)
            .into()
    } else {
        true.into()
    }
}

pub extern "C" fn window_delete(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_window_delete(order_info).into()
    } else {
        true.into()
    }
}

pub extern "C" fn notify_icon_create(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    notify_icon_state: *const NOTIFY_ICON_STATE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_notify_icon_create(order_info, notify_icon_state)
            .into()
    } else {
        true.into()
    }
}

pub extern "C" fn notify_icon_update(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    notify_icon_state: *const NOTIFY_ICON_STATE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_notify_icon_update(order_info, notify_icon_state)
            .into()
    } else {
        true.into()
    }
}

pub extern "C" fn notify_icon_delete(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_notify_icon_delete(order_info).into()
    } else {
        true.into()
    }
}

pub extern "C" fn monitored_desktop(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
    monitored_desktop: *const MONITORED_DESKTOP_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_monitored_desktop(order_info, monitored_desktop)
            .into()
    } else {
        true.into()
    }
}

pub extern "C" fn non_monitored_desktop(
    context: *mut rdpContext,
    order_info: *const WINDOW_ORDER_INFO,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_non_monitored_desktop(order_info).into()
    } else {
        true.into()
    }
}
