// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
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

pub mod window;

pub mod about;

pub mod logo;

#[derive(Debug)]
pub struct RawKey {
    pub keycode: winit::keyboard::KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

/// Proxy over the eframe proxy to capture keyboard input events
/// so we can get scancodes to send to RDP
pub struct RdpAppProxy<'a> {
    eframe_app: EframeWinitApplication<'a>,
    events: Sender<RawKey>,
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
            let raw_key = RawKey {
                keycode: code,
                pressed: event.state.is_pressed(),
                repeat: event.repeat,
            };
            if let Err(e) = self.events.send(raw_key) {
                // Chanel may be full or disconnected, log and continue
                log::warn!("Failed to send keyboard event: {}", e);
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
    let (keys_tx, keys_rx) = bounded::<RawKey>(1024);
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
