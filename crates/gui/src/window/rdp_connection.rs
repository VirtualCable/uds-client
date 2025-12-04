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
    Rdp, messaging::RdpMessage,
    settings::RdpSettings,
    sys::{rdpGdi, rdpInput},
};

use crate::geom::RectExt; // For extracting rects from framebuffer

use super::{
    AppWindow,
    types::{AppState, HotKey},
};

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
pub struct RdpConnectionState {
    update_rx: crossbeam::channel::Receiver<RdpMessage>,
    gdi: *mut rdpGdi,
    gdi_lock: Arc<RwLock<()>>,
    input: *mut rdpInput,
    texture: egui::TextureHandle,
    full_screen: Arc<AtomicBool>,
}

impl fmt::Debug for RdpConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdpState")
            .field("gdi", &self.gdi)
            .field("input", &self.input)
            .finish()
    }
}

impl AppWindow {
    pub fn enter_rdp_connection(
        &mut self,
        ctx: &eframe::egui::Context,
        rdp_settings: RdpSettings,
    ) -> Result<()> {
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events
        let (tx, rx): (Sender<RdpMessage>, Receiver<RdpMessage>) = bounded(FRAMES_IN_FLIGHT);

        let mut rdp_settings = rdp_settings;
        // TODO: Handle screen size changes during session with RDP display channel
        let is_full_screen = if rdp_settings.screen_size.is_fullscreen() {
            let real_size = ctx.content_rect().size();
            rdp_settings.screen_size =
                rdp::geom::ScreenSize::Fixed(real_size.x as u32, real_size.y as u32);
            true
        } else {
            false
        };

        // Rdp shouls be pinned, as build() inserts self reference inside freedrp structs
        let mut rdp = Box::pin(Rdp::new(rdp_settings, tx));
        
        // For reference: Currently, default callbacks are these also, so if no more are needed, this can be skipped
        // rdp.set_update_callbacks(vec![
        //     update_c::Callbacks::BeginPaint,
        //     update_c::Callbacks::EndPaint,
        //     update_c::Callbacks::DesktopResize,
        // ]);
        rdp.as_mut().build()?; // Build inserts "rdp" inside an struct for freedrp, must ensure that rdp does not move after this point

        log::debug!("** Rdp address: {:p}", &rdp);

        rdp.optimize();
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
        let texture = ctx.load_texture("rdp_framebuffer", image, egui::TextureOptions::LINEAR);

        self.set_app_state(AppState::RdpConnected(RdpConnectionState {
            update_rx: rx,
            gdi,
            input,
            gdi_lock,
            texture,
            full_screen: Arc::new(AtomicBool::new(is_full_screen)),
        }));

        std::thread::spawn(move || {
            // Note: This may already be marked as launched from external RDP launcher
            // But ensure it is marked here as well (to allow using from other gui launchers as test app)
            shared::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            shared::tasks::mark_internal_rdp_as_not_running();
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

    pub(super) fn update_rdp_connection(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        rdp_state: &mut RdpConnectionState,
    ) {
        // Calculate relation between gdi size and egui content size
        let scale = {
            let egui_size = ctx.content_rect().size();
            let gdi_width = unsafe { (*rdp_state.gdi).width as f32 };
            let gdi_height = unsafe { (*rdp_state.gdi).height as f32 };
            egui::Vec2::new(gdi_width / egui_size.x, gdi_height / egui_size.y)
        };

        if self.handle_hotkeys(ctx, frame, rdp_state) {
            // Hotkey handled, skip input processing this frame
            return;
        }
        let input = rdp_state.input;
        self.handle_input(ctx, frame, input, scale);

        // TODO: Allow this, but need to implement Display channel at least to send resize
        // let egui::Vec2 {
        //     x: actual_width,
        //     y: actual_height,
        // } = ctx.content_rect().size();
        // let (actual_width, actual_height) = (actual_width as i32, actual_height as i32);
        // let (gdi_width, gdi_height) = unsafe { ((*rdp_state.gdi).width, (*rdp_state.gdi).height) };

        // if actual_width != gdi_width || actual_height != gdi_height {
        //     log::debug!(
        //         "Viewport size changed: actual=({}, {}), gdi=({}, {}), resizing gdi and texture",
        //         actual_width,
        //         actual_height,
        //         gdi_width,
        //         gdi_height
        //     );
        // }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show(ctx, |ui| {
                // If the size of gdi is not equal to size of content, resize gdi and recreate texture
                let start = std::time::Instant::now();
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    log::trace!("Got message {:?}", message);
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
                                        egui::TextureOptions::LINEAR,
                                    );
                                }
                            }
                        }
                        RdpMessage::Disconnect => {
                            log::debug!("RDP Disconnected");
                            self.exit(ctx);
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::error!("RDP Error: {}", err);
                            self.exit(ctx);
                            break;
                        }
                        RdpMessage::FocusRequired => {
                            log::debug!("RDP Focus Required");
                        }
                    }
                }
                log::trace!("RDP update processing took {:?}", start.elapsed());
                // Show the texture on 0,0, full size
                let size = ui.available_size();
                ui.add_sized(
                    size,
                    egui::Image::new(&rdp_state.texture)
                        .maintain_aspect_ratio(false)
                        .fit_to_exact_size(size),
                );
                log::trace!("RDP frame rendered took {:?}", start.elapsed());
            });
    }

    fn handle_hotkeys(
        &mut self,
        ctx: &egui::Context,
        _frame: &eframe::Frame,
        rdp_state: &mut RdpConnectionState,
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
        rdp_state: &mut RdpConnectionState,
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
