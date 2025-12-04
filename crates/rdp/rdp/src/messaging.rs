use crossbeam::channel;

use crate::geom::Rect;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RdpMessage {
    UpdateRects(Vec<Rect>),
    Disconnect,
    FocusRequired,
    Error(String),
}

pub type Sender = channel::Sender<RdpMessage>;
