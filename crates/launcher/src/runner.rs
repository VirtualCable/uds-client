
use std::sync::mpsc;
use crate::gui::progress::{GuiMessage};

use shared::{broker::api};

pub async fn run(tx: mpsc::Sender<GuiMessage>, stop: shared::system::trigger::Trigger) {

    stop.set();
}