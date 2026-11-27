//! Musique : bus de mixage, réverbération, instruments, jingles d'élimination, thèmes et séquenceur.
//!
//! Portage de `packages/client/src/audio/music/instruments.ts`, `stings.ts`, `themes.ts` et de
//! `audio/music.ts` ([`sequencer`]) (issue #118).
//!
//! Modèle temporel : le web programme des nœuds à un instant `t` de `ctx.currentTime`. Ici, un
//! instrument ajoute des [`Note`](note::Note)s datées en **temps absolu du bus** à une [`Score`] ; la
//! phrase part en un seul message ([`BusCommand::Play`]) vers le [`MusicBus`] du fil audio, dont le jeu lit
//! l'horloge par [`MusicBusHandle::current_time`]. Contrairement à une [`Voice`](super::synth::Voice) (temps
//! relatif à sa création), une note garde son instant même si le message arrive avec un bloc de retard :
//! le séquenceur à anticipation de la PR suivante restera en rythme.

pub mod bus;
pub mod instruments;
pub mod note;
pub mod reverb;
pub mod sequencer;
pub mod stings;
pub mod themes;
pub mod tour;

pub use bus::{BusCommand, MusicBus, MusicBusHandle};
pub use note::Score;

#[cfg(test)]
mod tests {
    use super::super::mixer::{Channel, Mixer};
    use super::super::synth::SAMPLE_RATE;
    use super::instruments::*;
    use super::stings::{StingSide, first_blood_sting, kill_sting};
    use super::*;

    const SR: f64 = SAMPLE_RATE as f64;

    fn idx(t: f64) -> usize {
        (t * SR) as usize
    }

    /// Rend les notes d'une phrase sans le bus (sec, mono), sur `seconds`.
    fn dry(score: Score, seconds: f64) -> Vec<f64> {
        let noise: Vec<f32> = (0..SAMPLE_RATE)
            .map(|i| ((i * 7919) % 200) as f32 / 100.0 - 1.0)
            .collect();
        let mut notes = score.notes;
        (0..idx(seconds))
            .map(|i| {
                let t = i as f64 / SR;
                notes.iter_mut().map(|n| n.next(t, &noise)).sum()
            })
            .collect()
    }

    fn rms(s: &[f64]) -> f64 {
        (s.iter().map(|x| x * x).sum::<f64>() / s.len().max(1) as f64).sqrt()
    }

    type Build = fn(&mut Score, f64);

    /// Chaque instrument, avec les paramètres par défaut du web.
    const INSTRUMENTS: &[(&str, Build)] = &[
        ("pad", |s, t| pad(s, &[57, 60, 64], t, 2.0, 0.05, 1.0)),
        ("choir", |s, t| choir(s, &[69, 76], t, 2.0, 0.04)),
        ("strings", |s, t| strings(s, 64, t, 0.3, 0.035)),
        ("brass", |s, t| brass(s, &[60, 64, 67], t, 0.5, 0.045)),
        ("bass", |s, t| bass(s, 36, t, 0.5, 0.12)),
        ("bell", |s, t| bell(s, 81, t, 0.05, 2.4)),
        ("flute", |s, t| flute(s, 76, t, 0.8, 0.05)),
        ("pluck", |s, t| pluck(s, 72, t, 0.04)),
        ("taiko", |s, t| taiko(s, t, 0.32, 1.0)),
        ("snare", |s, t| snare(s, t, 0.12)),
        ("swell", |s, t| swell(s, t, 0.5, 0.06)),
        ("crash", |s, t| crash(s, t, 0.06)),
        ("soft_pad", |s, t| soft_pad(s, &[57, 64], t, 3.0, 0.03, 1.0)),
        ("kalimba", |s, t| kalimba(s, 76, t, 0.03)),
    ];

    #[test]
    fn midi_la4_vaut_440_hz() {
        assert_eq!(midi(69), 440.0);
        assert!((midi(81) - 880.0).abs() < 1e-9);
        assert!((midi(60) - 261.6256).abs() < 1e-3);
    }

    #[test]
    fn chaque_instrument_se_tait_avant_t_joue_pendant_et_s_eteint() {
        let t = 0.25;
        for &(name, build) in INSTRUMENTS {
            let mut s = Score::new();
            build(&mut s, t);
            let end = s.end();
            assert!(end > t + 0.1, "{name} : arrêt après le départ");
            let out = dry(s, end + 0.2);
            assert!(out.iter().all(|x| x.is_finite()), "{name} : NaN");
            assert!(
                out[..idx(t)].iter().all(|&x| x == 0.0),
                "{name} : silence avant t"
            );
            let body = rms(&out[idx(t)..idx(t + 0.1)]);
            assert!(body > 1e-4, "{name} : muet ({body})");
            assert!(out.iter().all(|x| x.abs() < 1.0), "{name} : trop fort");
            // enveloppe retombée à 0,0001 avant l'arrêt, puis seul le filtre résonne encore un peu
            assert!(
                rms(&out[idx(end - 0.06)..idx(end - 0.01)]) < 1e-3,
                "{name} : queue"
            );
            let after = out[idx(end) + 1..]
                .iter()
                .fold(0.0_f64, |m, x| m.max(x.abs()));
            assert!(after < 1e-3, "{name} : arrêt ({after})");
        }
    }

    #[test]
    fn le_vibrato_de_la_flute_s_installe() {
        // écart des périodes (passages par zéro interpolés) : la hauteur varie plus à la fin qu'au début
        let mut s = Score::new();
        flute(&mut s, 69, 0.0, 1.5, 0.05);
        let mut notes = s.notes;
        let tone: Vec<f64> = (0..idx(1.4))
            .map(|i| notes[0].next(i as f64 / SR, &[0.0]))
            .collect();
        let spread = |a: f64, b: f64| {
            let w = &tone[idx(a)..idx(b)];
            let z: Vec<f64> = (1..w.len())
                .filter(|&i| w[i - 1] < 0.0 && w[i] >= 0.0)
                .map(|i| i as f64 - 1.0 + w[i - 1] / (w[i - 1] - w[i]))
                .collect();
            let p: Vec<f64> = z.windows(2).map(|p| p[1] - p[0]).collect();
            let m = p.iter().sum::<f64>() / p.len() as f64;
            p.iter().map(|x| (x - m).abs()).fold(0.0, f64::max)
        };
        let (early, late) = (spread(0.1, 0.2), spread(0.9, 1.3));
        assert!(late > 0.4 && late > early * 2.0, "{early} → {late}");
    }

    fn render_bus(bus: &mut MusicBus, seconds: f64) -> Vec<f32> {
        let mut out = vec![0.0; 2 * idx(seconds)];
        bus.render_add(&mut out, 1.0);
        out
    }

    #[test]
    fn le_bus_est_muet_jusqu_au_fondu() {
        let (mut bus, handle) = MusicBus::new(1);
        let mut s = Score::new();
        taiko(&mut s, 0.1, 0.32, 1.0);
        bus.command(BusCommand::Play(s));
        let silent = render_bus(&mut bus, 0.5);
        assert!(
            silent.iter().all(|&x| x == 0.0),
            "gain global à 0 au départ"
        );
        assert!(
            (handle.current_time() - 0.5).abs() < 1e-9,
            "horloge publiée"
        );
    }

    #[test]
    fn le_bus_rend_tous_les_instruments_a_l_instant_dit_sans_nan() {
        let (mut bus, _) = MusicBus::new(1);
        let t = 0.2;
        let mut s = Score::new();
        for &(_, build) in INSTRUMENTS {
            build(&mut s, t);
        }
        let end = s.end();
        bus.command(BusCommand::Play(s));
        bus.command(BusCommand::SetMaster(1.0));
        let out = render_bus(&mut bus, end + 0.5);
        assert!(out.iter().all(|x| x.is_finite()));
        assert!(
            out[..2 * idx(t)].iter().all(|&x| x == 0.0),
            "silence avant t"
        );
        let peak = out.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05, "audible : {peak}");
        assert!(
            out.as_chunks::<2>().0.iter().any(|f| f[0] != f[1]),
            "réverbération stéréo"
        );
        assert_eq!(bus.active(), 0, "notes terminées retirées");
        // la réverbération et l'écho prolongent le son après la dernière note
        let late = &out[2 * (idx(end) + 1)..];
        assert!(late.iter().any(|x| x.abs() > 1e-5));
    }

    #[test]
    fn une_note_en_retard_part_tout_de_suite() {
        let (mut bus, _) = MusicBus::new(4);
        bus.command(BusCommand::SetMaster(1.0));
        render_bus(&mut bus, 0.5);
        let mut s = Score::new();
        bass(&mut s, 45, 0.4, 1.0, 0.12);
        bus.command(BusCommand::Play(s));
        let out = render_bus(&mut bus, 0.05);
        assert!(
            out.iter().any(|x| x.abs() > 0.01),
            "jouée dès le premier bloc"
        );
    }

    #[test]
    fn le_fondu_part_de_la_valeur_courante() {
        let (mut bus, _) = MusicBus::new(2);
        let level = |bus: &mut MusicBus| {
            let mut s = Score::new();
            let t = bus.now();
            bass(&mut s, 45, t, 0.4, 0.12);
            bus.command(BusCommand::Play(s));
            let out = render_bus(bus, 0.3);
            out.iter().fold(0.0_f32, |m, x| m.max(x.abs()))
        };
        bus.command(BusCommand::Fade {
            to: 1.0,
            seconds: 0.0,
        });
        let full = level(&mut bus);
        render_bus(&mut bus, 1.0);
        bus.command(BusCommand::Fade {
            to: 0.0,
            seconds: 10.0,
        });
        let fading = level(&mut bus);
        assert!(fading < full && fading > full * 0.8, "{fading} / {full}");
        bus.command(BusCommand::SetDelay(0.2));
        bus.command(BusCommand::Fade {
            to: 0.0,
            seconds: 0.0,
        });
        assert_eq!(level(&mut bus), 0.0);
    }

    /// Port de « chaque niveau de jingle se programme sans erreur » (`kill-audio.test.ts`).
    #[test]
    fn chaque_niveau_de_jingle_se_programme_et_s_enrichit() {
        for side in [StingSide::Own, StingSide::Ally, StingSide::Enemy] {
            let mut prev = 0;
            for level in 0..=9 {
                let mut s = Score::new();
                kill_sting(&mut s, 0.0, level, side);
                assert!(s.len() >= prev, "{side:?} {level} : plus de couches");
                prev = s.len();
                let out = dry(s, 0.5);
                assert!(out.iter().all(|x| x.is_finite()));
                assert!(rms(&out) > 1e-3, "{side:?} {level} : audible");
            }
        }
        for side in [StingSide::Ally, StingSide::Enemy] {
            let mut s = Score::new();
            first_blood_sting(&mut s, 0.0, side);
            assert_eq!(s.len(), 8);
            let out = dry(s, 1.5);
            assert!(out.iter().all(|x| x.is_finite()));
            assert!(rms(&out[idx(1.15)..]) > 1e-3);
        }
    }

    #[test]
    fn le_jingle_le_plus_riche_reste_borne_apres_mixage() {
        let mut mixer = Mixer::default();
        let (mut bus, handle) = MusicBus::new(3);
        bus.command(BusCommand::SetMaster(1.0));
        let mut s = Score::new();
        kill_sting(&mut s, 0.05, 9, StingSide::Own);
        first_blood_sting(&mut s, 0.05, StingSide::Ally);
        bus.command(BusCommand::Play(s));
        mixer.add_bus(Channel::Voice, bus);
        let mut out = vec![0.0; 2 * idx(2.5)];
        mixer.render(&mut out);
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(out.iter().any(|x| x.abs() > 0.1));
        assert!(handle.current_time() >= 2.5 - 1e-9);
        mixer.bus_command(
            3,
            BusCommand::Fade {
                to: 0.0,
                seconds: 0.0,
            },
        );
        let mut out = vec![0.0; 512];
        mixer.render(&mut out);
        assert!(
            out.iter().all(|&x| x == 0.0),
            "commande reçue par le bon bus"
        );
    }
}
