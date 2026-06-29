// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Emulated smartcard backend module.
//!
//! Sub-modules:
//! - `consts`: eUDS constants (AID, INS, status words, ATR, etc.)
//! - `helpers`: TLV, APDU, DER parsing, MGF1 helpers
//! - `euds_types`: PinMode, SessionState
//! - `euds_engine`: eUDS APDU processing engine
//! - `tests`: Unit tests

pub mod consts;
mod euds_engine;
mod helpers;
#[cfg(test)]
mod tests;
mod euds_types;

use std::sync::Mutex;
use std::time::Duration;

use rsa::RsaPrivateKey;

use rdp::integrations::smartcard::*;

use self::consts::*;
use self::euds_engine::EudsEngine;
use self::euds_types::PinMode;
use super::SmartcardBackend;

pub(crate) struct EmulatedBackend {
    engine: Mutex<EudsEngine>,
}

impl std::fmt::Debug for EmulatedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmulatedBackend").finish()
    }
}

impl EmulatedBackend {
    pub fn from_pem(cert_pem: &str, key_pem: &str, pin: &str) -> Result<Self, String> {
        use rsa::pkcs8::DecodePrivateKey;
        let cert_der = pem::parse(cert_pem)
            .map_err(|e| format!("cert PEM: {}", e))?
            .into_contents();
        let private_key =
            RsaPrivateKey::from_pkcs8_pem(key_pem).map_err(|e| format!("key PEM: {}", e))?;
        let pin_mode = if key_pem.contains("ENCRYPTED") {
            PinMode::Required
        } else {
            PinMode::NotRequired
        };
        Ok(EmulatedBackend {
            engine: Mutex::new(EudsEngine::new(
                cert_der,
                private_key,
                pin.to_string(),
                pin_mode,
            )),
        })
    }

    #[allow(dead_code)]
    pub fn from_der(cert_der: &[u8], key_pkcs8_der: &[u8], pin: &str) -> Result<Self, String> {
        use rsa::pkcs8::DecodePrivateKey;
        let private_key =
            RsaPrivateKey::from_pkcs8_der(key_pkcs8_der).map_err(|e| format!("key DER: {}", e))?;
        let pin_mode = PinMode::NotRequired;
        Ok(EmulatedBackend {
            engine: Mutex::new(EudsEngine::new(
                cert_der.to_vec(),
                private_key,
                pin.to_string(),
                pin_mode,
            )),
        })
    }

    pub fn try_from_env() -> Option<Self> {
        let cert = std::env::var("UDS_SMARTCARD_CERT_PEM").ok()?;
        let key = std::env::var("UDS_SMARTCARD_KEY_PEM").ok()?;
        let pin = std::env::var("UDS_SMARTCARD_PIN").unwrap_or_default();
        let cert_pem = std::fs::read_to_string(&cert).ok()?;
        let key_pem = std::fs::read_to_string(&key).ok()?;
        match Self::from_pem(&cert_pem, &key_pem, &pin) {
            Ok(b) => {
                log::info!("Emulated smartcard loaded: cert={}, key={}", cert, key);
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
    fn establish_context(&self, _scope: u32) -> Result<ScardContext, u32> {
        Ok(ScardContext::new())
    }

    fn release_context(&self, _ctx: &ScardContext) -> Result<(), u32> {
        Ok(())
    }

    fn is_valid_context(&self, _ctx: &ScardContext) -> bool {
        true
    }

    fn list_readers(&self, _ctx: &ScardContext, _: Option<&[String]>) -> Result<Vec<String>, u32> {
        Ok(vec![EUDS_READER_NAME.to_string()])
    }

    fn connect(
        &self,
        _ctx: &ScardContext,
        reader: &str,
        _: u32,
        _: u32,
    ) -> Result<ConnectResult, u32> {
        if reader != EUDS_READER_NAME {
            return Err(SCARD_E_UNKNOWN_READER);
        }
        Ok(ConnectResult {
            handle: ScardHandle::new(SCARD_PROTOCOL_T1),
            active_protocol: SCARD_PROTOCOL_T1,
        })
    }

    fn disconnect(&self, _handle: &ScardHandle, _disposition: u32) -> Result<(), u32> {
        Ok(())
    }

    fn reconnect(&self, _: &ScardHandle, _: u32, _: u32, _: u32) -> Result<u32, u32> {
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

    fn control(&self, _: &ScardHandle, control_code: u32, _: &[u8]) -> Result<Vec<u8>, u32> {
        const FEATURE_GET_TLV_PROPERTIES: u8 = 0x12;
        const CM_IOCTL_GET_FEATURE_REQUEST: u32 = 0x0031_3520;
        const CLASS2_IOCTL_MAGIC: u32 = 0x0033_0000;
        const IOCTL_FEATURE_GET_TLV_PROPERTIES: u32 =
            0x4200_0000 + (FEATURE_GET_TLV_PROPERTIES as u32) + CLASS2_IOCTL_MAGIC;

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
            reader_names: vec![EUDS_READER_NAME.to_string()],
            state: SCARD_STATE_PRESENT,
            protocol: SCARD_PROTOCOL_T1,
            atr: EUDS_ATR.to_vec(),
        })
    }

    fn get_status_change(
        &self,
        _: &ScardContext,
        timeout: Duration,
        readers: &[ReaderStateIn],
    ) -> Result<Vec<ReaderStateOut>, u32> {
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
                    atr: EUDS_ATR.to_vec(),
                }
            })
            .collect();

        let any_changed = results
            .iter()
            .any(|r| r.event_state & SCARD_STATE_CHANGED != 0);

        if !any_changed {
            let sleep_time = timeout.min(Duration::from_millis(50));
            std::thread::sleep(sleep_time);
        }

        Ok(results)
    }

    fn begin_transaction(&self, _: &ScardHandle) -> Result<(), u32> {
        Ok(())
    }

    fn end_transaction(&self, _: &ScardHandle, _: u32) -> Result<(), u32> {
        Ok(())
    }

    fn get_attrib(&self, _: &ScardHandle, _: u32) -> Result<Vec<u8>, u32> {
        Ok(vec![0x00])
    }

    fn set_attrib(&self, _: &ScardHandle, _: u32, _: &[u8]) -> Result<(), u32> {
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}
