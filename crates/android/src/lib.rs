// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Andres Schumann, aschumann at virtualcable dot es

use std::sync::OnceLock;

use jni::EnvUnowned;
use jni::objects::{JByteArray, JClass, JString};
use jni::strings::JNIString;
use jni::sys::{jint, jstring};
use serde_json::json;

use connection::broker::api::BrokerApi;
use connection::types::TunnelConnectInfo;
use crypt::types::SharedSecret;

// Manual NewStringUTF wrapper that does not panic on the
// jni-0.22.4 "Expected an exception after ExceptionCheck"
// inconsistency when called from a thread that has other
// tokio workers active. Returns Ok(None) when an exception
// is pending so the caller can surface it via ThrowRuntimeExAndDefault.
fn new_string_safe<'local>(
    env: &mut jni::Env<'local>,
    s: &str,
) -> jni::errors::Result<Option<JString<'local>>> {
    let string = JNIString::new(s);
    let raw: *mut jni::sys::JNIEnv = env.get_raw();
    let ptr = string.as_ptr();
    let result: jstring = unsafe {
        ((*(*raw)).v1_1.NewStringUTF)(raw, ptr)
    };
    if result.is_null() {
        if env.exception_check() {
            return Ok(None);
        }
        return Err(jni::errors::Error::NullPtr(
            "NewStringUTF returned null without a pending exception",
        ));
    }
    Ok(Some(unsafe { JString::from_raw(env, result) }))
}

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static PANIC_HOOK_INIT: OnceLock<()> = OnceLock::new();

#[cfg(target_os = "android")]
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        prio: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}

pub fn log_android(prio: i32, msg: &str) {
    #[cfg(target_os = "android")]
    {
        use std::ffi::CString;
        if let (Ok(tag), Ok(text)) = (CString::new("UdsNative"), CString::new(msg)) {
            unsafe {
                __android_log_write(prio, tag.as_ptr(), text.as_ptr());
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("[UdsNative] {msg}");
}

fn init_panic_hook() {
    PANIC_HOOK_INIT.get_or_init(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("UDS PANIC: {info}");
            log_android(6, &msg);
        }));
    });
}

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime for UDS Android")
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_freerdp_afreerdp_uds_UdsNative_getScript<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    host: JString<'local>,
    ticket: JString<'local>,
    scrambler: JString<'local>,
) -> jstring {
    init_panic_hook();
    let outcome = unowned_env.with_env(|env| -> Result<_, jni::errors::Error> {
        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> String {
            shared::tls::init_tls(None);

            let host_str = match host.try_to_string(env) {
                Ok(s) => s,
                Err(e) => {
                    return json!({
                        "error": {
                            "message": format!("Host error: {e}"),
                            "is_retryable": false,
                            "percent": 0
                        }
                    })
                    .to_string();
                }
            };
            let ticket_str = match ticket.try_to_string(env) {
                Ok(s) => s,
                Err(e) => {
                    return json!({
                        "error": {
                            "message": format!("Ticket error: {e}"),
                            "is_retryable": false,
                            "percent": 0
                        }
                    })
                    .to_string();
                }
            };
            let scrambler_str = match scrambler.try_to_string(env) {
                Ok(s) => s,
                Err(e) => {
                    return json!({
                        "error": {
                            "message": format!("Scrambler error: {e}"),
                            "is_retryable": false,
                            "percent": 0
                        }
                    })
                    .to_string();
                }
            };

            log_android(3, &format!("getScript: host={host_str}, ticket=<{} chars redacted>", ticket_str.len()));

            let rt = get_runtime();
            rt.block_on(async move {
                let broker_url = if host_str.starts_with("http://") || host_str.starts_with("https://") {
                    format!("{}/uds/rest/client", host_str.trim_end_matches('/'))
                } else {
                    format!("https://{}/uds/rest/client", host_str.trim_end_matches('/'))
                };

                let api = connection::broker::api::UdsBrokerApi::new(&broker_url, None, false);
                match api.get_script(&ticket_str, &scrambler_str).await {
                    Ok(script) => match script.decoded_params() {
                        Ok(mut params) => {
                            if let Some(ref ss) = script.shared_secret {
                                let ss_bytes: &[u8; 32] = ss.as_ref();
                                if let Some(tunnel_obj) =
                                    params.get_mut("tunnel").and_then(|t| t.as_object_mut())
                                {
                                    tunnel_obj.insert(
                                        "shared_secret".to_string(),
                                        serde_json::to_value(ss_bytes.to_vec()).unwrap_or_default(),
                                    );
                                } else {
                                    params["shared_secret"] =
                                        serde_json::to_value(ss_bytes.to_vec()).unwrap_or_default();
                                }
                            }
                            json!({ "result": params }).to_string()
                        }
                        Err(e) => json!({
                            "error": {
                                "message": format!("Failed to decode script parameters: {e}"),
                                "is_retryable": false,
                                "percent": 0
                            }
                        })
                        .to_string(),
                    },
                    Err(e) => json!({
                        "error": {
                            "message": e.message,
                            "is_retryable": e.is_retryable,
                            "percent": e.percent
                        }
                    })
                    .to_string(),
                }
            })
        }));

        let json_result = match panic_res {
            Ok(s) => s,
            Err(p) => {
                let msg = if let Some(s) = p.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = p.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown Rust panic".to_string()
                };
                log_android(6, &format!("Caught panic in getScript: {msg}"));
                json!({
                    "error": {
                        "message": format!("Rust panic: {msg}"),
                        "is_retryable": false,
                        "percent": 0
                    }
                })
                .to_string()
            }
        };

        let js = match new_string_safe(env, &json_result)? {
            Some(j) => j,
            None => return Ok(std::ptr::null_mut()),
        };
        Ok(js.into_raw())
    });

    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_freerdp_afreerdp_uds_UdsNative_startTunnel<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    host: JString<'local>,
    port: jint,
    ticket: JString<'local>,
    shared_secret: JByteArray<'local>,
) -> jint {
    init_panic_hook();
    let outcome = unowned_env.with_env(|env| -> Result<_, jni::errors::Error> {
        let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> jint {
            shared::tls::init_tls(None);

            let host_str = match host.try_to_string(env) {
                Ok(s) => s,
                Err(e) => {
                    log_android(6, &format!("startTunnel host error: {e}"));
                    return -1;
                }
            };
            let ticket_str = match ticket.try_to_string(env) {
                Ok(s) => s,
                Err(e) => {
                    log_android(6, &format!("startTunnel ticket error: {e}"));
                    return -2;
                }
            };

            let ss_bytes = match env.convert_byte_array(&shared_secret) {
                Ok(b) => b,
                Err(e) => {
                    log_android(6, &format!("startTunnel shared_secret error: {e}"));
                    return -3;
                }
            };

            if ss_bytes.len() != 32 {
                log_android(
                    6,
                    &format!("startTunnel invalid shared_secret len: {}", ss_bytes.len()),
                );
                return -4;
            }

            let mut ss_arr = [0u8; 32];
            ss_arr.copy_from_slice(&ss_bytes);
            let shared_sec = SharedSecret::new(ss_arr);

            let ticket_bytes = match ticket_str.as_bytes().try_into() {
                Ok(t) => t,
                Err(e) => {
                    log_android(6, &format!("startTunnel ticket length error: {e}"));
                    return -5;
                }
            };

            log_android(
                3,
                &format!("startTunnel: connecting to {host_str}:{port}"),
            );

            let info = TunnelConnectInfo {
                addr: host_str,
                port: port as u16,
                ticket: ticket_bytes,
                local_port: None,
                startup_time_ms: 120_000,
                keep_listening_after_timeout: false,
                enable_ipv6: false,
                shared_secret: Some(shared_sec),
                use_udp: false,
                udp_port: None,
            };

            let rt = get_runtime();
            let res = rt.block_on(async move { connection::start_tunnel(info).await });
            match res {
                Ok(local_port) => {
                    log_android(3, &format!("Tunnel listening on 127.0.0.1:{local_port}"));
                    local_port as jint
                }
                Err(e) => {
                    log_android(6, &format!("Failed to start UDS tunnel: {e:?}"));
                    -6
                }
            }
        }));

        match panic_res {
            Ok(code) => Ok(code),
            Err(p) => {
                let msg = if let Some(s) = p.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = p.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };
                log_android(6, &format!("Caught panic in startTunnel: {msg}"));
                Ok(-7)
            }
        }
    });

    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_freerdp_afreerdp_uds_UdsNative_stopTunnel<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    let outcome = unowned_env.with_env(|_env| -> Result<_, jni::errors::Error> {
        connection::registry::stop_tunnels();
        Ok(())
    });

    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
