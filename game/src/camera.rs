//! Bird's-eye board camera: orbit, pan, zoom, quarter-turns, glide-to-point,
//! all eased toward goals so nothing snaps.

use dust_engine::camera::OrbitCamera;
use dust_engine::glam::{Mat4, Vec3, Vec4};

pub const MIN_PITCH: f32 = 0.45; // ~25 deg
pub const MAX_PITCH: f32 = 1.45; // ~83 deg
pub const MIN_DIST: f32 = 5.0;
pub const MAX_DIST: f32 = 80.0;
pub const PAN_LIMIT: f32 = 13.0;

pub struct BoardCamera {
    pub cam: OrbitCamera,
    goal_target: Vec3,
    goal_yaw: f32,
    base_yaw: f32,
    t: f32,
    goal_pitch: f32,
    goal_dist: f32,
    /// Overview distance that fits this window; updated as the aspect changes.
    pub overview_dist: f32,
    /// True while the camera is in overview, so a resize re-fits the board.
    auto_fit: bool,
}

const OVERVIEW_PITCH: f32 = 1.05; // 55 deg
const OVERVIEW_DIST: f32 = 30.0;
/// Half the board's width and its foreshortened depth. The overview distance
/// fits both, so a narrow window sees the whole table instead of a crop.
const BOARD_HALF_W: f32 = 16.0;
const BOARD_HALF_D: f32 = 12.6;
/// The overview looks a little below the hub: the board (Court row on top,
/// power track along the bottom) is taller than it is centred on the hub.
const OVERVIEW_TARGET: Vec3 = Vec3::new(0.0, 0.0, 0.9);

impl BoardCamera {
    pub fn new() -> Self {
        let mut cam = OrbitCamera::new(OVERVIEW_TARGET, OVERVIEW_DIST);
        cam.pitch = OVERVIEW_PITCH;
        cam.yaw = 0.0;
        cam.min_distance = MIN_DIST;
        cam.max_distance = MAX_DIST;
        cam.fov_y_radians = 40f32.to_radians();
        cam.near = 0.5;
        cam.far = 420.0;
        Self {
            goal_target: OVERVIEW_TARGET,
            goal_yaw: 0.0,
            base_yaw: 0.0,
            t: 0.0,
            goal_pitch: OVERVIEW_PITCH,
            goal_dist: OVERVIEW_DIST,
            overview_dist: OVERVIEW_DIST,
            auto_fit: true,
            cam,
        }
    }

    pub fn overview(&mut self) {
        self.goal_target = OVERVIEW_TARGET;
        self.goal_pitch = OVERVIEW_PITCH;
        self.auto_fit = true;
        self.goal_dist = self.overview_dist;
    }

    /// Pulls the overview back until the whole board fits this aspect. Only the
    /// overview follows the window; a focused or zoomed camera is left alone.
    pub fn fit(&mut self, aspect: f32) {
        let tan_half = (self.cam.fov_y_radians * 0.5).tan();
        let aspect = aspect.max(0.2);
        let fit = (BOARD_HALF_W / (tan_half * aspect)).max(BOARD_HALF_D / tan_half);
        self.overview_dist = fit.clamp(20.0, MAX_DIST);
        if self.auto_fit {
            self.goal_dist = self.overview_dist;
        }
    }

    pub fn focus(&mut self, p: Vec3) {
        self.goal_target = Self::clamp_target(Vec3::new(p.x, 0.0, p.z));
        self.goal_dist = self.goal_dist.min(11.0);
        self.auto_fit = false;
    }

    /// Cinematic glide: eased move to `p` at distance `d`, optionally with a new pitch.
    pub fn glide_to(&mut self, p: Vec3, d: f32, pitch: f32) {
        self.goal_target = Self::clamp_target(Vec3::new(p.x, 0.0, p.z));
        self.goal_dist = d.clamp(MIN_DIST, MAX_DIST);
        self.goal_pitch = pitch.clamp(MIN_PITCH, MAX_PITCH);
        self.auto_fit = false;
    }

    pub fn set_distance(&mut self, d: f32) {
        self.goal_dist = d.clamp(MIN_DIST, MAX_DIST);
        self.auto_fit = false;
    }

    pub fn orbit(&mut self, dx_px: f32, dy_px: f32) {
        self.goal_yaw -= dx_px * 0.006;
        self.goal_pitch = (self.goal_pitch + dy_px * 0.005).clamp(MIN_PITCH, MAX_PITCH);
    }

    pub fn quarter_turn(&mut self, dir: f32) {
        self.goal_yaw += dir * std::f32::consts::FRAC_PI_2;
    }

    pub fn zoom(&mut self, wheel: f32) {
        self.goal_dist = (self.goal_dist * (1.0 - wheel * 0.09)).clamp(MIN_DIST, MAX_DIST);
        self.auto_fit = false;
    }

    /// `right`/`forward` in -1..1 (keys) or world units scaled by distance (drag).
    pub fn pan(&mut self, right: f32, forward: f32) {
        let yaw = self.goal_yaw;
        let r = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let f = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        self.goal_target = Self::clamp_target(self.goal_target + r * right + f * forward);
    }

    pub fn drag_pan(&mut self, dx_px: f32, dy_px: f32) {
        let k = self.goal_dist * 0.0016;
        self.pan(-dx_px * k, dy_px * k);
    }

    fn clamp_target(t: Vec3) -> Vec3 {
        Vec3::new(t.x.clamp(-PAN_LIMIT, PAN_LIMIT), 0.0, t.z.clamp(-PAN_LIMIT, PAN_LIMIT))
    }

    pub fn update(&mut self, dt: f32) {
        let k = 1.0 - (-9.0 * dt).exp();
        self.cam.target += (self.goal_target - self.cam.target) * k;
        self.base_yaw += (self.goal_yaw - self.base_yaw) * k;
        self.t += dt;
        // a slow idle drift so the board never feels frozen
        self.cam.yaw = self.base_yaw + (self.t * 0.15).sin() * 0.03;
        self.cam.pitch += (self.goal_pitch - self.cam.pitch) * k;
        self.cam.distance += (self.goal_dist - self.cam.distance) * k;
    }

    /// World-space ray through a window pixel.
    pub fn ray(&self, px: (f32, f32), window: (f32, f32), aspect: f32) -> (Vec3, Vec3) {
        let ndc = ((px.0 / window.0) * 2.0 - 1.0, 1.0 - (px.1 / window.1) * 2.0);
        let inv = (self.cam.projection_matrix(aspect) * self.cam.view_matrix()).inverse();
        let unproject = |z: f32| -> Vec3 {
            let v: Vec4 = inv * Vec4::new(ndc.0, ndc.1, z, 1.0);
            v.truncate() / v.w
        };
        let (near, far) = (unproject(-1.0), unproject(1.0));
        (near, (far - near).normalize())
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.cam.projection_matrix(aspect) * self.cam.view_matrix()
    }
}

/// Intersection of a ray with the horizontal plane at height `y`.
pub fn ray_plane(origin: Vec3, dir: Vec3, y: f32) -> Option<Vec3> {
    if dir.y.abs() < 1e-5 {
        return None;
    }
    let t = (y - origin.y) / dir.y;
    (t > 0.0).then(|| origin + dir * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_and_pitch_are_clamped() {
        let mut c = BoardCamera::new();
        for _ in 0..200 {
            c.zoom(5.0);
            c.orbit(0.0, 500.0);
        }
        c.update(10.0);
        assert!(c.cam.distance >= MIN_DIST - 1e-3 && c.cam.pitch <= MAX_PITCH + 1e-3);
        for _ in 0..200 {
            c.zoom(-5.0);
            c.orbit(0.0, -500.0);
        }
        c.update(10.0);
        assert!(c.cam.distance <= MAX_DIST + 1e-3 && c.cam.pitch >= MIN_PITCH - 1e-3);
    }

    #[test]
    fn pan_stays_on_board() {
        let mut c = BoardCamera::new();
        for _ in 0..100 {
            c.pan(5.0, 5.0);
        }
        c.update(10.0);
        assert!(c.cam.target.x.abs() <= PAN_LIMIT + 1e-3 && c.cam.target.z.abs() <= PAN_LIMIT + 1e-3);
    }

    #[test]
    fn centre_pixel_ray_hits_target() {
        let mut c = BoardCamera::new();
        c.update(10.0);
        let (o, d) = c.ray((400.0, 300.0), (800.0, 600.0), 800.0 / 600.0);
        let hit = ray_plane(o, d, 0.0).unwrap();
        assert!((hit - OVERVIEW_TARGET).length() < 0.05, "{hit:?}");
    }

    #[test]
    fn overview_fits_narrow_windows() {
        let mut c = BoardCamera::new();
        c.fit(1280.0 / 720.0);
        let wide = c.overview_dist;
        c.fit(720.0 / 936.0);
        assert!(c.overview_dist > wide * 1.6, "{wide} -> {}", c.overview_dist);
        c.update(10.0);
        assert!((c.cam.distance - c.overview_dist).abs() < 0.05);
    }

    #[test]
    fn quarter_turn_rotates_pan_axes() {
        let mut c = BoardCamera::new();
        c.quarter_turn(1.0);
        c.pan(0.0, 1.0); // "forward" after a quarter turn is along world -X
        c.update(10.0);
        assert!(c.cam.target.x < -0.9 && (c.cam.target.z - OVERVIEW_TARGET.z).abs() < 0.1, "{:?}", c.cam.target);
    }
}
