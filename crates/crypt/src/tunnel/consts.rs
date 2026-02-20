// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub const MAX_PACKET_SIZE: usize = 4096; // Hard limit for packet size. Anythig abobe this will be rejected.
pub const HEADER_LENGTH: usize = 8 + 2; // counter (8 bytes) + length (2 bytes)
pub const TAG_LENGTH: usize = 16; // AES-GCM tag length
// IPv6 minimum MTU is 1280 bytes, minus IP (40 bytes) and UDP (8 bytes, future) headers - leaves 1232 bytes for payload
// We use 1200 + HEADER_LENGTH + TAG_LENGTH = 1226 bytes to have some margin
pub const CRYPT_PACKET_SIZE: usize = 1200; // This is our preferred packet size for encryption/decryption

// Max time once a crypt packet is started before receive it completely, to avoid hanging connections
// Its long enough to allow for slow connections, but short enough to avoid a malformed packet to keep the connection hanging indefinitely
pub const CRYPT_HANDSHAKE_TIMEOUT_SECS: u64 = 5;
