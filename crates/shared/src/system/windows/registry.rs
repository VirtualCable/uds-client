// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use anyhow::Result;

use windows::{
    Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_NONE, REG_SZ,
        RRF_RT_REG_SZ, RegCloseKey, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    },
    core::PCWSTR,
};

pub fn write_hkcu_str(key: &str, value_name: &str, value_data: &str) -> Result<()> {
    let mut hkey = HKEY::default();

    // Keep the widestrings alive
    let key_w = widestring::U16CString::from_str(key)?; // To avoid dropping
    let key = PCWSTR::from_raw(key_w.as_ptr());
    let value_name_w = widestring::U16CString::from_str(value_name)?; // To avoid dropping
    let value_name = PCWSTR::from_raw(value_name_w.as_ptr());

    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, key, None, KEY_SET_VALUE, &mut hkey).ok()?;

        let data_bytes = value_data.as_bytes();
        RegSetValueExW(hkey, value_name, None, REG_SZ, Some(data_bytes)).ok()?;

        RegCloseKey(hkey).ok()?;
    }

    Ok(())
}

pub fn write_hkcu_dword(key: &str, value_name: &str, value_data: u32) -> Result<()> {
    let mut hkey = HKEY::default();
    // Keep the widestrings alive
    let key_w = widestring::U16CString::from_str(key)?; // To avoid dropping
    let key = PCWSTR::from_raw(key_w.as_ptr());

    // If value name is empty, use default value
    let value_name = if value_name.is_empty() {
        PCWSTR::null()
    } else {
        let value_name_w = widestring::U16CString::from_str(value_name)?;
        PCWSTR::from_raw(value_name_w.as_ptr())
    };

    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, key, None, KEY_SET_VALUE, &mut hkey).ok()?;

        // DWORD must be passed as a byte slice; use little-endian bytes for the u32
        let data_bytes = value_data.to_le_bytes();
        RegSetValueExW(hkey, value_name, None, REG_DWORD, Some(&data_bytes)).ok()?;

        RegCloseKey(hkey).ok()?;
    }

    Ok(())
}

#[derive(PartialEq, Eq, Hash)]
pub(super) enum KeyType {
    Hkcu,
    Hklm,
}

fn read_key_str(key_type: KeyType, key: &str, value_name: &str) -> anyhow::Result<String> {
    let mut hkey: HKEY = HKEY::default();

    // Keep the widestrings alive
    let key_w = widestring::U16CString::from_str(key)?; // To avoid dropping
    let key = PCWSTR::from_raw(key_w.as_ptr());
    let value_w = widestring::U16CString::from_str(value_name)?; // To avoid dropping
    let value = PCWSTR::from_raw(value_w.as_ptr());

    unsafe {
        // Open the key in HKCU
        RegOpenKeyExW(
            if key_type == KeyType::Hkcu {
                HKEY_CURRENT_USER
            } else {
                windows::Win32::System::Registry::HKEY_LOCAL_MACHINE
            },
            key,
            None,
            KEY_READ,
            &mut hkey,
        )
        .ok()?;

        let mut data_type = REG_NONE;
        let mut data_len: u32 = 0;

        // First call: get the required size
        RegGetValueW(
            hkey,
            PCWSTR::null(),
            value,
            RRF_RT_REG_SZ,
            Some(&mut data_type),
            None,
            Some(&mut data_len),
        )
        .ok()?;

        // Allocate buffer
        let mut buffer: Vec<u16> = vec![0; (data_len / 2) as usize];

        // Second call: read the actual value
        RegGetValueW(
            hkey,
            PCWSTR::null(),
            value,
            RRF_RT_REG_SZ,
            Some(&mut data_type),
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut data_len),
        )
        .ok()?;

        RegCloseKey(hkey).ok()?;

        Ok(widestring::U16CString::from_vec_truncate(buffer).to_string_lossy())
    }
}

pub fn read_hklm_str(key: &str, value_name: &str) -> anyhow::Result<String> {
    read_key_str(KeyType::Hklm, key, value_name)
}

pub fn read_hkcu_str(key: &str, value_name: &str) -> anyhow::Result<String> {
    read_key_str(KeyType::Hkcu, key, value_name)
}
