use anyhow::Result;
use std::ptr;

use base64::{Engine as _, engine::general_purpose};

use windows::Win32::{
    Foundation::{HLOCAL, LocalFree},
    Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    },
};

pub fn crypt_protect_data(input: &str) -> Result<String> {
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

    unsafe {
        // First try with CRYPTPROTECT_LOCAL_MACHINE, if fails, try with CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN, that is more permissive
        if CryptProtectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_LOCAL_MACHINE,
            &mut out_blob,
        )
        .is_err()
        {
            CryptProtectData(
                &in_blob,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )?;
        }
    }

    let encrypted =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };

    unsafe {
        LocalFree(Some(HLOCAL(out_blob.pbData as *mut _)));
    }

    let encoded = general_purpose::STANDARD.encode(&encrypted);
    Ok(encoded)
}
