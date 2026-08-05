// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Core types for the eUDS Engine.

use super::consts::DEFAULT_PIN_RETRIES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMode {
    Required,
    NotRequired,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub pin_verified: bool,
    pub pin_retries: u8,
    pub chaining_buffer: Option<Vec<u8>>,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState {
            pin_verified: false,
            pin_retries: DEFAULT_PIN_RETRIES,
            chaining_buffer: None,
        }
    }
}
