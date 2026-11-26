//! The ambient galaxy: the board floats inside a No Man's Sky-style star map.
//!
//! Everything here is drawn *around* the printed board and is pure atmosphere:
//! it never touches the rules or the pieces. Four layers, cheapest first:
//!
//! 1. **Sky** — one fullscreen pass sampling a procedurally painted
//!    equirectangular galaxy (violet gas, a hot core, a dust-laned band). It
//!    replaces the flat clear colour, so there is no dome geometry to draw.
//! 2. **Stars** — a single merged mesh of ~4,200 soft points, twinkling in the
//!    vertex shader, with a few crossed flares. One additive draw call.
//! 3. **Stardust** — a few hundred motes near the board that drift, wrap and
//!    twinkle. Rebuilt on the CPU each frame (they are few and tiny).
//! 4. **Nebula banks** — large, soft, slowly swirling gas quads far behind the
//!    board.
//!
//! The engine's opaque pass owns the depth buffer; every layer here is blended,
//! depth-tested and depth-write-off, so pieces still occlude the sky correctly.

use anyhow::Result;
use dust_engine::glam::Vec3;
use dust_engine::glow::{self, HasContext};
use dust_engine::mesh::{GpuMesh, MeshData, Vertex};
use dust_engine::shader::link_program;
use dust_engine::texture::{GpuTexture, TextureFilter};
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// small value noise, same flavour as planets.rs (no assets, deterministic)
// ---------------------------------------------------------------------------

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x.wrapping_mul(374761393) as u32)
        ^ (y.wrapping_mul(668265263) as u32)
        ^ (z.wrapping_mul(1274126177) as u32);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn vnoise(p: Vec3) -> f32 {
    let (ix, iy, iz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let f = p - p.floor();
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| hash(ix + dx, iy + dy, iz + dz);
    let x0 = l(l(c(0, 0, 0), c(1, 0, 0), s.x), l(c(0, 1, 0), c(1, 1, 0), s.x), s.y);
    let x1 = l(l(c(0, 0, 1), c(1, 0, 1), s.x), l(c(0, 1, 1), c(1, 1, 1), s.x), s.y);
    l(x0, x1, s.z)
}

fn fbm(mut p: Vec3, oct: u32) -> f32 {
    let (mut a, mut sum, mut norm) = (0.5, 0.0, 0.0);
    for _ in 0..oct {
        sum += a * vnoise(p);
        norm += a;
        p = p * 2.03 + Vec3::new(17.1, 3.7, 9.2);
        a *= 0.5;
    }
    sum / norm
}

fn ridge(p: Vec3, oct: u32) -> f32 {
    1.0 - (fbm(p, oct) * 2.0 - 1.0).abs()
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// A uniform direction on the unit sphere.
fn sphere_dir(rng: &mut Rng) -> Vec3 {
    let z = rng.range(-1.0, 1.0);
    let a = rng.range(0.0, TAU);
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * a.cos(), z, r * a.sin())
}

// ---------------------------------------------------------------------------
// the painted galaxy (equirectangular, 2048 x 1024)
// ---------------------------------------------------------------------------

/// The plane of the galactic band: a slow diagonal across the sky, so the board
/// sits under a bright lane rather than a uniform fog.
fn band_normal() -> Vec3 {
    Vec3::new(0.21, 0.62, -0.75).normalize()
}

/// The warm core sits low and just right of centre in the default camera's view
/// (looking down -Z), so the opening screen gets the galaxy-map money shot.
fn core_dir() -> Vec3 {
    Vec3::new(0.16, -0.95, -0.27).normalize()
}

fn sky_pixels(w: u32, h: u32) -> Vec<u8> {
    let n = band_normal();
    let core = core_dir();
    let mut px = vec![0f32; (w * h * 3) as usize];

    // Art-directed nebula masses: direction and angular radius. Small and
    // scattered on purpose — the dark space between them is the composition.
    let regions: [(Vec3, f32); 8] = [
        (Vec3::new(-0.50, -0.55, -0.67).normalize(), 0.17),
        (Vec3::new(-0.66, -0.22, -0.72).normalize(), 0.13),
        (Vec3::new(0.48, -0.60, -0.64).normalize(), 0.16),
        (Vec3::new(0.72, -0.26, -0.64).normalize(), 0.12),
        (Vec3::new(-0.10, -0.86, -0.50).normalize(), 0.15),
        (Vec3::new(0.82, -0.02, -0.57).normalize(), 0.13),
        (Vec3::new(-0.78, 0.14, -0.61).normalize(), 0.12),
        (Vec3::new(0.18, 0.16, -0.97).normalize(), 0.16),
    ];
    let violet = [0.44, 0.07, 1.00];
    let magenta = [1.00, 0.08, 0.40];
    let teal = [0.02, 0.42, 0.60];
    let blue = [0.08, 0.14, 0.95];

    for y in 0..h {
        let v = (y as f32 + 0.5) / h as f32;
        let ph = v * PI;
        for x in 0..w {
            // Same longitude convention as the sky shader (atan2(z, x)).
            let th = ((x as f32 + 0.5) / w as f32 - 0.5) * TAU;
            let d = Vec3::new(ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin());

            let bd = d.dot(n);
            let band = (-(bd * bd) * 10.0).exp();
            let mut c = [0.0015, 0.002, 0.006];

            // How much nebula lives here: a few soft regions, no hard edge.
            let mut region = 0.0f32;
            for (dir, rad) in regions {
                let ang = d.dot(dir).max(0.0);
                region = region.max(((ang - 1.0) / (rad * rad)).exp());
            }
            let mask = smooth(0.12, 0.70, region);

            // Most of the sphere has no gas at all. The mask multiplies every
            // nebula term, so skip the cloud noise where it is zero: that is
            // where nearly all of the generation time goes.
            if mask > 0.0 {
                let warp = fbm(d * 2.6 + Vec3::new(2.9, 6.1, 0.4), 2);
                let body = fbm(d * 4.6 + Vec3::new(9.2, 2.5, 1.8) + Vec3::splat(warp * 1.3), 4);
                let sheet = smooth(0.40, 0.60, body);
                let billow = sheet.powf(1.3);
                let fine = fbm(d * 9.5 + Vec3::new(1.3, 5.7, 2.2), 3);
                let lace = smooth(0.40, 0.74, fine);
                let thread = smooth(0.70, 0.90, ridge(d * 14.0 + Vec3::new(5.0, 1.0, 6.0), 2));

                // Colour zones: violet where the gas is thin, magenta in the
                // folds, teal where it cools at the edges.
                let zone = fbm(d * 2.2 + Vec3::new(7.0, 2.0, 4.0), 2);
                let mut col = mix(violet, magenta, smooth(0.46, 0.80, zone));
                let cool = fbm(d * 3.0 + Vec3::new(3.0, 9.0, 1.0), 2);
                col = mix(col, teal, smooth(0.58, 0.90, cool) * 0.55);
                col = mix(blue, col, smooth(0.18, 0.55, zone));

                let gas = mask * billow * (0.30 + 0.70 * lace) * 0.42;
                // Bright rims where the sheet folds, and thin hot threads.
                let rim = mask * smooth(0.52, 0.60, body) * smooth(0.72, 0.62, body) * 0.38;
                let fil = mask * thread * billow * 0.47;
                let k = gas + 1.5 * (rim + fil);
                c[0] += col[0] * k;
                c[1] += col[1] * k;
                c[2] += col[2] * k;
            }

            // The band's soft light, and cold dust cutting through everything.
            c = mix(c, [0.045, 0.035, 0.10], band * 0.08);
            // Dust lanes only matter where they cut visible gas or the band's
            // own glow; away from both, skipping them is invisible.
            let cut = if mask > 0.0 || band > 0.003 {
                let lane = smooth(0.54, 0.82, ridge(d * 5.0 + Vec3::new(6.0, 3.2, 9.0), 3));
                let lane2 = smooth(0.55, 0.78, ridge(d * 9.0 + Vec3::new(1.0, 8.0, 2.0), 2));
                1.0 - (lane * 0.72 + lane2 * 0.28)
            } else {
                1.0
            };
            c[0] *= cut;
            c[1] *= cut;
            c[2] *= cut;

            // Only a faint baked tint at the core: the light itself is added in
            // the sky shader so it can go into HDR and bloom.
            let cd = d.dot(core).max(0.0);
            let haze = cd.powf(90.0) * 0.10;
            c[0] += haze;
            c[1] += haze * 0.62;
            c[2] += haze * 0.30;
            let warm = band * smooth(0.2, 0.9, cd) * 0.05;
            c[0] += warm;
            c[1] += warm * 0.70;
            c[2] += warm * 0.40;

            let i = ((y * w + x) * 3) as usize;
            px[i] = c[0];
            px[i + 1] = c[1];
            px[i + 2] = c[2];
        }
    }

    // Baked star dust: a faint grain, denser along the band. These are only the
    // soft far background; the crisp, twinkling stars are real geometry.
    let mut rng = Rng(0x51ed_1a3);
    for _ in 0..16_000 {
        let d = sphere_dir(&mut rng);
        let bandish = (-(d.dot(n).powi(2)) * 3.0).exp();
        if rng.next() > 0.30 + 0.70 * bandish {
            continue;
        }
        let u = d.z.atan2(d.x) / TAU + 0.5;
        let v = d.y.clamp(-1.0, 1.0).acos() / PI;
        let cx = u * w as f32;
        let cy = v * h as f32;
        let warm = rng.next();
        let tint = if warm < 0.30 {
            [1.0, 0.86, 0.68]
        } else if warm < 0.62 {
            [0.78, 0.86, 1.0]
        } else {
            [1.0, 0.97, 0.92]
        };
        let mut bright = rng.range(0.012, 0.10).powf(1.6);
        // The core is a dense cluster, not just a bright smudge.
        if d.dot(core) > 0.985 {
            bright = rng.range(0.10, 0.55).powf(1.4);
        }
        let x = (cx as i32).rem_euclid(w as i32) as u32;
        let y = (cy as i32).clamp(0, h as i32 - 1) as u32;
        let i = ((y * w + x) * 3) as usize;
        px[i] += tint[0] * bright;
        px[i + 1] += tint[1] * bright;
        px[i + 2] += tint[2] * bright;
    }

    // Encode with a filmic shoulder: deep shadows, compressed highlights.
    let mut out = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        let i3 = i * 3;
        let i4 = i * 4;
        // A little noise before quantising, or a sky this dark bands in 8 bits.
        let x = i as u32 % w;
        let y = i as u32 / w;
        let dith = (((x.wrapping_mul(73856093)) ^ (y.wrapping_mul(19349663))) & 1023) as f32
            / 1023.0
            - 0.5;
        for k in 0..3 {
            let v = (px[i3 + k].clamp(0.0, 1.0)).powf(1.0 / 2.2) + dith * 0.5 / 255.0;
            out[i4 + k] = (v.clamp(0.0, 1.0) * 255.0) as u8;
        }
        out[i4 + 3] = 255;
    }
    out
}

/// Wispy cloud alpha for the nebula banks: soft in the middle, torn at the edge.
fn cloud_pixels(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5 - c) / c;
            let dy = (y as f32 + 0.5 - c) / c;
            let r = (dx * dx + dy * dy).sqrt();
            let disc = smooth(1.0, 0.15, r);
            let n = fbm(Vec3::new(dx * 2.6, dy * 2.6, 4.0), 5);
            let wisp = smooth(0.30, 0.72, n);
            let a = disc * (0.25 + 0.75 * wisp) * disc;
            let i = ((y * size + x) * 4) as usize;
            out[i] = 255;
            out[i + 1] = 255;
            out[i + 2] = 255;
            out[i + 3] = (a * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// shaders
// ---------------------------------------------------------------------------

const SKY_VERT: &str = r#"#version 330 core
out vec2 vClip;
void main() {
    vec2 p[3] = vec2[3](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    gl_Position = vec4(p[gl_VertexID], 1.0, 1.0);
    vClip = p[gl_VertexID];
}
"#;

const SKY_FRAG: &str = r#"#version 330 core
in vec2 vClip;
out vec4 FragColor;
uniform mat4 uInvViewProj;
uniform sampler2D uSky;
uniform float uTime;
uniform vec3 uCore;
uniform vec3 uBand;

void main() {
    // Rebuild this pixel's view ray, then read the painted sky along it.
    vec4 nearP = uInvViewProj * vec4(vClip, -1.0, 1.0);
    vec4 farP  = uInvViewProj * vec4(vClip,  1.0, 1.0);
    vec3 dir = normalize(farP.xyz / farP.w - nearP.xyz / nearP.w);

    // The whole sky turns imperceptibly slowly: the only cue that the galaxy is
    // a place and not a wallpaper.
    float a = uTime * 0.0035;
    float ca = cos(a), sa = sin(a);
    vec3 d = vec3(dir.x * ca - dir.z * sa, dir.y, dir.x * sa + dir.z * ca);

    float u = atan(d.z, d.x) * 0.15915494 + 0.5;
    float v = acos(clamp(d.y, -1.0, 1.0)) * 0.31830989;

    // A whisper of drift in the gas, so the nebula breathes without moving the stars.
    float wob = 0.0016 * sin(uTime * 0.045 + v * 7.0) + 0.0010 * sin(uTime * 0.031 - u * 11.0);
    vec3 c = texture(uSky, vec2(u + wob, v)).rgb;

    // The texture is painted in sRGB (that is what the eye judges); the scene is
    // linear, so decode before the tonemap or the whole sky turns to milky fog.
    c = pow(c, vec3(2.2));

    // Only the hottest gas is lifted into HDR: the core and its inner haze bloom,
    // everything else stays a deep, quiet backdrop for the board.
    float l = max(c.r, max(c.g, c.b));
    c *= 0.92 + 0.95 * smoothstep(0.12, 0.60, l);

    // The galaxy's own light: a hot white-gold core in an amber haze, plus a
    // luminous spine along the band. Deliberately above 2.0 so it blooms.
    float cd = max(dot(d, uCore), 0.0);
    c += vec3(9.0, 7.9, 6.1) * pow(cd, 6000.0);
    c += vec3(0.120, 0.085, 0.048) * pow(cd, 900.0);
    c += vec3(0.020, 0.014, 0.006) * pow(cd, 40.0);
    float bd = dot(d, uBand);
    c += vec3(0.018, 0.021, 0.045) * exp(-bd * bd * 14.0);
    FragColor = vec4(c, 1.0);
}
"#;

const POINT_VERT: &str = r#"#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aMeta;   // phase, twinkle rate, kind
layout (location = 2) in vec2 aLocal;  // quad-space -1..1
layout (location = 3) in vec3 aColor;
uniform mat4 uView;
uniform mat4 uProj;
uniform float uTime;
out vec3 vColor;
out vec2 vLocal;
void main() {
    float tw = 0.66 + 0.34 * sin(uTime * aMeta.y + aMeta.x);
    vColor = aColor * tw;
    vLocal = aLocal;
    gl_Position = uProj * uView * vec4(aPos, 1.0);
}
"#;

const POINT_FRAG: &str = r#"#version 330 core
in vec3 vColor;
in vec2 vLocal;
out vec4 FragColor;
void main() {
    float r = length(vLocal);
    float a = 1.0 - smoothstep(0.05, 1.0, r);
    a *= a;
    if (a < 0.004) discard;
    FragColor = vec4(vColor * a, 1.0);
}
"#;

const CLOUD_VERT: &str = r#"#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aMeta;   // phase, spin
layout (location = 2) in vec2 aUV;
layout (location = 3) in vec3 aColor;
uniform mat4 uView;
uniform mat4 uProj;
out vec2 vUV;
out vec3 vColor;
out vec3 vMeta;
void main() {
    vUV = aUV;
    vColor = aColor;
    vMeta = aMeta;
    gl_Position = uProj * uView * vec4(aPos, 1.0);
}
"#;

const CLOUD_FRAG: &str = r#"#version 330 core
in vec2 vUV;
in vec3 vColor;
in vec3 vMeta;
out vec4 FragColor;
uniform sampler2D uTex;
uniform float uTime;
void main() {
    // Slow swirl: rotate the sample, not the geometry, so the bank churns in place.
    float a = vMeta.x + uTime * vMeta.y;
    float ca = cos(a), sa = sin(a);
    vec2 p = vUV - 0.5;
    vec2 uv = vec2(p.x * ca - p.y * sa, p.x * sa + p.y * ca) + 0.5;
    float tex = texture(uTex, uv).a;
    float pulse = 0.86 + 0.14 * sin(uTime * 0.06 + vMeta.x * 4.0);
    FragColor = vec4(vColor * tex * pulse, 1.0);
}
"#;

// ---------------------------------------------------------------------------
// geometry helpers
// ---------------------------------------------------------------------------

/// Pushes one billboard quad. `ax`/`ay` are the half-extent vectors, `uvs` the
/// four (bottom-left, bottom-right, top-right, top-left) texture coordinates.
fn quad(
    verts: &mut Vec<Vertex>,
    idx: &mut Vec<u32>,
    centre: Vec3,
    ax: Vec3,
    ay: Vec3,
    color: [[f32; 3]; 4],
    meta: [f32; 3],
    uvs: [[f32; 2]; 4],
) {
    let base = verts.len() as u32;
    let corners = [
        centre - ax - ay,
        centre + ax - ay,
        centre + ax + ay,
        centre - ax + ay,
    ];
    for (k, p) in corners.into_iter().enumerate() {
        verts.push(Vertex {
            position: p.to_array(),
            normal: meta,
            uv: uvs[k],
            color: color[k],
        });
    }
    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A tangent basis facing the origin, so a far billboard always looks at the board.
fn face_origin(dir: Vec3) -> (Vec3, Vec3) {
    let up = if dir.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
    let ax = up.cross(dir).normalize_or_zero();
    (ax, dir.cross(ax).normalize_or_zero())
}

fn star_color(rng: &mut Rng) -> [f32; 3] {
    let t = rng.next();
    let base = if t < 0.30 {
        [0.55, 0.72, 1.0]
    } else if t < 0.56 {
        [1.0, 0.86, 0.62]
    } else if t < 0.80 {
        [0.88, 0.94, 1.0]
    } else if t < 0.92 {
        [1.0, 0.62, 0.52]
    } else {
        [0.96, 0.80, 0.96]
    };
    let k = rng.range(0.45, 1.65);
    [base[0] * k, base[1] * k, base[2] * k]
}

/// The star field: ~4,200 points plus a handful of flares, one mesh.
fn star_mesh() -> MeshData {
    let mut verts = Vec::with_capacity(24_000);
    let mut idx = Vec::with_capacity(30_000);
    let mut rng = Rng(0x9e37_79b9);
    let n = band_normal();

    for _ in 0..6200 {
        let mut d = sphere_dir(&mut rng);
        // Pull a good share of the stars into the galactic band.
        if rng.next() < 0.38 {
            let flat = Vec3::new(d.x, 0.0, d.z).normalize_or_zero();
            let target = (flat - n * flat.dot(n)).normalize_or_zero();
            d = (target + n * rng.range(-0.06, 0.06)).normalize_or_zero();
        }
        let r = rng.range(200.0, 280.0);
        let p = d * r;
        let (ax, ay) = face_origin(d);
        let size = rng.range(0.10, 0.52);
        let c = star_color(&mut rng);
        let phase = rng.range(0.0, TAU);
        let rate = rng.range(0.5, 2.3);
        quad(
            &mut verts,
            &mut idx,
            p,
            ax * size,
            ay * size,
            [c; 4],
            [phase, rate, 0.0],
            [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
        );
    }

    // A scatter of brighter, more colourful stars: the ones you notice.
    for _ in 0..520 {
        let d = sphere_dir(&mut rng);
        let p = d * rng.range(200.0, 275.0);
        let (ax, ay) = face_origin(d);
        let size = rng.range(0.48, 1.15);
        let mut c = star_color(&mut rng);
        c = [c[0] * 2.8, c[1] * 2.8, c[2] * 2.8];
        quad(
            &mut verts,
            &mut idx,
            p,
            ax * size,
            ay * size,
            [c; 4],
            [rng.range(0.0, TAU), rng.range(0.3, 1.1), 0.0],
            [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
        );
    }

    // A dense cluster around the galactic core: the galaxy map's bright heart.
    let core = core_dir();
    for _ in 0..520 {
        let jitter = Vec3::new(rng.range(-0.20, 0.20), rng.range(-0.20, 0.20), rng.range(-0.20, 0.20));
        let d = (core + jitter).normalize_or_zero();
        let p = d * rng.range(200.0, 270.0);
        let (ax, ay) = face_origin(d);
        let size = rng.range(0.12, 0.55);
        let c = star_color(&mut rng);
        let warm = [c[0] * 1.25, c[1] * 1.10, c[2] * 0.92];
        quad(
            &mut verts,
            &mut idx,
            p,
            ax * size,
            ay * size,
            [warm; 4],
            [rng.range(0.0, TAU), rng.range(0.4, 1.6), 0.0],
            [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
        );
    }

    // A few heroes with a soft four-point flare: brighter, and crossed.
    for _ in 0..18 {
        let d = sphere_dir(&mut rng);
        let p = d * rng.range(190.0, 250.0);
        let (ax, ay) = face_origin(d);
        let len = rng.range(2.2, 5.2);
        let wid = len * 0.022;
        let c = star_color(&mut rng);
        let bright = [c[0] * 1.25, c[1] * 1.25, c[2] * 1.25];
        let zero = [0.0; 3];
        let meta = [rng.range(0.0, TAU), rng.range(0.3, 1.0), 1.0];
        let uvs = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        // horizontal blade, then vertical blade; tips fade to nothing
        quad(&mut verts, &mut idx, p, ax * len, ay * wid, [zero, bright, bright, zero], meta, uvs);
        quad(&mut verts, &mut idx, p, ax * wid, ay * len, [zero, bright, bright, zero], meta, uvs);
    }

    MeshData { vertices: verts, indices: idx }
}

/// The far gas banks and the planet atmospheres share one textured quad pass.
fn cloud_mesh() -> MeshData {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let mut rng = Rng(0x1b87_3c05);
    let tints: [[f32; 3]; 6] = [
        [0.30, 0.12, 0.55],
        [0.52, 0.16, 0.40],
        [0.10, 0.30, 0.48],
        [0.20, 0.16, 0.52],
        [0.36, 0.26, 0.60],
        [0.12, 0.36, 0.42],
    ];
    for k in 0..4 {
        let mut d = sphere_dir(&mut rng);
        // Keep the banks low in the sky, where the camera actually looks.
        if d.y > 0.15 {
            d.y = -d.y * 0.6;
            d = d.normalize();
        }
        let p = d * rng.range(95.0, 190.0);
        let (ax, ay) = face_origin(d);
        let size = rng.range(110.0, 230.0);
        let tint = tints[k % tints.len()];
        let strength = rng.range(0.004, 0.012);
        let c = [tint[0] * strength, tint[1] * strength, tint[2] * strength];
        let meta = [rng.range(0.0, TAU), rng.range(-0.018, 0.018), 0.0];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        quad(&mut verts, &mut idx, p, ax * size, ay * size, [c; 4], meta, uvs);
    }
    MeshData { vertices: verts, indices: idx }
}

// ---------------------------------------------------------------------------
// the pass itself
// ---------------------------------------------------------------------------

struct Mote {
    pos: Vec3,
    vel: Vec3,
    phase: f32,
    rate: f32,
    size: f32,
    color: [f32; 3],
}

pub struct Galaxy {
    sky_program: glow::Program,
    sky_vao: glow::VertexArray,
    sky_tex: GpuTexture,
    sky_inv: Option<glow::UniformLocation>,
    sky_time: Option<glow::UniformLocation>,
    sky_sampler: Option<glow::UniformLocation>,
    sky_core: Option<glow::UniformLocation>,
    sky_band: Option<glow::UniformLocation>,

    point_program: glow::Program,
    point_view: Option<glow::UniformLocation>,
    point_proj: Option<glow::UniformLocation>,
    point_time: Option<glow::UniformLocation>,
    star_mesh: GpuMesh,

    cloud_program: glow::Program,
    cloud_view: Option<glow::UniformLocation>,
    cloud_proj: Option<glow::UniformLocation>,
    cloud_time: Option<glow::UniformLocation>,
    cloud_sampler: Option<glow::UniformLocation>,
    cloud_mesh: GpuMesh,
    cloud_tex: GpuTexture,

    dust: Vec<Mote>,
    dust_mesh: GpuMesh,
    dust_verts: Vec<Vertex>,

    /// Previous frame's clock, so the dust can advance without a dt argument.
    last_time: f32,
}

impl Galaxy {
    pub fn new(gl: &glow::Context) -> Result<Self> {
        let sky_program = link_program(gl, SKY_VERT, SKY_FRAG)?;
        let sky_vao = unsafe { gl.create_vertex_array().map_err(anyhow::Error::msg)? };
        let size = 2048u32;
        let sky_tex = GpuTexture::from_rgba8(gl, &sky_pixels(size, size / 2), size, size / 2, TextureFilter::Bilinear)?;
        // Equirect wraps around; the poles clamp.
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(sky_tex.handle));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        }

        let point_program = link_program(gl, POINT_VERT, POINT_FRAG)?;
        let star_mesh = GpuMesh::upload(gl, &star_mesh())?;

        let cloud_program = link_program(gl, CLOUD_VERT, CLOUD_FRAG)?;
        let cloud_mesh = GpuMesh::upload(gl, &cloud_mesh())?;
        let cloud_tex = GpuTexture::from_rgba8(gl, &cloud_pixels(512), 512, 512, TextureFilter::Bilinear)?;

        let mut rng = Rng(0x2f6e_2b1);
        let dust: Vec<Mote> = (0..260)
            .map(|_| {
                let warm = rng.next() < 0.45;
                Mote {
                    pos: Vec3::new(
                        rng.range(-24.0, 24.0),
                        rng.range(0.25, 9.0),
                        rng.range(-22.0, 22.0),
                    ),
                    vel: Vec3::new(rng.range(-0.22, 0.22), rng.range(-0.03, 0.03), rng.range(-0.22, 0.22)),
                    phase: rng.range(0.0, TAU),
                    rate: rng.range(0.7, 2.6),
                    size: rng.range(0.012, 0.038),
                    color: if warm {
                        [1.0, 0.72, 0.42]
                    } else {
                        [0.62, 0.78, 1.0]
                    },
                }
            })
            .collect();
        let dust_verts = dust_vertices(&dust);
        let dust_mesh = GpuMesh::upload_dynamic(
            gl,
            &MeshData { vertices: dust_verts.clone(), indices: quad_indices(260) },
        )?;

        Ok(Self {
            sky_program,
            sky_vao,
            sky_tex,
            sky_inv: unsafe { gl.get_uniform_location(sky_program, "uInvViewProj") },
            sky_time: unsafe { gl.get_uniform_location(sky_program, "uTime") },
            sky_sampler: unsafe { gl.get_uniform_location(sky_program, "uSky") },
            sky_core: unsafe { gl.get_uniform_location(sky_program, "uCore") },
            sky_band: unsafe { gl.get_uniform_location(sky_program, "uBand") },

            point_program,
            point_view: unsafe { gl.get_uniform_location(point_program, "uView") },
            point_proj: unsafe { gl.get_uniform_location(point_program, "uProj") },
            point_time: unsafe { gl.get_uniform_location(point_program, "uTime") },
            star_mesh,

            cloud_program,
            cloud_view: unsafe { gl.get_uniform_location(cloud_program, "uView") },
            cloud_proj: unsafe { gl.get_uniform_location(cloud_program, "uProj") },
            cloud_time: unsafe { gl.get_uniform_location(cloud_program, "uTime") },
            cloud_sampler: unsafe { gl.get_uniform_location(cloud_program, "uTex") },
            cloud_mesh,
            cloud_tex,

            dust,
            dust_mesh,
            dust_verts,

            last_time: 0.0,
        })
    }

    /// The painted galaxy, drawn first every frame: no depth, no geometry.
    pub fn draw_sky(&self, gl: &glow::Context, inv_view_proj: &[f32; 16], time: f32) {
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.sky_program));
            gl.uniform_matrix_4_f32_slice(self.sky_inv.as_ref(), false, inv_view_proj);
            gl.uniform_1_f32(self.sky_time.as_ref(), time);
            let core = core_dir();
            let band = band_normal();
            gl.uniform_3_f32(self.sky_core.as_ref(), core.x, core.y, core.z);
            gl.uniform_3_f32(self.sky_band.as_ref(), band.x, band.y, band.z);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.sky_tex.handle));
            gl.uniform_1_i32(self.sky_sampler.as_ref(), 0);
            gl.bind_vertex_array(Some(self.sky_vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.depth_mask(true);
            gl.enable(glow::DEPTH_TEST);
        }
    }

    /// Stars, dust and gas banks: one additive pass group. Call after the
    /// opaque pieces, before `present`.
    pub fn draw_glow(
        &mut self,
        gl: &glow::Context,
        view: &[f32; 16],
        proj: &[f32; 16],
        time: f32,
    ) {
        let dt = (time - self.last_time).clamp(0.0, 0.1);
        self.last_time = time;
        self.step_dust(dt);
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.disable(glow::CULL_FACE);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE);
        }

        // Far gas banks first: they are the backdrop the stars sit in.
        unsafe {
            gl.use_program(Some(self.cloud_program));
            gl.uniform_matrix_4_f32_slice(self.cloud_view.as_ref(), false, view);
            gl.uniform_matrix_4_f32_slice(self.cloud_proj.as_ref(), false, proj);
            gl.uniform_1_f32(self.cloud_time.as_ref(), time);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.cloud_tex.handle));
            gl.uniform_1_i32(self.cloud_sampler.as_ref(), 0);
        }
        self.cloud_mesh.draw(gl);

        // The star field: one draw for every point.
        unsafe {
            gl.use_program(Some(self.point_program));
            gl.uniform_matrix_4_f32_slice(self.point_view.as_ref(), false, view);
            gl.uniform_matrix_4_f32_slice(self.point_proj.as_ref(), false, proj);
            gl.uniform_1_f32(self.point_time.as_ref(), time);
        }
        self.star_mesh.draw(gl);

        // Near stardust last: tiny, warm and cool motes drifting through the board.
        if !self.dust.is_empty() {
            self.dust_mesh.update_vertices(gl, &self.dust_verts);
            unsafe {
                gl.use_program(Some(self.point_program));
                gl.uniform_matrix_4_f32_slice(self.point_view.as_ref(), false, view);
                gl.uniform_matrix_4_f32_slice(self.point_proj.as_ref(), false, proj);
                gl.uniform_1_f32(self.point_time.as_ref(), time);
            }
            self.dust_mesh.draw(gl);
        }

        unsafe {
            gl.depth_mask(true);
            gl.disable(glow::BLEND);
        }
    }

    fn step_dust(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for m in &mut self.dust {
            m.pos += m.vel * dt;
            if m.pos.x.abs() > 26.0 {
                m.pos.x = -m.pos.x.signum() * 26.0;
            }
            if m.pos.z.abs() > 24.0 {
                m.pos.z = -m.pos.z.signum() * 24.0;
            }
            if m.pos.y > 9.5 {
                m.pos.y = 0.3;
            } else if m.pos.y < 0.2 {
                m.pos.y = 9.2;
            }
        }
        self.dust_verts = dust_vertices(&self.dust);
    }

}

fn quad_indices(n: usize) -> Vec<u32> {
    let mut idx = Vec::with_capacity(n * 6);
    for q in 0..n {
        let b = (q * 4) as u32;
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    idx
}

fn dust_vertices(dust: &[Mote]) -> Vec<Vertex> {
    let mut verts = Vec::with_capacity(dust.len() * 4);
    for m in dust {
        let dir = m.pos.normalize_or_zero();
        let up = if dir.y.abs() > 0.92 { Vec3::X } else { Vec3::Y };
        let ax = up.cross(dir).normalize_or_zero() * m.size;
        let ay = dir.cross(ax).normalize_or_zero() * m.size;
        let base = m.color;
        // Tip vertices a little dimmer, so each mote fades rather than pops.
        let dim = [base[0] * 0.35, base[1] * 0.35, base[2] * 0.35];
        let corners = [m.pos - ax - ay, m.pos + ax - ay, m.pos + ax + ay, m.pos - ax + ay];
        let colors = [dim, dim, base, base];
        for (k, p) in corners.into_iter().enumerate() {
            verts.push(Vertex {
                position: p.to_array(),
                normal: [m.phase, m.rate, 2.0],
                uv: [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]][k],
                color: colors[k],
            });
        }
    }
    verts
}
