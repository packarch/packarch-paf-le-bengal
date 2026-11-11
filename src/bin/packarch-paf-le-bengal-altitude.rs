//! Paf le Bengal : Altitude — Baby Kart fait tournoyer le Gremlin du Bengal par la
//! queue et l'envoie vers le ciel. On le dirige pour rebondir de pelote en pelote
//! et monter le plus haut possible.
//!
//! Commandes :
//!   Clic / Espace (maintenu) : faire tourner le bengal, relacher pour le lancer
//!   Souris / fleches         : diriger le bengal en l'air
//!   G                        : activer / desactiver le gore
//!   Echap                    : quitter

use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// Constantes de jeu (unites : metres, secondes)
// ---------------------------------------------------------------------------

const G: f32 = 9.81;
const DT: f32 = 1.0 / 120.0;
const DRAG: f32 = 0.0022; // trainee quadratique

// Lancer par la queue : la main de Baby Kart tient la queue, le bengal tourne autour.
const SHOULDER: Vec2 = Vec2::new(-0.52, 1.82);
const HAND_READY: Vec2 = Vec2::new(0.08, 2.05); // bras tendu, le bengal pend
const HAND_SPIN: Vec2 = Vec2::new(-0.3, 2.45); // main au-dessus de la tete
const HAND_REST: Vec2 = Vec2::new(0.0, 1.35); // bras redescendu apres le lancer
const TAIL_LEN: f32 = 0.75; // de la main au centre du bengal
const OMEGA_MAX: f32 = 11.0; // vitesse de rotation max (rad/s), ~1,75 tour/s
const SPIN_ACCEL: f32 = 2.5; // acceleration de depart (rad/s^2)
const SPIN_GROWTH: f32 = 0.6; // plus ca tourne vite, plus ca accelere
const RELEASE_ZONE: f32 = 35.0 * PI / 180.0; // lacher ideal : +/- 35 deg autour de la verticale
const GRIP_LIMIT: f32 = 2.2; // secondes a pleine vitesse avant que la queue lache

const CAT_R: f32 = 0.2;
const CAT_HIT: f32 = 0.3; // rayon de contact avec les pelotes (corps + pattes)

/// Fichier du record, dans ~/.local/share/packarch-paf-le-bengal/
const SAVE_FILE: &str = "record_altitude.txt";

// Couleurs
const TABBY: Color = Color::new(0.6, 0.6, 0.62, 1.0); // chat tigre gris
const TABBY_SHADE: Color = Color::new(0.47, 0.47, 0.5, 1.0);
const TABBY_LIGHT: Color = Color::new(0.93, 0.93, 0.9, 1.0);
const TABBY_DARK: Color = Color::new(0.17, 0.17, 0.19, 1.0);
const CREAM: Color = Color::new(0.98, 0.93, 0.82, 1.0);
const BENGAL: Color = Color::new(0.86, 0.62, 0.30, 1.0);
const BENGAL_SPOT: Color = Color::new(0.30, 0.17, 0.08, 1.0);
const BENGAL_MID: Color = Color::new(0.62, 0.38, 0.16, 1.0);

fn blood(shade: f32) -> Color {
    Color::new(0.55 + 0.3 * shade, 0.0, 0.02 + 0.04 * shade, 1.0)
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

// ---------------------------------------------------------------------------
// Camera / transformation monde -> ecran
// ---------------------------------------------------------------------------

struct View {
    cam: Vec2,
    s: f32, // pixels par metre
    ground_sy: f32,
    anchor_x: f32,
    shake: Vec2,
    zoom: f32, // multiplicateur temporaire (effet de "punch" a l'impact)
}

impl View {
    fn p(&self, w: Vec2) -> Vec2 {
        let s = self.s * self.zoom;
        vec2(
            self.anchor_x + (w.x - self.cam.x) * s + self.shake.x,
            self.ground_sy - (w.y - self.cam.y) * s + self.shake.y,
        )
    }
    fn l(&self, m: f32) -> f32 {
        (m * self.s * self.zoom).max(1.0)
    }
}

fn ellipse(v: &View, c: Vec2, rx: f32, ry: f32, ang: f32, col: Color) {
    const N: usize = 22;
    let rot = Vec2::from_angle(ang);
    let center = v.p(c);
    let pt = |i: usize| {
        let t = i as f32 / N as f32 * TAU;
        v.p(c + rot.rotate(vec2(rx * t.cos(), ry * t.sin())))
    };
    let mut prev = pt(0);
    for i in 1..=N {
        let cur = pt(i);
        draw_triangle(center, prev, cur, col);
        prev = cur;
    }
}

fn seg(v: &View, a: Vec2, b: Vec2, th: f32, col: Color) {
    let (pa, pb) = (v.p(a), v.p(b));
    let t = v.l(th);
    draw_line(pa.x, pa.y, pb.x, pb.y, t, col);
    draw_circle(pa.x, pa.y, t * 0.5, col);
    draw_circle(pb.x, pb.y, t * 0.5, col);
}

fn tri(v: &View, a: Vec2, b: Vec2, c: Vec2, col: Color) {
    draw_triangle(v.p(a), v.p(b), v.p(c), col);
}

fn quad(v: &View, a: Vec2, b: Vec2, c: Vec2, d: Vec2, col: Color) {
    tri(v, a, b, c, col);
    tri(v, a, c, d, col);
}

// ---------------------------------------------------------------------------
// Entites gore
// ---------------------------------------------------------------------------

struct Particle {
    pos: Vec2,
    vel: Vec2,
    size: f32,
    shade: f32,
    life: f32,
}

struct Decal {
    x: f32,
    w: f32,
    shade: f32,
}

struct Piece {
    pos: Vec2,
    vel: Vec2,
    ang: f32,
    spin: f32,
    size: f32,
}

struct Splat {
    pos: Vec2,
    r: f32,
    life: f32,
    max_life: f32,
    drip: f32,
    drip_speed: f32,
    sats: [(Vec2, f32); 5],
}

#[derive(Clone, Copy, PartialEq)]
enum PartKind {
    Tail,
}

struct BodyPart {
    kind: PartKind,
    pos: Vec2,
    vel: Vec2,
    ang: f32,
    spin: f32,
    r: f32, // rayon de contact avec le sol
    bleed: f32,
}


// ---------------------------------------------------------------------------
// Objets du ciel
// ---------------------------------------------------------------------------

const FIELD_HALF: f32 = 6.0; // demi-largeur du terrain de jeu (m)
const SPACE_START: f32 = 90.0; // le ciel commence a virer au noir spatial
const SPACE_FULL: f32 = 140.0; // au-dela : l'espace
const MAX_VX: f32 = 7.0; // vitesse laterale max du bengal

/// Pelote de laine geante : on rebondit dessus.
struct Yarn {
    pos: Vec2,
    r: f32,
    color: Color,
    vx: f32,
    base_y: f32, // altitude autour de laquelle la pelote flotte
    bob: f32,    // amplitude du flottement vertical
    phase: f32,
    squish: f32,
    stain: f32,
    spin: f32,
}

/// Une pelote au hasard : plus on monte, plus elles bougent vite.
fn new_yarn(x: f32, y: f32, r: f32, diff: f32) -> Yarn {
    let moving = gen_range(0.0, 1.0) < 0.55 + 0.35 * diff;
    Yarn {
        pos: vec2(x, y),
        r,
        color: yarn_color(),
        vx: if moving { gen_range(0.8, 2.0 + 1.8 * diff) * rand_sign() } else { 0.0 },
        base_y: y,
        bob: gen_range(0.15, 0.5 + 0.4 * diff),
        phase: gen_range(0.0, TAU),
        squish: 0.0,
        stain: 0.0,
        spin: gen_range(0.0, TAU),
    }
}

/// Pigeon : explose au contact et donne un petit coup de pouce.
struct Bird {
    pos: Vec2,
    vx: f32,
    flap: f32,
}

/// Drone-aspirateur : casse l'elan du bengal.
struct Drone {
    pos: Vec2,
    vx: f32,
    hit_cd: f32,
}

/// Sardine doree : propulsion turbo.
struct Fish {
    pos: Vec2,
    taken: bool,
}

fn yarn_color() -> Color {
    let colors = [
        Color::new(0.85, 0.25, 0.35, 1.0),
        Color::new(0.25, 0.55, 0.85, 1.0),
        Color::new(0.95, 0.75, 0.2, 1.0),
        Color::new(0.45, 0.75, 0.35, 1.0),
        Color::new(0.65, 0.4, 0.8, 1.0),
        Color::new(0.95, 0.55, 0.25, 1.0),
    ];
    colors[gen_range(0, colors.len())]
}

fn darker(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Pseudo-aleatoire deterministe (decor qui ne "saute" pas d'une image a l'autre).
fn hash(i: i64, salt: u64) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x % 10_000) as f32 / 10_000.0
}

fn rand_sign() -> f32 {
    if gen_range(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 }
}

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Perched,
    Spinning,
    Rising,
    Plunge,
    Done,
}

struct Launch {
    quality: f32,
    speed: f32,
}

struct Game {
    phase: Phase,
    phase_t: f32,
    pos: Vec2,
    vel: Vec2,
    ang: f32,
    spin: f32,
    theta: f32,    // angle du bengal autour de la main
    omega: f32,    // vitesse de rotation
    grip_t: f32,   // temps passe a pleine vitesse
    throw_t: f32,  // temps depuis le lacher
    tailless: bool,
    bat_blood: f32,
    damage: f32,
    max_height: f32,
    launch: Option<Launch>,

    particles: Vec<Particle>,
    decals: Vec<Decal>,
    pieces: Vec<Piece>,
    splats: Vec<Splat>,
    parts: Vec<BodyPart>,

    yarns: Vec<Yarn>,
    birds: Vec<Bird>,
    drones: Vec<Drone>,
    fishes: Vec<Fish>,
    next_gen_y: f32,
    combo: u32,
    bounces: u32,
    steer: f32,
    mouse_ctrl: bool,
    last_mouse: Vec2,
    cam_floor: f32,
    plunge_h0: f32,   // altitude au debut de la chute libre
    plunge_dur: f32,  // duree de la chute (s)
    plunge_off0: f32, // decalage camera/chat au debut de la chute
    space_reached: bool,

    view: View,
    trauma: f32,     // intensite de secousse (0..1), decroit
    kick: Vec2,      // recul directionnel de la camera (px)
    kick_vel: Vec2,
    punch: f32,      // petit zoom a l'impact
    paf: Option<(Vec2, f32, &'static str)>, // onomatopee (position monde, age, texte)
    hitstop: f32,
    time: f32,

    best: f32,
    history: Vec<f32>,
    msg: String,
    msg_t: f32,
    new_record: bool,
    gore: bool,
}

fn ready_pos() -> Vec2 {
    HAND_READY - vec2(0.0, TAIL_LEN)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Coude d'un bras a deux segments (cinematique inverse).
fn elbow(shoulder: Vec2, hand: Vec2, l1: f32, l2: f32, bend: f32) -> Vec2 {
    let d = (hand - shoulder).length().clamp(0.01, l1 + l2 - 0.001);
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let base = (hand - shoulder).normalize_or_zero();
    shoulder + base.rotate(Vec2::from_angle(cos_a.acos() * bend)) * l1
}

impl Game {
    fn new() -> Self {
        let best = packarch_paf_le_bengal::load_record(SAVE_FILE);
        let mut g = Game {
            phase: Phase::Perched,
            phase_t: 0.0,
            pos: ready_pos(),
            vel: Vec2::ZERO,
            ang: 0.0,
            spin: 0.0,
            theta: -PI / 2.0,
            omega: 0.0,
            grip_t: 0.0,
            throw_t: 10.0,
            tailless: false,
            bat_blood: 0.0,
            damage: 0.0,
            max_height: 0.0,
            launch: None,
            particles: Vec::new(),
            decals: Vec::new(),
            pieces: Vec::new(),
            splats: Vec::new(),
            parts: Vec::new(),
            yarns: Vec::new(),
            birds: Vec::new(),
            drones: Vec::new(),
            fishes: Vec::new(),
            next_gen_y: 7.0,
            combo: 0,
            bounces: 0,
            steer: 0.0,
            mouse_ctrl: true,
            last_mouse: Vec2::ZERO,
            cam_floor: 0.0,
            plunge_h0: 0.0,
            plunge_dur: 1.0,
            plunge_off0: 0.0,
            space_reached: false,
            view: View {
                cam: vec2(0.0, 0.0),
                s: 90.0,
                ground_sy: 600.0,
                anchor_x: 600.0,
                shake: Vec2::ZERO,
                zoom: 1.0,
            },
            trauma: 0.0,
            kick: Vec2::ZERO,
            kick_vel: Vec2::ZERO,
            punch: 0.0,
            paf: None,
            hitstop: 0.0,
            time: 0.0,
            best,
            history: Vec::new(),
            msg: String::new(),
            msg_t: 0.0,
            new_record: false,
            gore: true,
        };
        g.reset();
        g
    }

    fn reset(&mut self) {
        self.set_phase(Phase::Perched);
        self.pos = ready_pos();
        self.vel = Vec2::ZERO;
        self.ang = 0.0;
        self.spin = 0.0;
        self.theta = -PI / 2.0;
        self.omega = 0.0;
        self.grip_t = 0.0;
        self.throw_t = 10.0;
        self.tailless = false;
        self.damage = 0.0;
        self.max_height = 0.0;
        self.launch = None;
        self.new_record = false;
        self.paf = None;
        self.combo = 0;
        self.bounces = 0;
        self.cam_floor = 0.0;
        self.space_reached = false;
        self.particles.clear();
        self.splats.clear();
        self.pieces.clear();
        self.parts.clear();
        self.decals.clear();
        self.yarns.clear();
        self.birds.clear();
        self.drones.clear();
        self.fishes.clear();
        self.next_gen_y = 7.0;
        self.generate(45.0);
        self.view.cam = Vec2::ZERO;
    }

    fn set_phase(&mut self, p: Phase) {
        self.phase = p;
        self.phase_t = 0.0;
    }

    fn say(&mut self, s: &str) {
        self.msg = s.to_string();
        self.msg_t = 2.2;
    }

    /// Fabrique le ciel au fur et a mesure de la montee (de plus en plus dur).
    fn generate(&mut self, up_to: f32) {
        while self.next_gen_y < up_to {
            let y = self.next_gen_y;
            let diff = (y / 300.0).min(1.0);
            let r = 0.8 - 0.35 * diff;
            let x = gen_range(-FIELD_HALF + 0.9, FIELD_HALF - 0.9);
            self.yarns.push(new_yarn(x, y, r, diff));
            // souvent une deuxieme, parfois une troisieme pelote au meme etage
            if gen_range(0.0, 1.0) < 0.7 {
                let x2 = (x + gen_range(3.0, 7.0) * rand_sign()).clamp(-FIELD_HALF + 0.9, FIELD_HALF - 0.9);
                self.yarns.push(new_yarn(x2, y + gen_range(-0.8, 0.8), r, diff));
            }
            if gen_range(0.0, 1.0) < 0.3 {
                let x3 = gen_range(-FIELD_HALF + 0.9, FIELD_HALF - 0.9);
                self.yarns.push(new_yarn(x3, y + gen_range(-1.0, 1.0), r, diff));
            }
            if y > 12.0 && y < SPACE_START && gen_range(0.0, 1.0) < 0.22 {
                let side = rand_sign();
                self.birds.push(Bird {
                    pos: vec2(side * (FIELD_HALF + 0.5), y + gen_range(1.0, 2.0)),
                    vx: -side * gen_range(1.5, 3.0),
                    flap: gen_range(0.0, TAU),
                });
            }
            if y > 35.0 && gen_range(0.0, 1.0) < 0.10 + 0.12 * diff {
                self.drones.push(Drone {
                    pos: vec2(gen_range(-FIELD_HALF + 1.0, FIELD_HALF - 1.0), y + gen_range(1.5, 2.5)),
                    vx: gen_range(0.8, 2.0) * rand_sign(),
                    hit_cd: 0.0,
                });
            }
            if y > 15.0 && gen_range(0.0, 1.0) < 0.05 {
                self.fishes.push(Fish {
                    pos: vec2(gen_range(-FIELD_HALF + 1.0, FIELD_HALF - 1.0), y + gen_range(1.0, 2.0)),
                    taken: false,
                });
            }
            self.next_gen_y += 2.4 + 3.0 * diff + gen_range(0.0, 1.0);
        }
    }

    // ---------------------------------------------------------------- input
    fn input(&mut self) {
        if is_key_pressed(KeyCode::G) {
            self.gore = !self.gore;
        }

        // pilotage : fleches / A-D, ou la souris des qu'elle bouge
        let (mx, my) = mouse_position();
        let m = vec2(mx, my);
        if (m - self.last_mouse).length() > 2.0 {
            self.mouse_ctrl = true;
        }
        self.last_mouse = m;
        let mut dir = 0.0;
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) || is_key_down(KeyCode::Q) {
            dir -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            dir += 1.0;
        }
        if dir != 0.0 {
            self.mouse_ctrl = false;
            self.steer = dir * MAX_VX;
        } else if self.mouse_ctrl {
            let s = self.view.s * self.view.zoom;
            let wx = (m.x - self.view.anchor_x) / s + self.view.cam.x;
            self.steer = ((wx - self.pos.x) * 3.0).clamp(-MAX_VX, MAX_VX);
        } else {
            self.steer = 0.0;
        }

        let pressed = is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Space);
        let held = is_mouse_button_down(MouseButton::Left) || is_key_down(KeyCode::Space);
        match self.phase {
            Phase::Perched if pressed => {
                self.set_phase(Phase::Spinning);
                self.omega = 1.5;
                self.grip_t = 0.0;
            }
            Phase::Spinning if !held => self.release(1.0),
            Phase::Done if pressed && self.phase_t > if self.gore { 1.8 } else { 0.6 } => self.reset(),
            _ => {}
        }
    }

    // ---------------------------------------------------------------- physique
    fn step(&mut self, dt: f32) {
        self.time += dt;
        self.msg_t -= dt;
        self.update_splats(dt);
        if let Some((_, age, _)) = &mut self.paf {
            *age += dt;
            if *age > 0.5 {
                self.paf = None;
            }
        }

        if self.hitstop > 0.0 {
            self.hitstop -= dt;
            return;
        }
        self.phase_t += dt;
        self.bat_blood = (self.bat_blood - dt * 0.02).max(0.0);

        self.throw_t += dt;

        // le ciel s'anime
        for y in self.yarns.iter_mut() {
            y.squish *= (-7.0 * dt).exp();
            y.pos.x += y.vx * dt;
            y.pos.y = y.base_y + (self.time * 1.6 + y.phase).sin() * y.bob;
            if y.pos.x.abs() > FIELD_HALF - y.r {
                y.pos.x = y.pos.x.clamp(-FIELD_HALF + y.r, FIELD_HALF - y.r);
                y.vx = -y.vx;
            }
            y.spin += y.vx * dt / y.r * 0.5;
        }
        for b in self.birds.iter_mut() {
            b.pos.x += b.vx * dt;
            b.flap += dt * 14.0;
            if b.pos.x.abs() > FIELD_HALF + 1.5 {
                b.vx = -b.vx;
                b.pos.x = b.pos.x.clamp(-FIELD_HALF - 1.5, FIELD_HALF + 1.5);
            }
        }
        for d in self.drones.iter_mut() {
            d.pos.x += d.vx * dt;
            d.hit_cd -= dt;
            if d.pos.x.abs() > FIELD_HALF - 0.5 {
                d.pos.x = d.pos.x.clamp(-FIELD_HALF + 0.5, FIELD_HALF - 0.5);
                d.vx = -d.vx;
            }
        }

        match self.phase {
            Phase::Perched => {
                // le bengal pend par la queue et se balance
                self.theta = -PI / 2.0 + (self.time * 2.5).sin() * 0.18;
                self.pos = self.hand_pos() + Vec2::from_angle(self.theta) * TAIL_LEN;
                self.ang = self.theta + PI;
                self.vel = Vec2::ZERO;
            }
            Phase::Spinning => {
                // l'elan s'emballe : l'acceleration grandit avec la vitesse
                self.omega = (self.omega + (SPIN_ACCEL + SPIN_GROWTH * self.omega) * dt).min(OMEGA_MAX);
                self.theta += self.omega * dt;
                self.pos = self.hand_pos() + Vec2::from_angle(self.theta) * TAIL_LEN;
                self.ang = self.theta + PI;
                self.vel = Vec2::from_angle(self.theta + PI / 2.0) * self.omega * TAIL_LEN;
                if self.omega >= OMEGA_MAX {
                    self.grip_t += dt;
                    if self.grip_t > GRIP_LIMIT {
                        self.rip_tail();
                    }
                }
            }
            Phase::Rising => {
                self.vel.x += (self.steer - self.vel.x) * (1.0 - (-6.0 * dt).exp());
                self.vel.y -= G * dt;
                let sp = self.vel.length();
                self.vel -= self.vel * sp * DRAG * dt;
                let old_pos = self.pos;
                self.pos += self.vel * dt;
                if self.pos.x.abs() > FIELD_HALF {
                    self.pos.x = self.pos.x.clamp(-FIELD_HALF, FIELD_HALF);
                    self.vel.x = -self.vel.x * 0.5;
                }
                self.ang += self.spin * dt;
                self.spin *= (-1.2 * dt).exp();
                self.max_height = self.max_height.max(self.pos.y - CAT_R);
                if !self.space_reached && self.pos.y > SPACE_FULL {
                    self.space_reached = true;
                    self.say("BIENVENUE DANS L'ESPACE !");
                }

                // traine de gouttes quand le chat est bien amoche
                if self.gore && self.damage > 0.25 && gen_range(0.0, 1.0) < self.damage * 0.4 {
                    self.particles.push(Particle {
                        pos: self.pos,
                        vel: self.vel * 0.3 + vec2(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)),
                        size: gen_range(0.02, 0.05),
                        shade: gen_range(0.0, 1.0),
                        life: 6.0,
                    });
                }

                self.collide(old_pos);
                let top = self.view.cam.y + 45.0;
                self.generate(top);

                // tombe par le bas de l'ecran : chute libre !
                let bottom = self.view.cam.y - (screen_height() - self.view.ground_sy) / self.view.s;
                if self.vel.y < 0.0 && self.view.cam.y > 1.0 && self.pos.y < bottom - 0.6 {
                    self.combo = 0;
                    self.say("CHUTE LIBRE !");
                    self.set_phase(Phase::Plunge);
                    self.plunge_h0 = self.pos.y;
                    self.plunge_dur = (0.9 + self.pos.y / 120.0).clamp(1.0, 2.6);
                    self.plunge_off0 = self.view.cam.y - self.pos.y;
                    // plus rien sous l'ecran : on ne doit pas voir le bengal traverser des objets
                    self.yarns.retain(|y| y.pos.y - y.r > bottom);
                    self.birds.retain(|b| b.pos.y > bottom);
                    self.drones.retain(|d| d.pos.y > bottom);
                    self.fishes.retain(|f| f.pos.y > bottom);
                } else if self.pos.y <= CAT_R {
                    self.ground_impact();
                }
            }
            Phase::Plunge => {
                // chute continue et acceleree, de duree bornee quelle que soit la hauteur :
                // la camera traverse tous les decors sans coupure
                self.vel.x *= (-2.0 * dt).exp();
                self.pos.x += self.vel.x * dt;
                self.ang += self.spin * dt;
                let k = (self.phase_t / self.plunge_dur).min(1.0);
                let h0 = self.plunge_h0 - CAT_R;
                self.pos.y = CAT_R + h0 * (1.0 - k * k);
                self.vel.y = -2.0 * h0 * k / self.plunge_dur;
                if k >= 1.0 {
                    self.vel.y = self.vel.y.max(-45.0); // impact violent mais borne
                    self.ground_impact();
                }
            }
            Phase::Done => {}
        }

        // menage : ce qui est loin sous la camera disparait
        let low = self.view.cam.y - 15.0;
        if self.phase == Phase::Rising {
            self.yarns.retain(|y| y.pos.y > low);
            self.birds.retain(|b| b.pos.y > low);
            self.drones.retain(|d| d.pos.y > low);
            self.fishes.retain(|f| f.pos.y > low && !f.taken);
        }
        let low_p = self.view.cam.y - 25.0;
        self.particles.retain(|p| p.pos.y > low_p || p.pos.y < 3.0);

        self.update_particles(dt);
        self.update_pieces(dt);
        self.update_parts(dt);
    }

    fn ground_impact(&mut self) {
        let impact = (-self.vel.y).max(12.0);
        self.pos.y = 0.0;
        self.splat_gore(impact);
        self.vel = Vec2::ZERO;
        self.ang = 0.0;
        self.finish();
    }

    /// Rebonds, pigeons, drones et sardines.
    fn collide(&mut self, old_pos: Vec2) {
        // Pelotes : on les traverse en montant, on rebondit dessus en descendant.
        // Test balaye entre l'ancienne et la nouvelle position pour ne rien rater,
        // avec un rayon proche de la taille dessinee du bengal.
        if self.vel.y < 0.0 {
            const SUB: usize = 6;
            'sweep: for k in 1..=SUB {
                let p = old_pos.lerp(self.pos, k as f32 / SUB as f32);
                for i in 0..self.yarns.len() {
                    let y = &self.yarns[i];
                    if p.distance(y.pos) < y.r + CAT_HIT {
                        self.pos = p;
                        self.bounce_yarn(i);
                        break 'sweep;
                    }
                }
            }
        }

        if let Some(i) = self.birds.iter().position(|b| b.pos.distance(self.pos) < 0.45) {
            let b = self.birds.remove(i);
            for _ in 0..30 {
                self.particles.push(Particle {
                    pos: b.pos + vec2(gen_range(-0.15, 0.15), gen_range(-0.1, 0.1)),
                    vel: vec2(gen_range(-3.0, 3.0), gen_range(-1.0, 3.0)),
                    size: gen_range(0.04, 0.08),
                    shade: -3.0, // plume
                    life: gen_range(1.5, 3.0),
                });
            }
            if self.gore {
                for _ in 0..40 {
                    self.particles.push(Particle {
                        pos: b.pos,
                        vel: vec2(gen_range(-5.0, 5.0), gen_range(-2.0, 6.0)),
                        size: gen_range(0.015, 0.05),
                        shade: gen_range(0.0, 1.0),
                        life: 6.0,
                    });
                }
                self.pieces.push(Piece {
                    pos: b.pos,
                    vel: vec2(gen_range(-3.0, 3.0), gen_range(1.0, 4.0)),
                    ang: 0.0,
                    spin: gen_range(-10.0, 10.0),
                    size: 0.07,
                });
            }
            self.vel.y = self.vel.y.max(0.0) + 9.0;
            self.combo += 1;
            self.add_trauma(0.35);
            self.punch = 0.05;
            self.paf = Some((b.pos, 0.0, "SPLOTCH !"));
            self.say("Pigeon en miettes !");
        }

        if let Some(i) = self
            .drones
            .iter()
            .position(|d| d.hit_cd <= 0.0 && d.pos.distance(self.pos) < 0.6)
        {
            let side = (self.pos.x - self.drones[i].pos.x).signum();
            self.drones[i].hit_cd = 1.0;
            self.vel.y = self.vel.y.min(-3.0);
            self.vel.x += side * 4.0;
            self.spin = gen_range(-20.0, 20.0);
            self.damage = (self.damage + 0.25).min(1.0);
            self.combo = 0;
            self.add_trauma(0.5);
            self.kick_vel += vec2(0.0, 500.0);
            self.paf = Some((self.pos, 0.0, "VRRRZZ !"));
            self.say("ASPIRE !");
            if self.gore {
                for _ in 0..50 {
                    self.particles.push(Particle {
                        pos: self.pos,
                        vel: vec2(gen_range(-4.0, 4.0), gen_range(-4.0, 3.0)),
                        size: gen_range(0.015, 0.06),
                        shade: gen_range(0.0, 1.0),
                        life: 6.0,
                    });
                }
                self.pieces.push(Piece {
                    pos: self.pos,
                    vel: vec2(gen_range(-3.0, 3.0), gen_range(-1.0, 2.0)),
                    ang: 0.0,
                    spin: gen_range(-15.0, 15.0),
                    size: 0.06,
                });
            }
        }

        if let Some(i) = self
            .fishes
            .iter()
            .position(|f| !f.taken && f.pos.distance(self.pos) < 0.5)
        {
            self.fishes[i].taken = true;
            let fp = self.fishes[i].pos;
            self.vel.y = self.vel.y.max(0.0) + 26.0;
            self.spin = -12.0;
            self.combo += 1;
            self.add_trauma(0.3);
            self.punch = 0.08;
            self.paf = Some((fp, 0.0, "MIAM !"));
            self.say("SARDINE TURBO !");
            for _ in 0..50 {
                self.particles.push(Particle {
                    pos: fp,
                    vel: Vec2::from_angle(gen_range(0.0, TAU)) * gen_range(1.0, 5.0),
                    size: gen_range(0.03, 0.07),
                    shade: -4.0, // paillette
                    life: gen_range(0.6, 1.4),
                });
            }
        }
    }

    fn bounce_yarn(&mut self, i: usize) {
        let (ypos, yr) = (self.yarns[i].pos, self.yarns[i].r);
        let d = self.pos - ypos;
        let centered = (1.0 - d.x.abs() / yr).clamp(0.0, 1.0);
        let perfect = centered > 0.75;
        self.combo += 1;
        self.bounces += 1;
        // un rebond ne freine jamais un bengal qui monte deja plus vite
        self.vel.y = self.vel.y.max(19.0 + 7.0 * centered + self.combo.min(15) as f32 * 0.7);
        self.vel.x += d.x * 2.5;
        // on repousse le bengal au-dessus de la pelote
        let n = vec2(d.x, d.y.max(0.3 * yr)).normalize_or_zero();
        self.pos = ypos + n * (yr + CAT_HIT + 0.02);
        self.spin = gen_range(-8.0, 8.0);
        self.yarns[i].squish = 1.0;
        self.add_trauma(if perfect { 0.3 } else { 0.15 });
        self.punch = if perfect { 0.05 } else { 0.02 };
        if perfect {
            self.paf = Some((ypos + vec2(0.0, yr), 0.0, "BOING !"));
            self.say("Rebond parfait !");
        } else if self.combo >= 3 && self.combo % 3 == 0 {
            let txt = format!("COMBO x{} !", self.combo);
            self.say(&txt);
        }
        if self.gore {
            self.yarns[i].stain = (self.yarns[i].stain + 0.3).min(1.0);
            self.damage = (self.damage + 0.03).min(1.0);
            for _ in 0..10 {
                self.particles.push(Particle {
                    pos: self.pos - vec2(0.0, CAT_R),
                    vel: vec2(gen_range(-2.5, 2.5), gen_range(-1.0, 2.5)),
                    size: gen_range(0.015, 0.04),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
        }
    }

    fn update_parts(&mut self, dt: f32) {
        let mut drops = Vec::new();
        for p in self.parts.iter_mut() {
            p.bleed = (p.bleed - dt).max(0.0);
            let moving = p.vel != Vec2::ZERO;
            if p.bleed > 0.0 && gen_range(0.0, 1.0) < 0.35 {
                drops.push(Particle {
                    pos: p.pos,
                    vel: p.vel * 0.2 + vec2(gen_range(-0.6, 0.6), gen_range(0.0, 1.2)),
                    size: gen_range(0.012, 0.035),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
            if !moving {
                continue;
            }
            p.vel.y -= G * dt;
            p.pos += p.vel * dt;
            p.ang += p.spin * dt;
            if p.pos.y <= p.r {
                p.pos.y = p.r;
                if p.vel.y < -1.5 {
                    p.vel.y = -p.vel.y * 0.3;
                    p.vel.x *= 0.6;
                    p.spin *= 0.6;
                    if p.bleed > 0.0 {
                        drops.push(Particle {
                            pos: vec2(p.pos.x, 0.01),
                            vel: Vec2::ZERO,
                            size: 0.06,
                            shade: 0.3,
                            life: 6.0,
                        });
                    }
                } else {
                    p.vel.y = 0.0;
                    p.vel.x *= 1.0 - 5.0 * dt;
                    p.spin *= 1.0 - 8.0 * dt;
                    if p.vel.x.abs() < 0.05 {
                        p.vel = Vec2::ZERO;
                    }
                }
            }
        }
        if self.gore {
            self.particles.extend(drops);
        }
    }

    fn draw_part(&self, p: &BodyPart) {
        let v = &self.view;
        let rot = Vec2::from_angle(p.ang);
        let t = |l: Vec2| p.pos + rot.rotate(l);
        let a = p.ang;
        let gore = self.gore;
        let flesh = blood(0.9);
        match p.kind {
            PartKind::Tail => {
                let mut prev = t(vec2(-0.13, 0.0));
                for i in 1..=8 {
                    let k = i as f32 / 8.0;
                    let pt = t(vec2(-0.13 + 0.26 * k, (k * 4.0).sin() * 0.03));
                    seg(v, prev, pt, 0.05, if i >= 7 { BENGAL_SPOT } else { BENGAL });
                    prev = pt;
                }
                if gore {
                    ellipse(v, t(vec2(-0.14, 0.0)), 0.025, 0.028, a, flesh);
                }
            }
        }
    }

    fn add_trauma(&mut self, x: f32) {
        self.trauma = (self.trauma + x).min(1.0);
    }

    /// Position de la main qui tient la queue.
    fn hand_pos(&self) -> Vec2 {
        match self.phase {
            Phase::Perched => HAND_READY + vec2(0.0, (self.time * 2.2).sin() * 0.015),
            Phase::Spinning => {
                // la main monte au-dessus de la tete puis decrit un petit cercle
                let k = smooth(self.phase_t / 0.25);
                HAND_READY.lerp(HAND_SPIN, k) + Vec2::from_angle(self.theta + PI) * 0.08 * k
            }
            _ => HAND_SPIN.lerp(HAND_REST, smooth(self.throw_t / 0.6)),
        }
    }

    /// Baby Kart lache la queue : le bengal part dans la direction ou il tournait.
    fn release(&mut self, factor: f32) {
        let power = self.omega / OMEGA_MAX;
        // ecart entre la direction du bengal au lacher et la verticale
        let tangent = Vec2::from_angle(self.theta + PI / 2.0);
        let dev = tangent.x.atan2(tangent.y);
        // coup de pouce : on redresse la trajectoire de moitie vers la verticale
        let dev2 = if dev.abs() <= PI / 2.0 { dev * 0.5 } else { dev - dev.signum() * PI / 4.0 };
        let dir = vec2(dev2.sin(), dev2.cos());
        let align = (1.0 - (dev.abs() - RELEASE_ZONE).max(0.0) / (PI / 2.0 - RELEASE_ZONE)).clamp(0.0, 1.0);
        let quality = (power * align).clamp(0.0, 1.0);
        let mut speed = 36.0 * (0.25 + 0.75 * power) * factor * gen_range(0.97, 1.03);
        let label = if dir.y < 0.2 {
            "Lance dans le decor..."
        } else if quality > 0.9 {
            speed *= 1.08;
            "LANCER PARFAIT !"
        } else if quality > 0.7 {
            "Joli lancer !"
        } else if quality > 0.4 {
            "Pas mal"
        } else {
            "Lancer foireux..."
        };
        self.say(label);

        self.vel = vec2(dir.x * speed * 0.6, dir.y * speed);
        self.spin = -self.omega * 1.5;
        self.throw_t = 0.0;
        self.launch = Some(Launch { quality, speed });
        self.add_trauma(0.15 + 0.3 * quality);
        self.kick_vel += vec2(dir.x, -dir.y) * (200.0 + 500.0 * quality);
        if quality > 0.6 {
            self.punch = 0.05 + 0.06 * quality;
            self.paf = Some((self.pos, 0.0, "ZIOUUU !"));
        }
        if self.gore && power > 0.7 && !self.tailless {
            // queue tiree a fond : ca saigne a la base
            for _ in 0..12 {
                self.particles.push(Particle {
                    pos: self.pos,
                    vel: self.vel * 0.2 + vec2(gen_range(-1.5, 1.5), gen_range(-1.5, 1.5)),
                    size: gen_range(0.015, 0.04),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
            self.damage = (self.damage + 0.1).min(1.0);
        }
        self.set_phase(Phase::Rising);
    }

    /// Trop tenu a pleine vitesse : la queue s'arrache et le bengal part de travers.
    fn rip_tail(&mut self) {
        let hand = self.hand_pos();
        let base = self.pos;
        let theta = self.theta;
        self.tailless = true;
        self.release(0.6);
        self.say("LA QUEUE A LACHE !");
        self.paf = Some((base, 0.0, "CRAC !"));
        self.add_trauma(0.5);
        self.parts.push(BodyPart {
            kind: PartKind::Tail,
            pos: hand.lerp(base, 0.4),
            vel: vec2(gen_range(-1.0, 1.0), gen_range(1.0, 3.0)),
            ang: theta,
            spin: gen_range(-10.0, 10.0),
            r: 0.03,
            bleed: 3.0,
        });
        if self.gore {
            self.bat_blood = 1.0;
            for _ in 0..60 {
                self.particles.push(Particle {
                    pos: base,
                    vel: self.vel * 0.2 + vec2(gen_range(-4.0, 4.0), gen_range(-2.0, 4.0)),
                    size: gen_range(0.015, 0.06),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
            for _ in 0..3 {
                self.add_splat();
            }
        }
    }

    /// Le chat s'ecrase a plat : petites giclees rasantes, pas d'explosion.
    fn splat_gore(&mut self, impact: f32) {
        self.add_trauma((impact * 0.02).min(0.6));
        self.kick_vel += vec2(0.0, impact * 20.0);
        self.punch = 0.06;
        self.paf = Some((self.pos + vec2(0.0, 0.35), 0.0, "SPLAT !"));
        for _ in 0..30 {
            let side = rand_sign();
            self.particles.push(Particle {
                pos: vec2(self.pos.x + side * gen_range(0.1, 0.4), 0.04),
                vel: vec2(side * gen_range(1.0, 5.0), gen_range(0.3, 2.0)),
                size: if self.gore { gen_range(0.015, 0.05) } else { gen_range(0.05, 0.1) },
                shade: if self.gore { gen_range(0.0, 1.0) } else { -1.0 },
                life: if self.gore { 6.0 } else { gen_range(0.4, 0.9) },
            });
        }
    }

    fn add_splat(&mut self) {
        let w = screen_width();
        let h = screen_height();
        let r = gen_range(14.0, 48.0);
        let mut sats = [(Vec2::ZERO, 0.0); 5];
        for s in sats.iter_mut() {
            let a = gen_range(0.0, TAU);
            let d = r * gen_range(0.9, 1.9);
            *s = (Vec2::from_angle(a) * d, r * gen_range(0.08, 0.3));
        }
        let life = gen_range(2.5, 4.5);
        self.splats.push(Splat {
            pos: vec2(gen_range(0.0, w), gen_range(0.0, h * 0.8)),
            r,
            life,
            max_life: life,
            drip: 0.0,
            drip_speed: gen_range(8.0, 35.0),
            sats,
        });
    }

    fn update_splats(&mut self, dt: f32) {
        for s in self.splats.iter_mut() {
            s.life -= dt;
            s.drip += s.drip_speed * dt;
        }
        self.splats.retain(|s| s.life > 0.0);
    }

    fn update_particles(&mut self, dt: f32) {
        let mut landed = Vec::new();
        for p in self.particles.iter_mut() {
            p.life -= dt;
            if p.shade < -2.5 {
                // plumes et paillettes : flottent en retombant doucement
                p.vel *= 1.0 - 3.0 * dt;
                p.vel.y -= 1.2 * dt;
                p.pos += p.vel * dt;
                p.pos.x += (p.life * 6.0).sin() * 0.4 * dt;
                continue;
            }
            if p.shade < -1.5 {
                // feu : monte et ralentit
                p.vel.y += 4.0 * dt;
                p.vel *= 1.0 - 2.5 * dt;
                p.pos += p.vel * dt;
                continue;
            }
            if p.shade < 0.0 {
                // poussiere : flotte
                p.vel *= 1.0 - 3.0 * dt;
                p.pos += p.vel * dt;
                continue;
            }
            p.vel.y -= G * dt;
            p.pos += p.vel * dt;
            if p.pos.y <= 0.0 {
                p.life = 0.0;
                if gen_range(0.0, 1.0) < 0.6 {
                    landed.push(Decal { x: p.pos.x, w: p.size * 2.5, shade: p.shade });
                }
            }
        }
        self.particles.retain(|p| p.life > 0.0);
        self.decals.extend(landed);
        if self.particles.len() > 1500 {
            let n = self.particles.len() - 1500;
            self.particles.drain(0..n);
        }
    }

    fn update_pieces(&mut self, dt: f32) {
        for p in self.pieces.iter_mut() {
            if p.vel == Vec2::ZERO {
                continue;
            }
            p.vel.y -= G * dt;
            p.pos += p.vel * dt;
            p.ang += p.spin * dt;
            if p.pos.y <= p.size * 0.5 {
                p.pos.y = p.size * 0.5;
                if p.vel.y.abs() > 1.5 {
                    p.vel.y = -p.vel.y * 0.35;
                    p.vel.x *= 0.6;
                    p.spin *= 0.5;
                } else {
                    p.vel.y = 0.0;
                    p.vel.x *= 1.0 - 6.0 * dt;
                    p.spin *= 1.0 - 6.0 * dt;
                    if p.vel.x.abs() < 0.05 {
                        p.vel = Vec2::ZERO;
                    }
                }
            }
        }
    }

    fn history_push(&mut self, d: f32) {
        self.history.insert(0, d);
        self.history.truncate(6);
    }

    fn finish(&mut self) {
        let alt = self.max_height;
        self.history_push(alt);
        if alt > self.best {
            self.best = alt;
            self.new_record = true;
            packarch_paf_le_bengal::save_record(SAVE_FILE, alt);
            self.say("NOUVEAU RECORD !");
        } else {
            self.say("Aplati comme une crepe !");
        }
        self.set_phase(Phase::Done);
    }

    fn altitude(&self) -> f32 {
        match self.phase {
            Phase::Rising | Phase::Plunge => (self.pos.y - CAT_R).max(0.0),
            _ => 0.0,
        }
    }

    // ---------------------------------------------------------------- camera
    fn update_camera(&mut self, dt: f32) {
        let w = screen_width();
        let h = screen_height();
        self.view.ground_sy = h - 90.0;
        self.view.anchor_x = w * 0.5;
        let base_s = (w / (2.0 * FIELD_HALF + 1.5)).min(h / 6.5).clamp(40.0, 140.0);
        let visible = self.view.ground_sy / base_s;

        let (target_cam, target_s, k) = match self.phase {
            Phase::Perched | Phase::Spinning => (vec2(0.0, 0.0), base_s, 5.0),
            Phase::Rising => {
                // la camera ne redescend jamais pendant la montee
                let ty = (self.pos.y - visible * 0.55).max(self.cam_floor).max(0.0);
                self.cam_floor = ty;
                (vec2(0.0, ty), base_s, 6.0)
            }
            Phase::Plunge => {
                let ty = (self.pos.y - visible * 0.3).max(0.0);
                (vec2(0.0, ty), base_s, 12.0)
            }
            Phase::Done => {
                // gros plan sur le chat, centre a l'ecran
                let s = base_s * 3.0;
                let cy = self.pos.y - (self.view.ground_sy - h * 0.68) / s;
                (vec2(self.pos.x, cy), s, 7.0)
            }
        };
        let kc = 1.0 - (-k * dt).exp();
        let ks = 1.0 - (-6.0 * dt).exp();
        self.view.cam += (target_cam - self.view.cam) * kc;
        self.view.s += (target_s - self.view.s) * ks;
        if self.phase == Phase::Plunge {
            // pendant la chute, la camera reste collee au chat (glissement doux au depart)
            let k = smooth(self.phase_t / 0.4);
            let off = self.plunge_off0 + (-visible * 0.3 - self.plunge_off0) * k;
            self.view.cam.y = (self.pos.y + off).max(0.0);
        }

        // Recul directionnel : ressort amorti
        let acc = -self.kick * 260.0 - self.kick_vel * 16.0;
        self.kick_vel += acc * dt;
        self.kick += self.kick_vel * dt;

        // Secousse "trauma" : bruit lisse proportionnel a trauma^2
        self.trauma = (self.trauma - dt * 1.4).max(0.0);
        let t = self.time;
        let tr = self.trauma * self.trauma;
        let noise = vec2(
            (t * 41.0).sin() + 0.6 * (t * 87.0 + 1.3).sin(),
            (t * 47.0 + 0.7).sin() + 0.6 * (t * 79.0 + 2.1).sin(),
        ) / 1.6;
        self.view.shake = noise * tr * 26.0 + self.kick;

        self.punch *= (-9.0 * dt).exp();
        self.view.zoom = 1.0 + self.punch;
    }

    // ---------------------------------------------------------------- rendu
    fn draw(&self) {
        let v = &self.view;
        self.draw_backdrop();
        self.draw_ruler();
        self.draw_decals();
        self.draw_tiger();
        for y in &self.yarns {
            self.draw_yarn(y);
        }
        for f in &self.fishes {
            if !f.taken {
                self.draw_fish(f);
            }
        }
        for b in &self.birds {
            self.draw_bird(b);
        }
        for d in &self.drones {
            self.draw_drone(d);
        }
        for p in &self.pieces {
            ellipse(v, p.pos, p.size, p.size * 0.6, p.ang, BENGAL);
            if self.gore {
                ellipse(v, p.pos, p.size * 0.6, p.size * 0.45, p.ang + 0.3, blood(0.4));
            }
        }
        if self.phase == Phase::Done {
            self.draw_flat_bengal();
        } else {
            self.draw_bengal();
        }
        for p in &self.parts {
            self.draw_part(p);
        }
        for p in &self.particles {
            if p.shade < -3.5 {
                // paillette doree
                let sp = v.p(p.pos);
                let a = (p.life * 1.5).min(1.0);
                let r = v.l(p.size);
                draw_circle(sp.x, sp.y, r, Color::new(1.0, 0.85, 0.2, a));
                draw_line(sp.x - r * 2.0, sp.y, sp.x + r * 2.0, sp.y, 1.0, Color::new(1.0, 1.0, 0.8, a));
                draw_line(sp.x, sp.y - r * 2.0, sp.x, sp.y + r * 2.0, 1.0, Color::new(1.0, 1.0, 0.8, a));
            } else if p.shade < -2.5 {
                // plume
                let a = (p.life * 0.8).min(1.0);
                ellipse(v, p.pos, p.size, p.size * 0.35, p.life * 3.0, Color::new(0.85, 0.85, 0.9, a));
            } else if p.shade < -1.5 {
                let k = (p.life / 0.8).clamp(0.0, 1.0);
                let c = Color::new(1.0, 0.35 + 0.6 * k, 0.1 * k, (k * 1.4).min(1.0));
                ellipse(v, p.pos, p.size * (0.4 + k), p.size * (0.4 + k), 0.0, c);
            } else if p.shade < 0.0 {
                let a = (p.life * 1.5).min(0.6);
                ellipse(v, p.pos, p.size, p.size, 0.0, Color::new(0.72, 0.65, 0.5, a));
            } else {
                let sp = v.p(p.pos);
                draw_circle(sp.x, sp.y, v.l(p.size).max(1.5), blood(p.shade));
            }
        }
        // fleche quand le bengal sort par le haut de l'ecran
        if matches!(self.phase, Phase::Rising) {
            let sp = v.p(self.pos);
            if sp.y < 0.0 {
                let x = sp.x.clamp(20.0, screen_width() - 20.0);
                draw_triangle(vec2(x, 8.0), vec2(x - 12.0, 28.0), vec2(x + 12.0, 28.0), Color::new(1.0, 0.85, 0.2, 1.0));
                let txt = format!("{:.0} m", self.altitude());
                draw_text(&txt, x + 16.0, 26.0, 22.0, WHITE);
            }
        }
        // presentation des deux protagonistes
        let alpha = match self.phase {
            Phase::Perched => 1.0,
            Phase::Spinning => 1.0 - self.phase_t * 4.0,
            _ => 0.0,
        };
        if alpha > 0.0 {
            let bob = (self.time * 3.0).sin() * 3.0;
            let tiger_head = v.p(vec2(-0.35, 2.72)) + vec2(0.0, bob);
            let bengal_head = v.p(self.pos + vec2(0.3, 0.15)) + vec2(0.0, -bob);
            bubble(tiger_head, "Baby Kart", alpha, 0.3);
            bubble(bengal_head, "Gremlin du Bengal", alpha, 0.05);
        }
        self.draw_paf();
        self.draw_splats();
        self.draw_hud();
    }

    fn draw_backdrop(&self) {
        let v = &self.view;
        let w = screen_width();
        let h = screen_height();
        let s = v.s * v.zoom;
        let alt = v.cam.y.max(0.0);
        let t = (alt / SPACE_FULL).clamp(0.0, 1.0);
        // 0 avant SPACE_START, 1 une fois dans l'espace
        let space = ((alt - SPACE_START) / (SPACE_FULL - SPACE_START)).clamp(0.0, 1.0);

        // ciel : du bleu vers la nuit, puis le noir de l'espace
        let top = lerp_color(Color::new(0.33, 0.55, 0.88, 1.0), Color::new(0.0, 0.0, 0.02, 1.0), t);
        let night_bottom = lerp_color(Color::new(0.08, 0.1, 0.25, 1.0), Color::new(0.01, 0.01, 0.05, 1.0), space);
        let bottom = lerp_color(Color::new(0.85, 0.92, 1.0, 1.0), night_bottom, t);
        let strips = 24;
        for i in 0..strips {
            let k = i as f32 / strips as f32;
            draw_rectangle(0.0, h * k, w, h / strips as f32 + 1.0, lerp_color(top, bottom, k));
        }

        // nebuleuses (voiles colores tres doux)
        if space > 0.0 {
            let tints = [
                Color::new(0.55, 0.25, 0.75, 1.0),
                Color::new(0.2, 0.35, 0.85, 1.0),
                Color::new(0.8, 0.25, 0.5, 1.0),
            ];
            for i in 0..5 {
                let x = hash(i, 20) * w;
                let y = (hash(i, 21) * h * 1.4 + alt * 0.3).rem_euclid(h * 1.4) - h * 0.2;
                let r = 120.0 + hash(i, 22) * 180.0;
                let c = tints[i as usize % 3];
                for k in 0..4 {
                    let kf = k as f32;
                    disc(x + kf * 18.0, y - kf * 12.0, r * (1.0 - kf * 0.2), Color::new(c.r, c.g, c.b, 0.05 * space));
                }
            }
        }

        // etoiles (bien plus nombreuses dans l'espace)
        if t > 0.25 {
            let a = ((t - 0.25) / 0.5).min(1.0);
            let n = 140 + (space * 180.0) as i64;
            for i in 0..n {
                let x = hash(i, 1) * w;
                let y = (hash(i, 2) * h + alt * 0.6).rem_euclid(h);
                let r = 0.8 + hash(i, 3) * 1.6;
                let tw = 0.6 + 0.4 * (self.time * 3.0 + i as f32).sin();
                let tint = hash(i, 7);
                let c = if tint < 0.1 {
                    Color::new(1.0, 0.8, 0.6, a * tw)
                } else if tint < 0.2 {
                    Color::new(0.7, 0.8, 1.0, a * tw)
                } else {
                    Color::new(1.0, 1.0, 1.0, a * tw)
                };
                draw_circle(x, y, r, c);
            }
        }

        // soleil (descend quand on monte)
        let sun_y = h * 0.16 + alt * 4.0;
        if sun_y < h + 60.0 {
            draw_circle(w * 0.82, sun_y, 46.0, Color::new(1.0, 0.95, 0.7, 1.0));
        }

        if space > 0.0 {
            let rel = alt - SPACE_START;
            // planete a anneaux, tres loin
            draw_ringed_planet(w * 0.86, 260.0 + (rel - 80.0) * 0.35, space);
            // la Lune
            draw_moon(w * 0.33, 90.0 + (rel - 80.0) * 0.5, space);
            // satellites qui passent
            for i in 0..2 {
                let span = w + 200.0;
                let x = (hash(i, 30) * span + self.time * (14.0 + 10.0 * i as f32)).rem_euclid(span) - 100.0;
                let y = h * (0.15 + 0.3 * hash(i, 31)) + rel * 0.2 - 120.0;
                draw_satellite(x, y, space);
            }
            // la station spatiale, qui derive doucement
            let span = w + 700.0;
            let sx = (w * 0.6 - self.time * 7.0).rem_euclid(span) - 350.0;
            draw_station(sx, 170.0 + (rel - 80.0) * 0.7, space, self.time);
            // la Terre, vue d'en haut : son bord courbe en bas de l'ecran
            draw_earth_limb(w, h, space, alt, self.time);
        }

        // nuages par tranches d'altitude (parallaxe verticale)
        let cam_eff = v.cam.y * 0.75;
        let band = 9.0;
        let y_lo = cam_eff - 3.0;
        let y_hi = cam_eff + h / s + 3.0;
        let b0 = (y_lo / band).floor() as i64;
        let b1 = (y_hi / band).ceil() as i64;
        for b in b0.max(1)..=b1 {
            let alt_c = b as f32 * band;
            if alt_c > SPACE_START {
                continue;
            }
            let a = 0.85 * (1.0 - alt_c / SPACE_START);
            for j in 0..2 {
                let id = b * 2 + j;
                let cx = (hash(id, 4) * 2.0 - 1.0) * (FIELD_HALF + 2.0);
                let cy = alt_c + hash(id, 5) * band;
                let sx = v.anchor_x + (cx - v.cam.x) * s;
                let sy = v.ground_sy - (cy - cam_eff) * s;
                let size = (0.5 + hash(id, 6) * 0.6) * s;
                let c = Color::new(1.0, 1.0, 1.0, a);
                draw_circle(sx, sy, size, c);
                draw_circle(sx + size * 1.1, sy - size * 0.4, size * 1.2, c);
                draw_circle(sx + size * 2.3, sy, size * 0.9, c);
            }
        }

        // collines pres du sol
        let layers = [
            (3.0, 150.0, Color::new(0.55, 0.68, 0.62, 1.0), 0.4),
            (9.0, 70.0, Color::new(0.42, 0.6, 0.38, 1.0), 0.7),
        ];
        for (k, base, col, py) in layers {
            let y0 = v.ground_sy + v.cam.y * s * py;
            if y0 - 220.0 > h {
                continue;
            }
            let mut x = 0.0;
            while x < w {
                let wx = x + v.cam.x * k * 10.0;
                let hh = base + 35.0 * (wx * 0.004).sin() + 22.0 * (wx * 0.011 + 1.3).sin();
                draw_rectangle(x, y0 - hh, 7.0, hh + h, col);
                x += 6.0;
            }
        }

        // sol
        let gy = v.ground_sy + v.cam.y * s + v.shake.y;
        if gy < h {
            draw_rectangle(0.0, gy, w, h - gy + 10.0, Color::new(0.35, 0.52, 0.24, 1.0));
            draw_rectangle(0.0, gy, w, 7.0, Color::new(0.47, 0.7, 0.3, 1.0));
        }
    }

    /// Regle d'altitude sur le bord gauche + ligne du record.
    fn draw_ruler(&self) {
        let v = &self.view;
        let w = screen_width();
        let s = v.s * v.zoom;
        let lo = v.cam.y - (screen_height() - v.ground_sy) / s;
        let hi = v.cam.y + v.ground_sy / s;
        let mut a = (lo / 5.0).floor().max(1.0) * 5.0;
        while a < hi {
            let sy = v.p(vec2(0.0, a)).y;
            let major = (a as i32) % 10 == 0;
            let len = if major { 30.0 } else { 16.0 };
            draw_line(0.0, sy, len, sy, 2.0, Color::new(1.0, 1.0, 1.0, 0.6));
            if major {
                draw_text(&format!("{} m", a as i32), 36.0, sy + 6.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.75));
            }
            a += 5.0;
        }
        if self.best > 0.0 {
            let sy = v.p(vec2(0.0, self.best + CAT_R)).y;
            if sy > -10.0 && sy < screen_height() {
                let gold = Color::new(1.0, 0.85, 0.1, 0.9);
                let mut x = 0.0;
                while x < w {
                    draw_line(x, sy, x + 14.0, sy, 2.0, gold);
                    x += 24.0;
                }
                draw_text(&format!("RECORD {:.0} m", self.best), w - 190.0, sy - 8.0, 22.0, gold);
            }
        }
    }

    fn draw_yarn(&self, y: &Yarn) {
        let v = &self.view;
        let rx = y.r * (1.0 + 0.25 * y.squish);
        let ry = y.r * (1.0 - 0.3 * y.squish);
        let c = y.pos - vec2(0.0, y.r - ry);
        ellipse(v, c, rx, ry, 0.0, darker(y.color, 0.7));
        ellipse(v, c, rx * 0.93, ry * 0.93, 0.0, y.color);
        // brins de laine
        let strand = darker(y.color, 0.6);
        for k in 0..4 {
            let a = y.spin + k as f32 * 0.8;
            let rot = Vec2::from_angle(a);
            let (ax, ay) = (0.85, 0.25 + 0.12 * k as f32);
            let mut prev = c + rot.rotate(vec2(rx * ax, 0.0));
            for i in 1..=16 {
                let t = i as f32 / 16.0 * TAU;
                let p = c + rot.rotate(vec2(rx * ax * t.cos(), ry * ay * t.sin()));
                seg(v, prev, p, 0.02, strand);
                prev = p;
            }
        }
        ellipse(v, c + vec2(-rx * 0.35, ry * 0.4), rx * 0.25, ry * 0.15, -0.5, Color::new(1.0, 1.0, 1.0, 0.35));
        if self.gore && y.stain > 0.0 {
            let k = y.stain.min(1.0);
            ellipse(v, c + vec2(0.0, ry * 0.65), rx * 0.55 * k, ry * 0.22, 0.0, blood(0.2));
            seg(v, c + vec2(rx * 0.2, ry * 0.5), c + vec2(rx * 0.25, ry * (0.5 - 0.5 * k)), 0.03, blood(0.2));
        }
        // fil qui pend
        let start = c + vec2(rx * 0.6, -ry * 0.6);
        let sway = (self.time * 2.0 + y.pos.y).sin() * 0.1;
        seg(v, start, start + vec2(0.2 + sway, -0.35), 0.02, darker(y.color, 0.8));
    }

    fn draw_bird(&self, b: &Bird) {
        let v = &self.view;
        let f = if b.vx >= 0.0 { 1.0 } else { -1.0 };
        let p = |x: f32, y: f32| b.pos + vec2(x * f, y);
        let grey = Color::new(0.6, 0.6, 0.66, 1.0);
        let dark = Color::new(0.35, 0.35, 0.4, 1.0);
        tri(v, p(-0.18, 0.02), p(-0.36, 0.06), p(-0.34, -0.04), dark); // queue
        ellipse(v, b.pos, 0.22, 0.12, 0.0, grey);
        ellipse(v, p(0.15, 0.06), 0.09, 0.08, 0.0, Color::new(0.35, 0.55, 0.5, 1.0)); // cou irise
        ellipse(v, p(0.23, 0.12), 0.07, 0.065, 0.0, grey);
        ellipse(v, p(0.25, 0.14), 0.015, 0.015, 0.0, Color::new(1.0, 0.5, 0.1, 1.0));
        tri(v, p(0.29, 0.13), p(0.36, 0.11), p(0.29, 0.1), Color::new(0.3, 0.25, 0.2, 1.0));
        let fl = b.flap.sin();
        tri(v, p(-0.1, 0.06), p(0.08, 0.06), p(-0.04, 0.06 + 0.22 * fl), dark);
        seg(v, p(-0.08, 0.06 + 0.1 * fl), p(0.02, 0.06 + 0.1 * fl), 0.015, Color::new(0.15, 0.15, 0.18, 1.0));
    }

    fn draw_drone(&self, d: &Drone) {
        let v = &self.view;
        let body = Color::new(0.25, 0.27, 0.3, 1.0);
        let c = d.pos;
        // tuyau d'aspiration
        let nozzle = c + vec2((self.time * 1.5 + c.y).sin() * 0.2, -0.6);
        seg(v, c + vec2(0.0, -0.08), c + vec2(0.05, -0.35), 0.06, Color::new(0.5, 0.5, 0.55, 1.0));
        seg(v, c + vec2(0.05, -0.35), nozzle, 0.06, Color::new(0.5, 0.5, 0.55, 1.0));
        tri(v, nozzle + vec2(-0.12, -0.04), nozzle + vec2(0.12, -0.04), nozzle + vec2(0.0, 0.06), body);
        // corps
        quad(v, c + vec2(-0.32, -0.1), c + vec2(0.32, -0.1), c + vec2(0.28, 0.1), c + vec2(-0.28, 0.1), body);
        ellipse(v, c + vec2(0.0, 0.1), 0.18, 0.08, 0.0, Color::new(0.4, 0.43, 0.48, 1.0));
        let blink = (self.time * 4.0).fract() < 0.5;
        ellipse(v, c + vec2(-0.2, 0.0), 0.03, 0.03, 0.0, if blink { RED } else { Color::new(0.4, 0.0, 0.0, 1.0) });
        ellipse(v, c + vec2(0.2, 0.0), 0.03, 0.03, 0.0, if blink { Color::new(0.0, 0.3, 0.0, 1.0) } else { GREEN });
        // bras et helices
        seg(v, c + vec2(-0.5, 0.14), c + vec2(0.5, 0.14), 0.04, body);
        for sx in [-0.5, 0.5] {
            let wdt = 0.05 + 0.22 * (self.time * 40.0 + sx).sin().abs();
            ellipse(v, c + vec2(sx, 0.18), wdt, 0.025, 0.0, Color::new(0.75, 0.75, 0.8, 0.8));
        }
        if d.hit_cd > 0.0 && self.gore {
            ellipse(v, nozzle + vec2(0.0, 0.02), 0.08, 0.04, 0.0, blood(0.3));
        }
    }

    fn draw_fish(&self, f: &Fish) {
        let v = &self.view;
        let c = f.pos + vec2(0.0, (self.time * 4.0 + f.pos.x).sin() * 0.08);
        let gold = Color::new(1.0, 0.78, 0.15, 1.0);
        ellipse(v, c, 0.32, 0.32, 0.0, Color::new(1.0, 0.9, 0.4, 0.25)); // halo
        tri(v, c + vec2(-0.2, 0.0), c + vec2(-0.36, 0.1), c + vec2(-0.36, -0.1), darker(gold, 0.85));
        ellipse(v, c, 0.25, 0.1, 0.0, gold);
        ellipse(v, c + vec2(0.0, 0.03), 0.18, 0.03, 0.0, Color::new(1.0, 0.95, 0.7, 1.0));
        ellipse(v, c + vec2(0.16, 0.02), 0.022, 0.022, 0.0, BLACK);
        for k in 0..3 {
            let a = self.time * 2.0 + k as f32 * 2.1;
            let p = c + Vec2::from_angle(a) * 0.38;
            let tw = (self.time * 8.0 + k as f32).sin().abs();
            ellipse(v, p, 0.03 * tw + 0.01, 0.03 * tw + 0.01, 0.0, Color::new(1.0, 1.0, 0.8, 0.9));
        }
    }

    fn draw_decals(&self) {
        let v = &self.view;
        for d in self.decals.iter().filter(|d| d.shade < 0.0) {
            ellipse(v, vec2(d.x, 0.0), d.w, 0.09, 0.0, Color::new(0.16, 0.11, 0.07, 1.0));
            ellipse(v, vec2(d.x, 0.0), d.w * 1.3, 0.05, 0.0, Color::new(0.05, 0.04, 0.03, 0.5));
        }
        if !self.gore {
            return;
        }
        for d in self.decals.iter().filter(|d| d.shade >= 0.0) {
            ellipse(
                v,
                vec2(d.x, 0.0),
                d.w,
                (d.w * 0.15).max(0.012),
                0.0,
                with_alpha(blood(d.shade * 0.5), 0.9),
            );
        }
    }

    fn draw_tiger(&self) {
        let v = &self.view;
        let bob = (self.time * 2.2).sin() * 0.015;
        let lean = if self.phase == Phase::Spinning { -0.06 * self.omega / OMEGA_MAX } else { 0.0 };
        let o = vec2(lean, bob);
        let hand = self.hand_pos();

        // queue
        let mut prev = vec2(-0.92, 1.05);
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            let p = vec2(
                -0.92 - 0.55 * t,
                1.05 - 0.45 * (PI * t).sin() + 0.7 * t * t + (self.time * 3.0 + t * 3.0).sin() * 0.04 * t,
            );
            let col = if i % 3 == 0 || i == 10 { TABBY_DARK } else { TABBY };
            seg(v, prev, p, 0.1, col);
            prev = p;
        }

        // jambes
        let legs = [
            (vec2(-0.75, 1.0), vec2(-0.9, 0.52), vec2(-0.86, 0.1)),
            (vec2(-0.48, 1.0), vec2(-0.32, 0.52), vec2(-0.38, 0.1)),
        ];
        for (hip, knee, foot) in legs {
            seg(v, hip, knee, 0.22, TABBY);
            seg(v, knee, foot, 0.19, TABBY);
            for k in 0..3 {
                let t = 0.25 + k as f32 * 0.3;
                let c = knee.lerp(foot, t);
                seg(v, c + vec2(-0.09, 0.02), c + vec2(0.0, -0.01), 0.035, TABBY_DARK);
            }
            ellipse(v, foot + vec2(0.1, -0.03), 0.18, 0.08, 0.0, TABBY);
        }
        // short
        quad(
            v,
            vec2(-0.95, 1.2) + o,
            vec2(-0.25, 1.2) + o,
            vec2(-0.22, 0.78),
            vec2(-0.97, 0.78),
            Color::new(0.15, 0.3, 0.75, 1.0),
        );
        seg(v, vec2(-0.95, 1.18) + o, vec2(-0.25, 1.18) + o, 0.05, WHITE);

        // bras arriere : poing sur la hanche
        let (l1, l2) = (0.34, 0.34);
        let sb = SHOULDER + o + vec2(-0.14, 0.02);
        let hip = vec2(-0.88, 1.28) + o;
        let eb = elbow(sb, hip, l1, l2, 1.0);
        seg(v, sb, eb, 0.16, TABBY_SHADE);
        seg(v, eb, hip, 0.14, TABBY_SHADE);
        ellipse(v, hip, 0.08, 0.07, 0.0, TABBY_SHADE);

        // torse
        ellipse(v, vec2(-0.6, 1.52) + o, 0.38, 0.5, -0.05, TABBY);
        ellipse(v, vec2(-0.46, 1.45) + o, 0.2, 0.36, -0.05, TABBY_LIGHT);
        for k in 0..4 {
            let y = 1.22 + k as f32 * 0.17;
            seg(v, vec2(-0.97, y) + o, vec2(-0.8, y + 0.04) + o, 0.04, TABBY_DARK);
            seg(v, vec2(-0.3, y + 0.05) + o, vec2(-0.24, y + 0.02) + o, 0.035, TABBY_DARK);
        }
        if self.gore && self.bat_blood > 0.0 && self.launch.is_some() {
            for (dx, dy, r) in [(-0.4, 1.7, 0.05), (-0.5, 1.55, 0.03), (-0.33, 1.4, 0.04), (-0.28, 2.0, 0.03)] {
                ellipse(v, vec2(dx, dy) + o, r, r * 0.8, 0.0, blood(0.2));
            }
        }

        // tete
        let hc = vec2(-0.45, 2.24) + o;
        tri(v, hc + vec2(-0.27, 0.12), hc + vec2(-0.2, 0.42), hc + vec2(-0.05, 0.22), TABBY);
        tri(v, hc + vec2(0.05, 0.22), hc + vec2(0.19, 0.42), hc + vec2(0.23, 0.12), TABBY);
        tri(v, hc + vec2(-0.21, 0.17), hc + vec2(-0.18, 0.33), hc + vec2(-0.1, 0.22), Color::new(1.0, 0.7, 0.7, 1.0));
        tri(v, hc + vec2(0.09, 0.22), hc + vec2(0.17, 0.33), hc + vec2(0.18, 0.17), Color::new(1.0, 0.7, 0.7, 1.0));
        // favoris
        tri(v, hc + vec2(-0.3, 0.0), hc + vec2(-0.42, -0.14), hc + vec2(-0.2, -0.18), TABBY_LIGHT);
        ellipse(v, hc, 0.33, 0.29, 0.0, TABBY);
        for k in 0..3 {
            let x = -0.12 + k as f32 * 0.09;
            seg(v, hc + vec2(x, 0.27), hc + vec2(x + 0.02, 0.15), 0.035, TABBY_DARK);
        }
        seg(v, hc + vec2(-0.33, 0.02), hc + vec2(-0.2, 0.0), 0.035, TABBY_DARK);
        seg(v, hc + vec2(-0.32, -0.08), hc + vec2(-0.2, -0.07), 0.035, TABBY_DARK);
        // museau
        ellipse(v, hc + vec2(0.2, -0.1), 0.16, 0.11, 0.0, TABBY_LIGHT);
        tri(v, hc + vec2(0.27, -0.03), hc + vec2(0.37, -0.03), hc + vec2(0.33, -0.09), Color::new(0.9, 0.45, 0.5, 1.0));
        // bouche : sourire carnassier apres un bon coup
        let happy = self.launch.as_ref().map_or(false, |l| l.quality > 0.6);
        if happy {
            tri(v, hc + vec2(0.12, -0.14), hc + vec2(0.34, -0.14), hc + vec2(0.24, -0.22), TABBY_DARK);
            tri(v, hc + vec2(0.15, -0.14), hc + vec2(0.18, -0.14), hc + vec2(0.165, -0.18), WHITE);
            tri(v, hc + vec2(0.28, -0.14), hc + vec2(0.31, -0.14), hc + vec2(0.295, -0.18), WHITE);
        } else {
            seg(v, hc + vec2(0.32, -0.1), hc + vec2(0.3, -0.16), 0.02, TABBY_DARK);
            seg(v, hc + vec2(0.3, -0.16), hc + vec2(0.2, -0.17), 0.02, TABBY_DARK);
        }
        // moustaches
        for k in 0..3 {
            let y = -0.08 - k as f32 * 0.035;
            seg(v, hc + vec2(0.28, y), hc + vec2(0.5, y + 0.03 - k as f32 * 0.03), 0.01, WHITE);
        }
        // oeil (clignement)
        let blink = (self.time % 4.0) < 0.12;
        let eye = hc + vec2(0.12, 0.07);
        if blink {
            seg(v, eye + vec2(-0.06, 0.0), eye + vec2(0.06, 0.0), 0.02, TABBY_DARK);
        } else {
            ellipse(v, eye, 0.065, 0.05, 0.0, Color::new(0.8, 0.9, 0.2, 1.0));
            ellipse(v, eye + vec2(0.015, 0.0), 0.018, 0.045, 0.0, BLACK);
        }
        // sourcil determine
        seg(v, eye + vec2(-0.08, 0.08), eye + vec2(0.08, 0.04), 0.03, TABBY_DARK);

        // bras avant : tient le bengal par la queue
        let sf = SHOULDER + o + vec2(0.05, -0.02);
        let ef = elbow(sf, hand, l1, l2, -1.0);
        seg(v, sf, ef, 0.17, TABBY);
        seg(v, ef, hand, 0.15, TABBY);
        for k in [0.35, 0.65] {
            let c = sf.lerp(ef, k);
            let nn = (ef - sf).normalize_or_zero().perp() * 0.07;
            seg(v, c - nn, c + nn * 0.4, 0.03, TABBY_DARK);
        }
        let c = ef.lerp(hand, 0.5);
        let nn = (hand - ef).normalize_or_zero().perp() * 0.06;
        seg(v, c - nn, c + nn * 0.4, 0.03, TABBY_DARK);
        ellipse(v, hand, 0.095, 0.085, 0.0, TABBY);

        // cercles de vitesse quand il fait tournoyer le bengal
        if self.phase == Phase::Spinning {
            let power = self.omega / OMEGA_MAX;
            let c = v.p(hand);
            let r = v.l(TAIL_LEN);
            for k in 0..3 {
                draw_circle_lines(
                    c.x,
                    c.y,
                    r * (0.8 + 0.15 * k as f32),
                    2.0 + 2.0 * power,
                    Color::new(1.0, 1.0, 1.0, 0.12 * power),
                );
            }
            // zone de lacher : arc vert a droite du cercle, qui s'allume quand le bengal y passe
            let th = (self.theta + PI).rem_euclid(TAU) - PI;
            let inside = th.abs() < RELEASE_ZONE;
            let col = if inside {
                Color::new(0.4, 1.0, 0.4, 1.0)
            } else {
                Color::new(0.3, 0.95, 0.3, 0.55)
            };
            const N: usize = 12;
            for k in 0..N {
                let a0 = -RELEASE_ZONE + 2.0 * RELEASE_ZONE * k as f32 / N as f32;
                let a1 = -RELEASE_ZONE + 2.0 * RELEASE_ZONE * (k + 1) as f32 / N as f32;
                seg(
                    v,
                    hand + Vec2::from_angle(a0) * (TAIL_LEN + 0.12),
                    hand + Vec2::from_angle(a1) * (TAIL_LEN + 0.12),
                    if inside { 0.16 } else { 0.1 },
                    col,
                );
            }
        }
    }

    /// Onomatopee "PAF !" facon BD au point d'impact.
    fn draw_paf(&self) {
        let Some((pos, age, txt)) = self.paf else { return };
        let v = &self.view;
        let c = v.p(pos) + vec2(0.0, -10.0);
        // apparition qui "pop" puis disparition
        let pop = if age < 0.08 { 0.6 + age / 0.08 * 0.6 } else { 1.2 - (age - 0.08) * 0.4 };
        let alpha = if age > 0.35 { 1.0 - (age - 0.35) / 0.15 } else { 1.0 };
        if alpha <= 0.0 {
            return;
        }
        let star = |r_out: f32, r_in: f32, col: Color| {
            const N: usize = 14;
            let pts: Vec<Vec2> = (0..N * 2)
                .map(|i| {
                    let ang = i as f32 / (N * 2) as f32 * TAU + 0.2;
                    let wob = 1.0 + ((i * 7) % 5) as f32 * 0.06;
                    let r = if i % 2 == 0 { r_out * wob } else { r_in };
                    c + Vec2::from_angle(ang) * r * pop
                })
                .collect();
            for i in 0..pts.len() {
                draw_triangle(c, pts[i], pts[(i + 1) % pts.len()], col);
            }
        };
        star(92.0, 58.0, Color::new(0.75, 0.05, 0.05, alpha));
        star(80.0, 50.0, Color::new(1.0, 0.86, 0.15, alpha));
        let size = (54.0 * pop) as u16;
        let dim = measure_text(txt, None, size, 1.0);
        let tx = c.x - dim.width / 2.0;
        let ty = c.y + dim.offset_y / 2.0;
        for (dx, dy) in [(-3.0, 0.0), (3.0, 0.0), (0.0, -3.0), (0.0, 3.0), (3.0, 3.0)] {
            draw_text(txt, tx + dx, ty + dy, size as f32, Color::new(0.1, 0.05, 0.05, alpha));
        }
        draw_text(txt, tx, ty, size as f32, Color::new(1.0, 1.0, 1.0, alpha));
    }

    fn draw_bengal(&self) {
        let v = &self.view;
        let flip = -1.0; // regarde vers le tigre
        let rot = Vec2::from_angle(self.ang);
        let pos = self.pos;
        let t = |l: Vec2| pos + rot.rotate(vec2(l.x * flip, l.y));
        let ea = |a: f32| self.ang + a * flip;
        let flying = matches!(self.phase, Phase::Rising | Phase::Spinning);
        let ko = matches!(self.phase, Phase::Rising | Phase::Plunge | Phase::Done);
        let flail = if flying { (self.time * 25.0).sin() * 0.06 } else { 0.0 };

        // queue : tendue jusqu'a la main de Baby Kart, libre, ou... arrachee
        if matches!(self.phase, Phase::Perched | Phase::Spinning) {
            let base = t(vec2(-0.24, 0.03));
            let hand = self.hand_pos();
            let mut prev = base;
            for i in 1..=6 {
                let k = i as f32 / 6.0;
                let p = base.lerp(hand, k);
                seg(v, prev, p, 0.05 - 0.01 * k, if i >= 5 { BENGAL_SPOT } else { BENGAL });
                prev = p;
            }
        } else if !self.tailless {
            let swish = (self.time * 3.0).sin() * 0.3;
            let mut prev = t(vec2(-0.24, 0.03));
            for i in 1..=8 {
                let k = i as f32 / 8.0;
                let l = vec2(-0.24 - 0.28 * k, 0.03 + 0.22 * k * k + swish * k * k * 0.5);
                let p = t(l);
                let col = if i >= 7 { BENGAL_SPOT } else { BENGAL };
                seg(v, prev, p, 0.05, col);
                prev = p;
            }
        } else if self.gore {
            ellipse(v, t(vec2(-0.25, 0.03)), 0.035, 0.03, ea(0.0), blood(0.8)); // moignon
        }

        // pattes
        let perched = self.phase == Phase::Perched;
        for (x, dx) in [(-0.16, -0.02), (-0.08, 0.03), (0.12, -0.03), (0.2, 0.03)] {
            let foot = if perched {
                vec2(x + dx * 0.3, -0.2)
            } else {
                vec2(x + dx * 2.5 + flail * (x * 10.0).signum(), -0.2 - flail.abs())
            };
            seg(v, t(vec2(x, -0.06)), t(foot), 0.055, BENGAL);
            ellipse(v, t(foot), 0.03, 0.025, 0.0, CREAM);
        }

        // corps
        ellipse(v, t(vec2(0.0, 0.0)), 0.27, 0.14, ea(0.0), BENGAL);
        ellipse(v, t(vec2(0.02, -0.06)), 0.2, 0.065, ea(0.0), CREAM);
        // rosettes
        for (x, y, r) in [
            (-0.17, 0.05, 0.035),
            (-0.08, 0.08, 0.04),
            (0.03, 0.07, 0.035),
            (0.12, 0.05, 0.03),
            (-0.12, -0.02, 0.03),
            (-0.01, 0.0, 0.035),
            (0.09, -0.01, 0.028),
            (-0.21, -0.03, 0.025),
        ] {
            ellipse(v, t(vec2(x, y)), r, r * 0.8, ea(0.3), BENGAL_SPOT);
            ellipse(v, t(vec2(x, y)), r * 0.55, r * 0.4, ea(0.3), BENGAL_MID);
        }
        // blessures
        if self.gore {
            let wounds = [
                (0.05, 0.03, 0.06),
                (-0.14, 0.0, 0.05),
                (0.22, 0.12, 0.04),
                (-0.04, -0.06, 0.05),
                (0.15, -0.02, 0.04),
            ];
            let n = (self.damage * wounds.len() as f32).ceil() as usize;
            for &(x, y, r) in wounds.iter().take(n) {
                ellipse(v, t(vec2(x, y)), r, r * 0.7, ea(0.5), blood(0.2));
                ellipse(v, t(vec2(x, y)), r * 0.5, r * 0.35, ea(0.5), blood(0.9));
            }
        }

        // tete
        let h = vec2(0.27, 0.1);
        tri(v, t(h + vec2(-0.09, 0.05)), t(h + vec2(-0.07, 0.17)), t(h + vec2(0.0, 0.08)), BENGAL);
        tri(v, t(h + vec2(0.02, 0.08)), t(h + vec2(0.08, 0.18)), t(h + vec2(0.1, 0.05)), BENGAL);
        tri(v, t(h + vec2(0.04, 0.09)), t(h + vec2(0.075, 0.15)), t(h + vec2(0.085, 0.07)), Color::new(1.0, 0.7, 0.7, 1.0));
        ellipse(v, t(h), 0.12, 0.105, ea(0.0), BENGAL);
        ellipse(v, t(h + vec2(0.07, -0.04)), 0.06, 0.045, ea(0.0), CREAM);
        // marques "mascara" du bengal
        seg(v, t(h + vec2(0.0, 0.07)), t(h + vec2(0.02, 0.1)), 0.015, BENGAL_SPOT);
        seg(v, t(h + vec2(-0.04, 0.06)), t(h + vec2(-0.03, 0.1)), 0.015, BENGAL_SPOT);
        tri(v, t(h + vec2(0.11, -0.02)), t(h + vec2(0.13, -0.02)), t(h + vec2(0.12, -0.04)), Color::new(0.85, 0.4, 0.45, 1.0));
        let eye = h + vec2(0.05, 0.025);
        if ko {
            for s in [-1.0, 1.0] {
                seg(v, t(eye + vec2(-0.025, 0.025 * s)), t(eye + vec2(0.025, -0.025 * s)), 0.014, BLACK);
            }
            // langue pendante
            seg(v, t(h + vec2(0.1, -0.06)), t(h + vec2(0.11, -0.11)), 0.025, Color::new(0.95, 0.45, 0.55, 1.0));
        } else {
            ellipse(v, t(eye), 0.03, 0.026, ea(0.0), Color::new(0.35, 0.8, 0.3, 1.0));
            ellipse(v, t(eye + vec2(0.006, 0.0)), 0.008, 0.022, ea(0.0), BLACK);
            seg(v, t(eye + vec2(0.02, -0.03)), t(eye + vec2(0.0, -0.045)), 0.012, BENGAL_SPOT);
        }
        if self.gore && self.damage > 0.5 {
            ellipse(v, t(h + vec2(-0.03, 0.02)), 0.035, 0.03, 0.0, blood(0.3));
        }
        // casque-bocal d'astronaute dans l'espace
        if self.pos.y > SPACE_FULL && matches!(self.phase, Phase::Rising | Phase::Plunge) {
            let c = t(h + vec2(0.01, 0.01));
            ellipse(v, c, 0.19, 0.19, 0.0, Color::new(0.75, 0.9, 1.0, 0.22));
            let sc = v.p(c);
            draw_circle_lines(sc.x, sc.y, v.l(0.19), 2.0, Color::new(0.9, 0.95, 1.0, 0.8));
            ellipse(v, c + vec2(-0.07, 0.08), 0.05, 0.025, 0.6, Color::new(1.0, 1.0, 1.0, 0.6));
        }
    }

    /// Le bengal aplati au sol comme une crepe (avec un petit effet gelee).
    fn draw_flat_bengal(&self) {
        let v = &self.view;
        let t = self.phase_t;
        let f = smooth(t / 0.1);
        let wob = 1.0 + 0.35 * (-6.0 * t).exp() * (t * 32.0).sin();
        let ry = (0.14 + (0.065 - 0.14) * f) * wob;
        let rx = (0.27 + (0.46 - 0.27) * f) / wob.sqrt();
        let c = vec2(self.pos.x, ry);

        // flaque de sang qui s'etale doucement
        if self.gore {
            let k = 1.0 - (-1.2 * t).exp();
            ellipse(v, vec2(c.x, 0.0), 0.3 + 0.7 * k, 0.03 + 0.05 * k, 0.0, blood(0.15));
            ellipse(v, vec2(c.x + 0.1, 0.0), 0.2 + 0.4 * k, 0.02 + 0.03 * k, 0.0, blood(0.5));
        }
        // pattes ecartees
        for (lx, dir) in [(-0.3, -1.0), (-0.16, -1.0), (0.16, 1.0), (0.3, 1.0)] {
            let base = c + vec2(lx * rx / 0.48, -ry * 0.3);
            let foot = base + vec2(dir * (0.04 + 0.16 * f), -0.01);
            seg(v, base, foot, 0.05, BENGAL);
            ellipse(v, foot, 0.035, 0.018, 0.0, CREAM);
        }
        // queue a plat (ou moignon)
        if !self.tailless {
            let tail0 = c + vec2(rx * 0.95, 0.0);
            let tail1 = tail0 + vec2(0.1 + 0.4 * f, -0.01);
            seg(v, tail0, tail1, 0.04, BENGAL);
            seg(v, tail0.lerp(tail1, 0.8), tail1, 0.04, BENGAL_SPOT);
        } else if self.gore {
            ellipse(v, c + vec2(rx, 0.0), 0.03, 0.02, 0.0, blood(0.8));
        }
        // corps-crepe
        ellipse(v, c, rx, ry, 0.0, BENGAL);
        ellipse(v, c + vec2(0.0, -ry * 0.35), rx * 0.8, ry * 0.45, 0.0, CREAM);
        let squash = (ry / 0.14).max(0.2);
        for (x, r) in [(-0.27, 0.04), (-0.12, 0.05), (0.03, 0.045), (0.18, 0.04), (0.32, 0.03)] {
            let p = c + vec2(x * rx / 0.48, ry * 0.25);
            ellipse(v, p, r, r * squash, 0.0, BENGAL_SPOT);
            ellipse(v, p, r * 0.55, r * 0.4 * squash, 0.0, BENGAL_MID);
        }
        if self.gore {
            ellipse(v, c + vec2(0.05, ry * 0.3), rx * 0.3, ry * 0.4, 0.0, blood(0.3));
        }
        // tete aplatie, cote gauche
        let hry = (ry * 0.9).max(0.03);
        let hc = c + vec2(-rx - 0.02 - 0.08 * f, 0.0);
        tri(v, hc + vec2(-0.02, hry * 0.6), hc + vec2(-0.1, hry * 0.6 + 0.04), hc + vec2(0.04, hry * 0.8), BENGAL);
        tri(v, hc + vec2(0.04, hry * 0.6), hc + vec2(0.1, hry * 0.6 + 0.04), hc + vec2(0.1, hry * 0.5), BENGAL);
        ellipse(v, hc, 0.14, hry, 0.0, BENGAL);
        ellipse(v, hc + vec2(-0.07, -hry * 0.3), 0.06, hry * 0.5, 0.0, CREAM);
        for ex in [-0.04, 0.03] {
            let e = hc + vec2(ex, hry * 0.15);
            let d = 0.016;
            seg(v, e + vec2(-d, -d * 0.6), e + vec2(d, d * 0.6), 0.01, BLACK);
            seg(v, e + vec2(-d, d * 0.6), e + vec2(d, -d * 0.6), 0.01, BLACK);
        }
        seg(v, hc + vec2(-0.11, -hry * 0.4), hc + vec2(-0.17, -hry * 0.4 - 0.015), 0.022, Color::new(0.95, 0.45, 0.55, 1.0));
    }

    fn draw_splats(&self) {
        for s in &self.splats {
            let a = (s.life / s.max_life * 1.6).min(0.85);
            let c = Color::new(0.5, 0.0, 0.02, a);
            draw_circle(s.pos.x, s.pos.y, s.r, c);
            for (off, r) in s.sats {
                draw_circle(s.pos.x + off.x, s.pos.y + off.y, r, c);
            }
            let dw = s.r * 0.22;
            draw_rectangle(s.pos.x - dw / 2.0, s.pos.y, dw, s.r * 0.6 + s.drip, c);
            draw_circle(s.pos.x, s.pos.y + s.r * 0.6 + s.drip, dw * 0.8, c);
        }
    }

    fn draw_hud(&self) {
        let w = screen_width();
        let h = screen_height();
        let panel = Color::new(0.05, 0.08, 0.15, 0.75);

        draw_rectangle(12.0, 12.0, 300.0, 140.0, panel);
        draw_text(&format!("Altitude : {:.1} m", self.altitude()), 24.0, 46.0, 34.0, WHITE);
        draw_text(&format!("Max      : {:.1} m", self.max_height), 24.0, 76.0, 26.0, WHITE);
        draw_text(&format!("Record   : {:.1} m", self.best), 24.0, 104.0, 24.0, Color::new(1.0, 0.85, 0.2, 1.0));
        let combo = if self.combo >= 2 { format!("   Combo x{}", self.combo) } else { String::new() };
        draw_text(&format!("Rebonds : {}{}", self.bounces, combo), 24.0, 134.0, 22.0, LIGHTGRAY);

        if let Some(l) = &self.launch {
            draw_rectangle(12.0, 160.0, 300.0, 34.0, panel);
            draw_text(
                &format!("Lancer : {:.0} %   {:.0} km/h", l.quality * 100.0, l.speed * 3.6),
                24.0,
                184.0,
                20.0,
                LIGHTGRAY,
            );
        }

        if !self.history.is_empty() {
            let x = w - 200.0;
            draw_rectangle(x, 12.0, 188.0, 40.0 + 24.0 * self.history.len() as f32, panel);
            draw_text("Derniers lancers", x + 12.0, 38.0, 22.0, WHITE);
            for (i, d) in self.history.iter().enumerate() {
                let txt = if *d <= 0.0 { "rate".to_string() } else { format!("{d:.1} m") };
                draw_text(&txt, x + 12.0, 64.0 + i as f32 * 24.0, 22.0, LIGHTGRAY);
            }
        }

        let hint = match self.phase {
            Phase::Perched => "Maintiens clic / Espace : faire tourner le bengal",
            Phase::Spinning => "Relache au bon moment pour le lancer !",
            Phase::Rising if self.phase_t < 3.0 => "Souris ou fleches : diriger le bengal",
            Phase::Rising | Phase::Plunge => "",
            Phase::Done if self.phase_t > if self.gore { 1.8 } else { 0.6 } => "Clic / Espace : rejouer",
            Phase::Done => "",
        };
        if !hint.is_empty() {
            let size = 30;
            let tw = measure_text(hint, None, size, 1.0).width;
            draw_rectangle(w / 2.0 - tw / 2.0 - 14.0, h - 58.0, tw + 28.0, 42.0, panel);
            draw_text(hint, w / 2.0 - tw / 2.0, h - 28.0, size as f32, WHITE);
        }

        // jauge d'elan pendant la rotation
        if self.phase == Phase::Spinning {
            let power = self.omega / OMEGA_MAX;
            let bw = 320.0;
            let bx = w / 2.0 - bw / 2.0;
            let by = h - 104.0;
            draw_rectangle(bx - 4.0, by - 4.0, bw + 8.0, 30.0, panel);
            let blink = self.grip_t > 0.0 && (self.time * 10.0).sin() > 0.0;
            let col = if blink {
                RED
            } else {
                lerp_color(Color::new(0.3, 0.8, 0.3, 1.0), Color::new(1.0, 0.7, 0.1, 1.0), power)
            };
            draw_rectangle(bx, by, bw * power, 22.0, col);
            draw_text("ELAN", bx + 8.0, by + 17.0, 20.0, WHITE);
            if self.grip_t > GRIP_LIMIT * 0.45 {
                let txt = "La queue va lacher !";
                let tw = measure_text(txt, None, 26, 1.0).width;
                draw_text(txt, w / 2.0 - tw / 2.0, by - 12.0, 26.0, Color::new(1.0, 0.3, 0.2, 1.0));
            }
        }

        // titre du jeu sur l'ecran de depart
        if self.phase == Phase::Perched {
            let y = 66.0 + (self.time * 2.0).sin() * 3.0;
            for (txt, size, yy) in [("PAF LE BENGAL", 64u16, y), ("ALTITUDE", 40u16, y + 44.0)] {
                let dim = measure_text(txt, None, size, 1.0);
                let x = w / 2.0 - dim.width / 2.0 + 60.0;
                for (dx, dy) in [(-3.0, 0.0), (3.0, 0.0), (0.0, -3.0), (0.0, 3.0), (4.0, 4.0)] {
                    draw_text(txt, x + dx, yy + dy, size as f32, Color::new(0.45, 0.02, 0.02, 1.0));
                }
                draw_text(txt, x, yy, size as f32, Color::new(1.0, 0.86, 0.15, 1.0));
            }
        }

        let gore_txt = if self.gore { "G : gore ON" } else { "G : gore OFF" };
        draw_text(gore_txt, w - 140.0, h - 20.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.7));

        if self.msg_t > 0.0 {
            let a = self.msg_t.min(1.0);
            let pop = 1.0 + (2.2 - self.msg_t).clamp(0.0, 0.15) * -2.0 + 0.3;
            let size = (58.0 * pop) as u16;
            let tw = measure_text(&self.msg, None, size, 1.0).width;
            let col = if self.msg.contains("PARFAIT") || self.msg.contains("RECORD") || self.msg.contains("parfait") {
                Color::new(1.0, 0.85, 0.1, a)
            } else if self.msg.contains("MANQUE") || self.msg.contains("ASPIRE") || self.msg.contains("CHUTE") {
                Color::new(1.0, 0.25, 0.2, a)
            } else {
                Color::new(1.0, 1.0, 1.0, a)
            };
            let y = h * 0.3;
            draw_text(&self.msg, w / 2.0 - tw / 2.0 + 3.0, y + 3.0, size as f32, Color::new(0.0, 0.0, 0.0, a * 0.6));
            draw_text(&self.msg, w / 2.0 - tw / 2.0, y, size as f32, col);
        }

        if self.phase == Phase::Done {
            let txt = format!("{:.1} m", self.max_height);
            let size = 80;
            let tw = measure_text(&txt, None, size, 1.0).width;
            draw_text(&txt, w / 2.0 - tw / 2.0 + 4.0, h * 0.45 + 4.0, size as f32, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_text(&txt, w / 2.0 - tw / 2.0, h * 0.45, size as f32, WHITE);
        }
    }
}

// ---------------------------------------------------------------------------
// Decor spatial (coordonnees ecran)
// ---------------------------------------------------------------------------

/// Disque bien rond, meme en grand (draw_circle n'a que 20 cotes).
fn disc(x: f32, y: f32, r: f32, c: Color) {
    let sides = (r * 0.5).clamp(24.0, 255.0) as u8;
    draw_poly(x, y, sides, r, 0.0, c);
}

fn draw_moon(x: f32, y: f32, a: f32) {
    if y < -100.0 || y > screen_height() + 100.0 {
        return;
    }
    let r = 52.0;
    disc(x, y, r + 10.0, Color::new(0.9, 0.9, 0.85, 0.08 * a)); // halo
    disc(x, y, r, Color::new(0.82, 0.82, 0.78, a));
    for (dx, dy, cr) in [(-18.0, -12.0, 11.0), (14.0, 8.0, 15.0), (-6.0, 22.0, 7.0), (22.0, -20.0, 6.0), (-26.0, 12.0, 5.0)] {
        disc(x + dx, y + dy, cr, Color::new(0.62, 0.62, 0.6, a));
        disc(x + dx + 1.5, y + dy + 1.5, cr * 0.75, Color::new(0.7, 0.7, 0.67, a));
    }
    // cote nuit
    disc(x + 22.0, y - 6.0, r * 0.95, Color::new(0.0, 0.0, 0.03, 0.45 * a));
}

fn draw_ringed_planet(x: f32, y: f32, a: f32) {
    if y < -100.0 || y > screen_height() + 100.0 {
        return;
    }
    let r = 30.0;
    let ring = |front: bool| {
        const N: usize = 48;
        let rot = Vec2::from_angle(-0.35);
        for k in 0..N {
            let a0 = k as f32 / N as f32 * TAU;
            let a1 = (k + 1) as f32 / N as f32 * TAU;
            let is_front = a0.sin() > 0.0;
            if is_front != front {
                continue;
            }
            let p0 = rot.rotate(vec2(a0.cos() * 62.0, a0.sin() * 14.0));
            let p1 = rot.rotate(vec2(a1.cos() * 62.0, a1.sin() * 14.0));
            draw_line(x + p0.x, y + p0.y, x + p1.x, y + p1.y, 5.0, Color::new(0.85, 0.75, 0.55, 0.8 * a));
        }
    };
    ring(false);
    disc(x, y, r, Color::new(0.85, 0.62, 0.35, a));
    draw_rectangle(x - r * 0.9, y - 8.0, r * 1.8, 5.0, Color::new(0.75, 0.5, 0.28, a));
    draw_rectangle(x - r * 0.8, y + 6.0, r * 1.6, 4.0, Color::new(0.92, 0.75, 0.5, a));
    disc(x + 10.0, y - 4.0, r * 0.9, Color::new(0.0, 0.0, 0.03, 0.35 * a));
    ring(true);
}

fn draw_satellite(x: f32, y: f32, a: f32) {
    let gold = Color::new(0.9, 0.72, 0.3, a);
    let panel = Color::new(0.15, 0.25, 0.55, a);
    draw_rectangle(x - 7.0, y - 5.0, 14.0, 10.0, gold);
    draw_rectangle(x - 32.0, y - 4.0, 22.0, 8.0, panel);
    draw_rectangle(x + 10.0, y - 4.0, 22.0, 8.0, panel);
    draw_line(x - 10.0, y, x - 7.0, y, 1.5, Color::new(0.7, 0.7, 0.7, a));
    draw_line(x + 7.0, y, x + 10.0, y, 1.5, Color::new(0.7, 0.7, 0.7, a));
    draw_line(x, y - 5.0, x + 4.0, y - 12.0, 1.0, Color::new(0.8, 0.8, 0.8, a));
}

/// Station spatiale : poutre centrale, ailes de panneaux solaires, modules et radiateurs.
fn draw_station(x: f32, y: f32, a: f32, time: f32) {
    if y < -200.0 || y > screen_height() + 200.0 {
        return;
    }
    let truss = Color::new(0.72, 0.72, 0.75, a);
    let truss_dark = Color::new(0.45, 0.45, 0.5, a);
    let panel = Color::new(0.12, 0.2, 0.48, a);
    let panel_line = Color::new(0.3, 0.42, 0.75, a);
    let module = Color::new(0.88, 0.88, 0.86, a);
    let module_dark = Color::new(0.65, 0.65, 0.63, a);

    // panneaux solaires (derriere la poutre)
    for px in [-170.0, -120.0, 120.0, 170.0] {
        for side in [-1.0, 1.0] {
            let top = if side < 0.0 { y - 128.0 } else { y + 8.0 };
            draw_rectangle(x + px - 20.0, top, 40.0, 120.0, panel);
            for k in 1..8 {
                let ly = top + k as f32 * 15.0;
                draw_line(x + px - 20.0, ly, x + px + 20.0, ly, 1.0, panel_line);
            }
            draw_line(x + px, top, x + px, top + 120.0, 1.0, panel_line);
            draw_line(x + px, y, x + px, if side < 0.0 { top + 120.0 } else { top }, 2.0, truss_dark);
        }
    }
    // poutre principale en treillis
    draw_rectangle(x - 200.0, y - 6.0, 400.0, 12.0, truss);
    let mut tx = x - 200.0;
    while tx < x + 200.0 {
        draw_line(tx, y - 6.0, tx + 12.0, y + 6.0, 1.0, truss_dark);
        tx += 12.0;
    }
    // radiateurs
    for rx in [-60.0, 60.0] {
        draw_triangle(vec2(x + rx - 6.0, y + 6.0), vec2(x + rx + 6.0, y + 6.0), vec2(x + rx + 30.0, y + 60.0), module);
    }
    // modules pressurises
    draw_rectangle(x - 70.0, y + 6.0, 140.0, 24.0, module);
    disc(x - 70.0, y + 18.0, 12.0, module);
    disc(x + 70.0, y + 18.0, 12.0, module);
    draw_line(x - 70.0, y + 24.0, x + 70.0, y + 24.0, 1.5, module_dark);
    draw_rectangle(x - 12.0, y - 60.0, 24.0, 66.0, module);
    disc(x, y - 60.0, 12.0, module);
    disc(x, y + 18.0, 16.0, module_dark);
    for wx in [-45.0, -20.0, 25.0, 50.0] {
        disc(x + wx, y + 18.0, 3.0, Color::new(0.2, 0.3, 0.5, a));
    }
    // feux clignotants
    let on = (time * 1.5).fract() < 0.15;
    if on {
        disc(x - 200.0, y, 4.0, Color::new(1.0, 0.2, 0.2, a));
        disc(x + 200.0, y, 4.0, Color::new(0.2, 1.0, 0.3, a));
    }
}

/// Bord de la Terre en bas de l'ecran : ocean, continents, nuages et atmosphere.
/// La Terre vue de l'espace : d'abord son bord courbe en bas de l'ecran, puis,
/// plus le bengal monte, un globe entier de plus en plus petit.
fn draw_earth_limb(w: f32, h: f32, a: f32, alt: f32, time: f32) {
    let r0 = w * 2.6;
    let extra = (alt - SPACE_FULL).max(0.0);
    let r = r0 / (1.0 + extra / 40.0);
    // 0 = simple bord de planete, 1 = petit globe
    let s = (1.0 - r / r0).clamp(0.0, 1.0);
    let rise = 40.0 + 110.0 * a;
    // le haut du globe remonte un peu pour qu'on finisse par le voir en entier
    let top = h - rise - (extra * 0.6).min(h * 0.35);
    let c = vec2(w * 0.5, top + r);

    // atmosphere, ocean
    disc(c.x, c.y, r + 22.0 * (1.0 - 0.6 * s), Color::new(0.45, 0.7, 1.0, 0.18 * a));
    disc(c.x, c.y, r + 9.0 * (1.0 - 0.5 * s), Color::new(0.55, 0.8, 1.0, 0.35 * a));
    disc(c.x, c.y, r, Color::new(0.1, 0.28, 0.62, a));

    // continents et nuages : serres pres du bord au debut, repartis sur le globe ensuite
    let span = 0.385 + (1.25 - 0.385) * s;
    let drift = time * 0.004 + alt * 0.0002;
    for k in 0..16 {
        let base = (k as f32 + 0.5) / 16.0; // 0..1
        let u = (base + drift).rem_euclid(1.0);
        let ang = -PI / 2.0 + (u * 2.0 - 1.0) * span;
        let depth = (45.0 + 20.0 * hash(k, 40)) * (1.0 - s) + r * (0.12 + 0.55 * hash(k, 40)) * s;
        let dist = (r - depth).max(0.0);
        let p = c + Vec2::from_angle(ang) * dist;
        let land = hash(k, 41) < 0.5;
        let col = if land {
            Color::new(0.3, 0.52, 0.28, a)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.55 * a)
        };
        let mut ry = (10.0 + 8.0 * hash(k, 43)) * (1.0 - s) + r * (0.06 + 0.06 * hash(k, 43)) * s;
        let mut rx = (50.0 + 70.0 * hash(k, 42)) * (1.0 - s) + r * (0.15 + 0.2 * hash(k, 42)) * s;
        // on reste a l'interieur du globe
        ry = ry.min((r - dist) * 0.9);
        let max_rx = ((r - ry).max(0.0).powi(2) - dist * dist).max(0.0).sqrt();
        rx = rx.min(max_rx);
        if rx > 1.0 && ry > 0.5 {
            draw_ellipse(p.x, p.y, rx, ry, (ang + PI / 2.0).to_degrees(), col);
        }
    }
    // reflet de lumiere sur le globe quand il devient petit
    if s > 0.3 {
        disc(c.x - r * 0.3, c.y - r * 0.3, r * 0.5, Color::new(1.0, 1.0, 1.0, 0.07 * a * s));
    }
}

fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    draw_rectangle(x + r, y, w - 2.0 * r, h, c);
    draw_rectangle(x, y + r, r, h - 2.0 * r, c);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, c);
    for (cx, cy) in [(x + r, y + r), (x + w - r, y + r), (x + r, y + h - r), (x + w - r, y + h - r)] {
        draw_circle(cx, cy, r, c);
    }
}

/// Bulle de BD dont la pointe touche `anchor` (coordonnees ecran).
fn bubble(anchor: Vec2, text: &str, alpha: f32, lean: f32) {
    if alpha <= 0.0 {
        return;
    }
    let size = 28u16;
    let dim = measure_text(text, None, size, 1.0);
    let pad = 16.0;
    let bw = dim.width + pad * 2.0;
    let bh = 46.0;
    let bx = (anchor.x - bw * lean).clamp(8.0, screen_width() - bw - 8.0);
    let by = anchor.y - 24.0 - bh;
    let border = Color::new(0.1, 0.1, 0.12, alpha);
    let fill = Color::new(1.0, 1.0, 1.0, alpha);
    let tx = anchor.x.clamp(bx + 20.0, bx + bw - 20.0);

    round_rect(bx - 3.0, by - 3.0, bw + 6.0, bh + 6.0, 14.0, border);
    draw_triangle(vec2(tx - 14.0, by + bh), vec2(tx + 6.0, by + bh), anchor + vec2(0.0, 3.0), border);
    round_rect(bx, by, bw, bh, 11.0, fill);
    draw_triangle(vec2(tx - 10.0, by + bh - 2.0), vec2(tx + 3.0, by + bh - 2.0), anchor, fill);

    let ty = by + bh / 2.0 + dim.offset_y / 2.0;
    let ink = Color::new(0.1, 0.1, 0.12, alpha);
    draw_text(text, bx + pad, ty, size as f32, ink);
    draw_text(text, bx + pad + 1.0, ty, size as f32, ink); // faux gras
}

fn conf() -> Conf {
    Conf {
        window_title: "Paf le Bengal : Altitude".to_owned(),
        window_width: 1200,
        window_height: 700,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    macroquad::rand::srand(macroquad::miniquad::date::now() as u64);
    let mut game = Game::new();
    let mut acc = 0.0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        let frame = get_frame_time().min(0.1);
        game.input();
        acc += frame;
        let mut steps = 0;
        while acc >= DT && steps < 20 {
            game.step(DT);
            acc -= DT;
            steps += 1;
        }
        game.update_camera(frame);

        clear_background(BLACK);
        game.draw();
        next_frame().await;
    }
}
