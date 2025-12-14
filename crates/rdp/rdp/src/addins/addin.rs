use std::ffi::CStr;
use std::sync::OnceLock;

use freerdp_sys::{
    DWORD, FREERDP_LOAD_CHANNEL_ADDIN_ENTRY_FN, LPCSTR, PVIRTUALCHANNELENTRY,
    freerdp_get_current_addin_provider, freerdp_register_addin_provider,
};

use shared::log;

static FREERDP_ADDIN_PROVIDER: OnceLock<FREERDP_LOAD_CHANNEL_ADDIN_ENTRY_FN> = OnceLock::new();

unsafe extern "C" fn custom_addin_provider(
    psz_name: LPCSTR,
    psz_subsystem: LPCSTR,
    psz_type: LPCSTR,
    dw_flags: DWORD,
) -> PVIRTUALCHANNELENTRY {
    if let Some(freerdp_addin_provider) = FREERDP_ADDIN_PROVIDER.get().unwrap() {
        unsafe {
            let name = if psz_name.is_null() {
                "<null>"
            } else {
                CStr::from_ptr(psz_name)
                    .to_str()
                    .unwrap_or("<invalid utf8>")
            };
            let subsystem = if psz_subsystem.is_null() {
                "<null>"
            } else {
                CStr::from_ptr(psz_subsystem)
                    .to_str()
                    .unwrap_or("<invalid utf8>")
            };
            let typ = if psz_type.is_null() {
                "<null>"
            } else {
                CStr::from_ptr(psz_type)
                    .to_str()
                    .unwrap_or("<invalid utf8>")
            };
            log::debug!(
                "Custom addin provider called for channel: {:?}, subsystem: {:?}, type: {:?}, flags: {}",
                name,
                subsystem,
                typ,
                dw_flags
            );
        }
        unsafe { freerdp_addin_provider(psz_name, psz_subsystem, psz_type, dw_flags) }
    } else {
        log::error!("Load function not set in custom addin provider.");
        None
    }
}

pub fn register_channel_addin_loader() {
    FREERDP_ADDIN_PROVIDER
        .set(unsafe { freerdp_get_current_addin_provider() })
        .ok();
    unsafe {
        freerdp_register_addin_provider(Some(custom_addin_provider), 0);
    }
}
