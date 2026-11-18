//! Code commun aux deux jeux de Paf le Bengal : l'enregistrement des records
//! et le panneau d'infos (version et nouveautes).
//!
//! Les records sont gardes dans `$XDG_DATA_HOME/packarch-paf-le-bengal/`
//! (par defaut `~/.local/share/packarch-paf-le-bengal/`).

use std::path::PathBuf;

/// Dossier des donnees du jeu.
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("packarch-paf-le-bengal"))
}

/// Lit un record (0 s'il n'existe pas encore).
pub fn load_record(file: &str) -> f32 {
    data_dir()
        .and_then(|d| std::fs::read_to_string(d.join(file)).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0)
}

/// Enregistre un record (les erreurs d'ecriture sont ignorees : le jeu continue).
pub fn save_record(file: &str, value: f32) {
    if let Some(d) = data_dir() {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(file), format!("{value:.2}"));
    }
}

// ---------------------------------------------------------------------------
// Panneau d'infos : version et nouveautes
// ---------------------------------------------------------------------------

use macroquad::prelude::*;

/// Version du jeu (celle de Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nouveautes, de la plus recente a la plus ancienne. A completer a chaque
/// version (sans accents : la police par defaut de macroquad n'en a pas).
pub const CHANGELOG: &[(&str, &[&str])] = &[
    (
        "1.1.0",
        &[
            "Panneau d'infos : la version et les nouveautes s'affichent au premier lancement apres une mise a jour",
            "Ensuite, bouton i en bas a gauche (ou touche I) pour le rouvrir : le jeu est en pause",
        ],
    ),
    (
        "1.0.2",
        &[
            "Baseball : un coup parfait enchaine trois mines, un joli coup deux",
            "Baseball : mines plus faciles a declencher, un coup parfait part toujours en longueur",
        ],
    ),
    ("1.0.1", &["Le premier jeu devient \"Paf le Bengal : Baseball\""]),
    ("1.0.0", &["Premiere version : Baseball et Altitude"]),
];

/// Panneau d'infos d'un jeu, avec son bouton. Ouvert d'office au premier
/// lancement de chaque version (memorisee dans le dossier des donnees).
pub struct Infos {
    title: &'static str,
    pub open: bool,
}

impl Infos {
    /// `game` distingue les jeux : chacun montre les nouveautes une fois.
    pub fn new(game: &str, title: &'static str) -> Self {
        let file = format!("version_vue_{game}.txt");
        let seen = data_dir().and_then(|d| std::fs::read_to_string(d.join(&file)).ok());
        let open = seen.as_deref().map(str::trim) != Some(VERSION);
        if open && let Some(d) = data_dir() {
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join(&file), VERSION);
        }
        Self { title, open }
    }

    fn button() -> (Vec2, f32) {
        (vec2(34.0, screen_height() - 34.0), 17.0)
    }

    fn hovered() -> bool {
        let (c, r) = Self::button();
        Vec2::from(mouse_position()).distance(c) <= r + 3.0
    }

    /// Gere l'ouverture et la fermeture. Renvoie `true` quand le jeu doit
    /// rester en pause (et ne pas lire les touches) pendant cette image.
    pub fn update(&mut self) -> bool {
        let click = is_mouse_button_pressed(MouseButton::Left);
        if self.open {
            if click
                || [KeyCode::Space, KeyCode::Enter, KeyCode::KpEnter, KeyCode::Escape, KeyCode::I]
                    .into_iter()
                    .any(is_key_pressed)
            {
                self.open = false;
            }
            // l'image qui ferme le panneau reste en pause : le clic ne relance pas le jeu
            return true;
        }
        if (click && Self::hovered()) || is_key_pressed(KeyCode::I) {
            self.open = true;
            return true;
        }
        false
    }

    /// Dessine le bouton, et le panneau s'il est ouvert (par-dessus le jeu).
    pub fn draw(&self) {
        if self.open {
            self.draw_panel();
        } else {
            let (c, r) = Self::button();
            let a = if Self::hovered() { 0.95 } else { 0.6 };
            draw_circle(c.x, c.y, r + 2.0, Color::new(0.0, 0.0, 0.0, a * 0.5));
            draw_circle(c.x, c.y, r, Color::new(0.05, 0.08, 0.15, a));
            draw_circle_lines(c.x, c.y, r, 2.0, Color::new(1.0, 1.0, 1.0, a));
            let dim = measure_text("i", None, 26, 1.0);
            draw_text("i", c.x - dim.width / 2.0, c.y + dim.offset_y / 2.0 - 1.0, 26.0, Color::new(1.0, 1.0, 1.0, a));
        }
    }

    fn draw_panel(&self) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));

        let pw = (sw - 32.0).min(660.0);
        let pad = 28.0;
        let text_w = pw - pad * 2.0;
        let gold = Color::new(1.0, 0.86, 0.15, 1.0);
        let footer_h = 56.0;
        let max_h = sh - 48.0;

        // lignes : (texte, taille, couleur, retrait, espace avant)
        let mut lines: Vec<(String, u16, Color, f32, f32)> = vec![
            (self.title.to_uppercase(), 36, gold, 0.0, 0.0),
            (format!("Version {VERSION}"), 24, LIGHTGRAY, 0.0, 6.0),
            ("Nouveautes".to_string(), 28, WHITE, 0.0, 22.0),
        ];
        let line_h = |size: u16| size as f32 * 1.15;
        let mut height = pad + lines.iter().map(|l| line_h(l.1) + l.4).sum::<f32>();

        for (i, (ver, changes)) in CHANGELOG.iter().enumerate() {
            let mut block = vec![(format!("Version {ver}"), 24u16, gold, 0.0, 14.0)];
            let (size, col) = if i == 0 { (22u16, WHITE) } else { (20u16, LIGHTGRAY) };
            for change in *changes {
                for (j, part) in wrap(change, size, text_w - 22.0).into_iter().enumerate() {
                    let txt = if j == 0 { format!("- {part}") } else { part };
                    block.push((txt, size, col, if j == 0 { 0.0 } else { 18.0 }, 4.0));
                }
            }
            let bh: f32 = block.iter().map(|l| line_h(l.1) + l.4).sum();
            if i > 0 && height + bh + footer_h > max_h {
                break; // plus de place : on garde les versions recentes
            }
            height += bh;
            lines.extend(block);
        }
        let ph = height + footer_h;
        let px = (sw - pw) / 2.0;
        let py = ((sh - ph) / 2.0).max(8.0);

        rounded(px - 3.0, py - 3.0, pw + 6.0, ph + 6.0, 16.0, Color::new(1.0, 0.86, 0.15, 1.0));
        rounded(px, py, pw, ph, 14.0, Color::new(0.05, 0.08, 0.15, 1.0));

        let mut y = py + pad;
        for (txt, size, col, indent, gap) in &lines {
            y += gap + line_h(*size);
            draw_text(txt, px + pad + indent, y - *size as f32 * 0.25, *size as f32, *col);
        }

        let hint = "PAUSE  -  Clic, Espace ou I : reprendre";
        let dim = measure_text(hint, None, 20, 1.0);
        draw_text(hint, px + (pw - dim.width) / 2.0, py + ph - 22.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.75));
    }
}

/// Coupe `text` en lignes d'au plus `max_w` pixels.
fn wrap(text: &str, size: u16, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && measure_text(&next, None, size, 1.0).width > max_w {
            out.push(std::mem::replace(&mut cur, word.to_string()));
        } else {
            cur = next;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    draw_rectangle(x + r, y, w - 2.0 * r, h, c);
    draw_rectangle(x, y + r, r, h - 2.0 * r, c);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, c);
    for (cx, cy) in [(x + r, y + r), (x + w - r, y + r), (x + r, y + h - r), (x + w - r, y + h - r)] {
        draw_circle(cx, cy, r, c);
    }
}
