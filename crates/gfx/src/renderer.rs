use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bytemuck::{bytes_of, cast_slice};

use crate::scene::{
    DECKS, FrameInput, PhraseView, PlacedSurface, QuadInst, QuadList, QuadUniform, SurfaceCtx,
    SurfaceId, SurfaceKind, WaveUniform, ghost_for, overview_quads, scroll_quads, wave_uniform,
};
use crate::style::Style;
use crate::text;
use crate::waveform::{Pyramid, WaveformData};

const INITIAL_QUADS: usize = 1024;

struct SurfaceRes {
    wave_buf: wgpu::Buffer,
    wave_bind: wgpu::BindGroup,
    quad_buf: wgpu::Buffer,
    quad_bind: wgpu::BindGroup,
}

struct DeckRes {
    token: Option<u64>,
    pyramid: Option<Pyramid>,
    bind: Option<wgpu::BindGroup>,
    phrase_token: Option<u64>,
    phrases: Vec<PhraseView>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    pub surfaces: usize,
    pub quads: usize,
}

/// Draws waveform surfaces into any colour target. Holds per-track GPU data for both decks.
struct TimerSlot {
    buffer: wgpu::Buffer,
    busy: Arc<AtomicBool>,
}

struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<TimerSlot>,
    next: usize,
    period_ns: f32,
    last_ns: Arc<AtomicU64>,
}

pub struct Renderer {
    gpu_timer: Option<GpuTimer>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    wave_pipeline: wgpu::RenderPipeline,
    quad_pipeline: wgpu::RenderPipeline,
    atlas_bind: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    surfaces: Vec<SurfaceRes>,
    decks: [DeckRes; DECKS],
    empty_bind: wgpu::BindGroup,
    quad_vbuf: wgpu::Buffer,
    quad_capacity: usize,
    scratch: QuadList,
    style: Style,
}

fn uniform_layout(
    device: &wgpu::Device,
    visibility: wgpu::ShaderStages,
    size: usize,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gfx-uniform-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size as u64),
            },
            count: None,
        }],
    })
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let wave_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gfx-wave"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/wave.wgsl").into()),
        });
        let quad_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gfx-quads"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/quads.wgsl").into()),
        });
        let wave_layout = uniform_layout(
            device,
            wgpu::ShaderStages::FRAGMENT,
            size_of::<WaveUniform>(),
        );
        let quad_layout =
            uniform_layout(device, wgpu::ShaderStages::VERTEX, size_of::<QuadUniform>());
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gfx-wave-texture-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let wave_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gfx-wave-pipeline-layout"),
            bind_group_layouts: &[Some(&wave_layout), Some(&tex_layout)],
            immediate_size: 0,
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gfx-atlas-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let quad_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gfx-quad-pipeline-layout"),
            bind_group_layouts: &[Some(&quad_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let target = |blend| wgpu::ColorTargetState {
            format,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        };
        let wave_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gfx-wave"),
            layout: Some(&wave_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &wave_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &wave_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(target(None))],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instance_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gfx-quads"),
            layout: Some(&quad_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &quad_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<QuadInst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attrs,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &quad_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(target(Some(wgpu::BlendState::ALPHA_BLENDING)))],
            }),
            multiview_mask: None,
            cache: None,
        });

        let make_buf = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let surfaces = SurfaceId::ALL
            .iter()
            .map(|_| {
                let wave_buf = make_buf("gfx-wave-uniform", size_of::<WaveUniform>());
                let quad_buf = make_buf("gfx-quad-uniform", size_of::<QuadUniform>());
                let bind = |layout: &wgpu::BindGroupLayout, buf: &wgpu::Buffer| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buf.as_entire_binding(),
                        }],
                    })
                };
                SurfaceRes {
                    wave_bind: bind(&wave_layout, &wave_buf),
                    quad_bind: bind(&quad_layout, &quad_buf),
                    wave_buf,
                    quad_buf,
                }
            })
            .collect();

        let empty_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gfx-empty-wave"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let empty_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gfx-empty-wave-bind"),
            layout: &tex_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &empty_tex.create_view(&Default::default()),
                ),
            }],
        });
        let atlas_bind = Self::atlas(device, queue, &atlas_layout);
        let quad_vbuf = Self::quad_buffer(device, INITIAL_QUADS);
        let deck = || DeckRes {
            token: None,
            pyramid: None,
            bind: None,
            phrase_token: None,
            phrases: Vec::new(),
        };
        Self {
            gpu_timer: None,
            device: device.clone(),
            queue: queue.clone(),
            wave_pipeline,
            quad_pipeline,
            atlas_bind,
            tex_layout,
            surfaces,
            decks: [deck(), deck()],
            empty_bind,
            quad_vbuf,
            quad_capacity: INITIAL_QUADS,
            scratch: QuadList::default(),
            style: Style::default(),
        }
    }

    fn atlas(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        let (width, height, pixels) = text::atlas_pixels().unwrap_or((1, 1, vec![0]));
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gfx-glyph-atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            size,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gfx-glyph-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gfx-glyph-atlas-bind"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }

    fn quad_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gfx-quad-instances"),
            size: (capacity * size_of::<QuadInst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Starts measuring GPU pass time when the device supports timestamp queries; returns whether it does.
    pub fn enable_gpu_timing(&mut self) -> bool {
        if !self
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return false;
        }
        let queries = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gfx-timer"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let buffer = |label, usage| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage,
                mapped_at_creation: false,
            })
        };
        let slots = (0..3)
            .map(|_| TimerSlot {
                buffer: buffer(
                    "gfx-timer-read",
                    wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                ),
                busy: Arc::new(AtomicBool::new(false)),
            })
            .collect();
        self.gpu_timer = Some(GpuTimer {
            queries,
            resolve: buffer(
                "gfx-timer-resolve",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            slots,
            next: 0,
            period_ns: self.queue.get_timestamp_period(),
            last_ns: Arc::new(AtomicU64::new(0)),
        });
        true
    }

    /// Most recent completed GPU pass time in milliseconds.
    pub fn last_gpu_ms(&self) -> Option<f64> {
        let t = self.gpu_timer.as_ref()?;
        let ns = f64::from_bits(t.last_ns.load(Ordering::Relaxed));
        (ns > 0.0).then_some(ns / 1e6)
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    pub fn deck_token(&self, deck: usize) -> Option<u64> {
        self.decks.get(deck).and_then(|d| d.token)
    }

    pub fn phrase_token(&self, deck: usize) -> Option<u64> {
        self.decks.get(deck).and_then(|d| d.phrase_token)
    }

    /// Uploads a track's waveform pyramid to the GPU; `None` clears the deck.
    pub fn set_waveform(
        &mut self,
        deck: usize,
        token: u64,
        waveform: Option<&WaveformData>,
    ) -> bool {
        let Some(slot) = self.decks.get_mut(deck) else {
            return false;
        };
        slot.token = Some(token);
        slot.pyramid = None;
        slot.bind = None;
        let Some(wf) = waveform else { return true };
        self.set_pyramid(deck, token, Pyramid::build(wf))
    }

    /// Uploads an already-built pyramid (built off the render thread).
    pub fn set_pyramid(&mut self, deck: usize, token: u64, pyramid: Pyramid) -> bool {
        let limit = self.device.limits().max_texture_dimension_2d;
        if pyramid.rows == 0 || pyramid.rows > limit || pyramid.width > limit {
            log::warn!(
                "waveform too large for GPU texture ({} rows, limit {limit})",
                pyramid.rows
            );
            return false;
        }
        let size = wgpu::Extent3d {
            width: pyramid.width,
            height: pyramid.rows,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gfx-wave"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            cast_slice(&pyramid.texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pyramid.width * 4),
                rows_per_image: Some(pyramid.rows),
            },
            size,
        );
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gfx-wave-bind"),
            layout: &self.tex_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            }],
        });
        let Some(slot) = self.decks.get_mut(deck) else {
            return false;
        };
        slot.token = Some(token);
        slot.pyramid = Some(pyramid);
        slot.bind = Some(bind);
        true
    }

    pub fn clear_deck(&mut self, deck: usize) {
        if let Some(slot) = self.decks.get_mut(deck) {
            slot.token = None;
            slot.pyramid = None;
            slot.bind = None;
            slot.phrase_token = None;
            slot.phrases.clear();
        }
    }

    pub fn set_phrases(&mut self, deck: usize, token: u64, phrases: Vec<PhraseView>) {
        if let Some(slot) = self.decks.get_mut(deck) {
            slot.phrase_token = Some(token);
            slot.phrases = phrases;
        }
    }

    fn deck_phrases(&self, deck: usize, token: u64) -> &[PhraseView] {
        let slot = &self.decks[deck];
        if slot.phrase_token == Some(token) {
            &slot.phrases
        } else {
            &[]
        }
    }

    /// Renders every placed surface into `target` and submits the work; clears the whole target to the background.
    pub fn render(&mut self, target: &wgpu::TextureView, input: &FrameInput<'_>) -> RenderStats {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.items.clear();
        let mut ranges: Vec<(usize, std::ops::Range<u32>, PlacedSurface)> = Vec::new();
        for surface in input.surfaces.iter().filter(|s| !s.rect.is_empty()) {
            let deck_index = surface.id.deck;
            let deck = &input.decks[deck_index];
            let phrases = self.deck_phrases(deck_index, deck.token);
            let ghost = (surface.id.kind == SurfaceKind::Scroll)
                .then(|| {
                    ghost_for(
                        input.decks,
                        [
                            self.deck_phrases(0, input.decks[0].token),
                            self.deck_phrases(1, input.decks[1].token),
                        ],
                        deck_index,
                    )
                })
                .flatten();
            let (w, h) = (f64::from(surface.rect.w), f64::from(surface.rect.h));
            let start = scratch.items.len();
            let ctx = SurfaceCtx {
                w,
                h,
                deck,
                deck_index,
                phrases,
                style: &self.style,
                scale: input.scale,
                ghost,
            };
            match surface.id.kind {
                SurfaceKind::Scroll => {
                    scroll_quads(&mut scratch, &ctx, input.zoom_css_pps[deck_index])
                }
                SurfaceKind::Overview => overview_quads(
                    &mut scratch,
                    &ctx,
                    input.hover_px[deck_index],
                    input.anim_secs,
                ),
            }
            let pyramid = self.decks[deck_index].pyramid.as_ref().filter(|p| {
                self.decks[deck_index].token == Some(deck.token) && deck.loaded && p.len > 0
            });
            let uniform = wave_uniform(
                surface,
                deck,
                pyramid,
                phrases,
                &self.style,
                input.scale,
                input.zoom_css_pps[deck_index],
            );
            let res = &self.surfaces[surface.id.index()];
            self.queue
                .write_buffer(&res.wave_buf, 0, bytes_of(&uniform));
            self.queue.write_buffer(
                &res.quad_buf,
                0,
                bytes_of(&QuadUniform {
                    size: [w as f32, h as f32],
                    pad: [0.0; 2],
                }),
            );
            ranges.push((
                surface.id.index(),
                start as u32..scratch.items.len() as u32,
                *surface,
            ));
        }
        if scratch.items.len() > self.quad_capacity {
            self.quad_capacity = scratch.items.len().next_power_of_two();
            self.quad_vbuf = Self::quad_buffer(&self.device, self.quad_capacity);
        }
        if !scratch.items.is_empty() {
            self.queue
                .write_buffer(&self.quad_vbuf, 0, cast_slice(&scratch.items));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gfx-frame"),
            });
        let timed_slot = self.gpu_timer.as_mut().and_then(|t| {
            let i = t.next;
            (!t.slots[i].busy.load(Ordering::Acquire)).then(|| {
                t.next = (i + 1) % t.slots.len();
                i
            })
        });
        {
            let bg = self.style.bg;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gfx-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(bg[0]),
                            g: f64::from(bg[1]),
                            b: f64::from(bg[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timed_slot.and(self.gpu_timer.as_ref()).map(|t| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: &t.queries,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for (index, range, surface) in &ranges {
                let r = surface.rect;
                pass.set_viewport(r.x as f32, r.y as f32, r.w as f32, r.h as f32, 0.0, 1.0);
                pass.set_scissor_rect(r.x, r.y, r.w, r.h);
                let res = &self.surfaces[*index];
                pass.set_pipeline(&self.wave_pipeline);
                pass.set_bind_group(0, &res.wave_bind, &[]);
                pass.set_bind_group(
                    1,
                    self.decks[surface.id.deck]
                        .bind
                        .as_ref()
                        .unwrap_or(&self.empty_bind),
                    &[],
                );
                pass.draw(0..3, 0..1);
                if !range.is_empty() {
                    pass.set_pipeline(&self.quad_pipeline);
                    pass.set_bind_group(0, &res.quad_bind, &[]);
                    pass.set_bind_group(1, &self.atlas_bind, &[]);
                    pass.set_vertex_buffer(0, self.quad_vbuf.slice(..));
                    pass.draw(0..4, range.clone());
                }
            }
        }
        if let (Some(i), Some(t)) = (timed_slot, self.gpu_timer.as_ref()) {
            encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.slots[i].buffer, 0, 16);
        }
        self.queue.submit([encoder.finish()]);
        if let (Some(i), Some(t)) = (timed_slot, self.gpu_timer.as_ref()) {
            let slot = &t.slots[i];
            slot.busy.store(true, Ordering::Release);
            let (buffer, busy, last, period) = (
                slot.buffer.clone(),
                slot.busy.clone(),
                t.last_ns.clone(),
                t.period_ns,
            );
            slot.buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    if r.is_ok() {
                        if let Ok(view) = buffer.slice(..).get_mapped_range() {
                            let w: &[u64] = cast_slice(&view);
                            let ns = w[1].abs_diff(w[0]) as f64 * f64::from(period);
                            last.store(ns.to_bits(), Ordering::Relaxed);
                        }
                        buffer.unmap();
                    }
                    busy.store(false, Ordering::Release);
                });
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        let stats = RenderStats {
            surfaces: ranges.len(),
            quads: scratch.items.len(),
        };
        self.scratch = scratch;
        stats
    }
}
