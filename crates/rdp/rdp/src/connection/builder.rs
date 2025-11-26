use std::{
    ffi::CString,
    sync::{Arc, RwLock},
};

use anyhow::Result;
use crossbeam::channel::Sender;

use freerdp_sys::*;
use shared::log;

use crate::{
    callbacks::{
        altsec_c, input_c, instance_c, pointer_update_c, primary_c, secondary_c, update_c, window_c,
    },
    utils::{SafeHandle, SafePtr},
};

use super::{
    super::settings::RdpSettings, Callbacks, Config, Rdp, RdpMessage, context::RdpContext,
};

#[allow(dead_code)]
impl Rdp {
    pub fn new(settings: RdpSettings, update_tx: Sender<RdpMessage>) -> Self {
        Rdp {
            config: Config {
                settings,
                ..Config::default()
            },
            instance: None,
            update_tx: Some(update_tx),
            gdi_lock: Arc::new(RwLock::new(())),
            stop_event: None,
            _pin: std::marker::PhantomPinned,
        }
    }

    pub fn context(&self) -> Option<&RdpContext> {
        unsafe {
            if let Some(instance) = self.instance {
                let ctx = instance.context as *mut RdpContext;
                if ctx.is_null() { None } else { Some(&*ctx) }
            } else {
                None
            }
        }
    }
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

    pub fn build(self: std::pin::Pin<&mut Self>) -> Result<()> {
        log::debug!("Building RDP connection... {:p}", self);
        let stop_event: HANDLE =
            unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };

        let mut_self = unsafe { self.get_unchecked_mut() };
        mut_self.stop_event = Some(SafeHandle::new(stop_event).unwrap());

        unsafe {
            let ctx = RdpContext::create(mut_self)?;
            let instance = (*ctx).common.context.instance;

            mut_self.instance = Some(SafePtr::new(instance).ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to create SafePtr for freerdp instance: {:?}",
                    instance
                )
            })?);

            instance_c::set_instance_callbacks(instance);

            let settings_ptr = (*ctx).common.context.settings;

            let host = CString::new(mut_self.config.settings.server.clone())?;
            let user = CString::new(mut_self.config.settings.user.clone())?;
            let pass = CString::new(mut_self.config.settings.password.clone())?;
            let domain = CString::new(mut_self.config.settings.domain.clone())?;
            let drives_to_redirect = CString::new(
                mut_self
                    .config
                    .settings
                    .drives_to_redirect
                    .iter()
                    .filter(|s| s.as_str() != "all")
                    .map(|s| s.as_str())
                    .collect::<Vec<&str>>()
                    .join(","),
            )
            .unwrap();

            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_ServerHostname,
                host.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Username,
                user.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Password,
                pass.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Domain,
                domain.as_ptr(),
            );
            freerdp_settings_set_uint32(
                settings_ptr,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_ServerPort,
                mut_self.config.settings.port,
            );
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_IgnoreCertificate,
                !mut_self.config.settings.verify_cert as BOOL,
            );

            // NLA setting
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_NlaSecurity,
                mut_self.config.settings.use_nla as BOOL,
            );

            let all_drives = mut_self
                .config
                .settings
                .drives_to_redirect
                .iter()
                .any(|s| s.as_str() == "all");
            let len_drives = mut_self.config.settings.drives_to_redirect.len();
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectDrives,
                (len_drives != 0) as BOOL,
            );
            if !all_drives {
                // Remove "all" and, if any rameaining, use FreeRDP_RedirectDrives
                freerdp_settings_set_string(
                    settings_ptr,
                    FreeRDP_Settings_Keys_String_FreeRDP_DrivesToRedirect,
                    drives_to_redirect.as_ptr(),
                );
            }

            Ok(())
        }
    }
}
