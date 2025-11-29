use freerdp_sys::{AccessTokenType, SmartcardCertInfo};

use shared::log::debug;

pub trait InstanceCallbacks {
    fn on_pre_connect(&mut self) -> bool {
        debug!(" **** Preparing connection...");
        true
    }

    fn on_post_connect(&mut self) -> bool {
        debug!(" **** Connected successfully!");
        true
    }

    fn on_context_new(&mut self) -> bool {
        debug!(" **** Context new...");
        true
    }

    fn on_context_free(&mut self) {
        debug!(" **** Context free...");
    }

    #[allow(unused_variables)]
    fn on_authenticate(
        &mut self,
        username: *mut *mut ::std::os::raw::c_char,
        password: *mut *mut ::std::os::raw::c_char,
        domain: *mut *mut ::std::os::raw::c_char,
    ) -> bool {
        debug!(" **** Authenticating...");
        true
    }

    #[allow(unused_variables)]
    fn on_authenticate_ex(
        &mut self,
        _username: *mut *mut ::std::os::raw::c_char,
        _password: *mut *mut ::std::os::raw::c_char,
        _domain: *mut *mut ::std::os::raw::c_char,
        _reason: i32,
    ) -> bool {
        debug!(" **** Authenticating (extended)...");
        true
    }

    #[allow(unused_variables)]
    fn on_gateway_authenticate(
        &mut self,
        username: *mut *mut ::std::os::raw::c_char,
        password: *mut *mut ::std::os::raw::c_char,
        domain: *mut *mut ::std::os::raw::c_char,
    ) -> bool {
        debug!(" **** Authenticating...");
        true
    }

    #[allow(unused_variables)]
    fn on_choose_smartcard(
        &mut self,
        cert_list: *mut *mut SmartcardCertInfo,
        count: u32,
        choice: *mut u32,
        gateway: bool,
    ) -> bool {
        debug!(" **** Choosing smartcard certificate...");
        true
    }

    #[allow(unused_variables)]
    fn on_get_access_token(
        &mut self,
        token_type: AccessTokenType,
        token: *mut *mut ::std::os::raw::c_char,
        count: usize,
        data: *const *const ::std::os::raw::c_char,
    ) -> bool {
        debug!(" **** Getting access token...");
        true
    }

    fn on_verify_x509_certificate(
        &mut self,
        data: *const u8,
        length: usize,
        hostname: &str,
        port: u16,
        flags: u32,
    ) -> bool {
        debug!(
            " **** Verifying certificate: Hostname: {}, Port: {}, Flags: {}, Data: {:?} Data Length: {}",
            hostname, port, flags, data, length
        );
        // For now, we accept all certificates. Implement proper verification as needed.
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn on_verify_certificate(
        &mut self,
        host: &str,
        port: u16,
        common_name: &str,
        subject: &str,
        issuer: &str,
        fingerprint: &str,
        flags: u32,
    ) -> u32 {
        debug!(
            " **** Verifying certificate: Host: {:?}, Port: {}, Common Name: {:?}, Subject: {:?}, Issuer: {:?}, Fingerprint: {:?}, Flags: {}",
            host, port, common_name, subject, issuer, fingerprint, flags
        );
        // For now, we accept all certificates. Implement proper verification as needed.
        1
    }

    fn on_logon_error_info(&mut self, data_str: &str, type_str: &str) -> bool {
        debug!(
            " **** Logon error info received... Data: {}, Type: {}",
            data_str, type_str
        );
        true
    }

    fn on_post_disconnect(&mut self) {
        debug!(" **** Disconnected.");
    }

    fn on_present_gateway_message(
        &mut self,
        msg_type: u32,
        is_display_mandatory: bool,
        is_consent_mandatory: bool,
        length: usize,
        message: String,
    ) -> bool {
        debug!(
            " **** Gateway message received. Type: {}, Display Mandatory: {}, Consent Mandatory: {}, Length: {}, Message: {}",
            msg_type, is_display_mandatory, is_consent_mandatory, length, message
        );
        true
    }

    fn on_redirect(&mut self) -> bool {
        debug!(" **** Redirecting...");
        true
    }

    fn on_load_channels(&mut self) -> bool {
        debug!(" **** Loading channels...");
        true
    }

    // fn on_send_channel_data(&mut self, channel_id: u16, data: *const u8, size: usize) -> bool {
    //     debug!(
    //         " **** Sending channel data... Channel ID: {}, Data: {:?}, Size: {}",
    //         channel_id, data, size
    //     );
    //     true
    // }

    // fn on_receive_channel_data(
    //     &mut self,
    //     channel_id: u16,
    //     data: *const u8,
    //     size: usize,
    //     flags: u32,
    //     total_size: usize,
    // ) -> bool {
    //     debug!(
    //         " **** Receiving channel data... Channel ID: {}, Data: {:?}, Size: {}, Flags: {}, Total Size: {}",
    //         channel_id, data, size, flags, total_size
    //     );
    //     true
    // }

    // fn on_send_channel_packet(
    //     &mut self,
    //     channel_id: u16,
    //     total_size: usize,
    //     flags: u32,
    //     data: *const u8,
    //     chunk_size: usize,
    // ) -> bool {
    //     debug!(
    //         " **** Sending channel packet... Channel ID: {}, Total Size: {}, Flags: {}, Data: {:?}, Chunk Size: {}",
    //         channel_id, total_size, flags, data, chunk_size
    //     );
    //     true
    // }

    fn on_post_final_disconnect(&mut self) {
        debug!(" **** Disconnected.");
    }

    fn on_retry_dialog(
        &mut self,
        what: &str,
        current: usize,
        userarg: *mut ::std::os::raw::c_void,
    ) -> i64 {
        debug!(
            " **** Retry dialog invoked. What: {}, Current: {}, UserArg: {:?}",
            what, current, userarg
        );
        -1 // Indicate no retry by default
    }
}
