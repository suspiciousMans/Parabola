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

/// A small fighter pointing along +X: swept wings, tail fin, glowing engine.
pub fn ship() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    let along_x = Quat::from_rotation_z(-PI / 2.0);
    // fuselage: tapered cone, wide at the tail
    team.add(&frustum(16, 0.11, 0.025, 0.62), t(Vec3::new(-0.3, 0.0, 0.0), along_x, Vec3::ONE));
    // wings, swept back
    for side in [-1.0f32, 1.0] {
        let yaw = Quat::from_rotation_y(side * 0.55);
        team.add(&primitives::cube(), t(Vec3::new(-0.12, -0.01, side * 0.17), yaw, Vec3::new(0.3, 0.018, 0.22)));
        hull.add(&primitives::cube(), t(Vec3::new(-0.2, -0.0, side * 0.30), yaw, Vec3::new(0.12, 0.03, 0.04)));
    }
    // tail fin
    team.add(&primitives::cube(), t(Vec3::new(-0.25, 0.09, 0.0), Quat::from_rotation_z(0.3), Vec3::new(0.18, 0.14, 0.018)));
    // cockpit
    hull.add(&primitives::uv_sphere(12, 8), t(Vec3::new(0.0, 0.07, 0.0), Quat::IDENTITY, Vec3::new(0.16, 0.08, 0.08)));
    // engine nozzle and glow
    hull.add(&frustum(12, 0.07, 0.09, 0.07), t(Vec3::new(-0.38, 0.0, 0.0), along_x, Vec3::ONE));
    glow.add(&primitives::uv_sphere(12, 8), t(Vec3::new(-0.43, 0.0, 0.0), Quat::IDENTITY, Vec3::new(0.09, 0.06, 0.06)));
    Model { team: team.finish(), hull: hull.finish(), glow: glow.finish() }
}

/// A domed city: round base, a cluster of towers with lit windows, a crown.
pub fn city() -> Model {
    let (mut team, mut hull, mut glow) = (MeshBuilder::default(), MeshBuilder::default(), MeshBuilder::default());
    hull.add(&frustum(28, 0.62, 0.55, 0.09), Mat4::IDENTITY);
    team.add(&torus(0.58, 0.035, 28, 8), t(Vec3::new(0.0, 0.1, 0.0), Quat::IDENTITY, Vec3::ONE));
    let towers = [
        (0.0, 0.0, 0.17, 0.62),
        (0.30, 0.05, 0.11, 0.38),
        (-0.22, 0.22, 0.12, 0.44),
        (-0.14, -0.28, 0.10, 0.30),
        (0.18, -0.26, 0.09, 0.26),
    ];
    for (x, z, r, h) in towers {
        team.add(&frustum(14, r, r * 0.72, h), t(Vec3::new(x, 0.09, z), Quat::IDENTITY, Vec3::ONE));
        hull.add(&frustum(14, r * 0.74, 0.0, h * 0.18), t(Vec3::new(x, 0.09 + h, z), Quat::IDENTITY, Vec3::ONE));
        // window rings
        let rows = (h / 0.09) as i32;
        for row in 1..rows {
            let y = 0.09 + row as f32 * 0.085;
            let rr = r - (r * 0.28) * (y - 0.09) / h + 0.004;
            for k in 0..6 {
                let a = (k as f32 + row as f32 * 0.5) / 6.0 * TAU;
                glow.add(
                    &primitives::cube(),
                    t(Vec3::new(x + a.cos() * rr, y, z + a.sin() * rr), Quat::from_rotation_y(-a), Vec3::new(0.01, 0.02, 0.022)),
                );
            }
        }
    }
    glow.add(&primitives::uv_sphere(12, 8), t(Vec3::new(0.0, 0.09 + 0.62 + 0.11, 0.0), Quat::IDENTITY, Vec3::splat(0.05)));
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
