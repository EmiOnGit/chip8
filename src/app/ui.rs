use std::path::PathBuf;

use egui::{ClippedPrimitive, Color32, ComboBox, Context, ScrollArea, Slider, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use pixels::{wgpu, PixelsContext};
use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::window::Window;

use crate::chip8::hardware::Generation;
use crate::chip8::EmulatorEvents;
use crate::display_bus::{AppEvents, DebugState};

use super::debug_map::map_op;
use super::emulator_view::EmulatorView;
use super::{fetch_global_ip, EmulatorKind, HostIp};

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Framework {
    // State for egui.
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,

    // State for the GUI
    pub gui: Gui,
}

impl Framework {
    /// Create egui.
    pub(crate) fn new(
        event_loop: &EventLoop<AppEvents>,
        width: u32,
        height: u32,
        scale_factor: f32,
        emulator_view: &EmulatorView,
    ) -> Self {
        let (max_texture_size, renderer) = emulator_view
            .on_pixels(|pixels| {
                let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
                let renderer =
                    Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);

                (max_texture_size, renderer)
            })
            .unwrap();

        let egui_ctx = Context::default();
        let event_bus = event_loop.create_proxy();
        let mut egui_state = egui_winit::State::new(event_loop);
        egui_state.set_max_texture_side(max_texture_size);
        egui_state.set_pixels_per_point(scale_factor);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let textures = TexturesDelta::default();
        let gui = Gui::new(event_bus);

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
            gui,
        }
    }

    /// Handle input events from the window manager.
    pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        let _ = self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Resize egui.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    /// Update scaling factor.
    pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    /// Prepare egui.
    pub(crate) fn prepare(&mut self, window: &Window) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw the application.
            self.gui.ui(egui_ctx);
        });

        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(window, &self.egui_ctx, output.platform_output);
        self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
    }

    /// Render egui.
    pub(crate) fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {
        // Upload all resources to the GPU.
        for (id, image_delta) in &self.textures.set {
            self.renderer
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        // Render egui with WGPU
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer
                .render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }

        // Cleanup
        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
    }
}
/// Example application state. A real application will need a lot more state than this.
pub struct Gui {
    pub color: Color32,
    /// Only show the egui window when true.
    window_open: bool,
    pub event_bus: EventLoopProxy<AppEvents>,
    pub debugger: Option<Debugger>,
    start_debugger: bool,
    generation: Generation,
    emulator_kind: EmulatorKind,
    file: Option<PathBuf>,
    fps: u32,
}
#[derive(Default, Debug, PartialEq)]
pub struct Debugger {
    pub current: DebugState,
    pub pc_hist: Vec<u16>,
}

impl Gui {
    /// Create a `Gui`.
    fn new(event_bus: EventLoopProxy<AppEvents>) -> Self {
        Self {
            window_open: true,
            color: Color32::LIGHT_GRAY,
            event_bus,
            debugger: None,
            start_debugger: false,
            generation: Generation::default(),
            emulator_kind: EmulatorKind::Single,
            file: None,
            fps: 60,
        }
    }
    pub fn update_debugger(&mut self, state: DebugState) {
        if let Some(debugger) = &mut self.debugger {
            debugger.pc_hist.push(state.pc);
            debugger.current = state;
        } else {
            let op = state.op;
            self.debugger = Some(Debugger {
                current: state,
                pc_hist: vec![op],
            });
        }
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &Context) {
        if let Some(debugger) = &self.debugger {
            debugger.ui(ctx, &self.event_bus);
        }
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("About...").clicked() {
                        self.window_open = true;
                        ui.close_menu();
                    }
                })
            });
        });
        egui::Window::new("Chip8")
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                ComboBox::from_label("Architecture")
                    .selected_text(format!("{:?}", self.generation))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.generation,
                            Generation::Super,
                            format!("{:?}", Generation::Super),
                        );
                        ui.selectable_value(
                            &mut self.generation,
                            Generation::COSMAC,
                            format!("{:?}", Generation::COSMAC),
                        );
                    });
                ComboBox::from_label("Emulator kind")
                    .selected_text(format!("{}", self.emulator_kind))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.emulator_kind,
                            EmulatorKind::Single,
                            "Singleplayer",
                        );
                        ui.selectable_value(
                            &mut self.emulator_kind,
                            EmulatorKind::Server { ip: HostIp::Empty },
                            "Server",
                        );
                        ui.selectable_value(
                            &mut self.emulator_kind,
                            EmulatorKind::Client {
                                host_ip: String::default(),
                            },
                            "Client",
                        );
                    });
                if let EmulatorKind::Client { host_ip } = &mut self.emulator_kind {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(host_ip);
                        ui.label("host ip addr");
                    });
                }
                if let EmulatorKind::Server { ip } = &mut self.emulator_kind {
                    if *ip == HostIp::Empty {
                        match fetch_global_ip() {
                            Some(fetched) => *ip = HostIp::Ip(fetched),
                            None => *ip = HostIp::NotFound,
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.link(format!("{ip:?}")).clicked() {
                            if let HostIp::Ip(ip) = ip {
                                ui.output_mut(|a| {
                                    a.copied_text = ip.clone();
                                    println!("ip: {:?}", a.copied_text);
                                });
                            }
                        }
                        ui.label("host ip addr");
                    });
                }
                if !matches!(self.emulator_kind, EmulatorKind::Client { host_ip: _ }) {
                    let file_name = self
                        .file
                        .as_ref()
                        .map(|file| {
                            file.file_name()
                                .map(|n| n.to_str().unwrap())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    if ui.button(format!("program [{file_name:?}]")).clicked() {
                        self.file = rfd::FileDialog::new().pick_file();
                    }
                }
                if ui.checkbox(&mut self.start_debugger, "debug").clicked() {
                    if self.start_debugger {
                        self.debugger = Some(Debugger::default());
                    } else {
                        self.debugger = None;
                    }
                    self.event_bus
                        .send_event(AppEvents::EmulatorEvent(EmulatorEvents::SetDebug(
                            self.start_debugger,
                        )))
                        .unwrap();
                }

                ui.separator();
                if ui.color_edit_button_srgba(&mut self.color).changed() {
                    self.event_bus
                        .send_event(AppEvents::EmulatorEvent(EmulatorEvents::ChangeColor(
                            self.color,
                        )))
                        .unwrap();
                }
                if ui
                    .add(Slider::new(&mut self.fps, 1..=100).text("fps"))
                    .changed()
                {
                    self.event_bus
                        .send_event(AppEvents::EmulatorEvent(EmulatorEvents::FpsChange(
                            self.fps,
                        )))
                        .unwrap();
                }
                ui.separator();
                if ui.button("Create Emulator").clicked() {
                    self.event_bus
                        .send_event(AppEvents::SpawnEmulator {
                            kind: self.emulator_kind.clone(),
                            generation: self.generation,
                            debugger: self.start_debugger,
                            path: self.file.clone(),
                            fps: self.fps,
                        })
                        .expect("couldn't send `SpawnEmulator` event to main app");
                }
            });
    }
}
impl Debugger {
    fn ui(&self, ctx: &Context, event_bus: &EventLoopProxy<AppEvents>) {
        let state = &self.current;
        egui::Window::new("Debugger").show(ctx, |ui| {
            if ui.button("next").clicked() {
                event_bus
                    .send_event(AppEvents::EmulatorEvent(EmulatorEvents::NextDebugCycle(1)))
                    .unwrap();
            }
            if ui.button("next 5").clicked() {
                event_bus
                    .send_event(AppEvents::EmulatorEvent(EmulatorEvents::NextDebugCycle(5)))
                    .unwrap();
            }
            if ui.button("next 10").clicked() {
                event_bus
                    .send_event(AppEvents::EmulatorEvent(EmulatorEvents::NextDebugCycle(10)))
                    .unwrap();
            }
            if ui.button("next 50").clicked() {
                event_bus
                    .send_event(AppEvents::EmulatorEvent(EmulatorEvents::NextDebugCycle(50)))
                    .unwrap();
            }
            let label = |v, name| format!("{name}: [{v}] ({v:x})");
            ui.label(label(state.pc, "pc"));
            ui.label(format!(
                "{name}: [{op}] ({op:x}) {desc}",
                name = "op",
                op = state.op,
                desc = map_op(state.op)
            ));
            ui.label(label(state.i, "i"));
            ui.separator();
            let label = |v, name| format!("{name}: [{v}] ({v:x})");
            for i in 0..state.reg.len() {
                let name = i.to_string();
                ui.label(label(state.reg[i] as u16, name));
            }
        });
        egui::Window::new("History op").show(ctx, |ui| {
            let label = |v, name| format!("{name}: [{v}] ({v:x})");
            ScrollArea::vertical().max_height(800.).show(ui, |ui| {
                for i in (0..self.pc_hist.len()).rev() {
                    ui.label(label(self.pc_hist[i] as u16, i.to_string()));
                }
            });
        });
    }
}
