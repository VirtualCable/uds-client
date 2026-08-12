// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Emulated smartcard backend module.
//!
//! Emulates a GIDS card so the Windows GIDS minidriver (msclmd) can drive it over
//! the RDP smartcard redirect exactly like with the physical reference card.
//!
//! Sub-modules:
//! - `gids_engine`: the GIDS APDU engine (SELECT, GET DATA, VERIFY, MSE SET, PSO)
//! - `helpers`: TLV, APDU, DER parsing helpers (shared)
//! - `consts`/`euds_engine`/`euds_types`: legacy eUDS custom protocol (kept for
//!   reuse in another project; not used by this backend)

pub mod consts;
#[allow(dead_code)]
mod euds_engine;
#[allow(dead_code)]
mod euds_types;
mod gids_engine;
#[cfg(test)]
mod gids_tests;
mod helpers;
#[cfg(test)]
mod tests;

use std::sync::Mutex;
use std::time::Duration;

use rsa::RsaPrivateKey;

use pcsc::ffi::DWORD;
use rdp::integrations::smartcard::*;

use self::gids_engine::{GIDS_ATR, GIDS_READER_NAME, GidsEngine};
use super::SmartcardBackend;

pub(crate) struct EmulatedBackend {
    engine: Mutex<GidsEngine>,
}

impl std::fmt::Debug for EmulatedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmulatedBackend").finish()
    }
}

impl EmulatedBackend {
    pub fn from_pem(cert_pem: &str, key_pem: &str) -> Result<Self, String> {
        let cert_der = pem::parse(cert_pem)
            .map_err(|e| format!("cert PEM: {}", e))?
            .into_contents();
        Ok(EmulatedBackend {
            engine: Mutex::new(GidsEngine::new(cert_der, key_pem.to_string())?),
        })
    }

    #[allow(dead_code)]
    pub fn from_der(cert_der: &[u8], key_pkcs8_der: &[u8]) -> Result<Self, String> {
        use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};
        // DER path: the key must be unencrypted (no PEM password to decrypt on VERIFY).
        let private_key =
            RsaPrivateKey::from_pkcs8_der(key_pkcs8_der).map_err(|e| format!("key DER: {}", e))?;
        let key_pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("key PEM: {}", e))?
            .to_string();
        Ok(EmulatedBackend {
            engine: Mutex::new(GidsEngine::new(cert_der.to_vec(), key_pem)?),
        })
    }

    /// Load the emulated card from an `emulated_certs` spec:
    /// - `file:<path>`  → a local PEM file (may contain cert + key blocks)
    /// - `pem:<cert_pem>,<key_pem>` → the certificate and key as PEM strings
    /// - `userdefined:` → reserved (not implemented yet)
    pub fn from_spec(spec: &str) -> Option<Self> {
        let (cert_pem, key_pem) = if let Some(path) = spec.strip_prefix("file:") {
            let content = std::fs::read_to_string(path).ok()?;
            extract_cert_and_key(&content)?
        } else if let Some(rest) = spec.strip_prefix("pem:") {
            let (cert, key) = rest.split_once(',')?;
            (cert.to_string(), key.to_string())
        } else {
            log::error!(
                "emulated_certs: unsupported spec prefix (expected file: or pem:) — got: {}",
                spec
            );
            return None;
        };
        match Self::from_pem(&cert_pem, &key_pem) {
            Ok(b) => Some(b),
            Err(e) => {
                log::error!("Failed to load emulated smartcard: {}", e);
                None
            }
        }
    }

    /// Load the emulated card from `UDS_SMARTCARD_KEYS="cert.pem;key.pem"`.
    /// The certificate is needed so msclmd can serve it (GET DATA DF24) and match
    /// the container key; the private key drives signing. If the key is encrypted,
    /// its password acts as the card PIN (asked by msclmd only when signing).
    pub fn try_from_env() -> Option<Self> {
        let spec = std::env::var("UDS_SMARTCARD_KEYS").ok()?;
        let mut parts = spec.split(';');
        let (cert_path, key_path) = (parts.next()?, parts.next()?);
        if cert_path.is_empty() || key_path.is_empty() {
            log::error!("UDS_SMARTCARD_KEYS must be \"cert.pem;key.pem\"");
            return None;
        }
        let cert_pem = std::fs::read_to_string(cert_path).ok()?;
        let key_pem = std::fs::read_to_string(key_path).ok()?;
        match Self::from_pem(&cert_pem, &key_pem) {
            Ok(b) => {
                log::info!(
                    "Emulated smartcard loaded: cert={}, key={}",
                    cert_path,
                    key_path
                );
                Some(b)
            }
            Err(e) => {
                log::error!("Failed to load emulated smartcard: {}", e);
                None
            }
        }
    }
}

impl SmartcardBackend for EmulatedBackend {
    fn establish_context(&self, _scope: DWORD) -> Result<ScardContext, u32> {
        Ok(ScardContext::new())
    }

    fn release_context(&self, _ctx: &ScardContext) -> Result<(), u32> {
        Ok(())
    }

    fn is_valid_context(&self, _ctx: &ScardContext) -> bool {
        true
    }

    fn list_readers(&self, _ctx: &ScardContext, _: Option<&[String]>) -> Result<Vec<String>, u32> {
        Ok(vec![GIDS_READER_NAME.to_string()])
    }

    fn connect(
        &self,
        _ctx: &ScardContext,
        reader: &str,
        _: DWORD,
        _: DWORD,
    ) -> Result<ConnectResult, u32> {
        if reader != GIDS_READER_NAME {
            return Err(SCARD_E_UNKNOWN_READER);
        }
        Ok(ConnectResult {
            handle: ScardHandle::new(SCARD_PROTOCOL_T1),
            active_protocol: SCARD_PROTOCOL_T1,
        })
    }

    fn disconnect(&self, _handle: &ScardHandle, _disposition: DWORD) -> Result<(), u32> {
        Ok(())
    }

    fn reconnect(&self, _: &ScardHandle, _: DWORD, _: DWORD, _: DWORD) -> Result<u32, u32> {
        Ok(SCARD_PROTOCOL_T1)
    }

    fn transmit(
        &self,
        _: &ScardHandle,
        _: &ScardIORequest,
        data: &[u8],
    ) -> Result<TransmitResult, u32> {
        let mut engine = self.engine.lock().map_err(|_| SCARD_F_INTERNAL_ERROR)?;
        Ok(TransmitResult {
            recv_pci: None,
            recv_buffer: engine.process_apdu(data),
        })
    }

    fn control(&self, _: &ScardHandle, control_code: DWORD, _: &[u8]) -> Result<Vec<u8>, u32> {
        const FEATURE_GET_TLV_PROPERTIES: u8 = 0x12;
        const CM_IOCTL_GET_FEATURE_REQUEST: DWORD = 0x0031_3520;
        const CLASS2_IOCTL_MAGIC: DWORD = 0x0033_0000;
        const IOCTL_FEATURE_GET_TLV_PROPERTIES: DWORD =
            0x4200_0000 + (FEATURE_GET_TLV_PROPERTIES as DWORD) + CLASS2_IOCTL_MAGIC;

        if control_code == CM_IOCTL_GET_FEATURE_REQUEST {
            let mut response = Vec::with_capacity(6);
            response.push(FEATURE_GET_TLV_PROPERTIES);
            response.push(4);
            response.extend_from_slice(&IOCTL_FEATURE_GET_TLV_PROPERTIES.to_be_bytes());
            Ok(response)
        } else if control_code == IOCTL_FEATURE_GET_TLV_PROPERTIES {
            let mut response = Vec::with_capacity(6);
            response.push(0x0A);
            response.push(4);
            let max_apdu: u32 = 0x0001_0000;
            response.extend_from_slice(&max_apdu.to_be_bytes());
            Ok(response)
        } else {
            Ok(vec![])
        }
    }

    fn status(&self, _: &ScardHandle) -> Result<ScardStatus, u32> {
        Ok(ScardStatus {
            reader_names: vec![GIDS_READER_NAME.to_string()],
            state: SCARD_STATE_PRESENT,
            protocol: SCARD_PROTOCOL_T1,
            atr: GIDS_ATR.to_vec(),
        })
    }

    fn get_status_change(
        &self,
        _: &ScardContext,
        timeout: Duration,
        readers: &[ReaderStateIn],
    ) -> Result<(Vec<ReaderStateOut>, u32), u32> {
        let results: Vec<ReaderStateOut> = readers
            .iter()
            .map(|rs| {
                let actual_state = SCARD_STATE_PRESENT;
                let changed = (rs.current_state & !SCARD_STATE_CHANGED) != actual_state;
                ReaderStateOut {
                    reader_name: rs.reader_name.clone(),
                    current_state: actual_state,
                    event_state: if changed {
                        actual_state | SCARD_STATE_CHANGED
                    } else {
                        actual_state
                    },
                    atr: GIDS_ATR.to_vec(),
                }
            })
            .collect();

        let any_changed = results
            .iter()
            .any(|r| r.event_state & SCARD_STATE_CHANGED != 0);

        let return_code = if any_changed {
            SCARD_S_SUCCESS
        } else {
            let sleep_time = timeout.min(Duration::from_millis(50));
            std::thread::sleep(sleep_time);
            SCARD_E_TIMEOUT
        };

        Ok((results, return_code))
    }

    fn cancel(&self, _: &ScardContext) -> Result<(), u32> {
        Ok(())
    }

    fn begin_transaction(&self, _: &ScardHandle) -> Result<(), u32> {
        Ok(())
    }

    fn end_transaction(&self, _: &ScardHandle, _: DWORD) -> Result<(), u32> {
        Ok(())
    }

    fn get_attrib(&self, _: &ScardHandle, _: DWORD) -> Result<Vec<u8>, u32> {
        Ok(vec![0x00])
    }

    fn set_attrib(&self, _: &ScardHandle, _: DWORD, _: &[u8]) -> Result<(), u32> {
        Ok(())
    }

    fn get_container_info(&self, _: &ScardContext, _: u8) -> Result<Vec<u8>, u32> {
        // The GIDS engine serves the container public key (7F 49) over TRANSMIT and
        // msclmd builds ContainerInfo_XX itself (same flow as the physical card), so
        // this stays as a MISS.
        Err(SCARD_E_UNSUPPORTED_FEATURE)
    }

    fn get_certificate(&self, _: &ScardContext) -> Result<Vec<u8>, u32> {
        // The GIDS engine serves the certificate (DF 24) over TRANSMIT; msclmd caches
        // it as kxc00 itself.
        Err(SCARD_E_UNSUPPORTED_FEATURE)
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Extract the CERTIFICATE and PRIVATE KEY PEM blocks from a PEM bundle.
fn extract_cert_and_key(content: &str) -> Option<(String, String)> {
    let blocks = pem::parse_many(content).ok()?;
    let mut cert_pem = None;
    let mut key_pem = None;
    for b in blocks {
        match b.tag() {
            "CERTIFICATE" => cert_pem = Some(b.to_string()),
            "PRIVATE KEY" | "ENCRYPTED PRIVATE KEY" | "RSA PRIVATE KEY" => {
                key_pem = Some(b.to_string());
            }
            _ => {}
        }
    }
    Some((cert_pem?, key_pem?))
}
