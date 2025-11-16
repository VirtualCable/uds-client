use shared::system::trigger::Trigger;
use std::sync::mpsc::Sender;

use crate::{gui, runner};
use shared::log;

pub fn run(
    tx: Sender<gui::progress::GuiMessage>,
    stop: Trigger,
    host: String,
    ticket: String,
    scrambler: String,
) {
    std::thread::spawn({
        let stop = stop.clone();
        move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // Blocking call to async code
            rt.block_on({
                let stop = stop.clone();
                async move {
                    if let Err(e) =
                        runner::run(tx.clone(), stop.clone(), &host, &ticket, &scrambler).await
                    {
                        log::error!("{}", e);
                        tx.send(gui::progress::GuiMessage::Error(e.to_string()))
                            .ok();
                    } else {
                        tx.send(gui::progress::GuiMessage::Close).ok();
                    }
                    stop.set();
                }
            });
        }
    });
}
