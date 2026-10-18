//! Shared offscreen egui/wgpu capture. Pixels retain the bottom-up convention of
//! the regression fixtures; only this readback boundary flips WebGPU's rows.
use super::{COLOR, Gpu, egui_wgpu, wgpu};
use eframe::egui;
use std::sync::Arc;

pub(super) struct Headless {
    pub state: egui_wgpu::RenderState,
}
impl Headless {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: std::env::var_os("CLAY_GPU_SOFTWARE").is_some(),
            ..Default::default()
        }))
        .expect("a Vulkan adapter (e.g. Mesa lavapipe) is required");
        eprintln!("wgpu adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("create wgpu device");
        let renderer = Arc::new(egui::mutex::RwLock::new(egui_wgpu::Renderer::new(
            &device,
            COLOR,
            egui_wgpu::RendererOptions {
                dithering: false,
                ..Default::default()
            },
        )));
        Self {
            state: egui_wgpu::RenderState {
                adapter,
                available_adapters: vec![],
                device,
                queue,
                target_format: COLOR,
                renderer,
            },
        }
    }
    pub fn wait(&self) {
        self.state
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}

pub(crate) struct UiCapture {
    pub state: egui_wgpu::RenderState,
    size: [u32; 2],
    texture: wgpu::Texture,
}
impl UiCapture {
    pub fn new(size: [i32; 2]) -> Self {
        Self::with_format(size, COLOR)
    }
    pub(super) fn with_format(size: [i32; 2], format: wgpu::TextureFormat) -> Self {
        let mut state = Headless::new().state;
        state.target_format = format;
        state.renderer = Arc::new(egui::mutex::RwLock::new(egui_wgpu::Renderer::new(
            &state.device,
            format,
            egui_wgpu::RendererOptions {
                dithering: false,
                ..Default::default()
            },
        )));
        let size = size.map(|n| n as u32);
        let texture = Gpu::texture(
            &state.device,
            size,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        Self {
            state,
            size,
            texture,
        }
    }
    pub fn render(&mut self, ctx: &egui::Context, output: egui::FullOutput) -> Vec<u8> {
        let mut renderer = self.state.renderer.write();
        let device = &self.state.device;
        let queue = &self.state.queue;
        for (id, delta) in &output.textures_delta.set {
            renderer.update_texture(device, queue, *id, delta);
        }
        let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.size,
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        let commands = renderer.update_buffers(device, queue, &mut encoder, &shapes, &screen);
        {
            let view = self.texture.create_view(&Default::default());
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI capture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.07,
                            b: 0.07,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &shapes, &screen);
        }
        queue.submit(commands.into_iter().chain([encoder.finish()]));
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }
        read_texture(device, queue, &self.texture)
    }
    pub fn save(&self, path: impl AsRef<std::path::Path>, rgba: &[u8]) {
        let flipped: Vec<_> = rgba
            .chunks_exact(self.size[0] as usize * 4)
            .rev()
            .flatten()
            .copied()
            .collect();
        image::save_buffer(
            path,
            &flipped,
            self.size[0],
            self.size[1],
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
/// Copy with WebGPU's 256-byte row alignment, wait for mapping, strip padding,
/// and expose the fixture coordinate convention. No window/surface is needed.
pub(super) fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Vec<u8> {
    let width = texture.width();
    let height = texture.height();
    let pitch = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: (pitch * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (send, recv) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    recv.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let pixels = mapped
        .chunks_exact(pitch as usize)
        .rev()
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    pixels
}
impl Gpu {
    pub(super) fn read(&self) -> Vec<u8> {
        read_texture(
            &self.device,
            &self.queue,
            &self.target.as_ref().unwrap().color,
        )
    }
    pub(super) fn sample(&self, x: usize, y: usize) -> [u8; 4] {
        let pixels = self.read();
        let i = (y * self.target.as_ref().unwrap().size[0] as usize + x) * 4;
        pixels[i..i + 4].try_into().unwrap()
    }
}
