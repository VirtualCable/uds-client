#![allow(dead_code)]
use std::{
    fmt,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use eframe::egui;

use crate::log;

use rdp::{
    connection::{Rdp, RdpMessage},
    settings::RdpSettings,
};

use crate::geom::RectExt; // For extracting rects from framebuffer

use super::{
    AppWindow,
    types::{AppState, HotKey},
};

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
pub struct RdpState {
    update_rx: crossbeam::channel::Receiver<RdpMessage>,
    gdi: *mut freerdp_sys::rdpGdi,
    gdi_lock: Arc<RwLock<()>>,
    input: *mut freerdp_sys::rdpInput,
    texture: egui::TextureHandle,
    full_screen: Arc<AtomicBool>,
}

impl fmt::Debug for RdpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdpState")
            .field("gdi", &self.gdi)
            .field("input", &self.input)
            .finish()
    }
}

impl AppWindow {
    pub fn enter_rdp_connected(
        &mut self,
        ctx: &eframe::egui::Context,
        rdp_settings: RdpSettings,
    ) -> Result<()> {
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events
        let (tx, rx): (Sender<RdpMessage>, Receiver<RdpMessage>) = bounded(FRAMES_IN_FLIGHT);

        let screen_size = rdp_settings.screen_size.clone();
        // TODO: We will need a reasonable for returning back from fullscreen later
        // Note that with this this should work correctly, as rdp will receibe a 1920x1080 framebuffer
        // and if different, on update, we will resize gdi, texture, etc. accordingly
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
            [screen_size.width() as f32, screen_size.height() as f32].into(),
        ));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([10.0, 10.0].into()));

        if screen_size.is_fullscreen() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }

        // Rdp shouls be pinned, as build() inserts self reference inside freedrp structs
        let mut rdp = Box::pin(Rdp::new(rdp_settings, tx, self.stop.clone()));
        // For reference: Currently, default callbacks are these also, so if no more are needed, this can be skipped
        // rdp.set_update_callbacks(vec![
        //     update_c::Callbacks::BeginPaint,
        //     update_c::Callbacks::EndPaint,
        //     update_c::Callbacks::DesktopResize,
        // ]);
        rdp.as_mut().build()?; // Build inserts "rdp" inside an struct for freedrp, must ensure that rdp does not move after this point

        log::debug!("** Rdp address: {:p}", &rdp);

        rdp.optimize();
        // TODO: We need to switch to fullscreeen before opening the connection if needed
        rdp.connect()?;

        #[cfg(debug_assertions)]
        {
            rdp.debug_assert_instance();
        }

        let rdpversion_str = rdp.get_rdp_version()?;

        log::debug!("Connected. RDP Version: {}", rdpversion_str);

        // Ge the gdi pointer
        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let input = rdp
            .input()
            .ok_or_else(|| anyhow::anyhow!("Input not initialized"))?;
        // And the lock
        let gdi_lock = rdp.gdi_lock();

        log::debug!("Obtained GDI pointer: {:?}", gdi);
        log::debug!("Gdi: {:?}", unsafe { *gdi });

        // Stride in bytes
        let pitch = unsafe { (*gdi).stride } as usize;
        let height = unsafe { (*gdi).height } as usize;
        let buffer = unsafe {
            std::slice::from_raw_parts((*gdi).primary_buffer as *const u8, pitch * height)
        };
        let image = egui::ColorImage::from_rgba_unmultiplied([pitch / 4, height], buffer);
        let texture = ctx.load_texture("rdp_framebuffer", image, egui::TextureOptions::NEAREST);

        self.set_app_state(AppState::RdpConnected(RdpState {
            update_rx: rx,
            gdi,
            input,
            gdi_lock,
            texture,
            full_screen: Arc::new(AtomicBool::new(screen_size.is_fullscreen())),
        }));

        std::thread::spawn(move || {
            let res = rdp.run();
            log::debug!("RDP thread exiting...");
            if let Err(e) = res {
                log::debug!("RDP thread ended with error: {}", e);
            } else {
                log::debug!("RDP thread ended.");
            }
        });
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events

        Ok(())
    }

    pub(super) fn update_rdp_client(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        rdp_state: &mut RdpState,
    ) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show(ctx, |ui| {
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    match message {
                        RdpMessage::UpdateRects(rects) => {
                            let _guard = rdp_state.gdi_lock.write().unwrap();
                            for rect in rects {
                                let img = rect.extract(
                                    unsafe {
                                        std::slice::from_raw_parts(
                                            (*rdp_state.gdi).primary_buffer as *const u8,
                                            ((*rdp_state.gdi).stride as usize)
                                                * (rdp_state.gdi.as_ref().unwrap().height as usize),
                                        )
                                    },
                                    unsafe { (*rdp_state.gdi).stride as usize },
                                    unsafe { (*rdp_state.gdi).width as usize },
                                    unsafe { (*rdp_state.gdi).height as usize },
                                );
                                if let Some(image) = img {
                                    rdp_state.texture.set_partial(
                                        [rect.x as usize, rect.y as usize],
                                        image,
                                        egui::TextureOptions::NEAREST,
                                    );
                                }
                            }
                        }
                        RdpMessage::Resize(width, height) => {
                            // TODO: Handle resize
                            log::debug!("Received resize to {}x{}", width, height);
                        }
                        RdpMessage::Disconnect => {
                            log::debug!("RDP Disconnected");
                            // TODO: Handle disconnection properly
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::debug!("RDP Error: {}", err);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            break;
                        }
                        RdpMessage::FocusRequired => {
                            log::debug!("RDP Focus Required");
                        }
                    }
                }
                // Show the texture on 0,0, full size
                ui.image(&rdp_state.texture);

                if self.handle_hotkeys(ctx, frame, rdp_state) {
                    // Hotkey handled, skip input processing this frame
                    return;
                }
                let input = rdp_state.input;
                self.handle_input(ctx, frame, input);
            });
    }

    fn handle_hotkeys(
        &mut self,
        ctx: &egui::Context,
        _frame: &eframe::Frame,
        rdp_state: &mut RdpState,
    ) -> bool {
        match HotKey::from_input(ctx) {
            HotKey::ToggleFullScreen => {
                self.toggle_fullscreen(ctx, _frame, rdp_state);
                true
            }
            HotKey::None => false,
        }
    }

    fn toggle_fullscreen(
        &mut self,
        ctx: &egui::Context,
        _frame: &eframe::Frame,
        rdp_state: &mut RdpState,
    ) {
        log::debug!("ALT+ENTER pressed, toggling fullscreen");
        if rdp_state.full_screen.load(Ordering::Relaxed) {
            // Switch to fixed size, restores original size
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            rdp_state.full_screen.store(false, Ordering::Relaxed);
        } else {
            // Switch to fullscreen
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            rdp_state.full_screen.store(true, Ordering::Relaxed);
        }
    }
}
