use std::ffi::CString;

use anyhow::Result;
use freerdp_sys::*;

use crate::{callbacks::instance_c, utils::SafePtr};
use shared::log::debug;

use super::{Rdp, context::RdpContext};

#[allow(dead_code)]
impl Rdp {
    pub fn build(self: std::pin::Pin<&mut Self>) -> Result<()> {
        debug!("Building RDP connection... {:p}", self);
        let mut_self = unsafe { self.get_unchecked_mut() };

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
                    .join(";"),
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
