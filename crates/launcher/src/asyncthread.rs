use crossbeam::channel::Sender;
use shared::system::trigger::Trigger;

use gui::window::types::GuiMessage;
use shared::log;

use crate::runner;

pub fn run(tx: Sender<GuiMessage>, stop: Trigger, host: String, ticket: String, scrambler: String) {
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
                    tx.send(GuiMessage::ShowProgress).ok();
                    if let Err(e) =
                        runner::run(tx.clone(), stop.clone(), &host, &ticket, &scrambler).await
                    {
                        log::error!("{}", e);
                        tx.send(GuiMessage::ShowError(e.to_string())).ok();
                    } else {
                        tx.send(GuiMessage::Close).ok();
                    }
                    stop.set();
                }
            });
        }
    });
}
