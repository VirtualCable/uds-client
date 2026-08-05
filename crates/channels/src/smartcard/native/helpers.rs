// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use pcsc::ffi;
use rdp::integrations::smartcard::consts::*;

/// Build a `Cached_GeneralFile/...` value (16-byte CSP header + data).
pub(crate) fn build_general_file_value(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + data.len());
    out.extend_from_slice(&[0x00, 0x00, 0x01, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
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

/// Map pcsc::Error to standard PC/SC u32 error codes
pub(crate) fn pcsc_error_to_u32(err: pcsc::Error) -> u32 {
    match err {
        pcsc::Error::Cancelled => SCARD_E_CANCELLED,
        pcsc::Error::InvalidHandle => SCARD_E_INVALID_HANDLE,
        pcsc::Error::InvalidParameter => SCARD_E_INVALID_PARAMETER,
        pcsc::Error::NoMemory => SCARD_E_NO_MEMORY,
        pcsc::Error::InsufficientBuffer => SCARD_E_INSUFFICIENT_BUFFER,
        pcsc::Error::UnknownReader => SCARD_E_UNKNOWN_READER,
        pcsc::Error::Timeout => SCARD_E_TIMEOUT,
        pcsc::Error::SharingViolation => SCARD_E_SHARING_VIOLATION,
        pcsc::Error::NoSmartcard => SCARD_E_NO_SMARTCARD,
        pcsc::Error::UnknownCard => SCARD_E_UNKNOWN_CARD,
        pcsc::Error::ProtoMismatch => SCARD_E_PROTO_MISMATCH,
        pcsc::Error::NotReady => SCARD_E_NOT_READY,
        pcsc::Error::InvalidValue => SCARD_E_INVALID_VALUE,
        pcsc::Error::SystemCancelled => SCARD_E_SYSTEM_CANCELLED,
        pcsc::Error::ReaderUnavailable => SCARD_E_READER_UNAVAILABLE,
        pcsc::Error::UnsupportedCard => SCARD_W_UNSUPPORTED_CARD,
        pcsc::Error::UnresponsiveCard => SCARD_W_UNRESPONSIVE_CARD,
        pcsc::Error::UnpoweredCard => SCARD_W_UNPOWERED_CARD,
        pcsc::Error::ResetCard => SCARD_W_RESET_CARD,
        pcsc::Error::RemovedCard => SCARD_W_REMOVED_CARD,
        pcsc::Error::UnsupportedFeature => SCARD_E_UNSUPPORTED_FEATURE,
        pcsc::Error::NoService => SCARD_E_NO_SERVICE,
        pcsc::Error::ServiceStopped => SCARD_E_SERVICE_STOPPED,
        _ => SCARD_F_UNKNOWN_ERROR,
    }
}

pub(crate) fn u32_to_share_mode(mode: u32) -> Result<pcsc::ShareMode, u32> {
    match mode {
        SCARD_SHARE_EXCLUSIVE => Ok(pcsc::ShareMode::Exclusive),
        SCARD_SHARE_SHARED => Ok(pcsc::ShareMode::Shared),
        SCARD_SHARE_DIRECT => Ok(pcsc::ShareMode::Direct),
        _ => Err(SCARD_E_INVALID_PARAMETER),
    }
}

pub(crate) fn u32_to_protocols(proto: u32) -> pcsc::Protocols {
    let mut p = pcsc::Protocols::empty();
    if proto & SCARD_PROTOCOL_T0 != 0 {
        p.insert(pcsc::Protocols::T0);
    }
    if proto & SCARD_PROTOCOL_T1 != 0 {
        p.insert(pcsc::Protocols::T1);
    }
    if p.is_empty() {
        pcsc::Protocols::ANY
    } else {
        p
    }
}

pub(crate) fn protocol_to_u32(proto: pcsc::Protocol) -> u32 {
    match proto {
        pcsc::Protocol::T0 => SCARD_PROTOCOL_T0,
        pcsc::Protocol::T1 => SCARD_PROTOCOL_T1,
        pcsc::Protocol::RAW => SCARD_PROTOCOL_RAW,
    }
}

pub(crate) fn u32_to_disposition(disp: u32) -> Result<pcsc::Disposition, u32> {
    match disp {
        SCARD_LEAVE_CARD => Ok(pcsc::Disposition::LeaveCard),
        SCARD_RESET_CARD => Ok(pcsc::Disposition::ResetCard),
        SCARD_UNPOWER_CARD => Ok(pcsc::Disposition::UnpowerCard),
        SCARD_EJECT_CARD => Ok(pcsc::Disposition::EjectCard),
        _ => Err(SCARD_E_INVALID_PARAMETER),
    }
}

pub(crate) fn u32_to_state(bits: u32) -> pcsc::State {
    // Preserve all raw bits. `from_bits_truncate` would drop unknown flags such
    // as 0x0001_0000 that the RDPSC protocol uses for the `\\?PnP?\Notification`
    // pseudo-reader; dropping them makes SCardGetStatusChange report a spurious
    // CHANGED state every poll, which breaks the remote reader monitoring loop.
    pcsc::State::from_bits_retain(bits)
}

pub(crate) fn pcsc_status_to_u32(status: pcsc::Status) -> u32 {
    let mut state = 0;
    if status.contains(pcsc::Status::SPECIFIC) {
        state |= SCARD_STATE_PRESENT | SCARD_STATE_INUSE;
    } else if status.contains(pcsc::Status::NEGOTIABLE)
        || status.contains(pcsc::Status::POWERED)
        || status.contains(pcsc::Status::PRESENT)
    {
        state |= SCARD_STATE_PRESENT;
    } else if status.contains(pcsc::Status::ABSENT) {
        state |= SCARD_STATE_EMPTY;
    } else if status.contains(pcsc::Status::UNKNOWN) {
        state |= SCARD_STATE_UNKNOWN;
    }
    state
}

pub(crate) fn u32_to_attribute(attr: u32) -> Result<pcsc::Attribute, u32> {
    let dword_attr = attr as pcsc::ffi::DWORD;
    match dword_attr {
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
