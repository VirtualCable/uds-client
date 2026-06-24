// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Dummy smartcard backend — always available, no hardware required.
//!
//! Used as the default backend for `SmartcardHandle`. Reports virtual
//! readers/cards and responds with success (0x90 0x00) to all APDUs.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::time::Duration;

use rdp::integrations::smartcard::*;

use super::SmartcardBackend;

#[derive(Debug)]
struct DummyCard {
    reader: String,
    atr: Vec<u8>,
    handle: Option<ScardHandle>,
}

#[derive(Debug)]
pub(crate) struct DummyBackend {
    cards: RwLock<Vec<DummyCard>>,
    contexts: RwLock<HashMap<u64, ScardContext>>,
    /// Readers for which we've already reported the initial state.
    /// Used to avoid repeating SCARD_STATE_CHANGED on every poll.
    seen_readers: RwLock<HashSet<String>>,
}

impl DummyBackend {
    pub(crate) fn new() -> Self {
        DummyBackend {
            cards: RwLock::new(vec![DummyCard {
                reader: "Virtual Smartcard Reader 0".to_string(),
                atr: vec![
                    0x3B, 0xF7, 0x18, 0x00, 0x00, 0x80, 0x31, 0xFE, 0x45, 0x73, 0x66, 0x74, 0x65,
                    0x2D, 0x6E, 0x66, 0xC4,
                ],
                handle: None,
            }]),
            contexts: RwLock::new(HashMap::new()),
            seen_readers: RwLock::new(HashSet::new()),
        }
    }

    /// Register an additional virtual card for testing.
    #[allow(dead_code)]
    pub(crate) fn add_card(&self, reader: &str, atr: Vec<u8>) {
        let mut cards = self.cards.write().unwrap();
        cards.push(DummyCard {
            reader: reader.to_string(),
            atr,
            handle: None,
        });
    }
}

impl SmartcardBackend for DummyBackend {
    fn establish_context(&self, _scope: u32) -> Result<ScardContext, u32> {
        let ctx = ScardContext::new();
        let mut contexts = self.contexts.write().unwrap();
        contexts.insert(ctx.raw(), ctx);
        Ok(ctx)
    }

    fn release_context(&self, ctx: &ScardContext) -> Result<(), u32> {
        let mut contexts = self.contexts.write().unwrap();
        contexts.remove(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        Ok(())
    }

    fn is_valid_context(&self, ctx: &ScardContext) -> bool {
        let contexts = self.contexts.read().unwrap();
        contexts.contains_key(&ctx.raw())
    }

    fn list_readers(
        &self,
        _ctx: &ScardContext,
        _groups: Option<&[String]>,
    ) -> Result<Vec<String>, u32> {
        let cards = self.cards.read().unwrap();
        Ok(cards.iter().map(|c| c.reader.clone()).collect())
    }

    fn connect(
        &self,
        _ctx: &ScardContext,
        reader: &str,
        _share_mode: u32,
        _preferred_protocols: u32,
    ) -> Result<ConnectResult, u32> {
        let mut cards = self.cards.write().unwrap();
        for card in cards.iter_mut() {
            if card.reader == reader {
                let handle = ScardHandle::new(SCARD_PROTOCOL_T0);
                card.handle = Some(handle);
                return Ok(ConnectResult {
                    handle,
                    active_protocol: SCARD_PROTOCOL_T0,
                });
            }
        }
        Err(SCARD_E_UNKNOWN_READER)
    }

    fn disconnect(&self, handle: &ScardHandle, _disposition: u32) -> Result<(), u32> {
        let mut cards = self.cards.write().unwrap();
        for card in cards.iter_mut() {
            if let Some(h) = &card.handle
                && h.raw() == handle.raw()
            {
                card.handle = None;
                return Ok(());
            }
        }
        Err(SCARD_E_INVALID_HANDLE)
    }

    fn reconnect(
        &self,
        handle: &ScardHandle,
        _share_mode: u32,
        _preferred_protocols: u32,
        _initialization: u32,
    ) -> Result<u32, u32> {
        let cards = self.cards.read().unwrap();
        for card in cards.iter() {
            if let Some(h) = &card.handle
                && h.raw() == handle.raw()
            {
                return Ok(SCARD_PROTOCOL_T0);
            }
        }
        Err(SCARD_E_INVALID_HANDLE)
    }

    fn transmit(
        &self,
        handle: &ScardHandle,
        _send_pci: &ScardIORequest,
        _data: &[u8],
    ) -> Result<TransmitResult, u32> {
        let cards = self.cards.read().unwrap();
        for card in cards.iter() {
            if let Some(h) = &card.handle
                && h.raw() == handle.raw()
            {
                return Ok(TransmitResult {
                    recv_pci: None,
                    recv_buffer: vec![0x90, 0x00],
                });
            }
        }
        Err(SCARD_E_INVALID_HANDLE)
    }

    fn control(
        &self,
        _handle: &ScardHandle,
        _control_code: u32,
        _in_data: &[u8],
    ) -> Result<Vec<u8>, u32> {
        Ok(vec![])
    }

    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32> {
        let cards = self.cards.read().unwrap();
        for card in cards.iter() {
            if let Some(h) = &card.handle
                && h.raw() == handle.raw()
            {
                return Ok(ScardStatus {
                    reader_names: vec![card.reader.clone()],
                    state: SCARD_STATE_PRESENT,
                    protocol: SCARD_PROTOCOL_T0,
                    atr: card.atr.clone(),
                });
            }
        }
        Err(SCARD_E_INVALID_HANDLE)
    }

    fn get_status_change(
        &self,
        _ctx: &ScardContext,
        _timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<Vec<ReaderStateOut>, u32> {
        let cards = self.cards.read().unwrap();
        let mut seen = self.seen_readers.write().unwrap();

        Ok(reader_states
            .iter()
            .map(|rs| {
                let present = cards.iter().any(|c| c.reader == rs.reader_name);
                let is_new = !seen.contains(&rs.reader_name);

                let actual_state = if present {
                    SCARD_STATE_PRESENT
                } else {
                    SCARD_STATE_EMPTY
                };

                // Report CHANGED only if the caller's current_state differs from
                // the actual state, or if this is the first time we see this reader.
                let mut event_state = actual_state;
                if is_new || ((rs.current_state & !SCARD_STATE_CHANGED) != actual_state) {
                    event_state |= SCARD_STATE_CHANGED;
                    seen.insert(rs.reader_name.clone());
                }

                ReaderStateOut {
                    reader_name: rs.reader_name.clone(),
                    current_state: actual_state,
                    event_state,
                    atr: cards
                        .iter()
                        .find(|c| c.reader == rs.reader_name)
                        .map(|c| c.atr.clone())
                        .unwrap_or_default(),
                }
            })
            .collect())
    }

    fn begin_transaction(&self, _handle: &ScardHandle) -> Result<(), u32> {
        Ok(())
    }

    fn end_transaction(&self, _handle: &ScardHandle, _disposition: u32) -> Result<(), u32> {
        Ok(())
    }

    fn get_attrib(&self, _handle: &ScardHandle, _attr_id: u32) -> Result<Vec<u8>, u32> {
        Ok(vec![0x00])
    }

    fn set_attrib(&self, _handle: &ScardHandle, _attr_id: u32, _data: &[u8]) -> Result<(), u32> {
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}
