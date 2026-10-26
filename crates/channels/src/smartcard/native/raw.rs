// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Janier Rodríguez, jrodriguez at virtualcable dot es

use std::ffi::CStr;
use std::ptr;
use std::time::Duration;

use pcsc::ffi::{self, DWORD, LONG, SCARD_IO_REQUEST, SCARD_READERSTATE};

// Invariant for every unsafe block here: the handle came from a successful
// SCardEstablishContext/SCardConnect and every buffer outlives the call it is lent to.

const READER_LIST_SIZE: usize = 4096;

pub(crate) fn scard_result(rc: LONG) -> Result<(), u32> {
    if rc == ffi::SCARD_S_SUCCESS {
        Ok(())
    } else {
        Err(rc as u32)
    }
}

pub(crate) struct RawContext {
    handle: ffi::SCARDCONTEXT,
}

impl RawContext {
    pub fn establish() -> Result<Self, u32> {
        let mut handle: ffi::SCARDCONTEXT = 0;
        scard_result(unsafe {
            ffi::SCardEstablishContext(
                ffi::SCARD_SCOPE_SYSTEM,
                ptr::null(),
                ptr::null(),
                &mut handle,
            )
        })?;
        Ok(RawContext { handle })
    }

    pub fn list_readers(&self) -> Result<Vec<String>, u32> {
        let mut buf = vec![0u8; READER_LIST_SIZE];
        let mut len = buf.len() as DWORD;
        scard_result(unsafe {
            ffi::SCardListReaders(self.handle, ptr::null(), buf.as_mut_ptr().cast(), &mut len)
        })?;
        Ok(parse_multi_string(&buf[..(len as usize).min(buf.len())]))
    }

    pub fn get_status_change(
        &self,
        timeout: Duration,
        states: &mut [SCARD_READERSTATE],
    ) -> Result<(), u32> {
        let timeout_ms = timeout.as_millis().min(ffi::INFINITE as u128) as DWORD;
        scard_result(unsafe {
            ffi::SCardGetStatusChange(
                self.handle,
                timeout_ms,
                states.as_mut_ptr(),
                states.len() as DWORD,
            )
        })
    }

    pub fn cancel(&self) -> Result<(), u32> {
        scard_result(unsafe { ffi::SCardCancel(self.handle) })
    }

    pub fn connect(
        &self,
        reader: &CStr,
        share_mode: DWORD,
        preferred_protocols: DWORD,
    ) -> Result<(RawCard, DWORD), u32> {
        let mut handle: ffi::SCARDHANDLE = 0;
        let mut active_protocol: DWORD = 0;
        scard_result(unsafe {
            ffi::SCardConnect(
                self.handle,
                reader.as_ptr(),
                share_mode,
                preferred_protocols,
                &mut handle,
                &mut active_protocol,
            )
        })?;
        Ok((RawCard { handle }, active_protocol))
    }
}

impl Drop for RawContext {
    fn drop(&mut self) {
        unsafe {
            ffi::SCardReleaseContext(self.handle);
        }
    }
}

pub(crate) struct CardStatus {
    pub state: DWORD,
    pub protocol: DWORD,
    pub atr: Vec<u8>,
}

pub(crate) struct RawCard {
    handle: ffi::SCARDHANDLE,
}

impl RawCard {
    pub fn disconnect(self, disposition: DWORD) -> Result<(), u32> {
        scard_result(unsafe { ffi::SCardDisconnect(self.handle, disposition) })
    }

    pub fn reconnect(
        &self,
        share_mode: DWORD,
        preferred_protocols: DWORD,
        initialization: DWORD,
    ) -> Result<DWORD, u32> {
        let mut active_protocol: DWORD = 0;
        scard_result(unsafe {
            ffi::SCardReconnect(
                self.handle,
                share_mode,
                preferred_protocols,
                initialization,
                &mut active_protocol,
            )
        })?;
        Ok(active_protocol)
    }

    pub fn begin_transaction(&self) -> Result<(), u32> {
        scard_result(unsafe { ffi::SCardBeginTransaction(self.handle) })
    }

    pub fn end_transaction(&self, disposition: DWORD) -> Result<(), u32> {
        scard_result(unsafe { ffi::SCardEndTransaction(self.handle, disposition) })
    }

    pub fn status(&self) -> Result<CardStatus, u32> {
        let mut state: DWORD = 0;
        let mut protocol: DWORD = 0;
        let mut atr = [0u8; ffi::ATR_BUFFER_SIZE];
        let mut atr_len = atr.len() as DWORD;
        scard_result(unsafe {
            ffi::SCardStatus(
                self.handle,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut state,
                &mut protocol,
                atr.as_mut_ptr(),
                &mut atr_len,
            )
        })?;
        Ok(CardStatus {
            state,
            protocol,
            atr: atr[..(atr_len as usize).min(atr.len())].to_vec(),
        })
    }

    pub fn transmit(
        &self,
        send_pci: &SCARD_IO_REQUEST,
        data: &[u8],
        recv_buf: &mut [u8],
    ) -> Result<usize, u32> {
        let mut recv_len = recv_buf.len() as DWORD;
        scard_result(unsafe {
            ffi::SCardTransmit(
                self.handle,
                send_pci,
                data.as_ptr(),
                data.len() as DWORD,
                ptr::null_mut(),
                recv_buf.as_mut_ptr(),
                &mut recv_len,
            )
        })?;
        Ok((recv_len as usize).min(recv_buf.len()))
    }

    pub fn control(
        &self,
        control_code: DWORD,
        in_data: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, u32> {
        let mut out_len: DWORD = 0;
        scard_result(unsafe {
            ffi::SCardControl(
                self.handle,
                control_code,
                in_data.as_ptr(),
                in_data.len() as DWORD,
                out_buf.as_mut_ptr(),
                out_buf.len() as DWORD,
                &mut out_len,
            )
        })?;
        Ok((out_len as usize).min(out_buf.len()))
    }

    pub fn get_attrib(&self, attr_id: DWORD, buf: &mut [u8]) -> Result<usize, u32> {
        let mut len = buf.len() as DWORD;
        scard_result(unsafe {
            ffi::SCardGetAttrib(self.handle, attr_id, buf.as_mut_ptr(), &mut len)
        })?;
        Ok((len as usize).min(buf.len()))
    }

    pub fn set_attrib(&self, attr_id: DWORD, data: &[u8]) -> Result<(), u32> {
        scard_result(unsafe {
            ffi::SCardSetAttrib(self.handle, attr_id, data.as_ptr(), data.len() as DWORD)
        })
    }
}

pub(crate) fn pci_for(winscard_protocol: u32) -> Option<&'static SCARD_IO_REQUEST> {
    match winscard_protocol {
        1 => Some(unsafe { &ffi::g_rgSCardT0Pci }),
        2 => Some(unsafe { &ffi::g_rgSCardT1Pci }),
        0x0001_0000 => Some(unsafe { &ffi::g_rgSCardRawPci }),
        _ => None,
    }
}

fn parse_multi_string(buf: &[u8]) -> Vec<String> {
    buf.split(|b| *b == 0)
        .take_while(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scard_result_passes_codes_through_unchanged() {
        assert_eq!(scard_result(ffi::SCARD_S_SUCCESS), Ok(()));
        assert_eq!(scard_result(ffi::SCARD_E_TIMEOUT), Err(0x8010_000A));
        assert_eq!(scard_result(ffi::SCARD_E_CANCELLED), Err(0x8010_0002));
        assert_eq!(scard_result(0x32_u32 as LONG), Err(0x32));
        assert_eq!(scard_result(0x8010_0066_u32 as LONG), Err(0x8010_0066));
    }

    #[test]
    fn pci_follows_winscard_protocol_numbers() {
        assert_eq!(
            pci_for(1).map(|p| p.dwProtocol),
            Some(ffi::SCARD_PROTOCOL_T0)
        );
        assert_eq!(
            pci_for(2).map(|p| p.dwProtocol),
            Some(ffi::SCARD_PROTOCOL_T1)
        );
        assert_eq!(
            pci_for(0x0001_0000).map(|p| p.dwProtocol),
            Some(ffi::SCARD_PROTOCOL_RAW)
        );
        assert!(pci_for(0).is_none());
        assert!(pci_for(3).is_none());
    }

    #[test]
    fn multi_string_stops_at_the_double_nul() {
        let names = parse_multi_string(b"Reader A\0Reader B\0\0garbage\0");
        assert_eq!(names, vec!["Reader A", "Reader B"]);
        assert!(parse_multi_string(b"\0").is_empty());
        assert!(parse_multi_string(b"").is_empty());
    }
}
