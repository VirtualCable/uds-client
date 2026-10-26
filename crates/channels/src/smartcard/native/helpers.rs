// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
//          Janier Rodríguez, jrodriguez at virtualcable dot es

use pcsc::ffi::{self, DWORD};
use rdp::integrations::smartcard::consts::*;

/// Build a `Cached_GeneralFile/...` value (16-byte CSP header + data).
pub(crate) fn build_general_file_value(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + data.len());
    out.extend_from_slice(&[
        0x00, 0x00, 0x01, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// Strip the DO wrapper from a GET DATA response (`tag` + BER length) and the trailing SW.
/// Handles `82 xx xx` (2-byte) BER lengths.
pub(crate) fn strip_do(response: &[u8]) -> Option<&[u8]> {
    if response.len() < 4 {
        return None;
    }
    let (header, len) = match response[2] {
        0x81 => (3, response[3] as usize),
        0x82 if response.len() >= 5 => (4, (response[3] as usize) << 8 | response[4] as usize),
        l if l < 0x80 => (3, l as usize),
        _ => return None,
    };
    let start = header;
    let end = start + len;
    let full_end = response.len() - 2; // strip SW1SW2
    if full_end >= end {
        Some(&response[start..end])
    } else {
        None
    }
}

pub(crate) fn dword_to_share_mode(mode: DWORD) -> Result<DWORD, u32> {
    match mode {
        ffi::SCARD_SHARE_EXCLUSIVE | ffi::SCARD_SHARE_SHARED | ffi::SCARD_SHARE_DIRECT => Ok(mode),
        _ => Err(SCARD_E_INVALID_PARAMETER),
    }
}

/// The active protocol as WinSCard numbers it; pcsc-lite uses 4 for RAW where
/// Windows uses 0x10000, and 0 (undefined) must stay 0 for direct connections.
pub(crate) fn active_protocol_to_u32(protocol: DWORD) -> u32 {
    match protocol {
        ffi::SCARD_PROTOCOL_T0 => SCARD_PROTOCOL_T0,
        ffi::SCARD_PROTOCOL_T1 => SCARD_PROTOCOL_T1,
        ffi::SCARD_PROTOCOL_RAW => SCARD_PROTOCOL_RAW,
        other => other as u32,
    }
}

pub(crate) fn dword_to_disposition(disp: DWORD) -> Result<DWORD, u32> {
    match disp {
        ffi::SCARD_LEAVE_CARD
        | ffi::SCARD_RESET_CARD
        | ffi::SCARD_UNPOWER_CARD
        | ffi::SCARD_EJECT_CARD => Ok(disp),
        _ => Err(SCARD_E_INVALID_PARAMETER),
    }
}

// SCardStatus card states (winscard.h). Unlike SCARD_STATE_* these are an
// enumeration, not flags.
const SCARD_UNKNOWN: u32 = 0;
const SCARD_ABSENT: u32 = 1;
const SCARD_PRESENT: u32 = 2;
const SCARD_SWALLOWED: u32 = 3;
const SCARD_POWERED: u32 = 4;
const SCARD_SPECIFIC: u32 = 6;

/// Card state for SCardStatus, as Windows reports it. A connected card is
/// reported as SCARD_SPECIFIC like FreeRDP does: pcsc-lite never sets it and
/// Windows minidrivers expect it.
pub(crate) fn pcsc_status_to_u32(status: pcsc::Status) -> u32 {
    if status.intersects(pcsc::Status::SPECIFIC | pcsc::Status::NEGOTIABLE) {
        SCARD_SPECIFIC
    } else if status.contains(pcsc::Status::POWERED) {
        SCARD_POWERED
    } else if status.contains(pcsc::Status::ABSENT) {
        SCARD_ABSENT
    } else if status.contains(pcsc::Status::PRESENT) {
        SCARD_PRESENT
    } else if status.contains(pcsc::Status::SWALLOWED) {
        SCARD_SWALLOWED
    } else {
        SCARD_UNKNOWN
    }
}

// SCARD_CLASS_SYSTEM attribute ids as Windows sends them (class 0x7FFF). pcsc-sys
// uses the pcsc-lite class value 0, so its `ffi::SCARD_ATTR_DEVICE_*` constants
// never match what arrives from the remote side.
const WIN_SCARD_ATTR_DEVICE_UNIT: DWORD = 0x7FFF_0001;
const WIN_SCARD_ATTR_DEVICE_FRIENDLY_NAME_A: DWORD = 0x7FFF_0003;
const WIN_SCARD_ATTR_DEVICE_SYSTEM_NAME_A: DWORD = 0x7FFF_0004;
const WIN_SCARD_ATTR_DEVICE_FRIENDLY_NAME_W: DWORD = 0x7FFF_0005;
const WIN_SCARD_ATTR_DEVICE_SYSTEM_NAME_W: DWORD = 0x7FFF_0006;

/// Error Windows returns from SCardGetAttrib for an attribute the reader lacks.
pub(crate) const ERROR_NOT_SUPPORTED: u32 = 0x32;

/// Answers the SCARD_CLASS_SYSTEM attributes the same way mstsc does for a
/// redirected reader: device unit 0, and the reader name as ASCII or UTF-16LE.
/// Byte layouts were taken from SCardGetAttrib in a Windows RDP session: the
/// friendly names end in a double NUL, the system names in a single NUL.
pub(crate) fn system_attribute(attr: DWORD, reader_name: &str) -> Option<Vec<u8>> {
    let ascii = |nuls: usize| {
        let mut out = reader_name.as_bytes().to_vec();
        out.extend(std::iter::repeat_n(0, nuls));
        out
    };
    let utf16 = |nuls: usize| {
        reader_name
            .encode_utf16()
            .chain(std::iter::repeat_n(0, nuls))
            .flat_map(u16::to_le_bytes)
            .collect()
    };
    match attr {
        WIN_SCARD_ATTR_DEVICE_UNIT => Some(0u32.to_le_bytes().to_vec()),
        WIN_SCARD_ATTR_DEVICE_FRIENDLY_NAME_A => Some(ascii(2)),
        WIN_SCARD_ATTR_DEVICE_SYSTEM_NAME_A => Some(ascii(1)),
        WIN_SCARD_ATTR_DEVICE_FRIENDLY_NAME_W => Some(utf16(2)),
        WIN_SCARD_ATTR_DEVICE_SYSTEM_NAME_W => Some(utf16(1)),
        _ => None,
    }
}

pub(crate) fn dword_to_attribute(attr: DWORD) -> Result<pcsc::Attribute, u32> {
    match attr {
        ffi::SCARD_ATTR_VENDOR_NAME => Ok(pcsc::Attribute::VendorName),
        ffi::SCARD_ATTR_VENDOR_IFD_TYPE => Ok(pcsc::Attribute::VendorIfdType),
        ffi::SCARD_ATTR_VENDOR_IFD_VERSION => Ok(pcsc::Attribute::VendorIfdVersion),
        ffi::SCARD_ATTR_VENDOR_IFD_SERIAL_NO => Ok(pcsc::Attribute::VendorIfdSerialNo),
        ffi::SCARD_ATTR_CHANNEL_ID => Ok(pcsc::Attribute::ChannelId),
        ffi::SCARD_ATTR_ASYNC_PROTOCOL_TYPES => Ok(pcsc::Attribute::AsyncProtocolTypes),
        ffi::SCARD_ATTR_DEFAULT_CLK => Ok(pcsc::Attribute::DefaultClk),
        ffi::SCARD_ATTR_MAX_CLK => Ok(pcsc::Attribute::MaxClk),
        ffi::SCARD_ATTR_DEFAULT_DATA_RATE => Ok(pcsc::Attribute::DefaultDataRate),
        ffi::SCARD_ATTR_MAX_DATA_RATE => Ok(pcsc::Attribute::MaxDataRate),
        ffi::SCARD_ATTR_MAX_IFSD => Ok(pcsc::Attribute::MaxIfsd),
        ffi::SCARD_ATTR_SYNC_PROTOCOL_TYPES => Ok(pcsc::Attribute::SyncProtocolTypes),
        ffi::SCARD_ATTR_POWER_MGMT_SUPPORT => Ok(pcsc::Attribute::PowerMgmtSupport),
        ffi::SCARD_ATTR_USER_TO_CARD_AUTH_DEVICE => Ok(pcsc::Attribute::UserToCardAuthDevice),
        ffi::SCARD_ATTR_USER_AUTH_INPUT_DEVICE => Ok(pcsc::Attribute::UserAuthInputDevice),
        ffi::SCARD_ATTR_CHARACTERISTICS => Ok(pcsc::Attribute::Characteristics),
        ffi::SCARD_ATTR_CURRENT_PROTOCOL_TYPE => Ok(pcsc::Attribute::CurrentProtocolType),
        ffi::SCARD_ATTR_CURRENT_CLK => Ok(pcsc::Attribute::CurrentClk),
        ffi::SCARD_ATTR_CURRENT_F => Ok(pcsc::Attribute::CurrentF),
        ffi::SCARD_ATTR_CURRENT_D => Ok(pcsc::Attribute::CurrentD),
        ffi::SCARD_ATTR_CURRENT_N => Ok(pcsc::Attribute::CurrentN),
        ffi::SCARD_ATTR_CURRENT_W => Ok(pcsc::Attribute::CurrentW),
        ffi::SCARD_ATTR_CURRENT_IFSC => Ok(pcsc::Attribute::CurrentIfsc),
        ffi::SCARD_ATTR_CURRENT_IFSD => Ok(pcsc::Attribute::CurrentIfsd),
        ffi::SCARD_ATTR_CURRENT_BWT => Ok(pcsc::Attribute::CurrentBwt),
        ffi::SCARD_ATTR_CURRENT_CWT => Ok(pcsc::Attribute::CurrentCwt),
        ffi::SCARD_ATTR_CURRENT_EBC_ENCODING => Ok(pcsc::Attribute::CurrentEbcEncoding),
        ffi::SCARD_ATTR_EXTENDED_BWT => Ok(pcsc::Attribute::ExtendedBwt),
        ffi::SCARD_ATTR_ICC_PRESENCE => Ok(pcsc::Attribute::IccPresence),
        ffi::SCARD_ATTR_ICC_INTERFACE_STATUS => Ok(pcsc::Attribute::IccInterfaceStatus),
        ffi::SCARD_ATTR_CURRENT_IO_STATE => Ok(pcsc::Attribute::CurrentIoState),
        ffi::SCARD_ATTR_ATR_STRING => Ok(pcsc::Attribute::AtrString),
        ffi::SCARD_ATTR_ICC_TYPE_PER_ATR => Ok(pcsc::Attribute::IccTypePerAtr),
        ffi::SCARD_ATTR_ESC_RESET => Ok(pcsc::Attribute::EscReset),
        ffi::SCARD_ATTR_ESC_CANCEL => Ok(pcsc::Attribute::EscCancel),
        ffi::SCARD_ATTR_ESC_AUTHREQUEST => Ok(pcsc::Attribute::EscAuthrequest),
        ffi::SCARD_ATTR_MAXINPUT => Ok(pcsc::Attribute::Maxinput),
        ffi::SCARD_ATTR_DEVICE_UNIT => Ok(pcsc::Attribute::DeviceUnit),
        ffi::SCARD_ATTR_DEVICE_IN_USE => Ok(pcsc::Attribute::DeviceInUse),
        ffi::SCARD_ATTR_DEVICE_FRIENDLY_NAME => Ok(pcsc::Attribute::DeviceFriendlyName),
        ffi::SCARD_ATTR_DEVICE_SYSTEM_NAME => Ok(pcsc::Attribute::DeviceSystemName),
        ffi::SCARD_ATTR_SUPRESS_T1_IFS_REQUEST => Ok(pcsc::Attribute::SupressT1IfsRequest),
        _ => Err(SCARD_E_UNSUPPORTED_FEATURE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lengths as returned by SCardGetAttrib under mstsc for this reader name.
    const READER: &str = "Generic Smart Card Reader Interface 0";

    #[test]
    fn system_attributes_match_mstsc_layout() {
        let name = |attr| system_attribute(attr, READER).unwrap();
        assert_eq!(name(0x7FFF_0001), vec![0, 0, 0, 0]);
        assert_eq!(name(0x7FFF_0003).len(), 39);
        assert_eq!(name(0x7FFF_0004).len(), 38);
        assert_eq!(name(0x7FFF_0005).len(), 78);
        assert_eq!(name(0x7FFF_0006).len(), 76);
        assert!(name(0x7FFF_0003).starts_with(READER.as_bytes()));
        assert_eq!(&name(0x7FFF_0006)[..4], &[b'G', 0, b'e', 0]);
        assert_eq!(&name(0x7FFF_0006)[74..], &[0, 0]);
    }

    #[test]
    fn connected_card_is_specific() {
        assert_eq!(pcsc_status_to_u32(pcsc::Status::SPECIFIC), 6);
        assert_eq!(pcsc_status_to_u32(pcsc::Status::NEGOTIABLE), 6);
        assert_eq!(
            pcsc_status_to_u32(
                pcsc::Status::PRESENT | pcsc::Status::POWERED | pcsc::Status::SPECIFIC
            ),
            6
        );
    }

    #[test]
    fn card_states_are_an_enumeration() {
        assert_eq!(pcsc_status_to_u32(pcsc::Status::empty()), 0);
        assert_eq!(pcsc_status_to_u32(pcsc::Status::ABSENT), 1);
        assert_eq!(pcsc_status_to_u32(pcsc::Status::PRESENT), 2);
        assert_eq!(pcsc_status_to_u32(pcsc::Status::SWALLOWED), 3);
        assert_eq!(
            pcsc_status_to_u32(pcsc::Status::PRESENT | pcsc::Status::POWERED),
            4
        );
    }

    #[test]
    fn active_protocol_keeps_undefined_and_renumbers_raw() {
        assert_eq!(active_protocol_to_u32(ffi::SCARD_PROTOCOL_UNDEFINED), 0);
        assert_eq!(active_protocol_to_u32(ffi::SCARD_PROTOCOL_T0), 1);
        assert_eq!(active_protocol_to_u32(ffi::SCARD_PROTOCOL_T1), 2);
        assert_eq!(active_protocol_to_u32(ffi::SCARD_PROTOCOL_RAW), 0x0001_0000);
    }

    #[test]
    fn share_modes_and_dispositions_are_validated() {
        assert_eq!(dword_to_share_mode(3), Ok(3));
        assert_eq!(dword_to_share_mode(4), Err(SCARD_E_INVALID_PARAMETER));
        assert_eq!(dword_to_disposition(0), Ok(0));
        assert_eq!(dword_to_disposition(3), Ok(3));
        assert_eq!(dword_to_disposition(4), Err(SCARD_E_INVALID_PARAMETER));
    }

    #[test]
    fn other_attributes_are_not_system_attributes() {
        assert_eq!(system_attribute(0x7FFF_0002, READER), None);
        assert_eq!(system_attribute(0x7FFF_0007, READER), None);
        assert_eq!(system_attribute(0x0000_0006, READER), None);
    }
}
