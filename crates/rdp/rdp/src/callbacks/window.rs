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
use freerdp_sys::{
    MONITORED_DESKTOP_ORDER, NOTIFY_ICON_STATE_ORDER, WINDOW_CACHED_ICON_ORDER, WINDOW_ICON_ORDER,
    WINDOW_ORDER_INFO, WINDOW_STATE_ORDER,
};

use shared::log::debug;

pub trait WindowCallbacks {
    fn on_window_create(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_window_create: order_info={:?}, window_state={:?}",
            order_info, window_state
        );
        true
    }

    fn on_window_update(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_window_update: order_info={:?}, window_state={:?}",
            order_info, window_state
        );
        true
    }

    fn on_window_icon(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_icon: *const WINDOW_ICON_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_window_icon: order_info={:?}, window_icon={:?}",
            order_info, window_icon
        );
        true
    }

    fn on_window_cached_icon(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_cached_icon: *const WINDOW_CACHED_ICON_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_window_cached_icon: order_info={:?}, window_cached_icon={:?}",
            order_info, window_cached_icon
        );
        true
    }

    fn on_window_delete(&self, order_info: *const WINDOW_ORDER_INFO) -> bool {
        debug!(
            "WindowCallbacks::on_window_delete: order_info={:?}",
            order_info
        );
        true
    }

    fn on_notify_icon_create(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        notify_icon_state: *const NOTIFY_ICON_STATE_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_notify_icon_create: order_info={:?}, notify_icon_state={:?}",
            order_info, notify_icon_state
        );
        true
    }

    fn on_notify_icon_update(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        notify_icon_state: *const NOTIFY_ICON_STATE_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_notify_icon_update: order_info={:?}, notify_icon_state={:?}",
            order_info, notify_icon_state
        );
        true
    }

    fn on_notify_icon_delete(&self, order_info: *const WINDOW_ORDER_INFO) -> bool {
        debug!(
            "WindowCallbacks::on_notify_icon_delete: order_info={:?}",
            order_info
        );
        true
    }

    fn on_monitored_desktop(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        monitored_desktop: *const MONITORED_DESKTOP_ORDER,
    ) -> bool {
        debug!(
            "WindowCallbacks::on_monitored_desktop: order_info={:?}, monitored_desktop={:?}",
            order_info, monitored_desktop
        );
        true
    }

    fn on_non_monitored_desktop(&self, order_info: *const WINDOW_ORDER_INFO) -> bool {
        debug!(
            "WindowCallbacks::on_non_monitored_desktop: order_info={:?}",
            order_info
        );
        true
    }
}
