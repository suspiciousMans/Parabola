//! Ember Reach: a bird's-eye 3D tabletop game on the Dust engine, rules in
//! Oxidized (see ../rules). Hot-seat or against bots; movable camera.
//!
//! Run: `cargo run -p ember_reach --release`
//!   --autoplay          start a 3-bot game and let it play itself
//!   --shot out.png      with --autoplay: save a screenshot (at game over if it ends first) and exit
//!   --after N           ...after N actions (default 40)
//!   --ledger            open the ledger immediately (screenshot/review aid)
//!   --size WxH          window size (default 1280x720)

mod camera;
mod cards;
mod courtart;
mod fieldmanual;
mod galaxy;
mod guides;
mod models;
mod planets;
mod probe;
mod site;
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
use dust_engine::shader::{ShaderVariantCache, PBR_MESH_FRAG, PBR_MESH_VERT};
use dust_engine::texture::{GpuTexture, TextureFilter};
use dust_engine::ui::EguiState;
use reach_rules::{Action, Card, Phase, Slot, State, SYSTEMS};
use std::collections::VecDeque;
use std::f32::consts::TAU;

type Ev = [i64; 5];

/// Screen angle of cluster/gate `c` (0..6): gate 1 sits at the top and the rest
/// follow clockwise, 60 degrees apart, as on the printed board.
fn cluster_angle(c: usize) -> f32 {
    (-85.0f32).to_radians() + c as f32 * TAU / 6.0
}

/// The power track sits inside the board's bottom margin, clear of the action
/// strip and hand, so low-power pieces stay visible behind the HUD.
const POWER_Z: f32 = 6.4;

/// Six clusters around a central hub. Each has a gate on the inner ring and
/// three planets fanning outward: two outer ones either side and a nearer
/// middle one. Systems are numbered cluster * 4 + (arrow, crescent, hex, gate).
fn system_pos(i: usize) -> Vec3 {
    let th = cluster_angle(i / 4);
    let (r, da) = match i % 4 {
        0 => (7.0, -0.34),
        1 => (5.4, 0.0),
        2 => (7.0, 0.34),
        _ => (2.55, 0.0),
    };
    let a = th + da;
    Vec3::new(a.cos() * r, 0.0, a.sin() * r)
}

/// Radius of a system's node on the board.
fn node_radius(i: usize) -> f32 {
    if i % 4 == 3 { 0.85 } else { 1.2 }
}

fn lanes_of(s: &State) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for a in 0..SYSTEMS {
        if !s.in_play(a) {
            continue;
        }
        for b in s.neighbors(a) {
            if a < b {
                v.push((a, b));
            }
        }
    }
    v
}

const PLANET_COLORS: [[u8; 3]; 5] = [[200, 160, 100], [240, 120, 60], [210, 80, 80], [110, 200, 230], [190, 120, 240]];

/// sRGB-ish byte colour to linear for the PBR shader.
fn lin(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| (v as f32 / 255.0).powf(2.2))
}

/// Model scale, column step (radians) and lane gap for a formation of `n` at
/// one system. Crowded formations fly smaller ships on a tighter step, so
/// hulls never overlap and four players still fit around one system.
fn ship_formation(n: usize) -> (f32, f32, f32) {
    let cols = n.div_ceil(3).max(1) as f32;
    let k = (1.8 / cols).clamp(0.55, 1.0);
    (0.52 * k, 0.66 * k, 0.48 * k)
}

/// Resting slot of ship `i` of `n` belonging to `player` around a system centre.
fn ship_slot(system: usize, player: usize, i: usize, n: usize) -> (Vec3, f32) {
    // Three lanes at different radii, filled left to right.
    let sector = player as f32 / 4.0 * TAU + 0.4;
    let (_, step, lane_gap) = ship_formation(n);
    let lane = i % 3;
    let col = i / 3;
    let cols = n.div_ceil(3).max(1);
    let a = sector + (col as f32 - (cols as f32 - 1.0) / 2.0) * step;
    let base = if system % 4 == 3 { 0.66 } else { 1.0 };
    let r = base + lane as f32 * lane_gap;
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
    /// The painted galaxy and its drifting layers, drawn around the board.
    galaxy: Option<galaxy::Galaxy>,
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
    hex_mesh: usize,
    planet_tex: Vec<(glow::Texture, glow::Texture)>,
    keep_tex: Vec<GpuTexture>,
    camera: BoardCamera,
    lighting: SceneLighting,

    screen: Screen,
    menu: ui::MenuState,
    bots: [bool; 4],
    truth: State,
    view: State,
    queue: VecDeque<(Ev, State, Action)>,
    lanes: Vec<(usize, usize)>,
    static_sys: Vec<Option<usize>>,
    anim: Option<Anim>,
    table: cards::Table,
    flies: Vec<cards::Fly>,
    card_slots: cards::Slots,
    win: [f32; 2],
    log: Vec<String>,
    toast: Option<(String, f32)>,
    selected: Option<usize>,
    hover_systems: Vec<usize>,
    hover_pick: Option<usize>,
    bot_timer: f32,
    handoff_seen: Option<usize>,
    /// Showing a site room: the Worker owns the game and plays the bots.
    site: bool,
    site_seen: i64,
    site_rev: i64,
    site_timer: f32,
    awaiting: bool,
    /// Seats the local machine plays with bot logic.
    ai: [bool; 4],
    online_hud: Option<ui::OnlineHud>,
    clock: f32,
    city_rise: [f32; SYSTEMS],
    cinematic: bool,
    cine_moved: bool,
    theme_done: bool,

    // input
    orbiting: bool,
    panning: bool,
    ui_wants_pointer: bool,

    // scripted run / screenshots
    autoplay: bool,
    actions_taken: usize,
    shot: Option<String>,
    shot_after: usize,
    probe: bool,
    hold: Option<usize>,
    holding: bool,
    probe_sent: bool,
    frame: u32,
    shot_pending: u32,
}

/// A six-sided stud mesh, uploaded once and cached.
fn models_hex(me: &mut Ember, gl: &glow::Context) -> anyhow::Result<usize> {
    if me.hex_mesh == usize::MAX {
        me.hex_mesh = me.upload(gl, &primitives::cylinder(6))?;
    }
    Ok(me.hex_mesh)
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

impl Ember {
    fn new() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let autoplay = args.iter().any(|a| a == "--autoplay");
        let state = State::new(3, 1, 0);
        let mut lighting = SceneLighting::default();
        lighting.sun_dir = [-0.35, 1.0, 0.45];
        lighting.sun_color = [3.0, 2.75, 2.4];
        lighting.sky_color = [0.16, 0.2, 0.32];
        lighting.ground_color = [0.05, 0.05, 0.07];
        lighting.point_lights.push(PointLight { position: [0.0, 1.6, 0.0], color: [1.0, 0.65, 0.35], intensity: 5.0, range: 7.0 });
        let mut me = Self {
            renderer: None,
            galaxy: None,
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
            hex_mesh: usize::MAX,
            planet_tex: Vec::new(),
            keep_tex: Vec::new(),
            camera: BoardCamera::new(),
            lighting,
            screen: Screen::Menu,
            menu: ui::MenuState::default(),
            bots: [false, true, true, true],
            truth: state.clone(),
            view: state,
            queue: VecDeque::new(),
            lanes: Vec::new(),
            static_sys: Vec::new(),
            anim: None,
            table: cards::Table::default(),
            flies: Vec::new(),
            card_slots: cards::Slots::default(),
            win: [1280.0, 720.0],
            log: Vec::new(),
            toast: None,
            selected: None,
            hover_systems: Vec::new(),
            hover_pick: None,
            bot_timer: 0.0,
            handoff_seen: None,
            site: site::enabled(),
            site_seen: 0,
            site_rev: -1,
            site_timer: 0.0,
            awaiting: false,
            ai: [false; 4],
            online_hud: None,
            clock: 0.0,
            city_rise: [-10.0; SYSTEMS],
            cinematic: true,
            cine_moved: false,
            theme_done: false,
            orbiting: false,
            panning: false,
            ui_wants_pointer: false,
            autoplay,
            actions_taken: 0,
            shot: arg_value(&args, "--shot"),
            shot_after: arg_value(&args, "--after").and_then(|v| v.parse().ok()).unwrap_or(40),
            probe: args.iter().any(|a| a == "--probe"),
            hold: arg_value(&args, "--hold").and_then(|v| v.parse().ok()),
            holding: false,
            probe_sent: false,
            frame: 0,
            shot_pending: 0,
        };
        me.lanes = lanes_of(&me.truth);
        if autoplay {
            me.menu.players = arg_value(&args, "--players").and_then(|v| v.parse().ok()).unwrap_or(3);
            me.menu.bots = [true; 4];
            let seed = arg_value(&args, "--seed").and_then(|v| v.parse().ok()).unwrap_or(7);
            me.start_game(seed);
        }
        // Deterministic live-page capture: play the bot game forward in rules
        // space, then hold that state. No animations or timers are involved, so
        // the browser screenshot and the probe describe the same action count.
        if let Some(jump) = arg_value(&args, "--jump").and_then(|v| v.parse::<usize>().ok()) {
            let mut st = me.truth.clone();
            let mut steps = 0;
            while steps < jump && !st.is_over() {
                st = st.apply(st.bot_choose());
                steps += 1;
            }
            me.truth = st.clone();
            me.view = st;
            me.actions_taken = steps;
            me.holding = true;
            me.city_rise = [me.clock; SYSTEMS];
        }
        // Screenshot helper: show the ledger immediately (also useful with --autoplay).
        if args.iter().any(|a| a == "--ledger") {
            ui::toggle_ledger();
        }
        if args.iter().any(|a| a == "--topdown") {
            me.camera.glide_to(Vec3::ZERO, 29.0, 1.45);
            me.cinematic = false;
        }
        if args.iter().any(|a| a == "--human") {
            // The HUD reads `bots` for enabling controls; the update loop reads
            // `ai` for auto-play. A human seat must be cleared in both.
            me.bots[0] = false;
            me.ai[0] = false;
        }
        if let Some(c) = arg_value(&args, "--focus").and_then(|v| v.parse::<usize>().ok()) {
            me.camera.focus(system_pos(c));
            me.camera.set_distance(7.0);
        }
        me
    }

    fn start_game(&mut self, seed: u64) {
        self.bots = self.menu.bots;
        self.ai = self.menu.bots;
        self.online_hud = None;
        let setups = State::setup_count(self.menu.players);
        let s = State::new(self.menu.players, seed, self.menu.setup % setups);
        self.lanes = lanes_of(&s);
        self.truth = s.clone();
        self.view = s;
        self.queue.clear();
        self.anim = None;
        self.table = cards::Table::default();
        self.flies.clear();
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
        let planet_mesh = self.upload(gl, &primitives::uv_sphere(64, 32))?;
        for ty in 0..5 {
            let t = planets::generate(ty, 384, 192);
            let c = GpuTexture::from_rgba8(gl, &t.color, t.w, t.h, TextureFilter::Bilinear)?;
            let n = GpuTexture::from_rgba8(gl, &t.normal, t.w, t.h, TextureFilter::Bilinear)?;
            self.planet_tex.push((c.handle, n.handle));
            self.keep_tex.push(c);
            self.keep_tex.push(n);
        }
        let tri_mesh = self.upload(gl, &primitives::cylinder(3))?;
        let sphere = self.upload(gl, &primitives::uv_sphere(24, 12))?;
        self.sphere_mesh = sphere;
        self.ring_mesh = self.upload(gl, &models::ring())?;
        self.bar_mesh = cube;
        self.ship = self.upload_model(gl, &models::ship())?;
        self.city = self.upload_model(gl, &models::city())?;
        let platform = self.upload_model(gl, &models::platform())?;

        let tr = |p: Vec3, r: Quat, s: Vec3| Mat4::from_scale_rotation_translation(s, r, p);
        let mut st: Vec<Item> = Vec::new();
        // ---- the printed board: plate, hub, gate ring, wedges, cracks, edge tracks
        let gold = PbrMaterial { emissive: [0.32, 0.24, 0.12], ..PbrMaterial::plastic([0.2, 0.15, 0.05]) };
        let dim_gold = PbrMaterial { emissive: [0.1, 0.08, 0.045], ..PbrMaterial::plastic([0.08, 0.065, 0.03]) };
        let panel = PbrMaterial { roughness: 0.8, emissive: [0.004, 0.012, 0.03], ..PbrMaterial::plastic([0.0; 3]) };
        // board border (no plate: the board floats in space)
        for (x, z, w, d) in [(0.0, -9.85, 26.2, 0.1), (0.0, 9.85, 26.2, 0.1), (-13.05, 0.0, 0.1, 19.8), (13.05, 0.0, 0.1, 19.8)] {
            st.push(Item { mesh: cube, model: tr(Vec3::new(x, 0.0, z), Quat::IDENTITY, Vec3::new(w, 0.06, d)), material: dim_gold, casts: false });
        }
        // wedge floors: three per cluster, tinted by the planet they hold
        for c in 0..6 {
            let th = cluster_angle(c);
            for (k, kind) in [0usize, 1, 2].iter().enumerate() {
                let a0 = th + (k as f32 - 1.5) * 0.35;
                let a1 = a0 + 0.35;
                let ty = State::planet_type(c * 4 + kind).unwrap_or(0);
                let col = lin(PLANET_COLORS[ty]);
                let sm = self.upload(gl, &models::sector(3.45, 9.7, a0, a1))?;
                st.push(Item { mesh: sm, model: tr(Vec3::new(0.0, 0.005 + 0.001 * k as f32, 0.0), Quat::IDENTITY, Vec3::ONE), material: PbrMaterial { roughness: 1.0, emissive: col.map(|v| v * 0.012), ..PbrMaterial::plastic(col.map(|v| v * 0.006)) }, casts: false });
                // thin gold wedge boundary
                let am = a0;
                let (mid, len) = (Vec3::new(am.cos() * 6.6, 0.02, am.sin() * 6.6), 6.2);
                st.push(Item { mesh: cube, model: tr(mid, Quat::from_rotation_y(-am), Vec3::new(len, 0.01, 0.035)), material: dim_gold, casts: false });
            }
        }
        // hub, gate ring and emblem
        st.push(Item { mesh: cyl, model: tr(Vec3::new(0.0, 0.02, 0.0), Quat::IDENTITY, Vec3::new(6.9, 0.04, 6.9)), material: PbrMaterial { roughness: 0.7, emissive: [0.03, 0.025, 0.06], ..PbrMaterial::plastic([0.012, 0.01, 0.03]) }, casts: false });
        st.push(Item { mesh: cyl, model: tr(Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Vec3::new(3.7, 0.04, 3.7)), material: PbrMaterial { roughness: 0.9, ..PbrMaterial::plastic([0.002, 0.002, 0.006]) }, casts: false });
        st.push(Item { mesh: self.ring_mesh, model: tr(Vec3::new(0.0, 0.08, 0.0), Quat::IDENTITY, Vec3::splat(2.3)), material: gold, casts: false });
        st.push(Item { mesh: self.ring_mesh, model: tr(Vec3::new(0.0, 0.08, 0.0), Quat::IDENTITY, Vec3::splat(1.2)), material: dim_gold, casts: false });
        // our own emblem: an eight-point ember star
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            st.push(Item { mesh: cube, model: tr(Vec3::new(0.0, 0.12, 0.0), Quat::from_rotation_y(a), Vec3::new(if k % 2 == 0 { 2.4 } else { 1.6 }, 0.02, 0.07)), material: PbrMaterial { emissive: [0.7, 0.5, 0.26], ..PbrMaterial::plastic([0.2; 3]) }, casts: false });
        }
        st.push(Item { mesh: sphere, model: tr(Vec3::new(0.0, 0.16, 0.0), Quat::IDENTITY, Vec3::splat(0.16)), material: PbrMaterial { emissive: [1.6, 1.0, 0.5], ..PbrMaterial::plastic([0.5; 3]) }, casts: false });
        // hex studs (path markers) round the gate ring, two per gate
        for c in 0..6 {
            for off in [-0.2f32, 0.2] {
                let a = cluster_angle(c) + off;
                st.push(Item { mesh: models_hex(self, gl)?, model: tr(Vec3::new(a.cos() * 3.55, 0.07, a.sin() * 3.55), Quat::IDENTITY, Vec3::new(0.16, 0.06, 0.16)), material: gold, casts: false });
            }
        }
        // jagged cracks between clusters, as bar chains
        for c in 0..6 {
            let a = cluster_angle(c) + TAU / 12.0;
            let dir = Vec3::new(a.cos(), 0.0, a.sin());
            let side = Vec3::new(-a.sin(), 0.0, a.cos());
            let mut prev = dir * 2.0;
            for k in 1..=18 {
                let r = 2.0 + k as f32 * 0.42;
                let jitter = (((c * 31 + k * 17) % 7) as f32 - 3.0) * 0.07 * (1.0 + k as f32 * 0.04);
                let p = dir * r + side * jitter;
                let d = p - prev;
                let mid = (p + prev) * 0.5 + Vec3::new(0.0, 0.03, 0.0);
                st.push(Item { mesh: cube, model: tr(mid, Quat::from_rotation_y(-d.z.atan2(d.x)), Vec3::new(d.length() + 0.02, 0.012, 0.05 + 0.04 * (1.0 - k as f32 / 20.0))), material: PbrMaterial { emissive: [0.32, 0.29, 0.25], ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
                prev = p;
            }
        }
        // power track along the bottom edge: 50 cells
        for i in 0..51 {
            let x = -11.5 + i as f32 * 0.46;
            let major = i % 5 == 0;
            st.push(Item { mesh: cube, model: tr(Vec3::new(x, 0.02, POWER_Z), Quat::IDENTITY, Vec3::new(0.38, 0.02, if major { 0.5 } else { 0.38 })), material: if major { gold } else { dim_gold }, casts: false });
        }
        // left edge: deck, surpass/copy/pivot slot, lead slot; top edge: court row
        for (z, d) in [(-5.6, 3.0), (-0.9, 5.6), (4.6, 3.2)] {
            st.push(Item { mesh: cube, model: tr(Vec3::new(-11.7, 0.045, z), Quat::IDENTITY, Vec3::new(2.1, 0.03, d)), material: panel, casts: false });
            st.push(Item { mesh: cube, model: tr(Vec3::new(-11.7, 0.02, z), Quat::IDENTITY, Vec3::new(2.2, 0.01, d + 0.1)), material: dim_gold, casts: false });
        }
        for k in 0..5 {
            let x = -4.8 + k as f32 * 2.4;
            st.push(Item { mesh: cube, model: tr(Vec3::new(x, 0.045, -9.0), Quat::IDENTITY, Vec3::new(2.0, 0.03, 1.3)), material: panel, casts: false });
            st.push(Item { mesh: cube, model: tr(Vec3::new(x, 0.02, -9.0), Quat::IDENTITY, Vec3::new(2.1, 0.01, 1.4)), material: dim_gold, casts: false });
        }
        // right edge: ambition boxes (lit when declared, see draw_items) and chapter track
        for k in 0..5 {
            let z = -6.4 + k as f32 * 2.1;
            st.push(Item { mesh: cube, model: tr(Vec3::new(11.3, 0.045, z), Quat::IDENTITY, Vec3::new(3.0, 0.03, 1.8)), material: panel, casts: false });
            st.push(Item { mesh: cube, model: tr(Vec3::new(11.3, 0.02, z), Quat::IDENTITY, Vec3::new(3.1, 0.01, 1.9)), material: dim_gold, casts: false });
        }
        for k in 0..3 {
            st.push(Item { mesh: models_hex(self, gl)?, model: tr(Vec3::new(10.2 + k as f32 * 1.1, 0.04, -8.2), Quat::IDENTITY, Vec3::new(0.45, 0.03, 0.45)), material: dim_gold, casts: false });
        }
        let mut static_sys: Vec<Option<usize>> = vec![None; st.len()];
        // systems: a glowing node disc for ships, a planet per type, a stargate per gate
        let bar = cube;
        for i in 0..SYSTEMS {
            let p = system_pos(i);
            let node_col = if State::planet_type(i).is_some() { [0.55, 0.62, 0.74] } else { [0.62, 0.7, 0.8] };
            st.push(Item { mesh: cyl, model: tr(p + Vec3::new(0.0, 0.02, 0.0), Quat::IDENTITY, Vec3::new(node_radius(i) * 2.0, 0.02, node_radius(i) * 2.0)), material: PbrMaterial { roughness: 0.6, emissive: node_col.map(|v| v * 0.02), ..PbrMaterial::plastic([0.004, 0.007, 0.02]) }, casts: false });
            st.push(Item { mesh: self.ring_mesh, model: tr(p + Vec3::new(0.0, 0.04, 0.0), Quat::IDENTITY, Vec3::splat(node_radius(i) / 1.55)), material: PbrMaterial { emissive: node_col.map(|v| v * 0.16), ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
            if State::planet_type(i).is_some() {
                for sl in 0..State::planet_slots(i) {
                    let off = Vec3::new(if sl == 0 { -0.45 } else { 0.45 }, 0.05, 0.0);
                    st.push(Item { mesh: tri_mesh, model: tr(p + off, Quat::IDENTITY, Vec3::new(0.34, 0.015, 0.34)), material: PbrMaterial { emissive: [0.05, 0.1, 0.2], ..PbrMaterial::plastic([0.003, 0.004, 0.01]) }, casts: false });
                }
            }
            if let Some(ty) = State::planet_type(i) {
                let pc = PLANET_COLORS[ty];
                let c = p + Vec3::new(-0.7, 1.15, -0.7);
                let tex = &self.planet_tex[ty];
                let pm = PbrMaterial { roughness: if ty == 1 { 0.55 } else { 0.85 }, base_texture: Some(tex.0), normal_map: Some(tex.1), normal_scale: 1.0, ..PbrMaterial::default() };
                let accent = PbrMaterial { emissive: lin(pc).map(|v| v * 0.5), ..PbrMaterial::plastic(lin(pc).map(|v| v * 0.5)) };
                let spin = i as f32 * 0.9;
                let spin_q = Quat::from_rotation_y(spin) * Quat::from_rotation_z(0.25);
                match ty {
                    0 => {
                        // Material: cratered rock with a thin dust belt
                        st.push(Item { mesh: planet_mesh, model: tr(c, spin_q, Vec3::splat(1.25)), material: pm, casts: true });
                        for k in 0..14 {
                            let a = k as f32 / 14.0 * TAU + spin;
                            let r = 0.78 + (k % 3) as f32 * 0.05;
                            st.push(Item { mesh: sphere, model: tr(c + Vec3::new(a.cos() * r, 0.06 * (k % 2) as f32 - 0.03, a.sin() * r), Quat::IDENTITY, Vec3::splat(0.045 + (k % 4) as f32 * 0.01)), material: PbrMaterial::plastic(lin([118, 100, 82])), casts: true });
                        }
                    }
                    1 => {
                        // Fuel: banded gas giant with a pale ring
                        st.push(Item { mesh: planet_mesh, model: tr(c, spin_q, Vec3::splat(1.4)), material: pm, casts: true });
                        st.push(Item { mesh: self.ring_mesh, model: tr(c, Quat::from_rotation_z(0.42), Vec3::new(0.68, 0.6, 0.68)), material: PbrMaterial { roughness: 0.8, ..PbrMaterial::plastic(lin([196, 176, 140])) }, casts: false });
                    }
                    2 => {
                        // Weapons: scorched world with dark spires
                        st.push(Item { mesh: planet_mesh, model: tr(c, spin_q, Vec3::splat(1.25)), material: pm, casts: true });
                        for k in 0..8 {
                            let a = k as f32 * 2.399 + spin;
                            let el = (k as f32 * 0.7).sin() * 0.9;
                            let d = Vec3::new(a.cos() * el.cos(), el.sin(), a.sin() * el.cos());
                            st.push(Item { mesh: bar, model: tr(c + d * 0.52, Quat::from_rotation_arc(Vec3::X, d), Vec3::new(0.26, 0.04, 0.04)), material: PbrMaterial::metal(lin([54, 52, 58]), 0.5), casts: true });
                        }
                    }
                    3 => {
                        // Relics: a crystal world with orbiting shards
                        st.push(Item { mesh: planet_mesh, model: tr(c, spin_q, Vec3::splat(1.1)), material: pm, casts: true });
                        for k in 0..3 {
                            let a = k as f32 * TAU / 3.0 + spin;
                            let q = Quat::from_rotation_y(a) * Quat::from_rotation_z(0.6) * Quat::from_rotation_x(0.5);
                            st.push(Item { mesh: bar, model: tr(c + Vec3::new(a.cos() * 0.7, 0.12 * (k as f32 - 1.0), a.sin() * 0.7), q, Vec3::new(0.09, 0.3, 0.09)), material: PbrMaterial { roughness: 0.2, ..PbrMaterial::plastic(lin([196, 228, 236])) }, casts: true });
                        }
                    }
                    _ => {
                        // Psionic: violet world with small moons
                        st.push(Item { mesh: planet_mesh, model: tr(c, spin_q, Vec3::splat(1.25)), material: pm, casts: true });
                        for k in 0..2 {
                            let a = k as f32 * std::f32::consts::PI + spin;
                            st.push(Item { mesh: sphere, model: tr(c + Vec3::new(a.cos() * 0.8, 0.15 * (k as f32 - 0.5), a.sin() * 0.8), Quat::IDENTITY, Vec3::splat(0.12)), material: accent, casts: true });
                        }
                    }
                }
            } else {
                // Gate: an upright ring of dark metal with a pale core
                let c = p + Vec3::new(0.0, 0.85, 0.0);
                let face = Quat::from_rotation_y(-((i / 4) as f32) * TAU / 6.0) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                st.push(Item { mesh: self.ring_mesh, model: tr(c, face, Vec3::splat(0.58)), material: PbrMaterial::metal(lin([150, 160, 172]), 0.35), casts: true });
                st.push(Item { mesh: self.ring_mesh, model: tr(c, face, Vec3::splat(0.5)), material: PbrMaterial { emissive: [0.12, 0.3, 0.42], ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
                st.push(Item { mesh: sphere, model: tr(c, Quat::IDENTITY, Vec3::splat(0.12)), material: PbrMaterial { emissive: [0.5, 0.9, 1.1], ..PbrMaterial::plastic([0.2; 3]) }, casts: false });
            }
            static_sys.resize(st.len(), Some(i));
        }
        static_sys.resize(st.len(), None);
        // a vast banded planet low in the frame
        st.push(Item { mesh: sphere, model: tr(Vec3::new(-75.0, -62.0, 35.0), Quat::IDENTITY, Vec3::splat(34.0)), material: PbrMaterial { roughness: 0.9, emissive: [0.05, 0.1, 0.22], ..PbrMaterial::plastic(lin([30, 50, 90])) }, casts: false });
        static_sys.resize(st.len(), None);
        self.static_sys = static_sys;
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
        if self.site {
            // the Worker applies it; the board follows when the new state arrives
            site::submit(a);
            self.awaiting = true;
            return;
        }
        let before = self.truth.clone();
        let next = before.apply(a);
        let ev = next.event();
        self.truth = next.clone();
        self.queue.push_back((ev, next, a));
        self.actions_taken += 1;
        let _ = before;
    }

    // ---- site room ---------------------------------------------------

    fn site_update(&mut self, dt: f32) {
        self.site_timer -= dt;
        if self.site_timer > 0.0 {
            return;
        }
        self.site_timer = 0.12;
        let Some(snap) = site::take() else { return };
        if let Some(e) = &snap.error {
            self.awaiting = false;
            self.toast = Some((format!("Not allowed: {e}"), 2.5));
        }
        let state = State(snap.state.clone());
        if snap.revision != self.site_rev {
            self.awaiting = false;
        }
        self.site_rev = snap.revision;
        let n = state.players().min(snap.seats.len());
        let who: Vec<String> = snap.seats.iter().map(|s| s.0.clone()).collect();
        let mut ai = [false; 4];
        for (p, s) in snap.seats.iter().enumerate().take(4) {
            ai[p] = s.1;
        }
        if !matches!(self.screen, Screen::Play) || self.online_hud.is_none() {
            if n == 0 || snap.you >= n {
                return;
            }
            self.site_seen = snap.log_total;
            self.begin_online(state, snap.you, ai, who);
            return;
        }
        // actions played since we last looked: animate each, then settle on the new state
        let fresh = ((snap.log_total - self.site_seen).max(0) as usize).min(snap.log.len());
        self.site_seen = snap.log_total;
        let old = self.truth.clone();
        for (i, ev) in snap.log[snap.log.len() - fresh..].iter().enumerate() {
            let last = i + 1 == fresh;
            let mut ev = *ev;
            if !last {
                // catching up on a run of moves: the cards and the log, not the ship flights
                ev[0] += 100;
            }
            self.queue.push_back((ev, if last { state.clone() } else { old.clone() }, Action([4, 0, 0, 0])));
            self.actions_taken += 1;
        }
        self.truth = state.clone();
        if fresh == 0 && self.queue.is_empty() && self.anim.is_none() {
            self.view = state;
        }
        if let Some(h) = self.online_hud.as_mut() {
            h.who = who;
            h.ai = ai;
        }
    }

    /// Enters the board for a room: `me` is this player's seat.
    fn begin_online(&mut self, s: State, me: usize, ai: [bool; 4], who: Vec<String>) {
        self.lanes = lanes_of(&s);
        self.truth = s.clone();
        self.view = s;
        self.queue.clear();
        self.anim = None;
        self.table = cards::Table::default();
        self.flies.clear();
        self.log.clear();
        self.selected = None;
        self.handoff_seen = None;
        self.city_rise = [-10.0; SYSTEMS];
        self.actions_taken = 0;
        // every seat but mine is somebody else to the HUD
        self.bots = [true; 4];
        self.bots[me] = false;
        self.ai = [false; 4];
        self.awaiting = false;
        self.online_hud = Some(ui::OnlineHud { me, who, ai });
        self.screen = Screen::Play;
        self.camera.overview();
    }

    fn human_turn(&self) -> bool {
        matches!(self.screen, Screen::Play) && !self.truth.is_over() && !self.bots[self.truth.current()] && !(self.site && (self.awaiting || !self.queue.is_empty()))
    }

    fn follow_target(&self, extra: usize) -> [f32; 2] {
        let s = &self.card_slots;
        let n = self.table.follows.len() + self.flies.iter().filter(|f| f.land == cards::Land::Follow).count() + extra;
        let dx = (n as f32 / 2.0) * s.card_w * 0.82;
        [s.follow[0] + dx, s.follow[1]]
    }

    /// Sends a played card from the hand (or a bot's row) to its slot.
    fn fly_card(&mut self, ev: Ev, dur: f32) {
        let p = ev[1] as usize;
        let human = !self.bots[p];
        let hand_mid = egui::pos2(self.win[0] * 0.5, self.win[1] - 110.0);
        let seize = ev[0] == 5;
        let card = (!seize).then(|| Card(ev[2] as u8));
        let from = if !human {
            cards::row_of(p)
        } else if seize {
            hand_mid
        } else {
            cards::origin_of(card, hand_mid)
        };
        let lead = ev[0] == 0;
        if lead && (self.table.lead.is_some() || !self.table.follows.is_empty()) {
            self.sweep_table();
        }
        let to = if lead { self.card_slots.lead } else { self.follow_target(0) };
        self.flies.push(cards::Fly {
            card,
            from,
            to: egui::pos2(to[0], to[1]),
            t: 0.0,
            dur,
            flip_from: if human && !seize { 0.0 } else { 1.0 },
            flip_to: if seize { 1.0 } else { 0.0 },
            land: if lead { cards::Land::Lead } else { cards::Land::Follow },
        });
    }

    /// The table's cards slide to the discard pile.
    fn sweep_table(&mut self) {
        let s = self.card_slots;
        let to = egui::pos2(s.discard[0], s.discard[1]);
        let mut out = Vec::new();
        if let Some(c) = self.table.lead.take() {
            out.push((Some(c), s.lead, 0.0));
        }
                    let n = self.table.follows.len();
            for (i, c) in self.table.follows.drain(..).enumerate() {
                let dx = (i as f32 - (n as f32 - 1.0) / 2.0) * s.card_w * 0.82;
                let flip = if c.is_some() { 0.0 } else { 1.0 };
                out.push((c, [s.follow[0] + dx, s.follow[1]], flip));
            }
        for (i, (card, at, flip)) in out.into_iter().enumerate() {
            self.flies.push(cards::Fly {
                card,
                from: egui::pos2(at[0], at[1]),
                to,
                t: -(i as f32) * 0.08,
                dur: 0.7,
                flip_from: flip,
                flip_to: flip,
                land: cards::Land::Discard,
            });
        }
    }

    fn start_anim(&mut self, ev: Ev, after: State, _a: Action) {
        // 100+ marks an event replayed while catching up: no ship flights or battles
        let quick = ev[0] >= 100;
        let mut ev = ev;
        if quick {
            ev[0] -= 100;
        }
        let mut fx = Fx::None;
        let mut dur = 0.18;
        let mut at_end = false;
        match ev[0] {
            12 if !quick => {
                let (f, t) = (ev[2] as usize, ev[3] as usize);
                fx = Fx::Fly { from: f, to: t, n: ev[4] as usize, player: ev[1] as usize };
                dur = if self.cinematic { 1.3 } else { 0.55 };
                at_end = true;
                if self.cinematic && !self.autoplay {
                    let mid = (system_pos(f) + system_pos(t)) * 0.5;
                    self.camera.glide_to(mid, 15.0, 0.85);
                    self.cine_moved = true;
                }
            }
            14 if !quick => {
                fx = Fx::Battle { system: ev[2] as usize };
                dur = if self.cinematic { 2.0 } else { 0.8 };
                at_end = true;
                if self.cinematic && !self.autoplay {
                    self.camera.glide_to(system_pos(ev[2] as usize), 8.0, 0.7);
                    self.cine_moved = true;
                }
            }
            9 if !quick => {
                fx = Fx::Rise { system: ev[2] as usize };
                self.city_rise[ev[2] as usize] = self.clock;
                dur = 0.5;
            }
            0..=3 => {
                dur = if self.cinematic { 0.95 } else { 0.5 };
                self.fly_card(ev, dur);
            }
            5 => {
                dur = if self.cinematic { 0.95 } else { 0.5 };
                self.fly_card(ev, dur);
            }
            20 | 21 => {
                dur = 0.9;
                self.sweep_table();
            }
            _ => {}
        }
        if let Some(line) = ui::describe(ev, &ui::NAMES) {
            self.log.push(line.clone());
            if matches!(ev[0], 14 | 20 | 21) {
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
        let ship_glow = PbrMaterial { emissive: [0.9, 0.55, 0.25], ..PbrMaterial::plastic([1.0, 0.6, 0.3]) };
        let city_glow = PbrMaterial { emissive: [1.0, 0.65, 0.3], ..PbrMaterial::plastic([1.0, 0.7, 0.3]) };
        let bob = |i: usize| (self.clock * 1.6 + i as f32 * 1.7).sin() * 0.02;

        // Power tokens along the bottom track: one lane per seat. A small
        // along-track stagger keeps equal scores readable as three pieces
        // instead of one stack at the start of the track.
        for p in 0..n {
            let pw = (s.power(p).max(0) as f32).min(50.0);
            let x = -11.5 + pw * 0.46 + p as f32 * 0.24;
            let col = lin(ui::COLORS_UI[p]);
            // Stand the pieces on the near side of the line, not behind it:
            // the gold cells would otherwise hide low-power pieces.
            let z = POWER_Z + 0.34 + p as f32 * 0.20;
            out.push(Item { mesh: self.hex_mesh, model: tr(Vec3::new(x, 0.10, z), Quat::IDENTITY, Vec3::new(0.27, 0.18, 0.27)), material: PbrMaterial { roughness: 0.8, metallic: 0.1, emissive: col.map(|v| v * 0.18), ..PbrMaterial::plastic(lin([28, 26, 24])) }, casts: false });
            out.push(Item { mesh: self.hex_mesh, model: tr(Vec3::new(x, 0.18, z), Quat::IDENTITY, Vec3::new(0.18, 0.18, 0.18)), material: PbrMaterial { roughness: 0.3, metallic: 0.5, emissive: col.map(|v| v * 0.7), ..PbrMaterial::plastic(col) }, casts: false });
        }
        // ambition boxes light up with their declaration rank
        let ranks = s.ambition_ranks();
        for k in 0..5 {
            if ranks[k] > 0 {
                let z = -6.4 + k as f32 * 2.1;
                let g = [0.12, 0.09, 0.07][(ranks[k] as usize - 1).min(2)];
                out.push(Item { mesh: self.bar_mesh, model: tr(Vec3::new(11.3, 0.06, z), Quat::IDENTITY, Vec3::new(2.9, 0.02, 1.7)), material: PbrMaterial { emissive: [g * 0.5, g * 0.9, g * 1.4], ..PbrMaterial::plastic([0.0; 3]) }, casts: false });
            }
        }
        // chapter track: five discs, lit up to the current chapter
        for k in 0..5 {
            let lit = (k as i64) < s.chapter();
            out.push(Item { mesh: self.hex_mesh, model: tr(Vec3::new(8.6 + k as f32 * 0.9, 0.06, 8.3), Quat::IDENTITY, Vec3::new(0.34, 0.04, 0.34)), material: PbrMaterial { emissive: if lit { [1.6, 1.0, 0.4] } else { [0.08, 0.06, 0.03] }, ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
        }
        // out-of-play clusters: drifting dust
        for sys in 0..SYSTEMS {
            if s.in_play(sys) {
                continue;
            }
            let c = system_pos(sys);
            for k in 0..9 {
                let a = k as f32 * 2.39 + sys as f32 * 1.7 + self.clock * 0.05 * (1.0 + (k % 3) as f32);
                let r = 0.3 + (k % 5) as f32 * 0.28;
                out.push(Item { mesh: self.sphere_mesh, model: tr(c + Vec3::new(a.cos() * r, 0.25 + (k % 4) as f32 * 0.15, a.sin() * r), Quat::IDENTITY, Vec3::splat(0.07 + (k % 3) as f32 * 0.04)), material: PbrMaterial { roughness: 1.0, emissive: [0.04, 0.035, 0.05], ..PbrMaterial::plastic(lin([70, 65, 80])) }, casts: false });
            }
        }
        // lanes
        for &(a, b) in &self.lanes {
            let (pa, pb) = (system_pos(a), system_pos(b));
            let d = pb - pa;
            let len = d.length() - node_radius(a) - node_radius(b) - 0.1;
            let mid = (pa + pb) * 0.5 + Vec3::new(0.0, 0.07, 0.0);
            let rot = Quat::from_rotation_y(-d.z.atan2(d.x));
            out.push(Item { mesh: self.bar_mesh, model: tr(mid, rot, Vec3::new(len.max(0.1), 0.01, 0.035)), material: PbrMaterial { emissive: [0.18, 0.26, 0.36], ..PbrMaterial::plastic([0.05, 0.07, 0.1]) }, casts: false });
        }
        // buildings
        for sys in 0..SYSTEMS {
            if !s.in_play(sys) {
                continue;
            }
            for sl in 0..2 {
                let off = Vec3::new(if sl == 0 { -0.45 } else { 0.45 }, 0.1, 0.0);
                let age = self.clock - self.city_rise[sys];
                let k = if age < 0.6 { (age / 0.6).clamp(0.02, 1.0) } else { 1.0 };
                let k = 1.0 - (1.0 - k).powi(3);
                match s.slot(sys, sl) {
                    Slot::City { owner, damaged } => {
                        let m = tr(system_pos(sys) + off, Quat::IDENTITY, Vec3::new(0.95, 0.95 * k, 0.95));
                        let mut tm = team_mat(owner);
                        if damaged {
                            tm.base_color = tm.base_color.map(|v| v * 0.35);
                        }
                        out.push(Item { mesh: self.city.team, model: m, material: tm, casts: true });
                        out.push(Item { mesh: self.city.hull, model: m, material: hull_mat, casts: true });
                        if !damaged {
                            out.push(Item { mesh: self.city.glow, model: m, material: city_glow, casts: false });
                        }
                    }
                    Slot::Starport { owner, damaged } => {
                        let m = tr(system_pos(sys) + off - Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Vec3::new(0.6, 0.35 * k, 0.6));
                        let mut tm = team_mat(owner);
                        if damaged {
                            tm.base_color = tm.base_color.map(|v| v * 0.35);
                        }
                        out.push(Item { mesh: self.bar_mesh, model: m, material: tm, casts: true });
                    }
                    Slot::Empty => {}
                }
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
                let cnt = s.ship_count(sys, p) as usize;
                let (scale, _, _) = ship_formation(cnt);
                for i in 0..cnt {
                    let (pos, heading) = ship_slot(sys, p, i, cnt);
                    ship_item(pos, heading, p, scale, bob(sys * 8 + p * 3 + i), true);
                }
            }
        }
        // flying ships
        let mut trail: Vec<Item> = Vec::new();
        if let Some(Anim { fx: Fx::Fly { from, to, n: fl, player }, t, dur, .. }) = &self.anim {
            let k = (t / dur).clamp(0.0, 1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let src_cnt = s.ship_count(*from, *player) as usize;
            let dst_cnt = s.ship_count(*to, *player) as usize + *fl;
            let src_scale = ship_formation(src_cnt + *fl).0;
            let dst_scale = ship_formation(dst_cnt).0;
            for i in 0..*fl {
                let (a, _) = ship_slot(*from, *player, src_cnt - i.min(src_cnt.saturating_sub(1)), src_cnt + *fl);
                let (b, _) = ship_slot(*to, *player, dst_cnt - 1 - i, dst_cnt);
                let mid = a.lerp(b, e) + Vec3::new(0.0, (std::f32::consts::PI * e).sin() * 1.1, 0.0);
                let d = b - a;
                let heading = -d.z.atan2(d.x);
                ship_item(mid, heading, *player, src_scale + (dst_scale - src_scale) * e, 0.0, false);
                // engine trail: fading glow beads behind the ship
                let col = lin(ui::COLORS_UI[*player]);
                for k in 1..=6 {
                    let e2 = (e - k as f32 * 0.035).max(0.0);
                    let pos = a.lerp(b, e2) + Vec3::new(0.0, (std::f32::consts::PI * e2).sin() * 1.1 + 0.05, 0.0);
                    let fade = 1.0 - k as f32 / 7.0;
                    trail.push(Item { mesh: self.sphere_mesh, model: tr(pos, Quat::IDENTITY, Vec3::splat(0.07 * fade + 0.01)), material: PbrMaterial { emissive: col.map(|v| v * 3.0 * fade + 0.5 * fade), ..PbrMaterial::plastic([0.1; 3]) }, casts: false });
                }
            }
        }
        out.extend(trail);
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
                if (0..2).any(|sl| matches!(s.slot(sys, sl), Slot::City { owner, .. } | Slot::Starport { owner, .. } if owner == cur)) {
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
        let mut renderer = Renderer::new(&gl, ctx.drawable_size(), 1.0, include_str!("bloom.frag"))?;
        renderer.set_shadow_settings(&gl, ShadowSettings::smooth());
        renderer.set_exposure(1.0);
        renderer.set_resolution_scale(1.0);
        renderer.set_target_options(&gl, TargetOptions::hifi())?;
        self.renderer = Some(renderer);
        let (v, f) = patch_mesh_shaders(PBR_MESH_VERT, PBR_MESH_FRAG, "FragColor")?;
        self.shaders = Some(ShaderVariantCache::new(v, f));
        self.white = Some(GpuTexture::from_rgba8(&gl, &[255; 4], 1, 1, TextureFilter::Bilinear)?);
        self.build_scene(&gl)?;
        self.galaxy = Some(galaxy::Galaxy::new(&gl)?);
        if site::enabled() {
            // The board and its sky are built: take the page's loading veil down.
            site::ready();
        }
        self.ui = Some(EguiState::new(ctx.gl_arc())?);
        Ok(())
    }

    fn handle_event(&mut self, ctx: &mut Context, event: &Event) {
        if let Some(ui) = self.ui.as_mut() {
            ui.handle_event(event);
        }
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                if ui::close_overlay() {
                    // the overlay took the key
                } else if self.site {
                    // the page owns navigation
                } else if matches!(self.screen, Screen::Play) {
                    self.screen = Screen::Menu;
                } else {
                    ctx.should_quit = true;
                }
            }
            Event::KeyDown { keycode: Some(Keycode::Space), .. } => self.camera.overview(),
            Event::KeyDown { keycode: Some(Keycode::C), .. } => {
                self.cinematic = !self.cinematic;
                self.cine_moved = false;
            }
            Event::KeyDown { keycode: Some(Keycode::L), .. } => {
                if matches!(self.screen, Screen::Play) {
                    ui::toggle_ledger();
                }
            }
            Event::KeyDown { keycode: Some(Keycode::R), .. } => {
                if matches!(self.screen, Screen::Play) {
                    ui::toggle_manual();
                }
            }
            Event::KeyDown { keycode: Some(Keycode::G), .. } => {
                if matches!(self.screen, Screen::Play) {
                    ui::toggle_guide();
                }
            }
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
        self.camera.fit(ctx.aspect_ratio());
        self.camera.update(dt);
        if let Some((_, t)) = self.toast.as_mut() {
            *t -= dt;
        }
        if self.toast.as_ref().is_some_and(|(_, t)| *t <= 0.0) {
            self.toast = None;
        }
        if self.site {
            self.site_update(dt);
        }
        if !matches!(self.screen, Screen::Play) {
            return Ok(());
        }

        // animation queue
        let speed = if self.autoplay { 4.0 } else { 1.0 };
        for f in self.flies.iter_mut() {
            f.t += dt * speed;
        }
        let mut landed = Vec::new();
        self.flies.retain(|f| {
            let done = f.t >= f.dur;
            if done {
                landed.push(f.clone());
            }
            !done
        });
        for f in landed {
            match f.land {
                cards::Land::Lead => self.table.lead = f.card,
                cards::Land::Follow => self.table.follows.push(f.card),
                cards::Land::Discard => {}
            }
        }
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
        if !busy && self.cine_moved {
            self.cine_moved = false;
            self.camera.overview();
        }

        // Screenshot/probe run: stop at the requested action, or settle the
        // final state at game over. The UI then stays still for the browser
        // screenshot and the probe export.
        if self.hold.is_some() {
            if self.truth.is_over() {
                if !self.holding {
                    self.view = self.truth.clone();
                    self.anim = None;
                    self.queue.clear();
                    self.holding = true;
                }
            } else if !busy && self.hold.is_some_and(|h| self.actions_taken >= h) {
                self.holding = true;
            }
        }

        // bots
        if !busy && !self.holding && !self.truth.is_over() && self.ai[self.truth.current()] {
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
        let view_state_play: Vec<bool> = (0..SYSTEMS).map(|y| self.view.in_play(y)).collect();
        let renderer = self.renderer.as_mut().unwrap();
        renderer.resize_if_needed(gl, size)?;
        let view = self.camera.cam.view_matrix();
        let proj = self.camera.cam.projection_matrix(aspect);
        let sun = Vec3::from(self.lighting.sun_dir).normalize();

        let bounds = ShadowBounds::Aabb { min: Vec3::new(-15.0, -1.0, -14.0), max: Vec3::new(15.0, 3.0, 14.0) };
        if renderer.begin_shadow_pass(gl, sun, &bounds) {
            for it in self.statics.iter().zip(&self.static_sys).filter(|(_, sy)| sy.is_none_or(|y| view_state_play[y])).map(|(i, _)| i).chain(dynamic.iter()).filter(|i| i.casts) {
                renderer.shadow_draw(gl, &it.model, &self.meshes[it.mesh]);
            }
            renderer.end_shadow_pass(gl);
        }
        renderer.begin_scene(gl);
        unsafe {
            gl.clear_color(0.006, 0.007, 0.016, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }
        // The sky first: one full-screen pass, no geometry, no depth writes.
        if let Some(g) = self.galaxy.as_ref() {
            let inv = (proj * view).inverse().to_cols_array();
            g.draw_sky(gl, &inv, self.clock);
        }
        unsafe {
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
        for it in self.statics.iter().zip(&self.static_sys).filter(|(_, sy)| sy.is_none_or(|y| view_state_play[y])).map(|(i, _)| i).chain(dynamic.iter()) {
            let mat = PbrMaterial { base_texture: it.material.base_texture.or(Some(white)), ..it.material };
            mat.upload(gl, program);
            unsafe {
                let loc = gl.get_uniform_location(program, "uModel");
                gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, &it.model.to_cols_array());
            }
            self.meshes[it.mesh].draw(gl);
        }
        unsafe { gl.disable(glow::CULL_FACE) };
        // Stars, gas banks and stardust: blended, depth-tested, no writes.
        if let Some(g) = self.galaxy.as_mut() {
            g.draw_glow(gl, &view.to_cols_array(), &proj.to_cols_array(), self.clock);
        }
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
        let labels: Vec<(String, [f32; 2])> = {
            let (ww2, wh2) = ctx.platform.window.size();
            let vp = self.camera.view_proj(aspect);
            (0..6)
                .filter_map(|c| {
                    let p = system_pos(c * 4 + 3);
                    let clip = vp * dust_engine::glam::Vec4::new(p.x, 0.9, p.z, 1.0);
                    (clip.w > 0.0).then(|| {
                        let (nx, ny) = (clip.x / clip.w, clip.y / clip.w);
                        ((c + 1).to_string(), [(nx * 0.5 + 0.5) * ww2 as f32, (1.0 - (ny * 0.5 + 0.5)) * wh2 as f32])
                    })
                })
                .collect()
        };
        let (ww3, wh3) = ctx.platform.window.size();
        self.win = [ww3 as f32, wh3 as f32];
        {
            let vp = self.camera.view_proj(aspect);
            let pt = |x: f32, z: f32| {
                let c = vp * dust_engine::glam::Vec4::new(x, 0.05, z, 1.0);
                [(c.x / c.w * 0.5 + 0.5) * ww3 as f32, (1.0 - (c.y / c.w * 0.5 + 0.5)) * wh3 as f32]
            };
            // Played cards are a horizontal row above the action strip. Keeping
            // them off the map's left edge stops a second card from sliding
            // under the status bar on a tall, narrow window.
            let w = if self.win[0] < 620.0 { 62.0 } else { 84.0 };
            let card_h = w * 136.0 / 96.0;
            let y = (self.win[1] - 360.0 - card_h * 0.5).max(40.0 + card_h * 0.5);
            let lead_x = self.win[0] * 0.5 - w * 1.6;
            let follow_x = lead_x + w * 1.2;
            self.card_slots = cards::Slots {
                lead: [lead_x, y],
                follow: [follow_x, y],
                discard: [follow_x + w * 2.3, y],
                card_w: w,
            };
        }
        let anchors = {
            let vp = self.camera.view_proj(aspect);
            let pt = |x: f32, z: f32| {
                let c = vp * dust_engine::glam::Vec4::new(x, 0.05, z, 1.0);
                [(c.x / c.w * 0.5 + 0.5) * ww3 as f32, (1.0 - (c.y / c.w * 0.5 + 0.5)) * wh3 as f32]
            };
            let gate = system_pos(3);
            guides::Anchors {
                ring: pt(0.0, 0.0),
                gates: pt(gate.x, gate.z),
                court: pt(0.0, -9.0),
                ambitions: pt(11.3, 0.0),
                power: pt(0.0, POWER_Z),
                played: [self.card_slots.follow[0], self.card_slots.follow[1]],
            }
        };
        let (table, flies, card_slots) = (self.table.clone(), self.flies.clone(), self.card_slots);
        let online_hud = self.online_hud.as_ref();
        let site_mode = self.site;
        let (screen_menu, view_state) = (matches!(self.screen, Screen::Menu), self.view.clone());
        let (bots, log, selected) = (self.bots, self.log.clone(), self.selected);
        let menu = &mut self.menu;
        let first_ui = !self.theme_done;
        self.theme_done = true;
        let output = ui_state.run(size, |ectx| {
            if first_ui {
                // fonts take effect on the next pass, so draw nothing this frame
                ui::theme(ectx);
                return;
            }
            if screen_menu && site_mode {
                ui::loading(ectx, "Joining the room…");
            } else if screen_menu {
                start = ui::menu(ectx, menu);
            } else {
                hud_out = ui::hud(
                    ectx,
                    &ui::HudInput { state: &view_state, bots: &bots, busy, selected, log: &log, toast: toast.as_deref(), handoff, cinematic: self.cinematic, labels: &labels, table: &table, flies: &flies, slots: card_slots, anchors: &anchors, online: online_hud },
                );
            }
        });
        ui_state.paint(size, output);
        self.ui_wants_pointer = ui_state.wants_pointer_input() || screen_menu;

        // The live frame is stable: publish the UI probe and mirror it into a
        // hidden DOM node for the browser-side assertions.
        if self.probe && (self.holding || self.view.is_over()) && !self.probe_sent && self.frame > 2 {
            let json = ui::probe_json(&view_state, self.holding);
            site::export_probe(&json, true);
            self.probe_sent = true;
        }

        self.hover_systems = hud_out.hover_systems;
        if start {
            let seed = (self.clock * 1000.0) as u64 ^ 0x9e37;
            self.start_game(seed);
        }
        if hud_out.toggle_cinematic {
            self.cinematic = !self.cinematic;
            if !self.cinematic {
                self.camera.overview();
                self.cine_moved = false;
            }
        }
        if hud_out.new_game {
            if self.site {
                site::leave();
            }
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
        // A shot at game over must show the final standings, even if the last
        // action's cosmetic animation never settled its view state.
        let over = self.truth.is_over();
        if over {
            self.view = self.truth.clone();
            self.anim = None;
            self.queue.clear();
        }
        if (over || self.actions_taken >= self.shot_after)
            && (over || (self.anim.is_none() && self.queue.is_empty()))
        {
            self.shot_pending += 1;
            if self.shot_pending == 12 {
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
    let args: Vec<String> = std::env::args().collect();
    let (w, h) = arg_value(&args, "--size")
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1280, 720));
    App::run("Ember Reach", w, h, Ember::new())
}
