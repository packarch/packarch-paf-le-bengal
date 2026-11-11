//! Paf le Bengal — Baby Kart, un grand chat tigre gris, envoie le Gremlin du Bengal
//! le plus loin possible d'un coup de batte.
//!
//! Commandes :
//!   Clic / Espace : faire sauter le bengal, puis FRAPPER au bon moment
//!   G             : activer / desactiver le gore
//!   Echap         : quitter

use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// Constantes de jeu (unites : metres, secondes)
// ---------------------------------------------------------------------------

const G: f32 = 9.81;
const DT: f32 = 1.0 / 120.0;
const DRAG: f32 = 0.0022; // trainee quadratique
const MU: f32 = 0.55; // frottement au sol

// Swing : les bras tournent autour de l'epaule, la batte est tenue dans les
// mains avec un retard de poignet qui se "relache" au moment de l'impact.
const SHOULDER: Vec2 = Vec2::new(-0.52, 1.82);
const ARM_R: f32 = 0.5; // distance epaule -> mains
const BAT_LEN: f32 = 1.25; // depuis les mains
const BAT_SWEET: f32 = 0.7; // position ideale de l'impact le long de la batte
const BAT_MID_DEG: f32 = -32.5; // angle de batte au moment le plus rapide

const PHI_READY: f32 = 125.0; // angle des bras en garde (degres)
const PHI_WIND: f32 = 150.0; // armement
const PHI_CONTACT: f32 = 327.5; // vitesse max
const PHI_END: f32 = 430.0; // accompagnement
const LAG_READY: f32 = 55.0; // poignets casses en garde
const LAG_WIND: f32 = 70.0;
const LAG_END: f32 = -30.0;

const WINDUP_TIME: f32 = 0.08;
const SWING_TIME: f32 = 0.30;
const HOLD_TIME: f32 = 0.35;
const RETURN_TIME: f32 = 0.6;

const DROP_X: f32 = 0.62;
const BRANCH_Y: f32 = 3.95;
const CAT_R: f32 = 0.2;

/// Fichier du record, dans ~/.local/share/packarch-paf-le-bengal/
const SAVE_FILE: &str = "meilleur_score.txt";

// Couleurs
const TABBY: Color = Color::new(0.6, 0.6, 0.62, 1.0); // chat tigre gris
const TABBY_SHADE: Color = Color::new(0.47, 0.47, 0.5, 1.0);
const TABBY_LIGHT: Color = Color::new(0.93, 0.93, 0.9, 1.0);
const TABBY_DARK: Color = Color::new(0.17, 0.17, 0.19, 1.0);
const CREAM: Color = Color::new(0.98, 0.93, 0.82, 1.0);
const BENGAL: Color = Color::new(0.86, 0.62, 0.30, 1.0);
const BENGAL_SPOT: Color = Color::new(0.30, 0.17, 0.08, 1.0);
const BENGAL_MID: Color = Color::new(0.62, 0.38, 0.16, 1.0);
const WOOD: Color = Color::new(0.62, 0.42, 0.22, 1.0);
const WOOD_DARK: Color = Color::new(0.42, 0.27, 0.13, 1.0);

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
    Head,
    TorsoFront,
    TorsoBack,
    Leg,
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

const BONE: Color = Color::new(0.96, 0.93, 0.84, 1.0);

struct Mine {
    x: f32,
    used: bool,
}

struct Explosion {
    pos: Vec2,
    age: f32,
}

const EXPLOSION_TIME: f32 = 0.9;

fn gen_mines() -> Vec<Mine> {
    let mut v = Vec::new();
    let mut x = DROP_X + gen_range(10.0, 18.0);
    while x < DROP_X + 600.0 {
        v.push(Mine { x, used: false });
        x += gen_range(7.0, 20.0);
    }
    v
}

// ---------------------------------------------------------------------------
// Etat du jeu
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Perched,
    Falling,
    Flying,
    Sliding,
    Done,
    Missed,
}

struct Launch {
    quality: f32,
    angle_deg: f32,
    speed: f32,
}

struct Game {
    phase: Phase,
    phase_t: f32,
    pos: Vec2,
    vel: Vec2,
    ang: f32,
    spin: f32,
    swing_t: Option<f32>,
    bat_blood: f32,
    damage: f32,
    max_height: f32,
    trail_acc: f32,
    launch: Option<Launch>,

    particles: Vec<Particle>,
    decals: Vec<Decal>,
    pieces: Vec<Piece>,
    splats: Vec<Splat>,
    mines: Vec<Mine>,
    explosions: Vec<Explosion>,
    mines_hit: u32,
    parts: Vec<BodyPart>,
    dismembered: bool,

    view: View,
    trauma: f32,     // intensite de secousse (0..1), decroit
    kick: Vec2,      // recul directionnel de la camera (px)
    kick_vel: Vec2,
    punch: f32,      // petit zoom a l'impact
    paf: Option<(Vec2, f32)>, // bulle "PAF !" (position monde, age)
    hitstop: f32,
    time: f32,

    best: f32,
    history: Vec<f32>,
    msg: String,
    msg_t: f32,
    new_record: bool,
    gore: bool,
}

fn perch_pos() -> Vec2 {
    vec2(DROP_X, BRANCH_Y + 0.07 + 0.14)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Pose du swing : angle des bras (phi) et retard des poignets (lag), en degres.
/// La vitesse des bras suit un profil triangulaire qui culmine a PHI_CONTACT.
fn swing_angles(u: f32) -> (f32, f32) {
    let u = u.clamp(0.0, 1.0);
    let a1 = PHI_CONTACT - PHI_WIND;
    let a2 = PHI_END - PHI_CONTACT;
    let peak = a1 / (a1 + a2);
    let wmax = 2.0 * (a1 + a2);
    let phi = if u < peak {
        PHI_WIND + wmax * u * u / (2.0 * peak)
    } else {
        let d = u - peak;
        PHI_CONTACT + wmax * d - wmax * d * d / (2.0 * (1.0 - peak))
    };
    let lag = if u < peak {
        LAG_WIND * (1.0 - smooth(u / peak))
    } else {
        LAG_END * smooth((u - peak) / (1.0 - peak))
    };
    (phi, lag)
}

#[derive(Clone, Copy)]
struct Pose {
    hands: Vec2,
    bat: f32,   // angle de la batte (radians, non normalise)
    speed: f32, // vitesse angulaire de la batte, normalisee 0..1
    lean: f32,  // inclinaison du corps vers l'avant pendant le swing (m)
}

fn bat_swing_angle(u: f32) -> f32 {
    let (phi, lag) = swing_angles(u);
    phi - lag
}

fn bat_speed_norm(u: f32) -> f32 {
    let h = 0.002;
    let w = |u: f32| (bat_swing_angle(u + h) - bat_swing_angle(u - h)) / (2.0 * h);
    let mut max = 1.0f32;
    for i in 0..=100 {
        max = max.max(w(i as f32 / 100.0));
    }
    (w(u) / max).clamp(0.0, 1.0)
}

fn pose(swing_t: Option<f32>, time: f32) -> Pose {
    let (phi, lag, speed, swing) = match swing_t {
        None => {
            let breathe = (time * 2.0).sin() * 2.0;
            (PHI_READY + breathe, LAG_READY + breathe, 0.0, 0.0)
        }
        Some(t) if t < WINDUP_TIME => {
            let k = (t / WINDUP_TIME * PI / 2.0).sin();
            (
                PHI_READY + (PHI_WIND - PHI_READY) * k,
                LAG_READY + (LAG_WIND - LAG_READY) * k,
                0.0,
                0.0,
            )
        }
        Some(t) if t < WINDUP_TIME + SWING_TIME => {
            let u = (t - WINDUP_TIME) / SWING_TIME;
            let (phi, lag) = swing_angles(u);
            (phi, lag, bat_speed_norm(u), u)
        }
        Some(t) if t < WINDUP_TIME + SWING_TIME + HOLD_TIME => (PHI_END, LAG_END, 0.0, 1.0),
        Some(t) => {
            let k = smooth((t - WINDUP_TIME - SWING_TIME - HOLD_TIME) / RETURN_TIME);
            (
                PHI_END + (PHI_READY + 360.0 - PHI_END) * k,
                LAG_END + (LAG_READY - LAG_END) * k,
                0.0,
                1.0 - k,
            )
        }
    };
    let lean = (swing * PI).sin() * 0.1;
    Pose {
        hands: SHOULDER + vec2(lean, 0.0) + Vec2::from_angle(phi.to_radians()) * ARM_R,
        bat: (phi - lag).to_radians(),
        speed,
        lean,
    }
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
        Game {
            phase: Phase::Perched,
            phase_t: 0.0,
            pos: perch_pos(),
            vel: Vec2::ZERO,
            ang: 0.0,
            spin: 0.0,
            swing_t: None,
            bat_blood: 0.0,
            damage: 0.0,
            max_height: 0.0,
            trail_acc: 0.0,
            launch: None,
            particles: Vec::new(),
            decals: Vec::new(),
            pieces: Vec::new(),
            splats: Vec::new(),
            mines: gen_mines(),
            explosions: Vec::new(),
            mines_hit: 0,
            parts: Vec::new(),
            dismembered: false,
            view: View {
                cam: vec2(0.5, 0.0),
                s: 100.0,
                ground_sy: 600.0,
                anchor_x: 400.0,
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
        }
    }

    fn reset(&mut self) {
        self.set_phase(Phase::Perched);
        self.pos = perch_pos();
        self.vel = Vec2::ZERO;
        self.ang = 0.0;
        self.spin = 0.0;
        self.swing_t = None;
        self.damage = 0.0;
        self.max_height = 0.0;
        self.launch = None;
        self.new_record = false;
        self.particles.clear();
        self.splats.clear();
        self.explosions.clear();
        self.mines = gen_mines();
        self.mines_hit = 0;
        self.dismembered = false;
        self.paf = None;
        // les restes des lancers precedents restent par terre (sans saigner)
        for p in self.parts.iter_mut() {
            p.bleed = 0.0;
        }
        if self.parts.len() > 40 {
            let n = self.parts.len() - 40;
            self.parts.drain(0..n);
        }
        // Les taches et morceaux restent sur le terrain d'un lancer a l'autre.
        if self.decals.len() > 500 {
            let n = self.decals.len() - 500;
            self.decals.drain(0..n);
        }
        if self.pieces.len() > 40 {
            let n = self.pieces.len() - 40;
            self.pieces.drain(0..n);
        }
    }

    fn set_phase(&mut self, p: Phase) {
        self.phase = p;
        self.phase_t = 0.0;
    }

    fn say(&mut self, s: &str) {
        self.msg = s.to_string();
        self.msg_t = 2.2;
    }

    // ---------------------------------------------------------------- input
    fn input(&mut self) {
        if is_key_pressed(KeyCode::G) {
            self.gore = !self.gore;
        }
        let action = is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Space);
        if !action {
            return;
        }
        match self.phase {
            Phase::Perched => {
                self.set_phase(Phase::Falling);
                self.vel = vec2(0.0, 1.3);
            }
            Phase::Falling => {
                if self.swing_t.is_none() {
                    self.swing_t = Some(0.0);
                }
            }
            Phase::Done | Phase::Missed
                if self.phase_t > if self.gore { 1.8 } else { 0.6 } =>
            {
                self.reset()
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- physique
    fn step(&mut self, dt: f32) {
        self.time += dt;
        self.msg_t -= dt;
        self.update_splats(dt);
        for e in self.explosions.iter_mut() {
            e.age += dt;
        }
        self.explosions.retain(|e| e.age < EXPLOSION_TIME);
        if let Some((_, age)) = &mut self.paf {
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

        // Coup de batte (avec sous-pas pour ne pas "traverser" le chat)
        if let Some(t) = self.swing_t {
            let nt = t + dt;
            self.swing_t = Some(nt);
            let active = WINDUP_TIME..WINDUP_TIME + SWING_TIME;
            if self.phase == Phase::Falling && t < active.end && nt > active.start {
                const SUB: usize = 8;
                for k in 1..=SUB {
                    let tt = t + dt * k as f32 / SUB as f32;
                    if !active.contains(&tt) {
                        continue;
                    }
                    let p = pose(Some(tt), self.time);
                    if let Some(r) = self.bat_contact(&p) {
                        self.do_hit(&p, r);
                        break;
                    }
                }
            }
        }

        match self.phase {
            Phase::Perched => {}
            Phase::Falling => {
                self.vel.y -= G * dt;
                self.pos += self.vel * dt;
                self.ang = (self.time * 9.0).sin() * 0.25;
                if self.pos.y <= CAT_R {
                    self.pos.y = CAT_R;
                    let impact = -self.vel.y;
                    self.vel = Vec2::ZERO;
                    self.ground_gore(impact.max(9.0));
                    self.ang = PI;
                    self.history_push(0.0);
                    self.say("MANQUE !");
                    self.set_phase(Phase::Missed);
                }
            }
            Phase::Flying => {
                self.vel.y -= G * dt;
                let sp = self.vel.length();
                self.vel -= self.vel * sp * DRAG * dt;
                self.pos += self.vel * dt;
                self.ang += self.spin * dt;
                self.max_height = self.max_height.max(self.pos.y - CAT_R);

                // traine de gouttes quand le chat est bien amoche
                if self.gore && self.damage > 0.25 && gen_range(0.0, 1.0) < self.damage * 0.6 {
                    self.particles.push(Particle {
                        pos: self.pos,
                        vel: self.vel * 0.3 + vec2(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)),
                        size: gen_range(0.02, 0.05),
                        shade: gen_range(0.0, 1.0),
                        life: 6.0,
                    });
                }

                if self.pos.y <= CAT_R {
                    self.pos.y = CAT_R;
                    let impact = -self.vel.y;
                    if self.check_mine() {
                        // BOUM : relance geree par explode()
                    } else if impact > 3.5 {
                        self.vel.y = impact * 0.42;
                        self.vel.x *= 0.72;
                        self.spin = -self.vel.x * gen_range(1.5, 3.0);
                        self.ground_gore(impact);
                    } else {
                        self.vel.y = 0.0;
                        self.set_phase(Phase::Sliding);
                    }
                }
            }
            Phase::Sliding => {
                let old_x = self.pos.x;
                self.vel.x = (self.vel.x - MU * G * dt).max(0.0);
                self.pos.x += self.vel.x * dt;
                let target = (self.ang / PI).round() * PI;
                self.ang += (target - self.ang) * (1.0 - (-10.0 * dt).exp());
                self.trail_acc += self.pos.x - old_x;
                if self.gore && self.trail_acc > 0.18 {
                    self.trail_acc = 0.0;
                    self.decals.push(Decal {
                        x: self.pos.x - 0.1,
                        w: gen_range(0.08, 0.16) * (0.5 + self.damage),
                        shade: gen_range(0.0, 1.0),
                    });
                }
                if self.check_mine() {
                    // relance par une mine
                } else if self.vel.x <= 0.01 {
                    self.finish();
                }
            }
            Phase::Done | Phase::Missed => {
                // on laisse la camera zoomer, puis ca eclate
                if self.gore && !self.dismembered && self.phase_t > 0.9 {
                    self.dismember();
                }
            }
        }

        self.update_particles(dt);
        self.update_pieces(dt);
        self.update_parts(dt);
    }

    /// Le bengal eclate en morceaux.
    fn dismember(&mut self) {
        self.dismembered = true;
        let flip = -1.0;
        let rot = Vec2::from_angle(self.ang);
        let center = self.pos;
        let world = |l: Vec2| center + rot.rotate(vec2(l.x * flip, l.y));

        let spawn = |kind: PartKind, local: Vec2, r: f32, parts: &mut Vec<BodyPart>| {
            let pos = world(local);
            let out = (pos - center).normalize_or_zero();
            parts.push(BodyPart {
                kind,
                pos,
                vel: out * gen_range(2.5, 6.0) + vec2(gen_range(-1.5, 1.5), gen_range(3.5, 7.5)),
                ang: self.ang + if flip < 0.0 { PI } else { 0.0 },
                spin: gen_range(-14.0, 14.0),
                r,
                bleed: gen_range(2.5, 4.5),
            });
        };
        let mut new_parts = Vec::new();
        spawn(PartKind::Head, vec2(0.27, 0.1), 0.1, &mut new_parts);
        spawn(PartKind::TorsoFront, vec2(0.12, 0.0), 0.12, &mut new_parts);
        spawn(PartKind::TorsoBack, vec2(-0.12, 0.0), 0.12, &mut new_parts);
        for x in [-0.16, -0.08, 0.12, 0.2] {
            spawn(PartKind::Leg, vec2(x, -0.13), 0.035, &mut new_parts);
        }
        spawn(PartKind::Tail, vec2(-0.38, 0.1), 0.03, &mut new_parts);
        self.parts.extend(new_parts);

        // geyser de sang
        for _ in 0..180 {
            let a = gen_range(0.15, PI - 0.15);
            self.particles.push(Particle {
                pos: self.pos + vec2(gen_range(-0.15, 0.15), gen_range(-0.1, 0.1)),
                vel: Vec2::from_angle(a) * gen_range(1.5, 9.0),
                size: gen_range(0.015, 0.06),
                shade: gen_range(0.0, 1.0),
                life: 6.0,
            });
        }
        self.decals.push(Decal { x: self.pos.x, w: 0.6, shade: 0.2 });
        for _ in 0..gen_range(5, 10) {
            self.add_splat();
        }
        self.add_trauma(0.7);
        self.punch = 0.08;
        self.hitstop = 0.1;
        if !self.new_record {
            self.say("EN MORCEAUX !");
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
        let dark = blood(0.1);
        match p.kind {
            PartKind::Head => {
                tri(v, t(vec2(-0.09, 0.05)), t(vec2(-0.07, 0.17)), t(vec2(0.0, 0.08)), BENGAL);
                tri(v, t(vec2(0.02, 0.08)), t(vec2(0.08, 0.18)), t(vec2(0.1, 0.05)), BENGAL);
                ellipse(v, p.pos, 0.12, 0.105, a, BENGAL);
                ellipse(v, t(vec2(0.07, -0.04)), 0.06, 0.045, a, CREAM);
                let eye = vec2(0.05, 0.025);
                for s in [-1.0, 1.0] {
                    seg(v, t(eye + vec2(-0.025, 0.025 * s)), t(eye + vec2(0.025, -0.025 * s)), 0.014, BLACK);
                }
                seg(v, t(vec2(0.1, -0.06)), t(vec2(0.11, -0.11)), 0.025, Color::new(0.95, 0.45, 0.55, 1.0));
                if gore {
                    // cou arrache
                    ellipse(v, t(vec2(-0.1, -0.03)), 0.04, 0.07, a, dark);
                    ellipse(v, t(vec2(-0.1, -0.03)), 0.025, 0.05, a, flesh);
                    ellipse(v, t(vec2(-0.12, -0.03)), 0.015, 0.015, a, BONE);
                }
            }
            PartKind::TorsoFront | PartKind::TorsoBack => {
                let cut = if p.kind == PartKind::TorsoFront { -1.0 } else { 1.0 };
                ellipse(v, p.pos, 0.15, 0.14, a, BENGAL);
                ellipse(v, t(vec2(0.0, -0.06)), 0.11, 0.06, a, CREAM);
                for (x, y, r) in [(-0.05, 0.06, 0.035), (0.05, 0.03, 0.03), (0.0, -0.01, 0.028)] {
                    ellipse(v, t(vec2(x, y)), r, r * 0.8, a + 0.3, BENGAL_SPOT);
                    ellipse(v, t(vec2(x, y)), r * 0.55, r * 0.4, a + 0.3, BENGAL_MID);
                }
                if gore {
                    let c = vec2(0.12 * cut, 0.0);
                    ellipse(v, t(c), 0.05, 0.13, a, dark);
                    ellipse(v, t(c), 0.035, 0.1, a, flesh);
                    // cotes / colonne
                    for k in 0..3 {
                        let y = 0.06 - k as f32 * 0.05;
                        seg(v, t(c + vec2(-0.01 * cut, y)), t(c + vec2(0.05 * cut, y + 0.01)), 0.014, BONE);
                    }
                    // tripes qui pendent
                    seg(v, t(c), t(c + vec2(0.07 * cut, -0.08)), 0.03, Color::new(0.85, 0.4, 0.45, 1.0));
                }
            }
            PartKind::Leg => {
                seg(v, t(vec2(0.0, 0.08)), t(vec2(0.0, -0.08)), 0.055, BENGAL);
                ellipse(v, t(vec2(0.01, -0.085)), 0.03, 0.025, a, CREAM);
                if gore {
                    ellipse(v, t(vec2(0.0, 0.09)), 0.03, 0.02, a, flesh);
                    seg(v, t(vec2(0.0, 0.09)), t(vec2(0.0, 0.13)), 0.014, BONE);
                    ellipse(v, t(vec2(0.0, 0.135)), 0.013, 0.013, a, BONE);
                }
            }
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

    /// Declenche une mine si le chat est dessus.
    fn check_mine(&mut self) -> bool {
        let x = self.pos.x;
        match self.mines.iter().position(|m| !m.used && (m.x - x).abs() < 0.8) {
            Some(i) => {
                self.explode(i);
                true
            }
            None => false,
        }
    }

    fn explode(&mut self, i: usize) {
        self.mines[i].used = true;
        let x = self.mines[i].x;
        self.mines_hit += 1;
        self.explosions.push(Explosion { pos: vec2(x, 0.3), age: 0.0 });
        self.decals.push(Decal { x, w: 0.9, shade: -1.0 }); // cratere

        self.pos.y = CAT_R + 0.05;
        self.vel.y = gen_range(16.0, 24.0);
        self.vel.x = self.vel.x.max(4.0) + gen_range(6.0, 12.0);
        self.spin = -gen_range(15.0, 30.0);
        self.add_trauma(1.0);
        self.kick_vel += vec2(0.0, -900.0);
        self.punch = 0.12;
        self.hitstop = 0.08;
        self.damage = 1.0;
        self.set_phase(Phase::Flying);
        self.say(if self.mines_hit > 1 { "ENCORE BOUM !" } else { "BOUM !" });

        // feu
        for _ in 0..45 {
            self.particles.push(Particle {
                pos: vec2(x + gen_range(-0.3, 0.3), 0.2),
                vel: vec2(gen_range(-6.0, 6.0), gen_range(2.0, 12.0)),
                size: gen_range(0.12, 0.3),
                shade: -2.0,
                life: gen_range(0.3, 0.8),
            });
        }
        // fumee / terre
        for _ in 0..25 {
            self.particles.push(Particle {
                pos: vec2(x + gen_range(-0.4, 0.4), 0.2),
                vel: vec2(gen_range(-4.0, 4.0), gen_range(1.0, 7.0)),
                size: gen_range(0.15, 0.35),
                shade: -1.0,
                life: gen_range(0.8, 1.6),
            });
        }
        if self.gore {
            for _ in 0..130 {
                self.particles.push(Particle {
                    pos: self.pos + vec2(gen_range(-0.15, 0.15), gen_range(-0.1, 0.1)),
                    vel: vec2(gen_range(-8.0, 14.0), gen_range(2.0, 20.0)),
                    size: gen_range(0.02, 0.09),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
            for _ in 0..3 {
                self.pieces.push(Piece {
                    pos: self.pos,
                    vel: vec2(gen_range(-5.0, 12.0), gen_range(8.0, 20.0)),
                    ang: gen_range(0.0, TAU),
                    spin: gen_range(-20.0, 20.0),
                    size: gen_range(0.05, 0.11),
                });
            }
            for _ in 0..gen_range(4, 9) {
                self.add_splat();
            }
        }
    }

    /// Distance le long de la batte si elle touche le chat.
    fn bat_contact(&self, p: &Pose) -> Option<f32> {
        let dir = Vec2::from_angle(p.bat);
        let rel = self.pos - p.hands;
        let r = rel.dot(dir).clamp(0.05, BAT_LEN);
        let closest = p.hands + dir * r;
        if closest.distance(self.pos) < CAT_R + 0.09 {
            Some(r)
        } else {
            None
        }
    }

    fn add_trauma(&mut self, x: f32) {
        self.trauma = (self.trauma + x).min(1.0);
    }

    fn do_hit(&mut self, p: &Pose, r: f32) {
        let speed_f = p.speed.sqrt();
        let sweet = (1.0 - ((r / BAT_LEN - BAT_SWEET).abs() / 0.5)).clamp(0.2, 1.0);
        let quality = (speed_f * sweet).clamp(0.0, 1.0);
        let bat_deg = (p.bat.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
        let angle_deg = (45.0 + (bat_deg - BAT_MID_DEG) * 0.75).clamp(5.0, 85.0);
        let mut speed = 42.0 * (0.2 + 0.8 * quality) * gen_range(0.96, 1.04);

        let label = if quality > 0.93 {
            speed *= 1.08;
            "COUP PARFAIT !"
        } else if quality > 0.75 {
            "Joli coup !"
        } else if quality > 0.45 {
            "Pas mal"
        } else {
            "Coup foireux..."
        };
        self.say(label);

        let dir = Vec2::from_angle(angle_deg.to_radians());
        self.vel = dir * speed;
        self.spin = -speed * gen_range(0.4, 0.8);
        self.hitstop = 0.06 + 0.1 * quality;
        self.add_trauma(0.3 + 0.45 * quality);
        // la camera encaisse le coup dans le sens de la frappe (y ecran inverse)
        self.kick_vel += vec2(dir.x, -dir.y) * (400.0 + 900.0 * quality);
        self.punch = 0.06 + 0.1 * quality;
        self.paf = Some((self.pos, 0.0));
        self.damage = (self.damage + 0.25 + 0.4 * quality).min(1.0);
        self.launch = Some(Launch { quality, angle_deg, speed });
        self.set_phase(Phase::Flying);

        if self.gore {
            self.bat_blood = 1.0;
            // le barbele arrache des lambeaux
            let shreds = 1 + (quality * 2.5) as usize;
            for _ in 0..shreds {
                self.pieces.push(Piece {
                    pos: self.pos,
                    vel: dir.rotate(Vec2::from_angle(gen_range(-0.6, 0.6))) * speed * gen_range(0.15, 0.4),
                    ang: gen_range(0.0, TAU),
                    spin: gen_range(-20.0, 20.0),
                    size: gen_range(0.04, 0.08),
                });
            }
            let n = (45.0 + 100.0 * quality) as usize;
            for i in 0..n {
                let v = if i % 4 == 0 {
                    // eclaboussure vers l'arriere (sur le tigre)
                    vec2(gen_range(-6.0, -1.0), gen_range(-1.0, 4.0))
                } else {
                    dir.rotate(Vec2::from_angle(gen_range(-0.5, 0.5)))
                        * speed
                        * gen_range(0.1, 0.55)
                };
                self.particles.push(Particle {
                    pos: self.pos + vec2(gen_range(-0.1, 0.1), gen_range(-0.1, 0.1)),
                    vel: v,
                    size: gen_range(0.02, 0.08),
                    shade: gen_range(0.0, 1.0),
                    life: 6.0,
                });
            }
            if quality > 0.6 {
                let ns = 2 + (quality * 7.0) as usize;
                for _ in 0..ns {
                    self.add_splat();
                }
            }
        }
    }

    fn ground_gore(&mut self, impact: f32) {
        self.add_trauma((impact * 0.025).min(0.5));
        self.kick_vel += vec2(0.0, impact * 25.0);
        self.damage = (self.damage + impact * 0.015).min(1.0);
        if !self.gore {
            // nuage de poussiere
            for _ in 0..(impact as usize).min(25) {
                self.particles.push(Particle {
                    pos: vec2(self.pos.x, 0.05),
                    vel: vec2(gen_range(-2.0, 2.0) + self.vel.x * 0.2, gen_range(0.5, 2.5)),
                    size: gen_range(0.05, 0.12),
                    shade: -1.0, // poussiere
                    life: gen_range(0.4, 0.9),
                });
            }
            return;
        }
        let n = ((impact * 3.0) as usize).min(90);
        for _ in 0..n {
            self.particles.push(Particle {
                pos: vec2(self.pos.x + gen_range(-0.1, 0.1), 0.08),
                vel: vec2(
                    self.vel.x * gen_range(0.1, 0.6) + gen_range(-2.5, 2.5),
                    gen_range(0.5, 1.0) * impact * 0.45,
                ),
                size: gen_range(0.02, 0.07),
                shade: gen_range(0.0, 1.0),
                life: 6.0,
            });
        }
        self.decals.push(Decal {
            x: self.pos.x,
            w: 0.2 + impact * 0.03,
            shade: gen_range(0.0, 1.0),
        });
        if impact > 13.0 {
            let n = if impact > 20.0 { 2 } else { 1 };
            for _ in 0..n {
                self.pieces.push(Piece {
                    pos: self.pos,
                    vel: vec2(self.vel.x * gen_range(0.3, 0.8), impact * gen_range(0.3, 0.6)),
                    ang: gen_range(0.0, TAU),
                    spin: gen_range(-15.0, 15.0),
                    size: gen_range(0.05, 0.1),
                });
            }
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
        let dist = (self.pos.x - DROP_X).max(0.0);
        self.history_push(dist);
        if dist > self.best {
            self.best = dist;
            self.new_record = true;
            packarch_paf_le_bengal::save_record(SAVE_FILE, dist);
            self.say("NOUVEAU RECORD !");
        }
        self.set_phase(Phase::Done);
    }

    fn distance(&self) -> f32 {
        match self.phase {
            Phase::Flying | Phase::Sliding | Phase::Done => (self.pos.x - DROP_X).max(0.0),
            _ => 0.0,
        }
    }

    // ---------------------------------------------------------------- camera
    fn update_camera(&mut self, dt: f32) {
        let w = screen_width();
        let h = screen_height();
        self.view.ground_sy = h - 90.0;
        self.view.anchor_x = w * 0.38;

        let (target_cam, target_s) = match self.phase {
            Phase::Perched | Phase::Falling => (vec2(0.5, 0.0), 100.0),
            Phase::Done | Phase::Missed => {
                // gros plan sur le chat, centre a l'ecran
                let s = if self.dismembered { 160.0 } else { 230.0 };
                let cx = self.pos.x + (w * 0.5 - self.view.anchor_x) / s;
                let cy = self.pos.y - (self.view.ground_sy - h * 0.68) / s;
                (vec2(cx, cy), s)
            }
            _ => {
                let usable = self.view.ground_sy - 80.0;
                let look_y = self.pos.y + self.vel.y.max(0.0) * 0.5;
                let s_h = usable / (look_y + 2.5);
                let s_v = 100.0 - self.vel.length() * 1.6;
                let s = s_h.min(s_v).clamp(9.0, 100.0);
                let visible_h = usable / s;
                let cy = (self.pos.y + 1.5 - visible_h).max(0.0);
                let cx = (self.pos.x + self.vel.x * 0.12).max(0.5);
                (vec2(cx, cy), s)
            }
        };
        let k = 1.0 - (-7.0 * dt).exp();
        let ks = 1.0 - (-6.0 * dt).exp();
        self.view.cam += (target_cam - self.view.cam) * k;
        self.view.s += (target_s - self.view.s) * ks;

        // Recul directionnel : ressort amorti (oscille une fois ou deux puis se calme)
        let acc = -self.kick * 260.0 - self.kick_vel * 16.0;
        self.kick_vel += acc * dt;
        self.kick += self.kick_vel * dt;

        // Secousse "trauma" : bruit lisse (somme de sinus) proportionnel a trauma^2
        self.trauma = (self.trauma - dt * 1.4).max(0.0);
        let t = self.time * 1.0;
        let tr = self.trauma * self.trauma;
        let noise = vec2(
            (t * 41.0).sin() + 0.6 * (t * 87.0 + 1.3).sin(),
            (t * 47.0 + 0.7).sin() + 0.6 * (t * 79.0 + 2.1).sin(),
        ) / 1.6;
        self.view.shake = noise * tr * 26.0 + self.kick;

        // Punch de zoom
        self.punch *= (-9.0 * dt).exp();
        self.view.zoom = 1.0 + self.punch;
    }

    // ---------------------------------------------------------------- rendu
    fn draw(&self) {
        let v = &self.view;
        self.draw_backdrop();
        self.draw_markers();
        self.draw_decals();
        self.draw_mines();
        draw_cat_tree(v, self.time);
        self.draw_tiger();
        for p in &self.pieces {
            ellipse(v, p.pos, p.size, p.size * 0.6, p.ang, BENGAL);
            if self.gore {
                ellipse(v, p.pos, p.size * 0.6, p.size * 0.45, p.ang + 0.3, blood(0.4));
            }
        }
        if !self.dismembered {
            self.draw_bengal();
        }
        for p in &self.parts {
            self.draw_part(p);
        }
        if v.s < 45.0 && matches!(self.phase, Phase::Flying | Phase::Sliding) {
            // reperes quand la camera a beaucoup dezoome
            let sp = v.p(self.pos);
            let a = ((45.0 - v.s) / 20.0).min(0.8);
            draw_circle_lines(sp.x, sp.y, 20.0, 2.0, Color::new(1.0, 1.0, 1.0, a));
        }
        for e in &self.explosions {
            self.draw_explosion(e);
        }
        for p in &self.particles {
            if p.shade < -1.5 {
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
        // presentation des deux protagonistes
        let alpha = match self.phase {
            Phase::Perched => 1.0,
            Phase::Falling => 1.0 - self.phase_t * 4.0,
            _ => 0.0,
        };
        if alpha > 0.0 {
            let bob = (self.time * 3.0).sin() * 3.0;
            let tiger_head = v.p(vec2(-0.35, 2.72)) + vec2(0.0, bob);
            let bengal_head = v.p(vec2(DROP_X - 0.27, BRANCH_Y + 0.5)) + vec2(0.0, -bob);
            bubble(tiger_head, "Baby Kart", alpha);
            bubble(bengal_head, "Gremlin du Bengal", alpha);
        }
        self.draw_paf();
        self.draw_splats();
        self.draw_hud();
    }

    fn draw_backdrop(&self) {
        let v = &self.view;
        let w = screen_width();
        let h = screen_height();
        // ciel en degrade
        let strips = 24;
        for i in 0..strips {
            let t = i as f32 / strips as f32;
            let c = Color::new(
                0.33 + 0.5 * t,
                0.55 + 0.37 * t,
                0.88 + 0.1 * t,
                1.0,
            );
            draw_rectangle(0.0, h * t, w, h / strips as f32 + 1.0, c);
        }
        draw_circle(w * 0.82, h * 0.16, 46.0, Color::new(1.0, 0.95, 0.7, 1.0));

        let gy = v.ground_sy + v.cam.y * v.s;
        let cx = v.cam.x;

        // nuages
        for i in 0..7 {
            let span = w + 400.0;
            let x = ((i as f32 * 397.0 - cx * 4.0).rem_euclid(span)) - 200.0;
            let y = 60.0 + (i as f32 * 1.7).sin().abs() * 120.0 + v.cam.y * v.s * 0.05;
            let c = Color::new(1.0, 1.0, 1.0, 0.85);
            draw_circle(x, y, 28.0, c);
            draw_circle(x + 30.0, y - 12.0, 34.0, c);
            draw_circle(x + 64.0, y, 26.0, c);
        }

        // collines (parallaxe)
        let layers = [
            (3.0, 150.0, Color::new(0.55, 0.68, 0.62, 1.0), 0.2),
            (9.0, 70.0, Color::new(0.42, 0.6, 0.38, 1.0), 0.5),
        ];
        for (k, base, col, py) in layers {
            let y0 = v.ground_sy + v.cam.y * v.s * py;
            let mut x = 0.0;
            while x < w {
                let wx = x + cx * k;
                let hh = base + 35.0 * (wx * 0.004).sin() + 22.0 * (wx * 0.011 + 1.3).sin();
                draw_rectangle(x, y0 - hh, 7.0, hh + h, col);
                x += 6.0;
            }
        }

        // sol
        if gy < h {
            draw_rectangle(0.0, gy, w, h - gy + 10.0, Color::new(0.35, 0.52, 0.24, 1.0));
            draw_rectangle(0.0, gy, w, 7.0, Color::new(0.47, 0.7, 0.3, 1.0));
        }
    }

    fn draw_markers(&self) {
        let v = &self.view;
        let w = screen_width();
        let step = if v.s > 40.0 {
            5.0
        } else if v.s > 16.0 {
            10.0
        } else {
            25.0
        };
        let left = v.cam.x - v.anchor_x / v.s - step;
        let right = v.cam.x + (w - v.anchor_x) / v.s + step;
        let mut d = ((left - DROP_X) / step).floor().max(0.0) * step;
        while DROP_X + d < right {
            let x = DROP_X + d;
            seg(v, vec2(x, 0.0), vec2(x, 0.45), 0.05, Color::new(1.0, 1.0, 1.0, 0.8));
            let sp = v.p(vec2(x, 0.0));
            let txt = format!("{} m", d as i32);
            let tw = measure_text(&txt, None, 22, 1.0).width;
            draw_text(&txt, sp.x - tw / 2.0, sp.y + 30.0, 22.0, Color::new(1.0, 1.0, 1.0, 0.85));
            d += step;
        }
        if self.best > 0.0 {
            let x = DROP_X + self.best;
            seg(v, vec2(x, 0.0), vec2(x, 1.2), 0.06, Color::new(1.0, 0.85, 0.1, 1.0));
            tri(v, vec2(x, 1.2), vec2(x + 0.6, 1.05), vec2(x, 0.9), Color::new(1.0, 0.85, 0.1, 1.0));
            let sp = v.p(vec2(x, 1.2));
            draw_text("RECORD", sp.x + 4.0, sp.y - 8.0, 20.0, Color::new(1.0, 0.85, 0.1, 1.0));
        }
    }

    fn draw_explosion(&self, e: &Explosion) {
        let v = &self.view;
        let k = e.age / EXPLOSION_TIME;
        let grow = 1.0 - (1.0 - k).powi(3);
        if e.age < 0.1 {
            ellipse(v, e.pos, 2.6, 2.6, 0.0, Color::new(1.0, 1.0, 0.85, 0.9));
        }
        // onde de choc
        let sp = v.p(e.pos);
        draw_circle_lines(sp.x, sp.y, v.l(0.5 + 5.0 * grow), 3.0, Color::new(1.0, 1.0, 1.0, (1.0 - k) * 0.7));
        // boule de feu qui vire a la fumee
        let a = 1.0 - k;
        let fire = Color::new(1.0, 0.85 - 0.6 * k, 0.2 * (1.0 - k), a);
        let smoke = Color::new(0.25, 0.22, 0.2, a * 0.8);
        for (dx, dy, r) in [(0.0, 0.6, 1.0), (-0.6, 0.3, 0.75), (0.6, 0.35, 0.8), (0.0, 1.4, 0.7)] {
            let c = e.pos + vec2(dx, dy) * (0.5 + grow);
            let rr = r * (0.4 + 1.6 * grow);
            ellipse(v, c + vec2(0.0, 0.5 * k), rr, rr, 0.0, if k > 0.45 { smoke } else { fire });
        }
        if k < 0.6 {
            ellipse(v, e.pos + vec2(0.0, 0.6), 0.7 * (0.5 + grow), 0.6 * (0.5 + grow), 0.0, Color::new(1.0, 1.0, 0.6, 1.0 - k / 0.6));
        }
    }

    fn draw_mines(&self) {
        let v = &self.view;
        for m in &self.mines {
            if m.used {
                continue;
            }
            ellipse(v, vec2(m.x, 0.03), 0.45, 0.14, 0.0, Color::new(0.2, 0.22, 0.2, 1.0));
            ellipse(v, vec2(m.x, 0.12), 0.28, 0.15, 0.0, Color::new(0.33, 0.36, 0.3, 1.0));
            seg(v, vec2(m.x - 0.36, 0.06), vec2(m.x + 0.36, 0.06), 0.04, Color::new(0.85, 0.75, 0.1, 1.0));
            let on = (self.time * 2.5 + m.x).fract() < 0.4;
            let c = if on { Color::new(1.0, 0.1, 0.1, 1.0) } else { Color::new(0.4, 0.05, 0.05, 1.0) };
            ellipse(v, vec2(m.x, 0.28), 0.055, 0.055, 0.0, c);
            if on {
                ellipse(v, vec2(m.x, 0.28), 0.16, 0.16, 0.0, Color::new(1.0, 0.2, 0.2, 0.3));
            }
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
        let ps = pose(self.swing_t, self.time);
        let dir = Vec2::from_angle(ps.bat);
        let o = vec2(ps.lean, bob);
        let hands = ps.hands;

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

        // bras arriere (main du haut sur le manche)
        let (l1, l2) = (0.34, 0.34);
        let sb = SHOULDER + o + vec2(-0.14, 0.02);
        let eb = elbow(sb, hands, l1, l2, -1.0);
        seg(v, sb, eb, 0.16, TABBY_SHADE);
        seg(v, eb, hands, 0.14, TABBY_SHADE);

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

        // trainee de vitesse derriere la batte
        if let Some(t) = self.swing_t {
            for k in 1..=8 {
                let pk = pose(Some(t - k as f32 * 0.007), self.time);
                if pk.speed < 0.15 {
                    continue;
                }
                let dk = Vec2::from_angle(pk.bat);
                let nk = dk.perp();
                let a = (1.0 - k as f32 / 9.0) * 0.28 * pk.speed;
                let c = Color::new(1.0, 1.0, 1.0, a);
                let p0 = pk.hands + dk * 0.35;
                let p1 = pk.hands + dk * BAT_LEN;
                quad(v, p0 + nk * 0.04, p1 + nk * 0.08, p1 - nk * 0.08, p0 - nk * 0.04, c);
            }
        }

        // batte (tenue par les mains, le pommeau depasse un peu)
        let at = |r: f32| hands + dir * r;
        let half_w = |r: f32| 0.028 + (0.075 - 0.028) * ((r + 0.1) / (BAT_LEN + 0.1)).clamp(0.0, 1.0);
        let a = at(-0.1);
        let b = at(BAT_LEN);
        let n = dir.perp();
        quad(v, a + n * 0.028, b + n * 0.075, b - n * 0.075, a - n * 0.028, WOOD);
        quad(v, a + n * 0.028, b + n * 0.075, b + n * 0.04, a + n * 0.01, Color::new(0.75, 0.55, 0.32, 1.0));
        ellipse(v, b, 0.075, 0.075, 0.0, WOOD_DARK);
        ellipse(v, a, 0.045, 0.045, 0.0, WOOD_DARK); // pommeau
        seg(v, at(-0.06), at(0.24), 0.065, Color::new(0.2, 0.2, 0.2, 1.0)); // grip
        if self.gore && self.bat_blood > 0.0 {
            let c = with_alpha(blood(0.3), self.bat_blood.min(1.0));
            quad(v, at(0.8) + n * 0.06, b + n * 0.078, b - n * 0.078, at(0.86) - n * 0.06, c);
            ellipse(v, b, 0.078, 0.078, 0.0, c);
            // goutte qui pend
            let drip = at(1.0) - n * 0.06;
            seg(v, drip, drip + vec2(0.0, -0.08 - (self.time * 0.7).fract() * 0.06), 0.03, c);
        }

        // fil barbele enroule autour de la batte
        let wire = Color::new(0.62, 0.64, 0.68, 1.0);
        let wire_dark = Color::new(0.35, 0.36, 0.4, 1.0);
        let mut r = 0.42;
        let mut side = 1.0;
        while r < BAT_LEN - 0.08 {
            let r2 = r + 0.09;
            let p1 = at(r) + n * half_w(r) * side;
            let p2 = at(r2) - n * half_w(r2) * side;
            seg(v, p1, p2, 0.016, wire);
            // ardillons
            let c = at(r + 0.045);
            let out = n * (half_w(r) + 0.05);
            seg(v, c + out + dir * 0.03, c - out - dir * 0.03, 0.011, wire_dark);
            seg(v, c + out - dir * 0.03, c - out + dir * 0.03, 0.011, wire_dark);
            if self.gore && self.bat_blood > 0.3 && side > 0.0 {
                ellipse(v, c + out * 0.9, 0.018, 0.018, 0.0, blood(0.1));
            }
            r = r2;
            side = -side;
        }

        // main du haut, refermee sur le manche
        ellipse(v, hands, 0.085, 0.075, ps.bat, TABBY_SHADE);

        // bras avant (main du bas), devant le corps
        let hand2 = at(0.15);
        let sf = SHOULDER + o + vec2(0.05, -0.02);
        let ef = elbow(sf, hand2, l1, l2, -1.0);
        seg(v, sf, ef, 0.17, TABBY);
        seg(v, ef, hand2, 0.15, TABBY);
        for k in [0.35, 0.65] {
            let c = sf.lerp(ef, k);
            let nn = (ef - sf).normalize_or_zero().perp() * 0.07;
            seg(v, c - nn, c + nn * 0.4, 0.03, TABBY_DARK);
        }
        let c = ef.lerp(hand2, 0.5);
        let nn = (hand2 - ef).normalize_or_zero().perp() * 0.06;
        seg(v, c - nn, c + nn * 0.4, 0.03, TABBY_DARK);
        ellipse(v, hand2, 0.095, 0.085, ps.bat, TABBY);
    }

    /// Onomatopee "PAF !" facon BD au point d'impact.
    fn draw_paf(&self) {
        let Some((pos, age)) = self.paf else { return };
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
        let txt = "PAF !";
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
        let flying = matches!(self.phase, Phase::Flying | Phase::Falling);
        let ko = matches!(self.phase, Phase::Flying | Phase::Sliding | Phase::Done | Phase::Missed);
        let flail = if flying { (self.time * 25.0).sin() * 0.06 } else { 0.0 };

        // queue
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

        draw_rectangle(12.0, 12.0, 300.0, 112.0, panel);
        draw_text(&format!("Distance : {:.1} m", self.distance()), 24.0, 46.0, 34.0, WHITE);
        draw_text(&format!("Record   : {:.1} m", self.best), 24.0, 76.0, 26.0, Color::new(1.0, 0.85, 0.2, 1.0));
        draw_text(&format!("Hauteur  : {:.1} m", self.max_height), 24.0, 104.0, 22.0, LIGHTGRAY);

        if let Some(l) = &self.launch {
            draw_rectangle(12.0, 132.0, 300.0, 62.0, panel);
            draw_text(
                &format!("Puissance : {:.0} %   Mines : {}", l.quality * 100.0, self.mines_hit),
                24.0,
                158.0,
                22.0,
                WHITE,
            );
            draw_text(
                &format!("Angle : {:.0} deg   Vitesse : {:.0} km/h", l.angle_deg, l.speed * 3.6),
                24.0,
                182.0,
                18.0,
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
            Phase::Perched => "Clic / Espace : faire sauter le bengal",
            Phase::Falling => "Clic / Espace : FRAPPER !",
            Phase::Flying | Phase::Sliding => "",
            Phase::Done | Phase::Missed if self.phase_t > if self.gore { 1.8 } else { 0.6 } => "Clic / Espace : rejouer",
            Phase::Done | Phase::Missed => "",
        };
        if !hint.is_empty() {
            let size = 30;
            let tw = measure_text(hint, None, size, 1.0).width;
            draw_rectangle(w / 2.0 - tw / 2.0 - 14.0, h - 58.0, tw + 28.0, 42.0, panel);
            draw_text(hint, w / 2.0 - tw / 2.0, h - 28.0, size as f32, WHITE);
        }

        // titre du jeu sur l'ecran de depart
        if self.phase == Phase::Perched {
            let txt = "PAF LE BENGAL";
            let size = 64u16;
            let dim = measure_text(txt, None, size, 1.0);
            let x = w / 2.0 - dim.width / 2.0 + 60.0;
            let y = 66.0 + (self.time * 2.0).sin() * 3.0;
            for (dx, dy) in [(-3.0, 0.0), (3.0, 0.0), (0.0, -3.0), (0.0, 3.0), (4.0, 4.0)] {
                draw_text(txt, x + dx, y + dy, size as f32, Color::new(0.45, 0.02, 0.02, 1.0));
            }
            draw_text(txt, x, y, size as f32, Color::new(1.0, 0.86, 0.15, 1.0));
        }

        let gore_txt = if self.gore { "G : gore ON" } else { "G : gore OFF" };
        draw_text(gore_txt, w - 140.0, h - 20.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.7));

        if self.msg_t > 0.0 {
            let a = self.msg_t.min(1.0);
            let pop = 1.0 + (2.2 - self.msg_t).clamp(0.0, 0.15) * -2.0 + 0.3;
            let size = (58.0 * pop) as u16;
            let tw = measure_text(&self.msg, None, size, 1.0).width;
            let col = if self.msg.contains("PARFAIT") || self.msg.contains("RECORD") {
                Color::new(1.0, 0.85, 0.1, a)
            } else if self.msg.contains("MANQUE") {
                Color::new(1.0, 0.25, 0.2, a)
            } else {
                Color::new(1.0, 1.0, 1.0, a)
            };
            let y = h * 0.3;
            draw_text(&self.msg, w / 2.0 - tw / 2.0 + 3.0, y + 3.0, size as f32, Color::new(0.0, 0.0, 0.0, a * 0.6));
            draw_text(&self.msg, w / 2.0 - tw / 2.0, y, size as f32, col);
        }

        if self.phase == Phase::Done {
            let txt = format!("{:.1} m", self.distance());
            let size = 80;
            let tw = measure_text(&txt, None, size, 1.0).width;
            draw_text(&txt, w / 2.0 - tw / 2.0 + 4.0, h * 0.45 + 4.0, size as f32, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_text(&txt, w / 2.0 - tw / 2.0, h * 0.45, size as f32, WHITE);
        }
    }
}

/// Arbre a chat geant : poteaux en sisal, plateformes moquettees, niche et pompon.
fn draw_cat_tree(v: &View, time: f32) {
    let carpet = Color::new(0.5, 0.44, 0.58, 1.0);
    let carpet_top = Color::new(0.62, 0.56, 0.7, 1.0);
    let carpet_dark = Color::new(0.38, 0.33, 0.45, 1.0);
    let sisal = Color::new(0.82, 0.7, 0.5, 1.0);
    let sisal_line = Color::new(0.6, 0.48, 0.32, 1.0);

    let post = |x0: f32, x1: f32, y0: f32, y1: f32| {
        quad(v, vec2(x0, y0), vec2(x1, y0), vec2(x1, y1), vec2(x0, y1), sisal);
        let mut y = y0 + 0.03;
        while y < y1 {
            seg(v, vec2(x0, y), vec2(x1, y + 0.012), 0.012, sisal_line);
            y += 0.055;
        }
    };
    let plat = |x0: f32, x1: f32, y0: f32, y1: f32| {
        let r = (y1 - y0) * 0.5;
        quad(v, vec2(x0 + r, y0), vec2(x1 - r, y0), vec2(x1 - r, y1), vec2(x0 + r, y1), carpet);
        ellipse(v, vec2(x0 + r, y0 + r), r, r, 0.0, carpet);
        ellipse(v, vec2(x1 - r, y0 + r), r, r, 0.0, carpet);
        seg(v, vec2(x0 + r, y1 - 0.015), vec2(x1 - r, y1 - 0.015), 0.03, carpet_top);
    };

    // socle et premier etage
    post(1.55, 1.8, 0.14, 1.55);
    post(2.05, 2.22, 0.14, 1.0);
    plat(1.0, 2.35, 0.0, 0.14);
    plat(1.85, 2.45, 1.0, 1.1);
    plat(1.1, 2.25, 1.55, 1.67);

    // niche avec son trou rond
    quad(v, vec2(1.45, 1.67), vec2(2.2, 1.67), vec2(2.2, 2.32), vec2(1.45, 2.32), carpet);
    ellipse(v, vec2(1.8, 1.97), 0.17, 0.17, 0.0, carpet_dark);
    ellipse(v, vec2(1.8, 1.94), 0.14, 0.14, 0.0, Color::new(0.12, 0.1, 0.14, 1.0));
    plat(1.4, 2.25, 2.3, 2.4);

    // grand poteau jusqu'au sommet
    post(1.62, 1.84, 2.4, BRANCH_Y - 0.11);

    // plateforme du sommet (le bengal y est perche) + coussin
    plat(0.32, 2.0, BRANCH_Y - 0.11, BRANCH_Y + 0.01);
    ellipse(v, vec2(1.72, BRANCH_Y + 0.06), 0.26, 0.07, 0.0, carpet_top);

    // pompon qui se balance
    let anchor = vec2(1.05, BRANCH_Y - 0.11);
    let a = -PI / 2.0 + (time * 2.1).sin() * 0.35;
    let ball = anchor + Vec2::from_angle(a) * 0.5;
    seg(v, anchor, ball, 0.012, Color::new(0.9, 0.9, 0.9, 1.0));
    ellipse(v, ball, 0.07, 0.07, 0.0, Color::new(0.95, 0.35, 0.6, 1.0));
    ellipse(v, ball + vec2(-0.02, 0.02), 0.03, 0.03, 0.0, Color::new(1.0, 0.6, 0.8, 1.0));
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
fn bubble(anchor: Vec2, text: &str, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let size = 28u16;
    let dim = measure_text(text, None, size, 1.0);
    let pad = 16.0;
    let bw = dim.width + pad * 2.0;
    let bh = 46.0;
    let bx = (anchor.x - bw * 0.3).clamp(8.0, screen_width() - bw - 8.0);
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
        window_title: "Paf le Bengal".to_owned(),
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
