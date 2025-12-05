use crossbeam::channel;

use crate::geom::Rect;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RdpMessage {
    UpdateRects(Vec<Rect>),
    Disconnect,
    FocusRequired,
    Error(String),
    SetCursorIcon(Vec<u8>, u32, u32, u32, u32),  // x, y, (of pointer "pointer") width, height
}

pub type Sender = channel::Sender<RdpMessage>;
