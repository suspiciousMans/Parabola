//! Procedural planet surfaces: a colour map and a matching normal map per
//! planet type, generated from 3D value noise sampled on the sphere (so there
//! is no seam and no pole pinching). Done once at start-up; no asset files.

use dust_engine::glam::Vec3;
use std::f32::consts::{PI, TAU};

pub struct PlanetTex {
    pub w: u32,
    pub h: u32,
    pub color: Vec<u8>,
    pub normal: Vec<u8>,
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x.wrapping_mul(374761393) as u32) ^ (y.wrapping_mul(668265263) as u32) ^ (z.wrapping_mul(1274126177) as u32);
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

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Colour (0..255) and height (0..1) for planet `kind` at unit direction `d`.
fn surface(kind: usize, d: Vec3) -> ([f32; 3], f32) {
    match kind {
        // Material: cratered rock
        0 => {
            let n = fbm(d * 3.4, 5);
            let c = vnoise(d * 6.5 + Vec3::splat(4.0));
            let crater = smooth(0.34, 0.2, c);
            let rim = smooth(0.34, 0.4, c) * smooth(0.5, 0.38, c);
            let h = (n * 0.7 - crater * 0.45 + rim * 0.12).clamp(0.0, 1.0);
            let col = mix([72.0, 62.0, 54.0], [158.0, 134.0, 102.0], fbm(d * 5.0 + Vec3::splat(2.0), 4));
            let k = 0.75 + 0.45 * h - crater * 0.18;
            ([col[0] * k, col[1] * k, col[2] * k], h)
        }
        // Fuel: banded gas giant
        1 => {
            let warp = fbm(d * 2.6, 4);
            let b1 = (d.y * 15.0 + warp * 5.5).sin() * 0.5 + 0.5;
            let b2 = (d.y * 6.5 + warp * 3.0 + 1.3).sin() * 0.5 + 0.5;
            let c = mix([138.0, 84.0, 52.0], [228.0, 200.0, 154.0], b1);
            let c = mix(c, [104.0, 62.0, 46.0], b2 * 0.45);
            let storm = smooth(0.72, 0.8, fbm(d * 4.0 + Vec3::splat(8.0), 3)) * 0.35;
            let c = mix(c, [236.0, 224.0, 196.0], storm);
            (c, 0.5 + 0.05 * warp)
        }
        // Weapons: scorched rock, ember cracks
        2 => {
            let n = fbm(d * 3.0, 5);
            let ridge = 1.0 - (fbm(d * 4.5 + Vec3::splat(5.0), 4) * 2.0 - 1.0).abs();
            let crack = smooth(0.9, 0.975, ridge);
            let base = mix([30.0, 28.0, 31.0], [70.0, 60.0, 58.0], n);
            let c = mix(base, [222.0, 98.0, 42.0], crack * 0.95);
            (c, (n * 0.8 - crack * 0.4).clamp(0.0, 1.0))
        }
        // Relics: pale crystal ice
        3 => {
            let n = fbm(d * 3.8, 5);
            let ridge = 1.0 - (fbm(d * 6.0 + Vec3::splat(1.0), 4) * 2.0 - 1.0).abs();
            let vein = smooth(0.86, 0.95, ridge);
            let c = mix([128.0, 176.0, 190.0], [224.0, 242.0, 246.0], n);
            let c = mix(c, [64.0, 112.0, 134.0], vein * 0.8);
            (c, (n * 0.6 + smooth(0.7, 0.9, ridge) * 0.3).clamp(0.0, 1.0))
        }
        // Psionic: warped violet swirl
        _ => {
            let w = fbm(d * 3.0, 3);
            let q = fbm(d * 2.2 + Vec3::splat(w * 1.8), 5);
            let c = mix([46.0, 34.0, 88.0], [140.0, 106.0, 196.0], q);
            let c = mix(c, [208.0, 192.0, 238.0], smooth(0.62, 0.8, q) * 0.7);
            (c, 0.5 + 0.1 * q)
        }
    }
}

pub fn generate(kind: usize, w: u32, h: u32) -> PlanetTex {
    let dir = |x: i32, y: i32| {
        let x = x.rem_euclid(w as i32);
        let y = y.clamp(0, h as i32 - 1);
        let (u, v) = ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
        let (th, ph) = (u * TAU, v * PI);
        Vec3::new(ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin())
    };
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mut color = vec![255u8; (w * h * 4) as usize];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (c, ht) = surface(kind, dir(x, y));
            let i = (y as u32 * w + x as u32) as usize;
            heights[i] = ht;
            for k in 0..3 {
                color[i * 4 + k] = c[k].clamp(0.0, 255.0) as u8;
            }
        }
    }
    let strength = if kind == 1 { 0.6 } else { 5.0 };
    let mut normal = vec![255u8; (w * h * 4) as usize];
    let at = |x: i32, y: i32| heights[(y.clamp(0, h as i32 - 1) as u32 * w + x.rem_euclid(w as i32) as u32) as usize];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let dx = (at(x + 1, y) - at(x - 1, y)) * strength;
            let dy = (at(x, y + 1) - at(x, y - 1)) * strength;
            let n = Vec3::new(-dx, -dy, 1.0).normalize();
            let i = (y as u32 * w + x as u32) as usize;
            normal[i * 4] = ((n.x * 0.5 + 0.5) * 255.0) as u8;
            normal[i * 4 + 1] = ((n.y * 0.5 + 0.5) * 255.0) as u8;
            normal[i * 4 + 2] = ((n.z * 0.5 + 0.5) * 255.0) as u8;
        }
    }
    PlanetTex { w, h, color, normal }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textures_have_expected_size_and_variation() {
        for k in 0..5 {
            let t = generate(k, 64, 32);
            assert_eq!(t.color.len(), 64 * 32 * 4);
            assert_eq!(t.normal.len(), 64 * 32 * 4);
            let (mn, mx) = t.color.chunks(4).fold((255u8, 0u8), |(a, b), p| (a.min(p[0]), b.max(p[0])));
            assert!(mx - mn > 20, "kind {k} is flat");
        }
    }

    #[test]
    fn wraps_without_a_seam() {
        let t = generate(0, 64, 32);
        let px = |x: usize, y: usize| t.color[(y * 64 + x) * 4] as i32;
        // neighbouring columns across the wrap differ about as much as any other pair
        let seam: i32 = (0..32).map(|y| (px(0, y) - px(63, y)).abs()).sum();
        let mid: i32 = (0..32).map(|y| (px(31, y) - px(32, y)).abs()).sum();
        assert!(seam < mid * 3 + 64);
    }
}
