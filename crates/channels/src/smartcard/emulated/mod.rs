// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Emulated smartcard backend module.
//!
//! Sub-modules:
//! - `consts`: GIDS constants (AID, EF/DO IDs, status words, etc.)
//! - `helpers`: TLV, APDU, DER parsing, MGF1 helpers
//! - `types`: VirtualFs, SessionState, GidsEngine structs
//! - `engine`: GIDS APDU processing engine
//! - `tests`: Unit tests

pub mod consts;
mod engine;
mod helpers;
#[cfg(test)]
mod tests;
mod types;

use std::sync::Mutex;
use std::time::Duration;

use rsa::RsaPrivateKey;

use rdp::integrations::smartcard::*;

use self::engine::GidsEngine;
use super::SmartcardBackend;

/// Emulated smartcard backend using GIDS protocol with a local certificate.
#[derive(Debug)]
pub(crate) struct EmulatedBackend {
    engine: Mutex<GidsEngine>,
}

impl EmulatedBackend {
    pub fn from_pem(cert_pem: &str, key_pem: &str, pin: &str) -> Result<Self, String> {
        use rsa::pkcs8::DecodePrivateKey;
        let cert_der = pem::parse(cert_pem)
            .map_err(|e| format!("cert PEM: {}", e))?
            .into_contents();
        let private_key =
            RsaPrivateKey::from_pkcs8_pem(key_pem).map_err(|e| format!("key PEM: {}", e))?;
        Ok(EmulatedBackend {
            engine: Mutex::new(GidsEngine::new(cert_der, private_key, pin.to_string())),
        })
    }

    #[allow(dead_code)]
    pub fn from_der(cert_der: &[u8], key_pkcs8_der: &[u8], pin: &str) -> Result<Self, String> {
        use rsa::pkcs8::DecodePrivateKey;
        let private_key =
            RsaPrivateKey::from_pkcs8_der(key_pkcs8_der).map_err(|e| format!("key DER: {}", e))?;
        Ok(EmulatedBackend {
            engine: Mutex::new(GidsEngine::new(
                cert_der.to_vec(),
                private_key,
                pin.to_string(),
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

const READER_NAME: &str = "Emulated Smartcard Reader";

impl SmartcardBackend for EmulatedBackend {
    fn establish_context(&self, _scope: u32) -> Result<ScardContext, u32> {
        Ok(ScardContext::new())
    }

    fn release_context(&self, _ctx: &ScardContext) -> Result<(), u32> {
        // Do NOT reset session — card state is shared across contexts
        Ok(())
    }

    fn is_valid_context(&self, _ctx: &ScardContext) -> bool {
        true
    }

    fn list_readers(&self, _ctx: &ScardContext, _: Option<&[String]>) -> Result<Vec<String>, u32> {
        Ok(vec![READER_NAME.to_string()])
    }

    fn connect(
        &self,
        _ctx: &ScardContext,
        reader: &str,
        _: u32,
        _: u32,
    ) -> Result<ConnectResult, u32> {
        if reader != READER_NAME {
            return Err(SCARD_E_UNKNOWN_READER);
        }
        // Do NOT reset session — Windows uses multiple concurrent connections
        // and resetting wipes the GIDS SELECT AID state from another connection.
        Ok(ConnectResult {
            handle: ScardHandle::new(SCARD_PROTOCOL_T0),
            active_protocol: SCARD_PROTOCOL_T0,
        })
    }

    fn disconnect(&self, _handle: &ScardHandle, _disposition: u32) -> Result<(), u32> {
        Ok(())
    }
    fn reconnect(&self, _: &ScardHandle, _: u32, _: u32, _: u32) -> Result<u32, u32> {
        Ok(SCARD_PROTOCOL_T0)
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

    fn control(&self, _: &ScardHandle, _: u32, _: &[u8]) -> Result<Vec<u8>, u32> {
        Ok(vec![])
    }

    fn status(&self, _: &ScardHandle) -> Result<ScardStatus, u32> {
        Ok(ScardStatus {
            reader_names: vec![READER_NAME.to_string()],
            state: SCARD_STATE_PRESENT,
            protocol: SCARD_PROTOCOL_T0,
            atr: vec![
                0x3B, 0xF7, 0x18, 0x00, 0x00, 0x80, 0x31, 0xFE, 0x45, 0x73, 0x66, 0x74, 0x65, 0x2D,
                0x6E, 0x66, 0xC4,
            ],
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
                    atr: vec![
                        0x3B, 0xF7, 0x18, 0x00, 0x00, 0x80, 0x31, 0xFE, 0x45, 0x73, 0x66, 0x74,
                        0x65, 0x2D, 0x6E, 0x66, 0xC4,
                    ],
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
