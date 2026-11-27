//! Procedural models: every piece is built from a handful of primitives and
//! baked into one mesh per material slot (team colour, hull metal, glow), so
//! the look is reproducible from code and the game needs no asset files.

use dust_engine::glam::{Mat3, Mat4, Quat, Vec3};
use dust_engine::mesh::{primitives, MeshData, Vertex};
use std::f32::consts::{PI, TAU};

#[derive(Default)]
pub struct MeshBuilder {
    verts: Vec<Vertex>,
    idx: Vec<u32>,
}

impl MeshBuilder {
    pub fn add(&mut self, m: &MeshData, xf: Mat4) -> &mut Self {
        let nm = Mat3::from_mat4(xf).inverse().transpose();
        let base = self.verts.len() as u32;
        for v in &m.vertices {
            let p = xf.transform_point3(Vec3::from(v.position));
            let n = (nm * Vec3::from(v.normal)).normalize_or_zero();
            self.verts.push(Vertex { position: p.to_array(), normal: n.to_array(), uv: v.uv, color: v.color });
        }
        self.idx.extend(m.indices.iter().map(|i| i + base));
        self
    }
    pub fn finish(self) -> MeshData {
        MeshData { vertices: self.verts, indices: self.idx }
    }
    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }
}

/// A cone or cylinder along +Y from y=0 to y=h with capped ends.
pub fn frustum(segs: u32, r0: f32, r1: f32, h: f32) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let slope = (r0 - r1) / h;
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * TAU;
        let (s, c) = a.sin_cos();
        let n = Vec3::new(c, slope, s).normalize();
        let u = i as f32 / segs as f32;
        vertices.push(Vertex { position: [c * r0, 0.0, s * r0], normal: n.to_array(), uv: [u, 0.0], color: [0.0; 3] });
        vertices.push(Vertex { position: [c * r1, h, s * r1], normal: n.to_array(), uv: [u, 1.0], color: [0.0; 3] });
    }
    for i in 0..segs {
        let b = i * 2;
        indices.extend_from_slice(&[b, b + 1, b + 2, b + 1, b + 3, b + 2]);
    }
    for (y, r, ny) in [(0.0, r0, -1.0f32), (h, r1, 1.0)] {
        if r <= 0.0 {
            continue;
        }
        let center = vertices.len() as u32;
        vertices.push(Vertex { position: [0.0, y, 0.0], normal: [0.0, ny, 0.0], uv: [0.5, 0.5], color: [0.0; 3] });
        for i in 0..=segs {
            let a = i as f32 / segs as f32 * TAU;
            let (s, c) = a.sin_cos();
            vertices.push(Vertex { position: [c * r, y, s * r], normal: [0.0, ny, 0.0], uv: [0.5 + c * 0.5, 0.5 + s * 0.5], color: [0.0; 3] });
        }
        for i in 0..segs {
            let (a, b) = (center + 1 + i, center + 2 + i);
            if ny > 0.0 {
                indices.extend_from_slice(&[center, b, a]);
            } else {
                indices.extend_from_slice(&[center, a, b]);
            }
        }
    }
    MeshData { vertices, indices }
}

/// A torus in the XZ plane.
pub fn torus(major: f32, minor: f32, segs: u32, rings: u32) -> MeshData {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * TAU;
        let (sa, ca) = a.sin_cos();
        for j in 0..=rings {
            let b = j as f32 / rings as f32 * TAU;
            let (sb, cb) = b.sin_cos();
            let n = Vec3::new(ca * cb, sb, sa * cb);
            let p = Vec3::new(ca * major, 0.0, sa * major) + n * minor;
            vertices.push(Vertex { position: p.to_array(), normal: n.to_array(), uv: [i as f32 / segs as f32, j as f32 / rings as f32], color: [0.0; 3] });
        }
    }
    let stride = rings + 1;
    for i in 0..segs {
        for j in 0..rings {
            let a = i * stride + j;
            let b = a + stride;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    MeshData { vertices, indices }
}

fn t(p: Vec3, r: Quat, s: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(s, r, p)
}

/// Which material a part gets when drawn.
pub struct Model {
    pub team: MeshData,
    pub hull: MeshData,
    pub glow: MeshData,
}

/// Six-sided parts read as built, not turned; the hex is the faction's shape.
const HEX: u32 = 6;

/// A light frigate pointing along +X. The silhouette is a long diamond-section
/// hull with a forward-swept delta and two engine pods, so it reads as a ship
/// (not a dart) at overview distance. Team colour covers the hull and wings,
/// bare metal the pods, spine and cockpit; only the engine mouths glow.
pub fn ship() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    let along_x = Quat::from_rotation_z(-PI / 2.0);
    // Hull: four segments give a keel and a ridge; flattened so it sits low.
    team.add(&frustum(4, 0.13, 0.018, 0.66), t(Vec3::new(-0.3, 0.0, 0.0), along_x * Quat::from_rotation_y(PI / 4.0), Vec3::new(1.0, 1.0, 0.62)));
    // Delta wings: one plate each side, swept back from mid-hull.
    for side in [-1.0f32, 1.0] {
        team.add(&primitives::cube(), t(Vec3::new(-0.16, -0.015, side * 0.15), Quat::from_rotation_y(side * 0.42), Vec3::new(0.34, 0.022, 0.2)));
        // Engine pods at the wing roots, bare metal.
        hull.add(&frustum(10, 0.045, 0.038, 0.2), t(Vec3::new(-0.42, 0.0, side * 0.11), along_x, Vec3::ONE));
        team.add(&frustum(10, 0.038, 0.0, 0.07), t(Vec3::new(-0.22, 0.0, side * 0.11), along_x, Vec3::ONE));
        glow.add(&frustum(10, 0.034, 0.034, 0.012), t(Vec3::new(-0.43, 0.0, side * 0.11), along_x, Vec3::ONE));
        // Wingtip fins, angled out.
        hull.add(&primitives::cube(), t(Vec3::new(-0.27, 0.035, side * 0.3), Quat::from_rotation_x(side * 0.5), Vec3::new(0.14, 0.08, 0.012)));
    }
    // Dorsal spine and cockpit canopy.
    hull.add(&primitives::cube(), t(Vec3::new(-0.12, 0.05, 0.0), Quat::IDENTITY, Vec3::new(0.34, 0.035, 0.03)));
    hull.add(&primitives::uv_sphere(12, 8), t(Vec3::new(0.08, 0.035, 0.0), Quat::IDENTITY, Vec3::new(0.11, 0.045, 0.05)));
    Model { team: team.finish(), hull: hull.finish(), glow: glow.finish() }
}

/// An arcology on a hex plinth: a stepped central tower with a metal spire and
/// three lower towers around it. Team colour is on every tower so ownership
/// reads from above; windows are thin lit bands, not dots, so the glow stays
/// a texture of the building and never a halo. Footprint radius 0.44, so the
/// two building slots of a system no longer overlap.
pub fn city() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    let rot = Quat::from_rotation_y(PI / 6.0);
    hull.add(&frustum(HEX, 0.44, 0.4, 0.07), t(Vec3::ZERO, rot, Vec3::ONE));
    team.add(&frustum(HEX, 0.37, 0.34, 0.05), t(Vec3::new(0.0, 0.07, 0.0), rot, Vec3::ONE));
    let base = 0.12;
    // Central tower: three set-back tiers and a spire.
    let tiers = [(0.19, 0.16, 0.26), (0.14, 0.115, 0.2), (0.095, 0.075, 0.16)];
    let mut y = base;
    for (i, (r0, r1, h)) in tiers.into_iter().enumerate() {
        team.add(&frustum(HEX, r0, r1, h), t(Vec3::new(0.0, y, 0.0), rot, Vec3::ONE));
        // A metal ledge at each setback.
        hull.add(&frustum(HEX, r1 + 0.018, r1 + 0.012, 0.018), t(Vec3::new(0.0, y + h, 0.0), rot, Vec3::ONE));
        for b in 0..2 {
            let yy = y + h * (0.3 + 0.35 * b as f32);
            let rr = r0 + (r1 - r0) * (0.3 + 0.35 * b as f32) + 0.003;
            if i < 2 || b == 0 {
                glow.add(&frustum(HEX, rr, rr, 0.008), t(Vec3::new(0.0, yy, 0.0), rot, Vec3::ONE));
            }
        }
        y += h + 0.018;
    }
    hull.add(&frustum(HEX, 0.035, 0.0, 0.3), t(Vec3::new(0.0, y, 0.0), rot, Vec3::ONE));
    glow.add(&primitives::uv_sphere(10, 6), t(Vec3::new(0.0, y + 0.31, 0.0), Quat::IDENTITY, Vec3::splat(0.022)));
    // Three outer towers on the plinth, different heights.
    for (k, h) in [0.3f32, 0.22, 0.17].into_iter().enumerate() {
        let a = k as f32 / 3.0 * TAU + 0.5;
        let p = Vec3::new(a.cos() * 0.25, base, a.sin() * 0.25);
        team.add(&frustum(HEX, 0.075, 0.06, h), t(p, rot, Vec3::ONE));
        hull.add(&frustum(HEX, 0.068, 0.02, 0.05), t(p + Vec3::Y * h, rot, Vec3::ONE));
        glow.add(&frustum(HEX, 0.071, 0.071, 0.007), t(p + Vec3::Y * (h * 0.62), rot, Vec3::ONE));
        // A skybridge to the central tower.
        let mid = p * 0.55 + Vec3::Y * (base + h * 0.55);
        hull.add(&primitives::cube(), t(Vec3::new(mid.x, mid.y, mid.z), Quat::from_rotation_y(-a), Vec3::new(0.14, 0.02, 0.03)));
    }
    Model { team: team.finish(), hull: hull.finish(), glow: glow.finish() }
}

/// A starport: hex landing pad in team colour, three metal pylons and a
/// raised docking ring. From above it reads as an open ring, which no other
/// piece has, so a port is told apart from a city at a glance.
pub fn starport() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    let rot = Quat::from_rotation_y(PI / 6.0);
    hull.add(&frustum(HEX, 0.42, 0.39, 0.06), t(Vec3::ZERO, rot, Vec3::ONE));
    team.add(&frustum(HEX, 0.33, 0.31, 0.03), t(Vec3::new(0.0, 0.06, 0.0), rot, Vec3::ONE));
    // Landing marks: a small metal hex at the centre.
    hull.add(&frustum(HEX, 0.1, 0.09, 0.012), t(Vec3::new(0.0, 0.09, 0.0), rot, Vec3::ONE));
    let ring_y = 0.4;
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + PI / 2.0;
        let p = Vec3::new(a.cos() * 0.31, 0.06, a.sin() * 0.31);
        hull.add(&frustum(8, 0.04, 0.026, ring_y - 0.02), t(p, Quat::IDENTITY, Vec3::ONE));
        // Clamp where the pylon meets the ring.
        team.add(&primitives::cube(), t(p + Vec3::Y * (ring_y - 0.06), Quat::from_rotation_y(-a), Vec3::new(0.05, 0.08, 0.07)));
        glow.add(&primitives::uv_sphere(8, 6), t(p + Vec3::Y * (ring_y + 0.05), Quat::IDENTITY, Vec3::splat(0.018)));
    }
    hull.add(&torus(0.31, 0.03, 40, 8), t(Vec3::new(0.0, ring_y, 0.0), Quat::IDENTITY, Vec3::ONE));
    team.add(&torus(0.31, 0.012, 40, 6), t(Vec3::new(0.0, ring_y + 0.032, 0.0), Quat::IDENTITY, Vec3::ONE));
    Model { team: team.finish(), hull: hull.finish(), glow: glow.finish() }
}

/// The platform under each star system: a plate with a rim and decorations.
pub fn platform() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    hull.add(&frustum(48, 1.4, 1.3, 0.1), Mat4::IDENTITY);
    hull.add(&frustum(48, 1.25, 1.25, 0.12), Mat4::IDENTITY);
    glow.add(&torus(1.33, 0.025, 64, 8), t(Vec3::new(0.0, 0.1, 0.0), Quat::IDENTITY, Vec3::ONE));
    // four notch markers
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        team.add(&primitives::cube(), t(Vec3::new(a.cos() * 1.38, 0.06, a.sin() * 1.38), Quat::from_rotation_y(-a), Vec3::new(0.12, 0.05, 0.05)));
    }
    Model { team: team.finish(), hull: hull.finish(), glow: glow.finish() }
}

/// A selection / hover ring.
pub fn ring() -> MeshData {
    torus(1.5, 0.045, 64, 8)
}

pub fn thin_bar() -> MeshData {
    primitives::cube()
}

/// A flat annular sector on y=0 (normal up): radii r0..r1, angles a0..a1 (radians).
pub fn sector(r0: f32, r1: f32, a0: f32, a1: f32) -> MeshData {
    let n = 12u32;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let (s, c) = a.sin_cos();
        for r in [r0, r1] {
            vertices.push(Vertex { position: [c * r, 0.0, s * r], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color: [0.0; 3] });
        }
    }
    for i in 0..n {
        let k = i * 2;
        // wound so the upward face is front-facing
        indices.extend_from_slice(&[k, k + 2, k + 1, k + 1, k + 2, k + 3]);
    }
    MeshData { vertices, indices }
}
