use eframe::AppCreator;
use egui::{Event, Image, ImageSource, Sense, TextureId, Vec2};
use egui::load::SizedTexture;
use egui_wgpu::RenderState;
use egui_wgpu::wgpu::{FilterMode, TextureView};
use galileo::{Map, Messenger};
use galileo::control::{EventProcessor, MouseButton, RawUserEvent,
                       UserEventHandler};
use galileo::galileo_types::cartesian::{Point2, Size};
use galileo::layer::attribution::Attribution;
use galileo::render::WgpuRenderer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn init_with_app(app_creator: AppCreator<'_>) -> eframe::Result {
    use std::time::Duration;

    use tokio::runtime::Runtime;

    let rt = Runtime::new().expect("Unable to create Runtime");
    let _enter = rt.enter();

    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        })
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 1000.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native("Galileo Dev Map", native_options, app_creator)
}

pub struct EguiMapState {
    map: Map,
    egui_render_state: RenderState,
    renderer: WgpuRenderer,
    requires_redraw: Arc<AtomicBool>,
    texture_id: TextureId,
    texture_view: TextureView,
    event_processor: EventProcessor,
}

impl<'a> EguiMapState {
    pub fn new(
        mut map: Map,
        ctx: egui::Context,
        render_state: RenderState,
    ) -> Self {
        let requires_redraw = Arc::new(AtomicBool::new(true));
        let messenger = MapStateMessenger {
            context: ctx.clone(),
            requires_redraw: requires_redraw.clone(),
        };

        map.set_messenger(Some(messenger.clone()));
        for layer in map.layers_mut().iter_mut() {
            layer.set_messenger(Box::new(messenger.clone()));
        }

        // Set a default size so that render target can be created.
        // This size will be replaced by the UI on the first frame.
        let size = Size::new(1, 1);
        map.set_size(size.cast());

        let renderer = WgpuRenderer::new_with_device_and_texture(
            render_state.device.clone(),
            render_state.queue.clone(),
            size,
        );
        let texture = renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture,
            FilterMode::Nearest,
        );

        Self {
            map: map,
            egui_render_state: render_state,
            renderer: renderer,
            requires_redraw: requires_redraw,
            texture_id: texture_id,
            texture_view: texture,
            event_processor: EventProcessor::default(),
        }
    }

    pub fn add_handler(&mut self, handler: impl UserEventHandler + 'static) {
        self.event_processor.add_handler(handler);
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        let map_size = self.renderer.size().cast::<f32>();

        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        let attributions = self.collect_attributions();
        if attributions.is_some() {
            egui::Window::new("Attributions")
                .collapsible(false)
                .title_bar(false)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-10., -10.])
                .auto_sized() // Position bottom-right
                .show(ui.ctx(), |ui| {
                    self.show_attributions(ui); // Render the attributions
                    // inside this window
                });
        }

        if self.event_processor.is_dragging() || response.contains_pointer() {
            let events = ui.input(|input_state| input_state.events.clone());
            self.process_events(&events, [-rect.left(), -rect.top()]);
        }

        self.map.animate();

        if available_size[0] != map_size.width() || available_size[1] != map_size.height() {
            self.resize_map(available_size);
        }

        if self.requires_redraw.swap(false, Ordering::Relaxed) {
            self.draw();
        }

        Image::new(ImageSource::Texture(SizedTexture::new(
            self.texture_id,
            Vec2::new(map_size.width(), map_size.height()),
        )))
        .paint_at(ui, rect);
    }

    pub fn collect_attributions(&mut self) -> Option<Vec<Attribution>> {
        let all_layer: Vec<Attribution> = self
            .map
            .layers()
            .iter()
            .filter_map(|layer| layer.attribution())
            .collect();
        if all_layer.is_empty() {
            None
        } else {
            Some(all_layer)
        }
    }

    fn add_attribution_entry(&mut self, ui: &mut egui::Ui, attribution: &Attribution) {
        if let Some(url) = attribution.get_url() {
            ui.hyperlink_to(attribution.get_text(), url);
        } else {
            ui.label(attribution.get_text());
        }
    }

    pub fn show_attributions(&mut self, ui: &mut egui::Ui) {
        let attributions = self
            .collect_attributions()
            .expect("Failed to collect attributions");

        let mut is_first = true;
        for attribution in &attributions {
            self.add_attribution_entry(ui, attribution);
            if !is_first {
                ui.label(" | ");
            }
            is_first = false;
        }
    }

    pub fn map_mut(&'a mut self) -> &'a mut Map {
        &mut self.map
    }

    fn resize_map(&mut self, size: Vec2) {

        let size = Size::new(size.x as f64, size.y as f64);
        self.map.set_size(size);

        let size = Size::new(size.width() as u32, size.height() as u32);
        self.renderer.resize(size);

        // After renderer is resized, a new texture is created, so we need to update its id that we
        // use in UI.
        let texture = self
            .renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let texture_id = self
            .egui_render_state
            .renderer
            .write()
            .register_native_texture(
                &self.egui_render_state.device,
                &texture,
                FilterMode::Nearest,
            );

        self.texture_id = texture_id;
        self.texture_view = texture;

        self.map.redraw();
    }

    fn draw(&mut self) {
        self.map.load_layers();
        self.renderer
            .render_to_texture_view(&self.map, &self.texture_view);
    }

    fn process_events(&mut self, events: &[Event], offset: [f32; 2]) {
        for event in events {
            if let Some(raw_event) = Self::convert_event(event, offset) {
                self.event_processor.handle(raw_event, &mut self.map);
            }
        }
    }

    fn convert_event(event: &Event, offset: [f32; 2]) -> Option<RawUserEvent> {
        match event {
            Event::PointerButton {
                button, pressed, ..
            } => {
                let button = match button {
                    egui::PointerButton::Primary => MouseButton::Left,
                    egui::PointerButton::Secondary => MouseButton::Right,
                    egui::PointerButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Other,
                };

                Some(match pressed {
                    true => RawUserEvent::ButtonPressed(button),
                    false => RawUserEvent::ButtonReleased(button),
                })
            }
            Event::PointerMoved(position) => {
                let scale = 1.0;
                let pointer_position = Point2::new(
                    (position.x + offset[0]) as f64 / scale,
                    (position.y + offset[1]) as f64 / scale,
                );
                Some(RawUserEvent::PointerMoved(pointer_position))
            }
            Event::MouseWheel { delta, .. } => {
                let zoom = delta[1] as f64;
                if zoom.abs() < 0.0001 {
                    return None;
                }

                Some(RawUserEvent::Scroll(zoom))
            }

            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapStateMessenger {
    pub requires_redraw: Arc<AtomicBool>,
    pub context: egui::Context,
}

impl Messenger for MapStateMessenger {
    fn request_redraw(&self) {
        if !self.requires_redraw.swap(true, Ordering::Relaxed) {
            self.context.request_repaint();
        }
    }
}
