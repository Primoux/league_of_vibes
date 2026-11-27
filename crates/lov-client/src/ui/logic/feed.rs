//! Fil des éliminations et annonces centrales : portage de `packages/client/src/ui/logic/feed.ts`.
//!
//! Les annonces de kills (premier sang, multi-éliminations, séries, arrêt de série, équipe anéantie), de
//! structures et d'objectifs sont dérivées des événements. Les événements `announcement` du serveur
//! sont affichés tels quels via `announce_text` (clés libres : minionsSpawn, leviathanSpawn…).

use std::collections::HashMap;

use lov_shared::types::{
    DragonElement, EntityId, EntityKind, GameEvent, MonsterType, ParamValue, StructureKind, Team,
    TeamOrNeutral, other_team,
};

use super::format::{dragon_element, dragon_label, monster_label, soul_label};

/// Ce que l'interface sait d'une unité pour l'afficher.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitRef {
    pub id: EntityId,
    /// `None` : inconnue.
    pub kind: Option<EntityKind>,
    pub team: TeamOrNeutral,
    /// Champion, type de sbire ou de monstre, palier de tour…
    pub subtype: Option<String>,
    pub name: Option<String>,
}

impl UnitRef {
    pub fn unknown(id: EntityId) -> Self {
        Self {
            id,
            kind: None,
            team: TeamOrNeutral::Neutral,
            subtype: None,
            name: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedKind {
    Kill,
    Execute,
    Structure,
    Objective,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedEntry {
    pub key: u32,
    /// Secondes de jeu.
    pub time: f64,
    pub kind: FeedKind,
    pub killer: Option<UnitRef>,
    pub victim: Option<UnitRef>,
    pub assists: Vec<UnitRef>,
    /// Équipe bénéficiaire (couleur).
    pub team: TeamOrNeutral,
    pub label: Option<String>,
    pub element: Option<DragonElement>,
    pub monster: Option<MonsterType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnounceTone {
    Ally,
    Enemy,
    Neutral,
    Gold,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Announcement {
    pub key: u32,
    pub title: String,
    pub subtitle: Option<String>,
    pub tone: AnnounceTone,
    pub killer: Option<UnitRef>,
    pub victim: Option<UnitRef>,
    /// Une annonce plus prioritaire passe devant la file.
    pub priority: u32,
    pub element: Option<DragonElement>,
}

impl Announcement {
    fn new(key: u32, title: impl Into<String>, tone: AnnounceTone, priority: u32) -> Self {
        Self {
            key,
            title: title.into(),
            subtitle: None,
            tone,
            killer: None,
            victim: None,
            priority,
            element: None,
        }
    }
}

/// Texte des annonces du serveur, par clé.
pub fn announce_text(key: &str) -> Option<&'static str> {
    Some(match key {
        "welcome" => "Bienvenue dans la Faille des Vibes",
        "minionsSoon" => "Les sbires apparaîtront dans 30 secondes",
        "minionsSpawn" => "Les sbires sont apparus !",
        "dragonSpawn" => "Un drake est apparu",
        "elderSpawn" => "Le Drake Ancien s’est réveillé",
        "grubsSpawn" => "Les Larves ont éclos",
        "heraldSpawn" => "Le Colosse du Rift est apparu",
        "leviathanSpawn" => "Le Léviathan a émergé des profondeurs",
        "inhibitorRespawn" | "inhibitor_respawn" => "Un inhibiteur s’est reconstruit",
        "dragonSoul" => "Âme du drake obtenue",
        "herald_summoned" => "Le Colosse du Rift charge !",
        _ => return None,
    })
}

/// Clés ignorées : déjà dérivées des événements, ou gérées par l'écran de fin.
const SKIPPED_KEYS: [&str; 7] = [
    "objective_taken",
    "victory",
    "defeat",
    "first_blood",
    "kill",
    "multikill",
    "ace",
];

const MULTI: [&str; 6] = [
    "",
    "",
    "Double élimination !",
    "Triple élimination !",
    "Quadra élimination !",
    "Penta élimination !",
];

fn spree(n: u32) -> &'static str {
    match n.min(8) {
        3 => "Série meurtrière",
        4 => "Massacre",
        5 => "Déchaîné",
        6 => "Inarrêtable",
        7 => "Divin",
        _ => "Légendaire",
    }
}

/// Secondes entre deux kills d'une même multi-élimination.
const MULTI_WINDOW: f64 = 10.0;
const TEAM_SIZE: usize = 4;

fn tone_of(team: Option<Team>, mine: Team) -> AnnounceTone {
    match team {
        Some(t) if t == mine => AnnounceTone::Ally,
        Some(_) => AnnounceTone::Enemy,
        None => AnnounceTone::Neutral,
    }
}

fn to_neutral(t: Team) -> TeamOrNeutral {
    match t {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

fn who(killer: &UnitRef, victim: &UnitRef) -> String {
    format!(
        "{} a abattu {}",
        killer.name.as_deref().unwrap_or("Un champion"),
        victim.name.as_deref().unwrap_or("un champion")
    )
}

/// Nom d'un objectif (« Drake infernal », « Colosse du Rift »…).
pub fn objective_name(monster: MonsterType, element: Option<DragonElement>) -> String {
    match (monster, element) {
        (MonsterType::Dragon, Some(e)) => dragon_label(e).into(),
        _ => monster_label(monster).map_or_else(|| format!("{monster:?}"), Into::into),
    }
}

/// Garde les `max` entrées les plus récentes, et pas celles plus vieilles que `ttl` secondes.
pub fn trim_feed<T: Clone>(
    entries: &[T],
    time: impl Fn(&T) -> f64,
    now: f64,
    max: usize,
    ttl: f64,
) -> Vec<T> {
    let fresh: Vec<T> = entries
        .iter()
        .filter(|e| now - time(e) <= ttl)
        .cloned()
        .collect();
    fresh[fresh.len().saturating_sub(max)..].to_vec()
}

/// Suivi des éliminations : séries, multi-éliminations, premier sang, équipe anéantie.
pub struct FeedTracker {
    my_team: Team,
    next_key: u32,
    first_blood: bool,
    /// Kills sans mourir, par champion.
    streak: HashMap<EntityId, u32>,
    multi: HashMap<EntityId, (u32, f64)>,
    /// Champions morts connus (« Équipe anéantie »).
    dead: HashMap<EntityId, Team>,
}

/// Résultat d'un lot d'événements.
#[derive(Debug, Default)]
pub struct FeedOut {
    pub feed: Vec<FeedEntry>,
    pub announce: Vec<Announcement>,
}

impl FeedTracker {
    pub fn new(my_team: Team) -> Self {
        Self {
            my_team,
            next_key: 1,
            first_blood: false,
            streak: HashMap::new(),
            multi: HashMap::new(),
            dead: HashMap::new(),
        }
    }

    pub fn set_team(&mut self, t: Team) {
        self.my_team = t;
    }

    pub fn mark_alive(&mut self, id: EntityId) {
        self.dead.remove(&id);
    }

    fn key(&mut self) -> u32 {
        let k = self.next_key;
        self.next_key += 1;
        k
    }

    pub fn process(
        &mut self,
        events: &[GameEvent],
        time: f64,
        lookup: impl Fn(EntityId) -> UnitRef,
    ) -> FeedOut {
        let mut out = FeedOut::default();
        for e in events {
            match e {
                GameEvent::Death {
                    target,
                    killer,
                    assists,
                    ..
                } => {
                    let victim = lookup(*target);
                    if victim.kind != Some(EntityKind::Champion) {
                        continue;
                    }
                    let killer = killer.map(&lookup);
                    let assists = assists.iter().flatten().map(|a| lookup(*a)).collect();
                    self.on_death(victim, killer, assists, time, &mut out);
                }
                GameEvent::StructureDestroyed {
                    target,
                    kind,
                    team,
                    plate,
                } => {
                    if *plate == Some(true) {
                        continue;
                    }
                    let mut victim = lookup(*target);
                    victim.team = to_neutral(*team);
                    victim.kind = Some(match kind {
                        StructureKind::Tower => EntityKind::Tower,
                        StructureKind::Inhibitor => EntityKind::Inhibitor,
                        StructureKind::Nexus => EntityKind::Nexus,
                    });
                    let ally_lost = *team == self.my_team;
                    let label = match kind {
                        StructureKind::Tower => "Tour",
                        StructureKind::Inhibitor => "Inhibiteur",
                        StructureKind::Nexus => "Nexus",
                    };
                    let key = self.key();
                    let winner = if ally_lost {
                        other_team(self.my_team)
                    } else {
                        self.my_team
                    };
                    out.feed.push(FeedEntry {
                        key,
                        time,
                        kind: FeedKind::Structure,
                        killer: None,
                        victim: Some(victim),
                        assists: vec![],
                        team: to_neutral(winner),
                        label: Some(label.into()),
                        element: None,
                        monster: None,
                    });
                    if *kind == StructureKind::Nexus {
                        continue;
                    }
                    let tower = *kind == StructureKind::Tower;
                    let title = match (tower, ally_lost) {
                        (true, true) => "Votre tour a été détruite",
                        (true, false) => "Tour ennemie détruite !",
                        (false, true) => "Votre inhibiteur a été détruit",
                        (false, false) => "Inhibiteur ennemi détruit !",
                    };
                    let tone = if ally_lost {
                        AnnounceTone::Enemy
                    } else {
                        AnnounceTone::Ally
                    };
                    let key = self.key();
                    let mut a = Announcement::new(key, title, tone, if tower { 2 } else { 3 });
                    if !tower {
                        a.subtitle = Some(
                            if ally_lost {
                                "Des super-sbires vont déferler"
                            } else {
                                "Vos super-sbires déferlent"
                            }
                            .into(),
                        );
                    }
                    out.announce.push(a);
                }
                GameEvent::ObjectiveTaken {
                    monster,
                    team,
                    killer,
                    element,
                } => {
                    let ally = *team == self.my_team;
                    let name = objective_name(*monster, *element);
                    let killer = killer.map(&lookup);
                    let key = self.key();
                    out.feed.push(FeedEntry {
                        key,
                        time,
                        kind: FeedKind::Objective,
                        killer: killer.clone(),
                        victim: None,
                        assists: vec![],
                        team: to_neutral(*team),
                        label: Some(name.clone()),
                        element: *element,
                        monster: Some(*monster),
                    });
                    // pas d'annonce par larve, seulement le fil
                    if *monster == MonsterType::Grub {
                        continue;
                    }
                    let verb = if *monster == MonsterType::Herald {
                        "invoqué"
                    } else {
                        "vaincu"
                    };
                    let title = if ally {
                        format!("Votre équipe a {verb} : {name}")
                    } else {
                        format!("L’équipe ennemie a {verb} : {name}")
                    };
                    let big = matches!(monster, MonsterType::Leviathan | MonsterType::ElderDragon);
                    let key = self.key();
                    let mut a = Announcement::new(
                        key,
                        title,
                        if ally {
                            AnnounceTone::Ally
                        } else {
                            AnnounceTone::Enemy
                        },
                        if big { 4 } else { 3 },
                    );
                    a.element = *element;
                    a.killer = killer;
                    out.announce.push(a);
                }
                GameEvent::Announcement { key, team, params } => {
                    if SKIPPED_KEYS.contains(&key.as_str()) {
                        continue;
                    }
                    let param = |name: &str| match params.as_ref().and_then(|p| p.get(name)) {
                        Some(ParamValue::Str(s)) => Some(s.clone()),
                        _ => None,
                    };
                    if key == "dragon_soul" || key == "dragonSoul" {
                        let el = param("element").and_then(|e| dragon_element(&e));
                        let ally = *team == Some(self.my_team);
                        let soul = el.map_or("Âme du drake".to_string(), |e| {
                            format!("Âme {}", soul_label(e))
                        });
                        let title = if ally {
                            format!("Votre équipe obtient l’{soul} !")
                        } else {
                            format!("L’équipe ennemie obtient l’{soul}")
                        };
                        let k = self.key();
                        let mut a = Announcement::new(
                            k,
                            title,
                            if ally {
                                AnnounceTone::Ally
                            } else {
                                AnnounceTone::Enemy
                            },
                            6,
                        );
                        a.subtitle = Some("Le Drake Ancien se réveillera bientôt".into());
                        a.element = el;
                        out.announce.push(a);
                        continue;
                    }
                    let Some(title) = announce_text(key)
                        .map(String::from)
                        .or_else(|| param("text"))
                    else {
                        continue;
                    };
                    let k = self.key();
                    out.announce
                        .push(Announcement::new(k, title, tone_of(*team, self.my_team), 1));
                }
                _ => {}
            }
        }
        out
    }

    fn on_death(
        &mut self,
        victim: UnitRef,
        killer: Option<UnitRef>,
        assists: Vec<UnitRef>,
        time: f64,
        out: &mut FeedOut,
    ) {
        let victim_team = match victim.team {
            TeamOrNeutral::Blue => Team::Blue,
            TeamOrNeutral::Red => Team::Red,
            TeamOrNeutral::Neutral => self.my_team,
        };
        let killer_champ = killer
            .as_ref()
            .filter(|k| k.kind == Some(EntityKind::Champion));
        let ally_killed = victim_team == self.my_team;
        let victim_streak = self.streak.insert(victim.id, 0).unwrap_or(0);
        self.multi.remove(&victim.id);
        self.dead.insert(victim.id, victim_team);

        let key = self.key();
        out.feed.push(FeedEntry {
            key,
            time,
            kind: if killer_champ.is_some() {
                FeedKind::Kill
            } else {
                FeedKind::Execute
            },
            killer: killer.clone(),
            victim: Some(victim.clone()),
            assists,
            team: to_neutral(other_team(victim_team)),
            label: None,
            element: None,
            monster: None,
        });

        let tone = if ally_killed {
            AnnounceTone::Enemy
        } else {
            AnnounceTone::Ally
        };
        let announce = |key: u32, title: String, subtitle: Option<String>, tone, priority| {
            let mut a = Announcement::new(key, title, tone, priority);
            a.subtitle = subtitle;
            a.killer.clone_from(&killer);
            a.victim = Some(victim.clone());
            a
        };

        let Some(k) = killer_champ.cloned() else {
            let key = self.key();
            let title = if ally_killed {
                "Un allié a été exécuté"
            } else {
                "Un ennemi a été exécuté"
            };
            out.announce
                .push(announce(key, title.into(), None, tone, 1));
            self.check_ace(victim_team, out);
            return;
        };

        let streak = self.streak.get(&k.id).copied().unwrap_or(0) + 1;
        self.streak.insert(k.id, streak);
        let multi = match self.multi.get(&k.id) {
            Some((count, last)) if time - last <= MULTI_WINDOW => count + 1,
            _ => 1,
        };
        self.multi.insert(k.id, (multi, time));

        let key = self.key();
        let line = Some(who(&k, &victim));
        if !self.first_blood {
            self.first_blood = true;
            out.announce.push(announce(
                key,
                "Premier sang !".into(),
                line,
                AnnounceTone::Gold,
                5,
            ));
        } else if multi >= 2 {
            let title = MULTI[multi.min(5) as usize];
            out.announce
                .push(announce(key, title.into(), line, tone, 4 + multi));
        } else if victim_streak >= 3 {
            let sub = format!(
                "{} ne fera plus de ravages",
                victim.name.as_deref().unwrap_or("Un champion")
            );
            out.announce
                .push(announce(key, "Arrêt de série !".into(), Some(sub), tone, 4));
        } else {
            let title = if ally_killed {
                "Un allié a été tué"
            } else {
                "Vous avez éliminé un ennemi"
            };
            out.announce
                .push(announce(key, title.into(), line, tone, 2));
        }
        if streak >= 3 && multi < 2 {
            let enemy = k.team != to_neutral(self.my_team);
            let title = if enemy && streak >= 8 {
                "Un ennemi est légendaire !".to_string()
            } else {
                format!(
                    "{} : {} !",
                    k.name.as_deref().unwrap_or("Un champion"),
                    spree(streak)
                )
            };
            let key = self.key();
            let tone = if enemy {
                AnnounceTone::Enemy
            } else {
                AnnounceTone::Ally
            };
            out.announce.push(announce(key, title, None, tone, 3));
        }
        self.check_ace(victim_team, out);
    }

    fn check_ace(&mut self, team: Team, out: &mut FeedOut) {
        let dead = self.dead.values().filter(|t| **t == team).count();
        if dead >= TEAM_SIZE {
            let key = self.key();
            let tone = if team == self.my_team {
                AnnounceTone::Enemy
            } else {
                AnnounceTone::Ally
            };
            out.announce
                .push(Announcement::new(key, "Équipe anéantie !", tone, 9));
        }
    }
}

/// Portage de la partie « kill feed et annonces » de `packages/client/test/ui-logic.test.ts`.
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn unit(id: EntityId, kind: EntityKind, team: TeamOrNeutral, name: Option<&str>) -> UnitRef {
        UnitRef {
            id,
            kind: Some(kind),
            team,
            subtype: None,
            name: name.map(Into::into),
        }
    }

    fn lookup(id: EntityId) -> UnitRef {
        use EntityKind::{Champion, Minion, Tower};
        use TeamOrNeutral::{Blue, Red};
        match id {
            1 => unit(1, Champion, Blue, Some("Ilyra")),
            2 => unit(2, Champion, Blue, Some("Sable")),
            10 => unit(10, Champion, Red, Some("Nyx")),
            11 => unit(11, Champion, Red, Some("Grondak")),
            12 => unit(12, Champion, Red, Some("Corvin")),
            13 => unit(13, Champion, Red, Some("Zephyr")),
            50 => unit(50, Tower, Red, None),
            60 => unit(60, Minion, Red, None),
            _ => UnitRef::unknown(id),
        }
    }

    fn death(target: EntityId, killer: EntityId) -> GameEvent {
        GameEvent::Death {
            target,
            killer: Some(killer),
            assists: None,
            bounty: None,
        }
    }

    fn titles(out: &FeedOut) -> Vec<&str> {
        out.announce.iter().map(|a| a.title.as_str()).collect()
    }

    #[test]
    fn premier_sang_puis_double_elimination() {
        let mut t = FeedTracker::new(Team::Blue);
        let r1 = t.process(&[death(10, 1)], 100.0, lookup);
        assert_eq!(r1.feed.len(), 1);
        assert_eq!(r1.announce[0].title, "Premier sang !");
        let r2 = t.process(&[death(11, 1)], 105.0, lookup);
        assert_eq!(r2.announce[0].title, "Double élimination !");
        assert_eq!(r2.announce[0].tone, AnnounceTone::Ally);
        // hors fenêtre
        let r3 = t.process(&[death(12, 1)], 130.0, lookup);
        assert!(titles(&r3).contains(&"Ilyra : Série meurtrière !"));
    }

    #[test]
    fn arret_de_serie_execution_et_equipe_aneantie() {
        let mut t = FeedTracker::new(Team::Blue);
        t.process(&[death(1, 10)], 10.0, lookup);
        t.process(&[death(2, 10)], 40.0, lookup);
        // Nyx : 3 kills
        t.process(&[death(1, 10)], 80.0, lookup);
        let shut = t.process(&[death(10, 2)], 120.0, lookup);
        assert_eq!(shut.announce[0].title, "Arrêt de série !");
        let exec = t.process(&[death(11, 60)], 130.0, lookup);
        assert_eq!(exec.feed[0].kind, FeedKind::Execute);
        assert_eq!(exec.announce[0].title, "Un ennemi a été exécuté");
        let ace = t.process(&[death(12, 1), death(13, 1)], 131.0, lookup);
        assert!(titles(&ace).contains(&"Équipe anéantie !"));
    }

    #[test]
    fn ignore_les_morts_de_sbires_annonce_tours_et_objectifs() {
        let mut t = FeedTracker::new(Team::Blue);
        assert!(t.process(&[death(60, 1)], 10.0, lookup).feed.is_empty());
        let tower = |plate| GameEvent::StructureDestroyed {
            target: 50,
            kind: StructureKind::Tower,
            team: Team::Red,
            plate,
        };
        let s = t.process(&[tower(None)], 20.0, lookup);
        assert_eq!(s.announce[0].title, "Tour ennemie détruite !");
        let o = t.process(
            &[GameEvent::ObjectiveTaken {
                monster: MonsterType::Dragon,
                team: Team::Red,
                killer: None,
                element: Some(DragonElement::Infernal),
            }],
            30.0,
            lookup,
        );
        assert_eq!(
            o.announce[0].title,
            "L’équipe ennemie a vaincu : Drake infernal"
        );
        assert_eq!(o.announce[0].tone, AnnounceTone::Enemy);
        assert!(
            t.process(&[tower(Some(true))], 40.0, lookup)
                .feed
                .is_empty()
        );
    }

    #[test]
    fn annonces_du_serveur_ame_colosse_doublons_ignores() {
        let mut t = FeedTracker::new(Team::Blue);
        let ann = |key: &str, team, params: Option<BTreeMap<String, ParamValue>>| {
            GameEvent::Announcement {
                key: key.into(),
                team: Some(team),
                params,
            }
        };
        let el = BTreeMap::from([("element".to_string(), ParamValue::Str("infernal".into()))]);
        let soul = t.process(&[ann("dragon_soul", Team::Blue, Some(el))], 10.0, lookup);
        assert_eq!(
            soul.announce[0].title,
            "Votre équipe obtient l’Âme infernale !"
        );
        assert_eq!(soul.announce[0].element, Some(DragonElement::Infernal));
        let herald = t.process(&[ann("herald_summoned", Team::Red, None)], 11.0, lookup);
        assert_eq!(herald.announce[0].tone, AnnounceTone::Enemy);
        let monster = BTreeMap::from([("monster".to_string(), ParamValue::Str("dragon".into()))]);
        let dup = t.process(
            &[ann("objective_taken", Team::Red, Some(monster))],
            12.0,
            lookup,
        );
        assert!(dup.announce.is_empty());
        assert!(
            t.process(&[ann("victory", Team::Red, None)], 13.0, lookup)
                .announce
                .is_empty()
        );
    }

    #[test]
    fn elague_le_feed() {
        let e = [1.0, 2.0, 3.0, 20.0, 21.0];
        assert_eq!(trim_feed(&e, |t| *t, 22.0, 6, 10.0), vec![20.0, 21.0]);
        assert_eq!(trim_feed(&e, |t| *t, 22.0, 1, 100.0), vec![21.0]);
    }
}
