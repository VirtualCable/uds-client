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
    let value_name_w = widestring::U16CString::from_str(value_name)?; // To avoid dropping
    let value_name = PCWSTR::from_raw(value_name_w.as_ptr());

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

pub fn read_hklm_str(key: &str, value_name: &str) -> anyhow::Result<String> {
    read_key_str(KeyType::Hklm, key, value_name)
}

pub fn read_hkcu_str(key: &str, value_name: &str) -> anyhow::Result<String> {
    read_key_str(KeyType::Hkcu, key, value_name)
}
