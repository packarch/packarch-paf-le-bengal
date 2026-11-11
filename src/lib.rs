//! Code commun aux deux jeux de Paf le Bengal : l'enregistrement des records.
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
