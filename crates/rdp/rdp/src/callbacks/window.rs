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
