use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use eframe::{EframeWinitApplication, UserEvent, egui};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
};

use shared::{log, system::trigger::Trigger};

mod geom;

pub mod consts;
pub mod input;
pub mod window;

pub mod about;

pub mod logo;

/// Proxy over the eframe proxy to capture keyboard input events
/// so we can get scancodes to send to RDP
pub struct RdpAppProxy<'a> {
    eframe_app: EframeWinitApplication<'a>,
    events: Sender<input::RawKey>,
    processing_events: Arc<AtomicBool>,
    stop: Trigger,
}

// Implement ApplicationHandler to intercept events
impl ApplicationHandler<UserEvent> for RdpAppProxy<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.eframe_app.resumed(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        // If a window event, try to push keyboard events to the channel
        // If close event, trigger stop but allow eframe to handle closing
        if let winit::event::WindowEvent::CloseRequested = &event {
            self.stop.set();
        }
        
        if self.processing_events.load(Ordering::Relaxed)
            && let winit::event::WindowEvent::KeyboardInput { event, .. } = &event
            && let PhysicalKey::Code(code) = event.physical_key
        {
            let raw_key = input::RawKey {
                keycode: code,
                pressed: event.state.is_pressed(),
                repeat: event.repeat,
            };
            if let Err(e) = self.events.send(raw_key) {
                log::error!("Failed to send keyboard event: {}", e);
            }
        }
        // We can process Unidentified::NativeKeyCode if needed
        // log::debug!(
        //     "Keyboard event: {:?}, pressed: {}",
        //     event.physical_key,
        //     event.state == winit::event::ElementState::Pressed,
        // );
        self.eframe_app.window_event(event_loop, window_id, event);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        self.eframe_app.new_events(event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        self.eframe_app.user_event(event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.eframe_app.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.eframe_app.about_to_wait(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.eframe_app.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.eframe_app.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.eframe_app.memory_warning(event_loop);
    }
}

/// Run the GUI application
pub fn run_gui(
    catalog: gettext::Catalog,
    initial_state: Option<window::types::AppState>,
    messages_rx: Receiver<window::types::GuiMessage>,
    stop: Trigger,
) -> Result<()> {
    let (keys_tx, keys_rx): (Sender<input::RawKey>, Receiver<input::RawKey>) = bounded(1024);
    let processing_events = Arc::new(AtomicBool::new(false));

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_app_id("UDSLauncher")
            .with_icon(logo::load_icon())
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let winit_app = {
        let processing_events = processing_events.clone();
        let stop = stop.clone();
        eframe::create_native(
            "UDS Launcher",
            native_options,
            Box::new(|cc| {
                Ok(Box::new(window::AppWindow::new(
                    processing_events,
                    keys_rx,
                    messages_rx,
                    stop,
                    catalog,
                    initial_state,
                    cc,
                )))
            }),
            &event_loop,
        )
    };
    let mut eframe_app_proxy = RdpAppProxy {
        events: keys_tx,
        eframe_app: winit_app,
        processing_events,
        stop,
    };

    event_loop.run_app(&mut eframe_app_proxy)?;

    Ok(())
}
