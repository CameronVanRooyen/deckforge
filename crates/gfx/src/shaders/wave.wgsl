struct Surf {
    rect: vec4<f32>,
    c0: vec4<f32>,
    c1: vec4<f32>,
    c3: vec4<f32>,
    bg: vec4<f32>,
    ghost: vec4<f32>,
    tint: vec4<f32>,
    low: array<vec4<f32>, 3>,
    mid: array<vec4<f32>, 3>,
    high: array<vec4<f32>, 3>,
    levels: array<vec4<u32>, 24>,
}

@group(0) @binding(0) var<uniform> u: Surf;
@group(1) @binding(0) var wave_tex: texture_2d<f32>;

const TEX_W: i32 = 4096;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn texel(level: i32, idx: i32) -> vec4<f32> {
    let info = u.levels[level];
    if (idx < 0 || idx >= i32(info.y)) {
        return vec4<f32>(0.0);
    }
    return textureLoad(wave_tex, vec2<i32>(idx % TEX_W, i32(info.x) + idx / TEX_W), 0);
}

fn envelope(a: f32, b: f32) -> vec4<f32> {
    let span = b - a;
    if (span < 1.0) {
        let c = 0.5 * (a + b) - 0.5;
        let i0 = floor(c);
        return mix(texel(0, i32(i0)), texel(0, i32(i0) + 1), c - i0);
    }
    let top = i32(u.c3.z) - 1;
    let level = clamp(i32(floor(log2(span))), 0, top);
    let step = exp2(f32(level));
    let i0 = i32(floor(a / step));
    let i1 = min(i32(floor((b - 0.001) / step)), i0 + 16);
    var m = vec4<f32>(0.0);
    for (var i = i0; i <= i1; i++) {
        m = max(m, texel(level, i));
    }
    return m;
}

fn curve(v: f32) -> f32 {
    return v;
}

fn mirrored_gradient(s: array<vec4<f32>, 3>, r: f32) -> vec4<f32> {
    if (r < 0.16) {
        return mix(s[0], s[1], r / 0.16);
    }
    return mix(s[1], s[2], clamp((r - 0.16) / 0.84, 0.0, 1.0));
}

fn over(dst: vec3<f32>, src: vec4<f32>, coverage: f32) -> vec3<f32> {
    return mix(dst, src.rgb, src.a * coverage);
}

fn column(bg: vec3<f32>, y: f32, h: f32, env: vec4<f32>) -> vec3<f32> {
    let cy = h * 0.5;
    let d = abs(y - cy);
    let amp = h * 0.46;
    let r = d / cy;
    var col = over(bg, u.ghost, clamp(curve(env.w) * amp - d + 0.5, 0.0, 1.0));
    col = over(col, mirrored_gradient(u.low, r), clamp(curve(env.x) * amp - d + 0.5, 0.0, 1.0));
    col = over(col, mirrored_gradient(u.mid, r), clamp(curve(env.y) * amp * 0.82 - d + 0.5, 0.0, 1.0));
    return over(col, mirrored_gradient(u.high, r), clamp(curve(env.z) * amp * 0.62 - d + 0.5, 0.0, 1.0));
}

fn scroll(local_in: vec2<f32>) -> vec3<f32> {
    let w = u.rect.z;
    let top = u.c3.x;
    let h = u.rect.w - top;
    let local = vec2<f32>(local_in.x, local_in.y - top);
    let cx = floor(w * 0.5);
    let pps = u.c0.z;
    let points_per_px = u.c0.w / pps;
    var col = u.bg.rgb;

    if (local.y >= 0.0 && u.c1.x > 0.5) {
        let dxs = local.x - cx + u.c0.y;
        let k_off = floor(dxs);
        let frac = dxs - k_off;
        let k = u.c0.x + k_off;
        col = column(col, local.y, h, envelope(k * points_per_px, (k + 1.0) * points_per_px));
        if (u.c1.w > 0.5) {
            let before = frac < 0.5;
            let other = select(k + 1.0, k - 1.0, before);
            let neighbour = column(u.bg.rgb, local.y, h, envelope(other * points_per_px, (other + 1.0) * points_per_px));
            col = mix(col, neighbour, select(frac - 0.5, 0.5 - frac, before));
        }
    } else if (local.y >= 0.0) {
        let d = abs(local.y - h * 0.5);
        col = over(col, u.ghost, clamp(1.0 - d + 0.5, 0.0, 1.0) * 0.8);
    }

    if (local.x < cx) {
        col = over(col, u.tint, 1.0);
    }

    return col;
}

fn overview(local: vec2<f32>) -> vec3<f32> {
    let w = u.rect.z;
    let wh = u.c1.z;
    var col = u.bg.rgb;
    if (u.c1.x < 0.5 || local.y >= wh) {
        return col;
    }
    let ppp = u.c3.w / w;
    let k = floor(local.x);
    let env = envelope(k * ppp, (k + 1.0) * ppp);
    let d = wh - local.y;
    let amp = wh * 0.96;
    let t = local.y / wh;
    let cov_peak = clamp(curve(env.w) * amp - d + 0.5, 0.0, 1.0);
    let cov_low = clamp(curve(env.x) * amp - d + 0.5, 0.0, 1.0);
    let cov_mid = clamp(curve(env.y) * amp * 0.82 - d + 0.5, 0.0, 1.0);
    let cov_high = clamp(curve(env.z) * amp * 0.62 - d + 0.5, 0.0, 1.0);
    col = over(col, u.ghost, cov_peak);
    col = over(col, mix(u.low[1], u.low[0], t), cov_low);
    col = over(col, mix(u.mid[1], u.mid[0], t), cov_mid);
    col = over(col, mix(u.high[1], u.high[0], t), cov_high);
    return col;
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let local = frag.xy - u.rect.xy;
    var col: vec3<f32>;
    if (u.c1.y < 0.5) {
        col = scroll(local);
    } else {
        col = overview(local);
    }
    return vec4<f32>(col, 1.0);
}
