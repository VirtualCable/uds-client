use anyhow::Result;
use freerdp_sys::{BYTE, CAM_MEDIA_TYPE_DESCRIPTION, CAM_MSG_ID, UINT32};

/// PDU header on the wire: Version(1) + MessageId(4) = 5 bytes
pub const PDU_HEADER_SIZE: usize = 5;

/// Parse a PDU header from raw bytes.
/// Returns (version, msg_id, remaining_payload).
pub fn parse_pdu_header(data: &[u8]) -> Result<(u8, CAM_MSG_ID, &[u8])> {
    if data.len() < PDU_HEADER_SIZE {
        anyhow::bail!("PDU too short: {} < {PDU_HEADER_SIZE}", data.len());
    }
    let version = data[0];
    let msg_id = u32::from_le_bytes(data[1..PDU_HEADER_SIZE].try_into().unwrap()) as CAM_MSG_ID;
    Ok((version, msg_id, &data[PDU_HEADER_SIZE..]))
}

/// Parse a CAM_MEDIA_TYPE_DESCRIPTION (26 bytes) from raw bytes.
pub fn parse_media_type(data: &[u8]) -> Result<CAM_MEDIA_TYPE_DESCRIPTION> {
    let size = std::mem::size_of::<CAM_MEDIA_TYPE_DESCRIPTION>();
    anyhow::ensure!(
        data.len() >= size,
        "Too short for media type: {} < {size}",
        data.len()
    );
    // SAFETY: CAM_MEDIA_TYPE_DESCRIPTION is repr(C) and contains only integer fields
    let mt = unsafe { *(data.as_ptr() as *const CAM_MEDIA_TYPE_DESCRIPTION) };
    Ok(mt)
}

/// Build a PDU with just a header (version + msg_id), no body.
pub fn build_response_header(msg_id: UINT32) -> Vec<u8> {
    let mut pdu = Vec::with_capacity(PDU_HEADER_SIZE);
    pdu.push(1u8); // version
    pdu.extend_from_slice(&msg_id.to_le_bytes());
    pdu
}

/// Build a SampleResponse PDU.
/// Wire format: Version(1) + MessageId(4) + StreamIndex(1) + SampleSize(8) + data.
pub fn build_sample_response(stream_index: u8, sample: &[u8]) -> Vec<u8> {
    let sample_size = sample.len();
    let mut pdu = Vec::with_capacity(PDU_HEADER_SIZE + 1 + 8 + sample_size);
    pdu.push(1u8);
    pdu.extend_from_slice(&(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_SampleResponse as u32).to_le_bytes());
    pdu.push(stream_index);
    pdu.extend_from_slice(&(sample_size as u64).to_le_bytes());
    pdu.extend_from_slice(sample);
    pdu
}

/// Write a PDU to the DVC channel.
/// SAFETY: `channel` must be a valid IWTSVirtualChannel pointer.
pub unsafe fn write_to_channel(channel: *mut freerdp_sys::IWTSVirtualChannel, pdu: &[u8]) {
    unsafe {
        if let Some(write_fn) = (*channel).Write {
            write_fn(
                channel,
                pdu.len() as u32,
                pdu.as_ptr() as *mut BYTE,
                std::ptr::null_mut(),
            );
        }
    }
}
