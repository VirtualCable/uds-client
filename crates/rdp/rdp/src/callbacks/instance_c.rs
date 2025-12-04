use freerdp_sys::{
    AccessTokenType, BOOL, BYTE, DWORD, SSIZE_T, SmartcardCertInfo, UINT16, UINT32, WCHAR, freerdp,
    freerdp_client_load_channels, freerdp_get_logon_error_info_data,
    freerdp_get_logon_error_info_type, gdi_free, gdi_init, rdp_auth_reason,
};

use shared::log::debug;

use crate::{
    callbacks::{graphics_c, primary_c},
    events, utils,
};

use super::{
    super::{context::OwnerFromCtx, utils::ToStringLossy},
    altsec_c,
    channels_c::{on_channel_connected, on_channel_disconnected},
    input_c,
    instance::InstanceCallbacks,
    pointer_update_c, secondary_c, update_c, window_c,
};

#[cfg(windows)]
type SSizeT = i64; // SSIZE_T en Windows

#[cfg(unix)]
type SSizeT = isize; // ssize_t en Linux/macOS

#[cfg(windows)]
type ReasonType = i32; // DWORD

#[cfg(unix)]
type ReasonType = i32; // int32_t

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_instance_callbacks(instance: *mut freerdp) {
    unsafe {
        // Callback assignments
        // All commented methods are provided by freerdp3
        // Have to make some tests, but probably, the already
        // manages the internal GDI state
        // and we don't need to override them
        debug!("Setting instance callbacks");
        // Setups the channels event listeners
        (*instance).PreConnect = Some(pre_connect);
        // Setups the gdi after connection is done
        (*instance).PostConnect = Some(post_connect);
        (*instance).ContextNew = Some(context_new);
        (*instance).ContextFree = Some(context_free);
        (*instance).Authenticate = Some(authenticate);
        (*instance).AuthenticateEx = Some(authenticate_ex);
        (*instance).VerifyX509Certificate = Some(verify_x509_certificate);
        (*instance).LogonErrorInfo = Some(logon_error_info);
        (*instance).PostDisconnect = Some(post_disconnect);
        (*instance).GatewayAuthenticate = Some(gateway_authenticate);
        (*instance).PresentGatewayMessage = Some(present_gateway_message);
        (*instance).Redirect = Some(redirect);
        (*instance).LoadChannels = Some(load_channels);
        (*instance).PostFinalDisconnect = Some(post_final_disconnect);
        // Channel data methods, commented to use the internal freerdp ones
        // (*instance).SendChannelData = Some(send_channel_data);
        // (*instance).ReceiveChannelData = Some(receive_channel_data);
        // (*instance).SendChannelPacket = Some(send_channel_packet);
        (*instance).VerifyCertificateEx = Some(verify_certificate);
        (*instance).ChooseSmartcard = Some(choose_smartcard);
        //(*instance).GetAccessToken = Some(freerdp_sys::get_access_token_wrapper);
        (*instance).RetryDialog = Some(retry_dialog);
    }
}

extern "C" fn pre_connect(instance: *mut freerdp) -> BOOL {
    // Here we can override the transport_io callbacks. Look rdpTransportIo
    // Transport and security layers:
    //
    // [RDP Application]
    //        |
    //        v
    //   RDP Protocol (PDUs)
    //        |
    //        v
    //   TLS (if the server requires it)
    //        |
    //        v
    //   rdp_transport_io (transport function table)
    //        |
    //        v
    //   UDS Tunnel local encapsulation  (if used the tunnel)
    //        |
    //        v
    //   Internet / Physical network
    //        |
    //        v
    //   UDS Tunnnel remote decapsulation (if used the tunnel)
    //        |
    //        v
    //   Remote RDP Server
    //
    // Notes:
    // - The UDS tunnel adds an additional transport layer, which may include
    //   custom encryption or multiplexing.
    // - The RDP server still requires its standard TLS layer, which is negotiated
    //   inside the tunnel.
    // - The tunnel must remain transparent to TLS: it encapsulates the data,
    //   but does not remove or alter the TLS negotiation required by RDP.

    // Register the channel events
    debug!(" **** Registering channel events on pre_connect...");

    let pubsub = unsafe { (*instance).context.as_ref().unwrap().pubSub };
    events::ChannelConnected::subscribe(pubsub, Some(on_channel_connected));
    events::ChannelDisconnected::subscribe(pubsub, Some(on_channel_disconnected));

    if let Some(owner) = instance.owner() {
        owner.on_pre_connect().into()
    } else {
        true.into()
    }
}

// Initialize GDI here after the connection is established
extern "C" fn post_connect(instance: *mut freerdp) -> BOOL {
    debug!(" **** Post connect called... {instance:?}");
    // Initialize GDI, must be after the settings are set
    // const PIXEL_FORMAT_BGRA32: u32 = 0x20048888;
    // const PIXEL_FORMAT_RGBA32: u32 = 0x20038888;
    // const PIXEL_FORMAT_RGB24: u32 = 0x18018888;

    // Use 24 bits per pixel, ARGB=1, ABGR=2, RGBA=3, BGRA=4 (if 24 bit, ofc, no alpha is used, must be 0)
    unsafe { gdi_init(instance, utils::pixel_format(32, 4, 8, 8, 8, 8)) };
    if let Some(owner) = instance.owner() {
        debug!(" Owner: {:?}", &owner);
        let context = unsafe { (*instance).context };
        // Setup our callbacks
        unsafe {
            update_c::set_callbacks(context, &owner.get_callbacks().update);
            window_c::set_callbacks(context, &owner.get_callbacks().window);
            altsec_c::set_callbacks(context, &owner.get_callbacks().altsec);
            primary_c::set_callbacks(context, &owner.get_callbacks().primary);
            secondary_c::set_callbacks(context, &owner.get_callbacks().secondary);
            pointer_update_c::set_callbacks(context, &owner.get_callbacks().pointer);
            input_c::set_callbacks(context, &owner.get_callbacks().input);
            graphics_c::set_callbacks(context);
        }

        owner.on_post_connect().into()
    } else {
        true.into()
    }
}

extern "C" fn context_new(instance: *mut freerdp, context: *mut freerdp_sys::rdpContext) -> BOOL {
    debug!(" **** Context new called... {instance:?} -- {context:?}");
    if let Some(owner) = instance.owner() {
        owner.on_context_new().into()
    } else {
        true.into()
    }
}

extern "C" fn context_free(instance: *mut freerdp, context: *mut freerdp_sys::rdpContext) {
    debug!(" **** Context free called... {instance:?} -- {context:?}");
    if let Some(owner) = instance.owner() {
        owner.on_context_free();
    }
}

extern "C" fn post_disconnect(instance: *mut freerdp) {
    debug!(" **** Post disconnect called...");

    unsafe {
        gdi_free(instance);
    }

    if let Some(owner) = instance.owner() {
        owner.on_post_disconnect();
    }
}

extern "C" fn post_final_disconnect(instance: *mut freerdp) {
    debug!(" **** Post final disconnect called...");

    let pubsub = unsafe { (*instance).context.as_ref().unwrap().pubSub };
    events::ChannelConnected::unsubscribe(pubsub, Some(on_channel_connected));
    events::ChannelDisconnected::unsubscribe(pubsub, Some(on_channel_disconnected));

    if let Some(owner) = instance.owner() {
        owner.on_post_final_disconnect();
    }
}

extern "C" fn authenticate(
    instance: *mut freerdp,
    username: *mut *mut ::std::os::raw::c_char,
    password: *mut *mut ::std::os::raw::c_char,
    domain: *mut *mut ::std::os::raw::c_char,
) -> BOOL {
    debug!(" **** Authenticate called... {instance:?}");
    if let Some(owner) = instance.owner() {
        owner.on_authenticate(username, password, domain).into()
    } else {
        true.into()
    }
}

extern "C" fn authenticate_ex(
    instance: *mut freerdp,
    username: *mut *mut ::std::os::raw::c_char,
    password: *mut *mut ::std::os::raw::c_char,
    domain: *mut *mut ::std::os::raw::c_char,
    reason: rdp_auth_reason,
) -> BOOL {
    debug!(" **** Authenticate (extended) called...");
    if let Some(owner) = instance.owner() {
        owner
            .on_authenticate_ex(username, password, domain, reason as ReasonType)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn verify_x509_certificate(
    instance: *mut freerdp,
    data: *const BYTE,
    length: usize,
    hostname: *const ::std::os::raw::c_char,
    port: UINT16,
    flags: DWORD,
) -> ::std::os::raw::c_int {
    debug!(" **** Verify X.509 certificate called...");
    if let Some(owner) = instance.owner() {
        // Convert hostname to Rust string
        let hostname = hostname.to_string_lossy();
        owner
            .on_verify_x509_certificate(data, length, &hostname, port, flags)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn verify_certificate(
    instance: *mut freerdp,
    host: *const ::std::os::raw::c_char,
    port: UINT16,
    common_name: *const ::std::os::raw::c_char,
    subject: *const ::std::os::raw::c_char,
    issuer: *const ::std::os::raw::c_char,
    fingerprint: *const ::std::os::raw::c_char,
    flags: DWORD,
) -> DWORD {
    debug!(" **** Verify certificate called...");
    if let Some(owner) = instance.owner() {
        // Convert host, commmon name, subject, issuer, fingerprint from *const c_char String
        let host = host.to_string_lossy();
        let common_name = common_name.to_string_lossy();
        let subject = subject.to_string_lossy();
        let issuer = issuer.to_string_lossy();
        let fingerprint = fingerprint.to_string_lossy();

        owner.on_verify_certificate(
            &host,
            port,
            &common_name,
            &subject,
            &issuer,
            &fingerprint,
            flags,
        )
    } else {
        0
    }
}
extern "C" fn logon_error_info(
    instance: *mut freerdp,
    data: UINT32,
    type_: UINT32,
) -> ::std::os::raw::c_int {
    debug!(" **** Logon error info called...");
    let str_data = unsafe { freerdp_get_logon_error_info_data(data) };
    let str_type = unsafe { freerdp_get_logon_error_info_type(type_) };

    let str_data = str_data.to_string_lossy();
    let str_type = str_type.to_string_lossy();

    if let Some(owner) = instance.owner() {
        owner.on_logon_error_info(&str_data, &str_type).into()
    } else {
        true.into()
    }
}

extern "C" fn gateway_authenticate(
    instance: *mut freerdp,
    username: *mut *mut ::std::os::raw::c_char,
    password: *mut *mut ::std::os::raw::c_char,
    domain: *mut *mut ::std::os::raw::c_char,
) -> BOOL {
    debug!(" **** Gateway authenticate called...");
    if let Some(owner) = instance.owner() {
        owner
            .on_gateway_authenticate(username, password, domain)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn present_gateway_message(
    instance: *mut freerdp,
    msg_type: UINT32,
    is_display_mandatory: BOOL,
    is_consent_mandatory: BOOL,
    length: usize,
    message: *const WCHAR,
) -> BOOL {
    debug!(" **** Present gateway message called...");
    if let Some(owner) = instance.owner() {
        // Convert message to Rust string if needed, messages is in UTF-16 format
        let message = if !message.is_null() && length > 0 {
            let slice = unsafe { std::slice::from_raw_parts(message, length) };
            String::from_utf16_lossy(slice)
        } else {
            String::new()
        };

        owner
            .on_present_gateway_message(
                msg_type,
                is_display_mandatory != 0,
                is_consent_mandatory != 0,
                length,
                message,
            )
            .into()
    } else {
        true.into()
    }
}

extern "C" fn redirect(instance: *mut freerdp) -> BOOL {
    debug!(" **** Redirect called...");
    if let Some(owner) = instance.owner() {
        owner.on_redirect().into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn load_channels(instance: *mut freerdp) -> BOOL {
    debug!(" **** Load channels called...");

    // Invoke original, ours is only a wrapper
    unsafe {
        freerdp_client_load_channels(instance);
    }

    if let Some(owner) = instance.owner() {
        owner.on_load_channels().into()
    } else {
        true.into()
    }
}

// #[allow(dead_code)]
// extern "C" fn send_channel_data(
//     instance: *mut freerdp,
//     channel_id: UINT16,
//     data: *const BYTE,
//     size: usize,
// ) -> BOOL {
//     debug!(" **** Send channel data called...");

//     // Convert BYTE to u8, just the pointer only
//     if let Some(owner) = instance.owner() {
//         owner.on_send_channel_data(channel_id, data, size).into()
//     } else {
//         true.into()
//     }
// }

// #[allow(dead_code)]
// extern "C" fn receive_channel_data(
//     instance: *mut freerdp,
//     channel_id: UINT16,
//     data: *const BYTE,
//     size: usize,
//     flags: UINT32,
//     total_size: usize,
// ) -> BOOL {
//     debug!(" **** Receive channel data called...");
//     if let Some(owner) = instance.owner() {
//         owner
//             .on_receive_channel_data(channel_id, data, size, flags, total_size)
//             .into()
//     } else {
//         true.into()
//     }
// }

// #[allow(dead_code)]
// extern "C" fn send_channel_packet(
//     instance: *mut freerdp,
//     channel_id: UINT16,
//     total_size: usize,
//     flags: UINT32,
//     data: *const BYTE,
//     chunk_size: usize,
// ) -> BOOL {
//     debug!(" **** Send channel packet called...");
//     if let Some(owner) = instance.owner() {
//         owner
//             .on_send_channel_packet(channel_id, total_size, flags, data, chunk_size)
//             .into()
//     } else {
//         true.into()
//     }
// }

extern "C" fn choose_smartcard(
    instance: *mut freerdp,
    cert_list: *mut *mut SmartcardCertInfo,
    count: DWORD,
    choice: *mut DWORD,
    gateway: BOOL,
) -> BOOL {
    debug!(" **** Choose smartcard called...");
    if let Some(owner) = instance.owner() {
        owner
            .on_choose_smartcard(cert_list, count, choice, gateway != 0)
            .into()
    } else {
        true.into()
    }
}

// No mangled because will be invoked from C code directly
// It's a trick to avoid the varargs, as Rust does not support them directly
// Note tha the wrapper on the sys crate translates de vargs to a pointer and a count
#[unsafe(no_mangle)]
pub extern "C" fn get_access_token_no_varargs(
    instance: *mut freerdp,
    token_type: AccessTokenType,
    token: *mut *mut ::std::os::raw::c_char,
    count: usize,
    data: *const *const ::std::os::raw::c_char,
) -> BOOL {
    debug!(" **** Get Access Token called...");
    if let Some(owner) = instance.owner() {
        owner
            .on_get_access_token(token_type, token, count, data)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn retry_dialog(
    instance: *mut freerdp,
    what: *const ::std::os::raw::c_char,
    current: usize,
    userarg: *mut ::std::os::raw::c_void,
) -> SSIZE_T {
    debug!(" **** Retry dialog called...");
    if let Some(owner) = instance.owner() {
        // Convert what to Rust string
        let what = what.to_string_lossy();
        owner.on_retry_dialog(&what, current, userarg) as SSizeT
    } else {
        -1
    }
}
