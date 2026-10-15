use std::path::Path;

use crate::gpu::{GfxError, Gpu, OFFSCREEN_FORMAT};
use crate::renderer::Renderer;
use crate::scene::FrameInput;

/// Straight RGBA8 pixels read back from an offscreen render.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    pub fn write_png(&self, path: &Path) -> Result<(), GfxError> {
        let file = std::fs::File::create(path).map_err(|e| GfxError::Surface(e.to_string()))?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| GfxError::Surface(e.to_string()))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|e| GfxError::Surface(e.to_string()))
    }
}

/// Renders frames to a texture and reads them back; works on any OS with a wgpu adapter.
pub struct OffscreenRenderer {
    pub gpu: Gpu,
    pub renderer: Renderer,
}

impl OffscreenRenderer {
    pub fn new() -> Result<Self, GfxError> {
        let gpu = Gpu::headless()?;
        let renderer = Renderer::new(&gpu.device, &gpu.queue, OFFSCREEN_FORMAT);
        Ok(Self { gpu, renderer })
    }

    pub fn render(&mut self, input: &FrameInput<'_>) -> Result<Image, GfxError> {
        let (width, height) = input.size;
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gfx-offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.renderer
            .render(&texture.create_view(&Default::default()), input);
        let padded = (width * 4).div_ceil(256) * 256;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gfx-readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.gpu
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GfxError::Device(e.to_string()))?;
        rx.recv()
            .map_err(|e| GfxError::Device(e.to_string()))?
            .map_err(|e| GfxError::Device(e.to_string()))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| GfxError::Device(e.to_string()))?;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let start = row * padded as usize;
            rgba.extend_from_slice(&data[start..start + (width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        Ok(Image {
            width,
            height,
            rgba,
        })
    }
}
