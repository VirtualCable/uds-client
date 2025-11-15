
use std::sync::mpsc;
use crate::gui::{GuiMessage};

pub async fn run(tx: mpsc::Sender<GuiMessage>, stop: shared::system::trigger::Trigger) {
    for i in 0..=100 {
        tx.send(GuiMessage::Progress(i as f32 / 100.0))
            .ok();
        if stop.async_wait_timeout(std::time::Duration::from_millis(20)).await {
            break;  // Exit if triggered
        }
    }
    tx.send(GuiMessage::Error("Simulated error\nlets see how it looks\nhttps://www.udsenterprise.com\nwith several lines\nand more".to_string()))
         .ok();
    stop.set();
}