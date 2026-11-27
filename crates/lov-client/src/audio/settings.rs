//! Réglages de son enregistrés d'un lancement à l'autre : volume de chaque canal et coupure générale.
//!
//! Portage de la persistance de `packages/client/src/audio/AudioManager.ts`, qui les gardait dans
//! `localStorage` (clé `lov.audio`). Le client natif les écrit dans un petit fichier texte du dossier de
//! configuration de l'utilisateur :
//!
//! - Windows : `%APPDATA%\LeagueOfVibes\son.txt`
//! - macOS : `~/Library/Application Support/LeagueOfVibes/son.txt`
//! - Linux : `$XDG_CONFIG_HOME/league-of-vibes/son.txt` (sinon `~/.config/…`)
//!
//! `LOV_CONFIG_DIR` remplace ce dossier. Une ligne par réglage (`musique = 0.5`) : le fichier se lit et
//! se corrige à la main ; une ligne inconnue ou illisible est ignorée, une valeur absente garde son
//! défaut, comme le `Partial<…>` du web.

use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::AudioManager;
use super::mixer::{AUDIO_CHANNELS, Channel, Volumes};

const FILE: &str = "son.txt";

/// Clé de chaque canal dans le fichier.
fn key(ch: Channel) -> &'static str {
    match ch {
        Channel::Master => "general",
        Channel::Sfx => "effets",
        Channel::Voice => "voix",
        Channel::Music => "musique",
        Channel::Ui => "interface",
    }
}
const MUTED: &str = "coupe";

/// Dossier de configuration du client natif, `None` si le système n'en donne pas.
pub fn config_dir() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = var("LOV_CONFIG_DIR") {
        return Some(dir);
    }
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join("LeagueOfVibes"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/LeagueOfVibes"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("league-of-vibes"))
    }
}

/// Texte du fichier.
pub fn format(v: &Volumes) -> String {
    let mut s = String::from("# Réglages de son de League of Vibes (volumes de 0 à 1)\n");
    for ch in AUDIO_CHANNELS {
        s.push_str(&std::format!("{} = {}\n", key(ch), v.get(ch)));
    }
    s.push_str(&std::format!("{MUTED} = {}\n", v.muted));
    s
}

/// Lit le texte du fichier par-dessus `base` (les défauts).
pub fn parse(text: &str, base: Volumes) -> Volumes {
    let mut v = base;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, val)) = line.split_once('=') else {
            continue;
        };
        let (k, val) = (k.trim(), val.trim());
        if k == MUTED {
            if let Ok(m) = val.parse::<bool>() {
                v.muted = m;
            }
        } else if let Some(ch) = AUDIO_CHANNELS.into_iter().find(|&ch| key(ch) == k)
            && let Ok(x) = val.parse::<f32>()
            && x.is_finite()
        {
            v.set(ch, x);
        }
    }
    v
}

/// Réglages enregistrés dans `dir`, ou les défauts si le fichier n'existe pas.
pub fn load(dir: &Path) -> Volumes {
    std::fs::read_to_string(dir.join(FILE))
        .map_or_else(|_| Volumes::default(), |t| parse(&t, Volumes::default()))
}

/// Enregistre les réglages dans `dir` (créé au besoin).
pub fn save(dir: &Path, v: &Volumes) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(FILE), format(v))
}

/// Dossier où lire et écrire les réglages ; `None` : rien n'est lu ni écrit (tests, système sans
/// dossier de configuration).
#[derive(Resource, Debug, Clone, Default)]
pub struct SettingsDir(pub Option<PathBuf>);

/// Au démarrage : applique les réglages enregistrés.
pub fn load_volumes(dir: Res<SettingsDir>, mut audio: ResMut<AudioManager>) {
    if let Some(dir) = &dir.0 {
        audio.restore(load(dir));
    }
}

/// Après un changement (`set_volume`, `set_muted`) : enregistre les réglages, comme le web à chaque
/// réglage. Une écriture ratée est signalée une fois, sans bloquer le jeu.
pub fn save_volumes(dir: Res<SettingsDir>, mut audio: ResMut<AudioManager>) {
    if !audio.take_changed() {
        return;
    }
    if let Some(dir) = &dir.0
        && let Err(e) = save(dir, &audio.volumes())
    {
        warn!(
            "réglages de son non enregistrés dans {} : {e}",
            dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(std::format!("lov-son-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn un_volume_change_est_retrouve_au_lancement_suivant() {
        let dir = temp_dir("relance");
        assert_eq!(load(&dir), Volumes::default(), "pas encore de fichier");
        let mut v = Volumes::default();
        v.set(Channel::Music, 0.2);
        v.set(Channel::Sfx, 0.0);
        v.muted = true;
        save(&dir, &v).expect("écriture");
        assert_eq!(load(&dir), v);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn une_ligne_illisible_garde_le_defaut() {
        let base = Volumes::default();
        let v = parse(
            "# commentaire\nmusique = 0.3\neffets = beaucoup\nvoix = NaN\ngeneral = 7\ninconnu = 1\ncoupe = peut-être\n",
            base,
        );
        assert_eq!(v.get(Channel::Music), 0.3);
        assert_eq!(v.get(Channel::Sfx), base.get(Channel::Sfx));
        assert_eq!(v.get(Channel::Voice), base.get(Channel::Voice));
        assert_eq!(v.get(Channel::Master), 1.0, "borné à 1");
        assert!(!v.muted);
        assert_eq!(parse(&format(&v), Volumes::default()), v);
    }

    /// Le vrai circuit : réglages relus au démarrage, puis réécrits après un `set_volume`.
    #[test]
    fn les_systemes_relisent_et_enregistrent() {
        use bevy::ecs::system::RunSystemOnce;
        let dir = temp_dir("systemes");
        let mut saved = Volumes::default();
        saved.set(Channel::Ui, 0.4);
        save(&dir, &saved).expect("écriture");

        let (am, _rx) = AudioManager::for_tests();
        let mut world = World::new();
        world.insert_resource(am);
        world.insert_resource(SettingsDir(Some(dir.clone())));
        assert!(world.run_system_once(load_volumes).is_ok());
        assert_eq!(world.resource::<AudioManager>().volume(Channel::Ui), 0.4);
        // relire n'est pas un changement : rien n'est réécrit
        assert!(world.run_system_once(save_volumes).is_ok());
        world
            .resource_mut::<AudioManager>()
            .set_volume(Channel::Music, 0.1);
        assert!(world.run_system_once(save_volumes).is_ok());
        assert_eq!(load(&dir).get(Channel::Music), 0.1);
        assert_eq!(load(&dir).get(Channel::Ui), 0.4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
