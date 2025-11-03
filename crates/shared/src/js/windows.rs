use anyhow::Result;
use std::ptr;

use base64::{Engine as _, engine::general_purpose};

use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_NONE, REG_SZ, RRF_RT_REG_SZ,
            RegCloseKey, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
        },
    },
    core::PCWSTR,
};

pub(super) fn crypt_protect_data(input: &str) -> Result<String> {
    let wide = widestring::U16CString::from_str(input)?;
    let input_bytes = unsafe {
        std::slice::from_raw_parts(
            wide.as_ptr() as *const u8,
            wide.as_slice_with_nul().len() * 2,
        )
    };

    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input_bytes.len() as u32,
        pbData: input_bytes.as_ptr() as *mut u8,
    };

    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe { CryptProtectData(&in_blob, None, None, None, None, 0, &mut out_blob) }?;

    let encrypted =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };

    unsafe {
        LocalFree(Some(HLOCAL(out_blob.pbData as *mut _)));
    }

    let encoded = general_purpose::STANDARD.encode(&encrypted);
    Ok(encoded)
}

pub(super) fn write_hkcu(key: &str, value_name: &str, value_data: &str) -> Result<()> {
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

#[derive(PartialEq, Eq, Hash)]
pub(super) enum KeyType {
    Hkcu,
    Hklm,
}

pub(super) fn read_key(key_type: KeyType, key: &str, value_name: &str) -> anyhow::Result<String> {
    let mut hkey: HKEY = HKEY::default();

    // Keep the widestrings alive
    let key_w = widestring::U16CString::from_str(key)?; // To avoid dropping
    let key = PCWSTR::from_raw(key_w.as_ptr());
    let value_w = widestring::U16CString::from_str(value_name)?; // To avoid dropping
    let value = PCWSTR::from_raw(value_w.as_ptr());

    unsafe {
        // Abrimos la clave en HKCU
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

        // Primera llamada: obtener tamaño
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

        // Reservamos buffer
        let mut buffer: Vec<u16> = vec![0; (data_len / 2) as usize];

        // Segunda llamada: obtener valor real
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
