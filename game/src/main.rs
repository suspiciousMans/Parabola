//! Ember Reach: a bird's-eye 3D tabletop game on the Dust engine, rules in
//! Oxidized (see ../rules). Hot-seat or against bots; movable camera.
//!
//! Run: `cargo run -p ember_reach --release`
//!   --autoplay          start a 3-bot game and let it play itself
//!   --shot out.png      with --autoplay: save a screenshot mid-game and exit
//!   --after N           ...after N actions (default 40)

mod camera;
mod models;
mod ui;

use camera::{ray_plane, BoardCamera};
use dust_engine::app::{App, Context, Game};
use dust_engine::glam::{Mat4, Quat, Vec3};
use dust_engine::glow::{self, HasContext};
use dust_engine::mesh::{primitives, GpuMesh};
use dust_engine::renderer::{
    patch_mesh_shaders, PostParams, Renderer, ShadowBounds, ShadowSettings, TargetOptions,
};
use dust_engine::sdl2::event::Event;
use dust_engine::sdl2::keyboard::Keycode;
use dust_engine::sdl2::mouse::MouseButton;
use dust_engine::shader::builtin::{PbrMaterial, PointLight, SceneLighting};
use dust_engine::shader::{ShaderVariantCache, PBR_MESH_FRAG, PBR_MESH_VERT, TONEMAP_COMPOSITE_FRAG};
use dust_engine::texture::{GpuTexture, TextureFilter};
use dust_engine::ui::EguiState;
use reach_rules::{Action, Event as GameEvent, Phase, State, SYSTEMS};
use std::collections::VecDeque;
use std::f32::consts::TAU;

const RING_R: f32 = 5.3;

fn system_pos(i: usize) -> Vec3 {
    if i == 0 {
        return Vec3::ZERO;
    }
    let a = (i as f32 - 1.0) / 6.0 * TAU + 0.5236; // flat edge toward the camera
    Vec3::new(a.cos() * RING_R, 0.0, a.sin() * RING_R)
}

fn adjacent(a: usize, b: usize) -> bool {
    if a == b {
        return false;
    }
    if a == 0 || b == 0 {
        return true;
    }
    let d = (a + 6 - b) % 6;
    d == 1 || d == 5
}

/// sRGB-ish byte colour to linear for the PBR shader.
fn lin(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| (v as f32 / 255.0).powf(2.2))
}

/// Resting slot of ship `i` of `n` belonging to `player` around a system centre.
fn ship_slot(system: usize, player: usize, i: usize, n: usize) -> (Vec3, f32) {
    // Three lanes at slightly different radii, filled left to right.
    let sector = player as f32 / 4.0 * TAU + 0.4;
    let lane = i % 3;
    let col = i / 3;
    let cols = n.div_ceil(3);
    let a = sector + (col as f32 - (cols as f32 - 1.0) / 2.0) * 0.40 + lane as f32 * 0.13;
    let r = 0.94 + lane as f32 * 0.11;
    let p = system_pos(system) + Vec3::new(a.cos() * r, 0.13, a.sin() * r);
    (p, -a - std::f32::consts::FRAC_PI_2) // heading along the ring
}

struct Item {
    mesh: usize,
    model: Mat4,
    material: PbrMaterial,
    casts: bool,
}

struct Slots {
    team: usize,
    hull: usize,
    glow: usize,
}

enum Fx {
    Fly { from: usize, to: usize, n: usize, player: usize },
    Battle { system: usize },
    Rise { system: usize },
    None,
}

struct Anim {
    fx: Fx,
    t: f32,
    dur: f32,
    after: State,
    apply_at_end: bool,
}

enum Screen {
    Menu,
    Play,
}

struct Ember {
    renderer: Option<Renderer>,
    shaders: Option<ShaderVariantCache>,
    ui: Option<EguiState>,
    meshes: Vec<GpuMesh>,
    white: Option<GpuTexture>,
    statics: Vec<Item>,
    ship: Slots,
    city: Slots,
    ring_mesh: usize,
    bar_mesh: usize,
    sphere_mesh: usize,
    camera: BoardCamera,
    lighting: SceneLighting,

    screen: Screen,
    menu: ui::MenuState,
    bots: [bool; 4],
    truth: State,
    view: State,
    queue: VecDeque<(GameEvent, State, Action)>,
    anim: Option<Anim>,
    log: Vec<String>,
    toast: Option<(String, f32)>,
    selected: Option<usize>,
    hover_systems: Vec<usize>,
    hover_pick: Option<usize>,
    bot_timer: f32,
    handoff_seen: Option<usize>,
    clock: f32,
    city_rise: [f32; SYSTEMS],

    // input
    orbiting: bool,
    panning: bool,
    ui_wants_pointer: bool,

    // scripted run / screenshots
    autoplay: bool,
    actions_taken: usize,
    shot: Option<String>,
    shot_after: usize,
    frame: u32,
    shot_pending: u32,
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

impl Ember {
    fn new() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let autoplay = args.iter().any(|a| a == "--autoplay");
        let state = State::new(3, 1);
        let mut lighting = SceneLighting::default();
        lighting.sun_dir = [-0.35, 1.0, 0.45];
        lighting.sun_color = [3.0, 2.75, 2.4];
        lighting.sky_color = [0.16, 0.2, 0.32];
        lighting.ground_color = [0.05, 0.05, 0.07];
        lighting.point_lights.push(PointLight { position: [0.0, 1.6, 0.0], color: [1.0, 0.65, 0.35], intensity: 5.0, range: 7.0 });
        let mut me = Self {
            renderer: None,
            shaders: None,
            ui: None,
            meshes: Vec::new(),
            white: None,
            statics: Vec::new(),
            ship: Slots { team: 0, hull: 0, glow: 0 },
            city: Slots { team: 0, hull: 0, glow: 0 },
            ring_mesh: 0,
            bar_mesh: 0,
            sphere_mesh: 0,
            camera: BoardCamera::new(),
            lighting,
            screen: Screen::Menu,
            menu: ui::MenuState::default(),
            bots: [false, true, true, true],
            truth: state.clone(),
            view: state,
            queue: VecDeque::new(),
            anim: None,
            log: Vec::new(),
            toast: None,
            selected: None,
            hover_systems: Vec::new(),
            hover_pick: None,
            bot_timer: 0.0,
            handoff_seen: None,
            clock: 0.0,
            city_rise: [-10.0; SYSTEMS],
            orbiting: false,
            panning: false,
            ui_wants_pointer: false,
            autoplay,
            actions_taken: 0,
            shot: arg_value(&args, "--shot"),
            shot_after: arg_value(&args, "--after").and_then(|v| v.parse().ok()).unwrap_or(40),
            frame: 0,
            shot_pending: 0,
        };
        if autoplay {
            me.menu.players = 3;
            me.menu.bots = [true; 4];
            me.start_game(7);
        }
        if let Some(c) = arg_value(&args, "--focus").and_then(|v| v.parse::<usize>().ok()) {
            me.camera.focus(system_pos(c));
            me.camera.set_distance(7.0);
        }
        me
    }

    fn start_game(&mut self, seed: u64) {
        self.bots = self.menu.bots;
        let s = State::new(self.menu.players, seed);
        self.truth = s.clone();
        self.view = s;
        self.queue.clear();
        self.anim = None;
        self.log.clear();
        self.selected = None;
        self.handoff_seen = None;
        self.city_rise = [-10.0; SYSTEMS];
        self.actions_taken = 0;
        self.screen = Screen::Play;
        self.camera.overview();
        // Cities present at the start have already risen.
        self.bot_timer = 0.6;
    }

    fn upload(&mut self, gl: &glow::Context, m: &dust_engine::mesh::MeshData) -> anyhow::Result<usize> {
        self.meshes.push(GpuMesh::upload(gl, m)?);
        Ok(self.meshes.len() - 1)
    }

    fn upload_model(&mut self, gl: &glow::Context, m: &models::Model) -> anyhow::Result<Slots> {
        Ok(Slots { team: self.upload(gl, &m.team)?, hull: self.upload(gl, &m.hull)?, glow: self.upload(gl, &m.glow)? })
    }

    fn build_scene(&mut self, gl: &glow::Context) -> anyhow::Result<()> {
        let cube = self.upload(gl, &primitives::cube())?;
        let cyl = self.upload(gl, &primitives::cylinder(48))?;
        let sphere = self.upload(gl, &primitives::uv_sphere(24, 12))?;
        self.sphere_mesh = sphere;
        self.ring_mesh = self.upload(gl, &models::ring())?;
        self.bar_mesh = cube;
        self.ship = self.upload_model(gl, &models::ship())?;
        self.city = self.upload_model(gl, &models::city())?;
        let platform = self.upload_model(gl, &models::platform())?;

        let tr = |p: Vec3, r: Quat, s: Vec3| Mat4::from_scale_rotation_translation(s, r, p);
        let mut st: Vec<Item> = Vec::new();
        // table and board
        st.push(Item { mesh: cube, model: tr(Vec3::new(0.0, -0.45, 0.0), Quat::IDENTITY, Vec3::new(40.0, 0.7, 34.0)), material: PbrMaterial { roughness: 0.55, ..PbrMaterial::plastic(lin([60, 38, 26])) }, casts: false });
        st.push(Item { mesh: cube, model: tr(Vec3::new(0.0, -0.12, 0.0), Quat::IDENTITY, Vec3::new(17.5, 0.3, 15.5)), material: PbrMaterial { roughness: 0.35, metallic: 0.6, ..PbrMaterial::plastic(lin([26, 28, 34])) }, casts: true });
        st.push(Item { mesh: cube, model: tr(Vec3::new(0.0, 0.0, 0.0), Quat::IDENTITY, Vec3::new(16.4, 0.1, 14.4)), material: PbrMaterial { roughness: 0.9, ..PbrMaterial::plastic(lin([14, 18, 40])) }, casts: false });
        // nebula glow patches on the board
        for (x, z, r, c) in [(-5.5, 3.0, 2.8, [60u8, 30, 110]), (5.5, -3.2, 3.2, [20, 70, 120]), (0.5, 5.2, 2.0, [110, 40, 70])] {
            st.push(Item { mesh: cyl, model: tr(Vec3::new(x, 0.052, z), Quat::IDENTITY, Vec3::new(r * 2.0, 0.004, r * 2.0)), material: PbrMaterial { roughness: 1.0, emissive: lin(c).map(|v| v * 0.22), ..PbrMaterial::plastic(lin(c).map(|v| v * 0.1)) }, casts: false });
        }
        // lanes
        for a in 0..SYSTEMS {
            for b in (a + 1)..SYSTEMS {
                if !adjacent(a, b) {
                    continue;
                }
                let (pa, pb) = (system_pos(a), system_pos(b));
                let d = pb - pa;
                let len = d.length() - 2.9;
                let mid = (pa + pb) * 0.5 + Vec3::new(0.0, 0.07, 0.0);
                let rot = Quat::from_rotation_y(-d.z.atan2(d.x));
                st.push(Item { mesh: cube, model: tr(mid, rot, Vec3::new(len.max(0.1), 0.012, 0.08)), material: PbrMaterial { emissive: [0.18, 0.5, 0.9], ..PbrMaterial::plastic([0.05, 0.1, 0.2]) }, casts: false });
            }
        }
        // platforms + planets
        let planet_cols: [[u8; 3]; 7] = [[255, 190, 90], [90, 170, 230], [200, 110, 90], [120, 200, 130], [180, 140, 230], [230, 220, 150], [110, 150, 200]];
        for i in 0..SYSTEMS {
            let p = system_pos(i);
            st.push(Item { mesh: platform.hull, model: tr(p + Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Vec3::ONE), material: PbrMaterial::metal(lin([150, 150, 165]), 0.4), casts: true });
            st.push(Item { mesh: platform.team, model: tr(p + Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Vec3::ONE), material: PbrMaterial::metal(lin([200, 160, 90]), 0.35), casts: true });
            st.push(Item { mesh: platform.glow, model: tr(p + Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Vec3::ONE), material: PbrMaterial { emissive: [0.6, 0.4, 0.15], ..PbrMaterial::plastic([0.2, 0.2, 0.2]) }, casts: false });
            let a = i as f32 * 2.1 + 0.3;
            let (pc, off) = (planet_cols[i], Vec3::new(a.cos() * 1.0, 0.0, a.sin() * 1.0));
            let _ = off;
            st.push(Item { mesh: sphere, model: tr(p + Vec3::new(-1.35, 0.62, -1.15), Quat::IDENTITY, Vec3::splat(0.34)), material: PbrMaterial { roughness: 0.7, ..PbrMaterial::plastic(lin(pc)) }, casts: true });
            st.push(Item { mesh: sphere, model: tr(p + Vec3::new(-1.05, 0.38, -1.5), Quat::IDENTITY, Vec3::splat(0.1)), material: PbrMaterial::plastic(lin([200, 200, 205])), casts: true });
        }
        // hub star
        st.push(Item { mesh: sphere, model: tr(Vec3::new(0.0, 1.9, 0.0), Quat::IDENTITY, Vec3::splat(0.38)), material: PbrMaterial { emissive: [6.0, 3.6, 1.4], ..PbrMaterial::plastic([1.0, 0.8, 0.4]) }, casts: false });
        // starfield dome (not shadow casting)
        let mut seed = 12345u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..220 {
            let az = rnd() * TAU;
            let el = 0.05 + rnd() * 1.1;
            let r = 70.0;
            let p = Vec3::new(az.cos() * el.cos() * r, el.sin() * r - 4.0, az.sin() * el.cos() * r);
            let b = 0.6 + rnd() * 2.4;
            st.push(Item { mesh: sphere, model: tr(p, Quat::IDENTITY, Vec3::splat(0.12 + rnd() * 0.2)), material: PbrMaterial { unlit: true, base_color: [b, b, b * 1.1, 1.0], ..PbrMaterial::default() }, casts: false });
        }
        self.statics = st;
        Ok(())
    }

    fn pick_system(&self, ctx: &Context, mouse: (i32, i32)) -> Option<usize> {
        let (ww, wh) = ctx.platform.window.size();
        let (o, d) = self.camera.ray((mouse.0 as f32, mouse.1 as f32), (ww as f32, wh as f32), ctx.aspect_ratio());
        let hit = ray_plane(o, d, 0.2)?;
        (0..SYSTEMS)
            .map(|i| (i, (system_pos(i) - hit).length()))
            .filter(|(_, dist)| *dist < 1.5)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn do_action(&mut self, a: Action) {
        let before = self.truth.clone();
        let next = before.apply(a);
        let ev = next.event();
        self.truth = next.clone();
        self.queue.push_back((ev, next, a));
        self.actions_taken += 1;
        let _ = before;
    }

    fn human_turn(&self) -> bool {
        matches!(self.screen, Screen::Play) && !self.truth.is_over() && !self.bots[self.truth.current()]
    }

    fn start_anim(&mut self, ev: GameEvent, after: State, _a: Action) {
        let mut fx = Fx::None;
        let mut dur = 0.18;
        let mut at_end = false;
        match ev {
            GameEvent::Moved { from, to, ships, player } => {
                fx = Fx::Fly { from: from as usize, to: to as usize, n: ships as usize, player: player as usize };
                dur = 0.55;
                at_end = true;
            }
            GameEvent::Fought { system, .. } => {
                fx = Fx::Battle { system: system as usize };
                dur = 0.8;
                at_end = true;
            }
            GameEvent::Built { system, .. } => {
                fx = Fx::Rise { system: system as usize };
                self.city_rise[system as usize] = self.clock;
                dur = 0.5;
            }
            GameEvent::TrickWon { .. } | GameEvent::RoundScored { .. } => dur = 0.9,
            _ => {}
        }
        if let Some(line) = ui::describe(ev, &ui::NAMES) {
            self.log.push(line.clone());
            if matches!(ev, GameEvent::TrickWon { .. } | GameEvent::RoundScored { .. } | GameEvent::Fought { .. }) {
                self.toast = Some((line, 2.2));
            }
        }
        if !at_end {
            self.view = after.clone();
        }
        self.anim = Some(Anim { fx, t: 0.0, dur, after, apply_at_end: at_end });
    }

    fn draw_items(&self) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::new();
        let tr = |p: Vec3, r: Quat, s: Vec3| Mat4::from_scale_rotation_translation(s, r, p);
        let s = &self.view;
        let n = s.players();
        let flying = match &self.anim {
            Some(Anim { fx: Fx::Fly { n, .. }, .. }) => *n,
            _ => 0,
        };
        let _ = flying;
        let team_mat = |p: usize| PbrMaterial { roughness: 0.32, metallic: 0.55, ..PbrMaterial::plastic(lin(ui::COLORS_UI[p])) };
        let hull_mat = PbrMaterial::metal(lin([170, 175, 190]), 0.3);
        let ship_glow = PbrMaterial { emissive: [2.2, 1.2, 0.5], ..PbrMaterial::plastic([1.0, 0.6, 0.3]) };
        let city_glow = PbrMaterial { emissive: [2.6, 1.5, 0.5], ..PbrMaterial::plastic([1.0, 0.7, 0.3]) };
        let bob = |i: usize| (self.clock * 1.6 + i as f32 * 1.7).sin() * 0.02;

        // cities
        for sys in 0..SYSTEMS {
            if let Some(owner) = s.city(sys) {
                let age = self.clock - self.city_rise[sys];
                let k = if age < 0.6 { (age / 0.6).clamp(0.02, 1.0) } else { 1.0 };
                let k = 1.0 - (1.0 - k).powi(3);
                let m = tr(system_pos(sys) + Vec3::new(0.0, 0.1, 0.0), Quat::IDENTITY, Vec3::new(1.35, 1.35 * k, 1.35));
                out.push(Item { mesh: self.city.team, model: m, material: team_mat(owner), casts: true });
                out.push(Item { mesh: self.city.hull, model: m, material: hull_mat, casts: true });
                out.push(Item { mesh: self.city.glow, model: m, material: city_glow, casts: false });
            }
        }
        // ships at rest
        let mut ship_item = |pos: Vec3, heading: f32, player: usize, scale: f32, bobv: f32, casts: bool| {
            let m = tr(pos + Vec3::new(0.0, bobv, 0.0), Quat::from_rotation_y(heading), Vec3::splat(scale * 1.35));
            out.push(Item { mesh: self.ship.team, model: m, material: team_mat(player), casts });
            out.push(Item { mesh: self.ship.hull, model: m, material: hull_mat, casts });
            out.push(Item { mesh: self.ship.glow, model: m, material: ship_glow, casts: false });
        };
        for sys in 0..SYSTEMS {
            for p in 0..n {
                let cnt = s.ships(sys, p) as usize;
                for i in 0..cnt {
                    let (pos, heading) = ship_slot(sys, p, i, cnt);
                    ship_item(pos, heading, p, 1.0, bob(sys * 8 + p * 3 + i), true);
                }
            }
        }
        // flying ships
        if let Some(Anim { fx: Fx::Fly { from, to, n: fl, player }, t, dur, .. }) = &self.anim {
            let k = (t / dur).clamp(0.0, 1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let src_cnt = s.ships(*from, *player) as usize;
            let dst_cnt = s.ships(*to, *player) as usize + *fl;
            for i in 0..*fl {
                let (a, _) = ship_slot(*from, *player, src_cnt - i.min(src_cnt.saturating_sub(1)), src_cnt + *fl);
                let (b, _) = ship_slot(*to, *player, dst_cnt - 1 - i, dst_cnt);
                let mid = a.lerp(b, e) + Vec3::new(0.0, (std::f32::consts::PI * e).sin() * 1.1, 0.0);
                let d = b - a;
                let heading = -d.z.atan2(d.x);
                ship_item(mid, heading, *player, 1.0, 0.0, false);
            }
        }
        // battle effect
        if let Some(Anim { fx: Fx::Battle { system }, t, dur, .. }) = &self.anim {
            let k = (t / dur).clamp(0.0, 1.0);
            let c = system_pos(*system);
            for i in 0..14 {
                let a = i as f32 * 2.399 + self.clock * 9.0;
                let r = 0.5 + (i % 4) as f32 * 0.22;
                let flash = ((k * 18.0 + i as f32 * 1.3).sin() * 0.5 + 0.5) * (1.0 - k * 0.5);
                out.push(Item {
                    mesh: self.bar_mesh,
                    model: tr(c + Vec3::new(a.cos() * r, 0.45 + (i % 3) as f32 * 0.12, a.sin() * r), Quat::from_rotation_y(-a + 0.8) * Quat::from_rotation_z(0.4), Vec3::new(0.5, 0.02, 0.02)),
                    material: PbrMaterial { unlit: false, emissive: [5.0 * flash, 2.2 * flash, 0.8 * flash], ..PbrMaterial::plastic([1.0, 0.5, 0.2]) },
                    casts: false,
                });
            }
        }
        // selection / hover rings
        let mut ring = |sys: usize, col: [f32; 3]| {
            out.push(Item { mesh: self.ring_mesh, model: tr(system_pos(sys) + Vec3::new(0.0, 0.17, 0.0), Quat::IDENTITY, Vec3::ONE), material: PbrMaterial { emissive: col, ..PbrMaterial::plastic([0.1, 0.1, 0.1]) }, casts: false });
        };
        if let Some(sel) = self.selected {
            ring(sel, [2.2, 2.2, 2.4]);
        }
        for &h in &self.hover_systems {
            ring(h, [0.5, 2.0, 0.8]);
        }
        if let Some(h) = self.hover_pick {
            if self.selected != Some(h) {
                ring(h, [0.5, 0.6, 1.0]);
            }
        }
        // current-player marker over their home city: a floating chevron-ish ring
        if matches!(self.screen, Screen::Play) && !s.is_over() {
            let cur = s.current();
            let pulse = 0.5 + 0.5 * (self.clock * 3.0).sin();
            for sys in 0..SYSTEMS {
                if s.city(sys) == Some(cur) {
                    let c = lin(ui::COLORS_UI[cur]);
                    out.push(Item { mesh: self.ring_mesh, model: tr(system_pos(sys) + Vec3::new(0.0, 0.14, 0.0), Quat::IDENTITY, Vec3::splat(0.92)), material: PbrMaterial { emissive: c.map(|v| v * (1.0 + pulse * 2.5)), ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
                }
            }
        }
        out
    }
}

impl Game for Ember {
    fn init(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let gl = ctx.gl_arc();
        let mut renderer = Renderer::new(&gl, ctx.drawable_size(), 1.0, TONEMAP_COMPOSITE_FRAG)?;
        renderer.set_shadow_settings(&gl, ShadowSettings::smooth());
        renderer.set_exposure(1.05);
        renderer.set_resolution_scale(1.0);
        renderer.set_target_options(&gl, TargetOptions::hifi())?;
        self.renderer = Some(renderer);
        let (v, f) = patch_mesh_shaders(PBR_MESH_VERT, PBR_MESH_FRAG, "FragColor")?;
        self.shaders = Some(ShaderVariantCache::new(v, f));
        self.white = Some(GpuTexture::from_rgba8(&gl, &[255; 4], 1, 1, TextureFilter::Bilinear)?);
        self.build_scene(&gl)?;
        self.ui = Some(EguiState::new(ctx.gl_arc())?);
        Ok(())
    }

    fn handle_event(&mut self, ctx: &mut Context, event: &Event) {
        if let Some(ui) = self.ui.as_mut() {
            ui.handle_event(event);
        }
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                if matches!(self.screen, Screen::Play) {
                    self.screen = Screen::Menu;
                } else {
                    ctx.should_quit = true;
                }
            }
            Event::KeyDown { keycode: Some(Keycode::Space), .. } => self.camera.overview(),
            Event::KeyDown { keycode: Some(Keycode::Q), .. } => self.camera.quarter_turn(-1.0),
            Event::KeyDown { keycode: Some(Keycode::E), .. } => self.camera.quarter_turn(1.0),
            Event::MouseButtonDown { mouse_btn: MouseButton::Right, .. } => self.orbiting = true,
            Event::MouseButtonUp { mouse_btn: MouseButton::Right, .. } => self.orbiting = false,
            Event::MouseButtonDown { mouse_btn: MouseButton::Middle, .. } => self.panning = true,
            Event::MouseButtonUp { mouse_btn: MouseButton::Middle, .. } => self.panning = false,
            Event::MouseMotion { xrel, yrel, x, y, .. } => {
                if self.orbiting {
                    self.camera.orbit(*xrel as f32, *yrel as f32);
                } else if self.panning {
                    self.camera.drag_pan(*xrel as f32, *yrel as f32);
                } else if matches!(self.screen, Screen::Play) && !self.ui_wants_pointer {
                    self.hover_pick = self.pick_system(ctx, (*x, *y));
                }
            }
            Event::MouseWheel { y, .. } if !self.ui_wants_pointer => self.camera.zoom(*y as f32),
            Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, clicks, .. }
                if matches!(self.screen, Screen::Play) && !self.ui_wants_pointer =>
            {
                let hit = self.pick_system(ctx, (*x, *y));
                if *clicks >= 2 {
                    if let Some(h) = hit {
                        self.camera.focus(system_pos(h));
                    }
                }
                self.selected = if self.selected == hit { None } else { hit };
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut Context, dt: f32) -> anyhow::Result<()> {
        let dt = dt.min(0.1);
        self.clock += dt;
        // keyboard pan
        let k = |key| ctx.input.is_key_down(key);
        let (mut r, mut f) = (0.0, 0.0);
        if k(Keycode::D) || k(Keycode::Right) { r += 1.0; }
        if k(Keycode::A) || k(Keycode::Left) { r -= 1.0; }
        if k(Keycode::W) || k(Keycode::Up) { f += 1.0; }
        if k(Keycode::S) || k(Keycode::Down) { f -= 1.0; }
        if r != 0.0 || f != 0.0 {
            let speed = self.camera.cam.distance * 0.9 * dt;
            self.camera.pan(r * speed, f * speed);
        }
        self.camera.update(dt);
        if let Some((_, t)) = self.toast.as_mut() {
            *t -= dt;
        }
        if self.toast.as_ref().is_some_and(|(_, t)| *t <= 0.0) {
            self.toast = None;
        }
        if !matches!(self.screen, Screen::Play) {
            return Ok(());
        }

        // animation queue
        let speed = if self.autoplay { 4.0 } else { 1.0 };
        if let Some(a) = self.anim.as_mut() {
            a.t += dt * speed;
            if a.t >= a.dur {
                let a = self.anim.take().unwrap();
                if a.apply_at_end {
                    self.view = a.after;
                }
            }
        }
        if self.anim.is_none() {
            if let Some((ev, after, act)) = self.queue.pop_front() {
                self.start_anim(ev, after, act);
            }
        }
        let busy = self.anim.is_some() || !self.queue.is_empty();

        // bots
        if !busy && !self.truth.is_over() && self.bots[self.truth.current()] {
            self.bot_timer -= dt * speed;
            if self.bot_timer <= 0.0 {
                let a = self.truth.bot_choose();
                self.do_action(a);
                self.bot_timer = if matches!(self.truth.phase(), Phase::Act) { 0.55 } else { 0.8 };
            }
        }
        Ok(())
    }

    fn render(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let size = ctx.drawable_size();
        let aspect = ctx.aspect_ratio();
        let gl = ctx.gl_arc();
        let gl = &*gl;
        let dynamic = self.draw_items();
        let renderer = self.renderer.as_mut().unwrap();
        renderer.resize_if_needed(gl, size)?;
        let view = self.camera.cam.view_matrix();
        let proj = self.camera.cam.projection_matrix(aspect);
        let sun = Vec3::from(self.lighting.sun_dir).normalize();

        let bounds = ShadowBounds::Aabb { min: Vec3::new(-10.0, -1.0, -9.0), max: Vec3::new(10.0, 3.0, 9.0) };
        if renderer.begin_shadow_pass(gl, sun, &bounds) {
            for it in self.statics.iter().chain(dynamic.iter()).filter(|i| i.casts) {
                renderer.shadow_draw(gl, &it.model, &self.meshes[it.mesh]);
            }
            renderer.end_shadow_pass(gl);
        }
        renderer.begin_scene(gl);
        unsafe {
            gl.clear_color(0.01, 0.012, 0.025, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
        }
        let program = self.shaders.as_mut().unwrap().get_or_compile(gl, 0)?;
        renderer.bind_shadows(gl, program);
        self.lighting.upload(gl, program, self.camera.cam.position().to_array());
        unsafe {
            let loc = |n: &str| gl.get_uniform_location(program, n);
            gl.uniform_matrix_4_f32_slice(loc("uView").as_ref(), false, &view.to_cols_array());
            gl.uniform_matrix_4_f32_slice(loc("uProj").as_ref(), false, &proj.to_cols_array());
        }
        let white = self.white.as_ref().unwrap().handle;
        for it in self.statics.iter().chain(dynamic.iter()) {
            let mat = PbrMaterial { base_texture: Some(white), ..it.material };
            mat.upload(gl, program);
            unsafe {
                let loc = gl.get_uniform_location(program, "uModel");
                gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, &it.model.to_cols_array());
            }
            self.meshes[it.mesh].draw(gl);
        }
        unsafe { gl.disable(glow::CULL_FACE) };
        renderer.present(gl, size, &PostParams::default());

        // ---- UI
        let (ww, _) = ctx.platform.window.size();
        let handoff = self.handoff_target();
        let ui_state = self.ui.as_mut().unwrap();
        ui_state.ui_scale = (size.0 as f32 / ww.max(1) as f32).max(0.5);
        let busy = self.anim.is_some() || !self.queue.is_empty();
        let mut start = false;
        let mut hud_out = ui::HudOutput::default();
        let toast = self.toast.as_ref().map(|(s, _)| s.clone());
        let (screen_menu, view_state) = (matches!(self.screen, Screen::Menu), self.view.clone());
        let (bots, log, selected) = (self.bots, self.log.clone(), self.selected);
        let menu = &mut self.menu;
        let output = ui_state.run(size, |ectx| {
            if screen_menu {
                start = ui::menu(ectx, menu);
            } else {
                hud_out = ui::hud(
                    ectx,
                    &ui::HudInput { state: &view_state, bots: &bots, busy, selected, log: &log, toast: toast.as_deref(), handoff },
                );
            }
        });
        ui_state.paint(size, output);
        self.ui_wants_pointer = ui_state.wants_pointer_input() || screen_menu;

        self.hover_systems = hud_out.hover_systems;
        if start {
            let seed = (self.clock * 1000.0) as u64 ^ 0x9e37;
            self.start_game(seed);
        }
        if hud_out.new_game {
            self.screen = Screen::Menu;
        }
        if hud_out.ready {
            self.handoff_seen = Some(self.truth.current());
        }
        if let Some(a) = hud_out.action {
            if self.human_turn() && !busy && self.truth.is_legal(a) {
                self.do_action(a);
                self.selected = None;
            }
        }
        self.screenshot_step(gl, size, ctx)?;
        Ok(())
    }
}

impl Ember {
    /// Hot-seat: the next human seat must confirm before their hand is shown,
    /// but only when more than one human is playing.
    fn handoff_target(&self) -> Option<usize> {
        if !matches!(self.screen, Screen::Play) || self.truth.is_over() {
            return None;
        }
        let humans = (0..self.truth.players()).filter(|&p| !self.bots[p]).count();
        let cur = self.truth.current();
        if humans < 2 || self.bots[cur] || self.anim.is_some() || !self.queue.is_empty() {
            return None;
        }
        (self.handoff_seen != Some(cur)).then_some(cur)
    }

    fn screenshot_step(&mut self, gl: &glow::Context, size: (u32, u32), ctx: &mut Context) -> anyhow::Result<()> {
        self.frame += 1;
        let Some(path) = self.shot.clone() else { return Ok(()) };
        if self.actions_taken >= self.shot_after && self.anim.is_none() && self.queue.is_empty() {
            self.shot_pending += 1;
            if self.shot_pending == 4 {
                let (w, h) = size;
                let mut px = vec![0u8; (w * h * 4) as usize];
                unsafe {
                    gl.read_pixels(0, 0, w as i32, h as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut px));
                }
                let row = (w * 4) as usize;
                let flipped: Vec<u8> = px.chunks(row).rev().flatten().copied().collect();
                dust_engine::texture::save_rgba8_png(std::path::Path::new(&path), &flipped, w, h)?;
                log::info!("saved {path}");
                ctx.should_quit = true;
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    App::run("Ember Reach", 1280, 720, Ember::new())
}
