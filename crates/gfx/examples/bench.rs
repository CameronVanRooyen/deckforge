use std::time::Instant;

use deckforge_gfx::PixelRect;
use deckforge_gfx::offscreen::OffscreenRenderer;
use deckforge_gfx::scene::{FrameInput, PlacedSurface, SurfaceId, SurfaceKind};
use deckforge_gfx::synth::{synth_deck, synth_phrases, synth_track};

fn main() {
    let (w, h) = (3200u32, 1900u32);
    let mut r = OffscreenRenderer::new().expect("gpu");
    println!("adapter: {}", r.gpu.adapter_summary());
    println!("gpu timing: {}", r.renderer.enable_gpu_timing());
    let wf = synth_track(360.0, 126.0);
    for d in 0..2 {
        r.renderer.set_waveform(d, d as u64 + 1, Some(&wf));
        r.renderer
            .set_phrases(d, d as u64 + 1, synth_phrases(360.0, 126.0));
    }
    let surf = |kind, deck, y, h| PlacedSurface {
        id: SurfaceId { kind, deck },
        rect: PixelRect { x: 0, y, w, h },
    };
    let surfaces = [
        surf(SurfaceKind::Overview, 0, 0, 160),
        surf(SurfaceKind::Scroll, 0, 180, 700),
        surf(SurfaceKind::Scroll, 1, 900, 700),
        surf(SurfaceKind::Overview, 1, 1620, 160),
    ];
    let texture = r
        .gpu
        .device
        .create_texture(&deckforge_gfx::wgpu::TextureDescriptor {
            label: None,
            size: deckforge_gfx::wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: deckforge_gfx::wgpu::TextureDimension::D2,
            format: deckforge_gfx::gpu::OFFSCREEN_FORMAT,
            usage: deckforge_gfx::wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    let view = texture.create_view(&Default::default());
    let frames = 600;
    let mut cpu = Vec::new();
    let mut gpu = Vec::new();
    for i in 0..frames {
        let decks = [
            synth_deck(60.0 + i as f64 / 120.0, 360.0, 126.0, 1),
            synth_deck(90.0 + i as f64 / 120.0, 360.0, 126.0, 2),
        ];
        let input = FrameInput {
            size: (w, h),
            scale: 2.0,
            surfaces: &surfaces,
            decks: &decks,
            zoom_css_pps: [90.0, 90.0],
            hover_px: [None, None],
            anim_secs: i as f64 / 120.0,
        };
        let t = Instant::now();
        r.renderer.render(&view, &input);
        cpu.push(t.elapsed().as_secs_f64() * 1000.0);
        r.gpu
            .device
            .poll(deckforge_gfx::wgpu::PollType::wait_indefinitely())
            .ok();
        if let Some(g) = r.renderer.last_gpu_ms() {
            gpu.push(g);
        }
    }
    let stat = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        if v.is_empty() {
            return "n/a".into();
        }
        format!(
            "p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms",
            v[v.len() / 2],
            v[(v.len() as f64 * 0.99) as usize],
            v[v.len() - 1]
        )
    };
    println!("target {w}x{h} px, 4 surfaces, 2 decks playing");
    println!("cpu encode+submit: {}", stat(cpu));
    println!("gpu pass:          {}", stat(gpu));
}
