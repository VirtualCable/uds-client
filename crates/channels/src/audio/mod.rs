// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// See LICENSE file for full license text.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub mod input;
pub mod output;
pub mod tools;

pub use input::{MicCommand, MicHandle};
pub use output::{AudioCommand, AudioHandle, AudioStats};
