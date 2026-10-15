struct Surf {
    size: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> u: Surf;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct Inst {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) kind: vec4<f32>,
    @location(3) uv: vec4<f32>,
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) kind: vec4<f32>,
    @location(4) @interpolate(flat) uv: vec4<f32>,
}

const MARGIN: f32 = 1.0;

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Inst) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = inst.rect.xy - MARGIN + corner * (inst.rect.zw + 2.0 * MARGIN);
    var o: VOut;
    o.pos = vec4<f32>(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
    o.local = p;
    o.rect = inst.rect;
    o.color = inst.color;
    o.kind = inst.kind;
    o.uv = inst.uv;
    return o;
}

fn box_sdf(p: vec2<f32>, center: vec2<f32>, half: vec2<f32>) -> f32 {
    let d = abs(p - center) - half;
    return max(d.x, d.y);
}

fn coverage(sdf: f32) -> f32 {
    return clamp(0.5 - sdf, 0.0, 1.0);
}

fn triangle_cov(p: vec2<f32>, rect: vec4<f32>, down: bool) -> f32 {
    let q = (p - rect.xy) / rect.zw;
    let width = select(q.y, 1.0 - q.y, down);
    let across = (abs(q.x - 0.5) * 2.0 - width) * rect.z * 0.5;
    let vertical = clamp(rect.w * min(q.y, 1.0 - q.y) + 0.5, 0.0, 1.0);
    return coverage(across) * vertical;
}

fn round_cov(p: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let half = rect.zw * 0.5;
    let r = min(radius, min(half.x, half.y));
    let q = abs(p - rect.xy - half) - (half - r);
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    return coverage(d);
}

fn hatch_cov(p: vec2<f32>, rect: vec4<f32>, period: f32, anchor: f32) -> f32 {
    let box_cov = coverage(box_sdf(p, rect.xy + rect.zw * 0.5, rect.zw * 0.5));
    let t = (p.x - anchor + p.y) / period;
    let stripe = abs(fract(t) - 0.5) * period - period * 0.22;
    return box_cov * clamp(0.5 - stripe * 0.7071, 0.0, 1.0);
}

fn glyph_cov(p: vec2<f32>, rect: vec4<f32>, uv: vec4<f32>, px_per_unit: f32, bias: f32) -> f32 {
    let t = clamp((p - rect.xy) / rect.zw, vec2<f32>(0.0), vec2<f32>(1.0));
    let sdf = textureSampleLevel(atlas, atlas_sampler, uv.xy + t * uv.zw, 0.0).r;
    return clamp((sdf - 0.5) * px_per_unit + 0.5 + bias, 0.0, 1.0);
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let kind = u32(in.kind.x + 0.5);
    var cov = 0.0;
    if (kind == 0u) {
        let c = in.rect.xy + in.rect.zw * 0.5;
        cov = coverage(box_sdf(in.local, c, in.rect.zw * 0.5));
    } else if (kind == 1u) {
        cov = triangle_cov(in.local, in.rect, true);
    } else if (kind == 2u) {
        cov = triangle_cov(in.local, in.rect, false);
    } else if (kind == 4u) {
        cov = round_cov(in.local, in.rect, in.kind.y);
    } else if (kind == 5u) {
        cov = hatch_cov(in.local, in.rect, in.kind.y, in.kind.z);
    } else {
        cov = glyph_cov(in.local, in.rect, in.uv, in.kind.y, in.kind.z);
    }
    return vec4<f32>(in.color.rgb, in.color.a * cov);
}
