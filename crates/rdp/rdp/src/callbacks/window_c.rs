use freerdp_sys::{
    BOOL, MONITORED_DESKTOP_ORDER, NOTIFY_ICON_STATE_ORDER, WINDOW_CACHED_ICON_ORDER,
    WINDOW_ICON_ORDER, WINDOW_ORDER_INFO, WINDOW_STATE_ORDER, rdpContext,
};

use super::{super::connection::context::OwnerFromCtx, window::WindowCallbacks};

use shared::log;

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
/// Interoperability with C code.
/// Ensure that the context pointer is valid.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {

        let update = (*context).update;
        let window = (*update).window;
        if update.is_null() || window.is_null() {
            log::debug!(" **** Window not initialized, cannot override callbacks.");
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
