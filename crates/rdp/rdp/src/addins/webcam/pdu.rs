use anyhow::Result;
use freerdp_sys::{BYTE, CAM_MEDIA_TYPE_DESCRIPTION};

/// PDU header on the wire: Version(1) + MessageId(1) = 2 bytes
pub const PDU_HEADER_SIZE: usize = 2;

/// Parse a PDU header from raw bytes.
/// Returns (version, msg_id, remaining_payload).
pub fn parse_pdu_header(data: &[u8]) -> Result<(u8, u8, &[u8])> {
    if data.len() < PDU_HEADER_SIZE {
        anyhow::bail!("PDU too short: {} < {PDU_HEADER_SIZE}", data.len());
    }
    let version = data[0];
    let msg_id = data[1];
    Ok((version, msg_id, &data[PDU_HEADER_SIZE..]))
}

/// Parse a CAM_MEDIA_TYPE_DESCRIPTION (26 bytes) from raw bytes.
pub fn parse_media_type(data: &[u8]) -> Result<CAM_MEDIA_TYPE_DESCRIPTION> {
    anyhow::ensure!(
        data.len() >= 26,
        "Too short for media type: {} < 26",
        data.len()
    );

    let format = data[0] as i32;
    let width = u32::from_le_bytes(data[1..5].try_into()?);
    let height = u32::from_le_bytes(data[5..9].try_into()?);
    let fps_num = u32::from_le_bytes(data[9..13].try_into()?);
    let fps_den = u32::from_le_bytes(data[13..17].try_into()?);
    let aspect_num = u32::from_le_bytes(data[17..21].try_into()?);
    let aspect_den = u32::from_le_bytes(data[21..25].try_into()?);
    let flags = data[25] as i32;

    Ok(CAM_MEDIA_TYPE_DESCRIPTION {
        Format: format,
        Width: width,
        Height: height,
        FrameRateNumerator: fps_num,
        FrameRateDenominator: fps_den,
        PixelAspectRatioNumerator: aspect_num,
        PixelAspectRatioDenominator: aspect_den,
        Flags: flags,
    })
}

/// Serialize a CAM_MEDIA_TYPE_DESCRIPTION into 26 packed bytes.
pub fn serialize_media_type(mt: &CAM_MEDIA_TYPE_DESCRIPTION) -> Vec<u8> {
    let mut buf = Vec::with_capacity(26);
    buf.push(mt.Format as u8);
    buf.extend_from_slice(&mt.Width.to_le_bytes());
    buf.extend_from_slice(&mt.Height.to_le_bytes());
    buf.extend_from_slice(&mt.FrameRateNumerator.to_le_bytes());
    buf.extend_from_slice(&mt.FrameRateDenominator.to_le_bytes());
    buf.extend_from_slice(&mt.PixelAspectRatioNumerator.to_le_bytes());
    buf.extend_from_slice(&mt.PixelAspectRatioDenominator.to_le_bytes());
    buf.push(mt.Flags as u8);
    buf
}

/// Build a PDU with just a header (version + msg_id), no body.
pub fn build_response_header(msg_id: u8) -> Vec<u8> {
    let mut pdu = Vec::with_capacity(PDU_HEADER_SIZE);
    pdu.push(1u8); // version
    pdu.push(msg_id);
    pdu
}

pub fn build_sample_response(stream_index: u8, sample: &[u8]) -> Vec<u8> {
    let sample_size = sample.len();
    let mut pdu = Vec::with_capacity(PDU_HEADER_SIZE + 1 + sample_size);
    pdu.push(1u8); // version
    pdu.push(0x12); // CAM_MSG_ID_SampleResponse = 0x12
    pdu.push(stream_index);
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
