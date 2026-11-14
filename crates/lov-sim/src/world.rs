//! Monde de simulation : port de `packages/server/src/world.ts`.
//!
//! Différences structurelles avec le TypeScript (toutes imposées par l'emprunt Rust) :
//! - les contextes et hooks désignent les entités par `EntityId`, pas par référence ;
//! - les écouteurs d'événements et les hooks mutables reçoivent `&mut World` : l'émission et la
//!   distribution passent donc par `World` (`World::emit`, `World::run_before_damage`, …) et non par
//!   l'`EventBus` seul ;
//! - les systèmes ne sont pas stockés dans le monde : l'appelant les passe à `init` et `step`
//!   (l'ordre canonique vit dans un contrat qui n'existe pas encore) ;
//! - `entity.data` est une table typée (`DataTable`), indexée par type Rust ;
//! - `Math.random` devient `World::rng`, un générateur semé.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::fmt;

use lov_shared::constants::TICK_DT;
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{
    AbilityMap, Cooldowns, DamageType, EntityId, EntityKind, EntitySnapshot, GameEvent, PlayerId,
    SelfState, Stats, SummonerId, SummonerMap, Team, TeamOrNeutral, Vec2,
};

// ------------------------------------------------------------------------------------------------
// Générateur pseudo-aléatoire semé
// ------------------------------------------------------------------------------------------------

/// xoshiro256** initialisé par splitmix64. Déterministe : même graine, même suite, sur toute
/// plateforme. Remplace `Math.random()` du serveur TypeScript.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let mut next = || {
            sm = sm.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        // splitmix64 ne produit jamais quatre zéros d'affilée : l'état n'est pas dégénéré.
        Self {
            s: [next(), next(), next(), next()],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Flottant uniforme dans `[0, 1)` (53 bits), équivalent de `Math.random()`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Flottant uniforme dans `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Entier uniforme dans `[0, n)` (0 si `n == 0`), équivalent de `floor(random() * n)`.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((self.next_f64() * n as f64) as usize).min(n - 1)
    }

    /// Vrai avec la probabilité `p` (`Math.random() < p`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Élément uniforme d'une tranche, `None` si elle est vide.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.below(items.len()))
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Table de données typée (`entity.data`)
// ------------------------------------------------------------------------------------------------

/// Remplace `Record<string, unknown>` : une valeur par type Rust. Chaque domaine définit sa propre
/// structure dans son module (`MinionData`, `Buffs`…) ; rien ne les liste ici. Aucune itération n'est
/// exposée : l'ordre de la table de hachage n'influence donc jamais la simulation.
#[derive(Default)]
pub struct DataTable {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl DataTable {
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }

    pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut::<T>()
    }

    /// Valeur du domaine, créée par `init` si absente (`data.x ??= {...}`).
    pub fn get_or_insert_with<T: Any + Send + Sync>(&mut self, init: impl FnOnce() -> T) -> &mut T {
        let slot = self
            .map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(init()));
        match slot.downcast_mut::<T>() {
            Some(v) => v,
            // Invariant : la clé est le `TypeId` de T, donc la valeur stockée est un T.
            None => unreachable!("DataTable : valeur stockée sous le TypeId d'un autre type"),
        }
    }

    /// Remplace la valeur du domaine ; renvoie l'ancienne.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) -> Option<T> {
        let old = self.map.insert(TypeId::of::<T>(), Box::new(value))?;
        old.downcast::<T>().ok().map(|b| *b)
    }

    pub fn remove<T: Any + Send + Sync>(&mut self) -> Option<T> {
        let old = self.map.remove(&TypeId::of::<T>())?;
        old.downcast::<T>().ok().map(|b| *b)
    }

    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl fmt::Debug for DataTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DataTable({} domaines)", self.map.len())
    }
}

// ------------------------------------------------------------------------------------------------
// Entités
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionState {
    pub player_id: PlayerId,
    pub champion_id: String,
    pub name: String,
    pub level: u32,
    pub xp: f64,
    pub skill_points: u32,
    pub ability_levels: AbilityMap<u32>,
    pub cooldowns: Cooldowns,
    pub summoners: SummonerMap<SummonerId>,
    pub gold: f64,
    pub items: Vec<Option<String>>,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub respawn_in: f64,
    pub damage_to_champions: f64,
}

#[derive(Debug)]
pub struct Entity {
    /// Attribué par `World::spawn` (la valeur passée à `spawn` est ignorée).
    pub id: EntityId,
    pub kind: EntityKind,
    pub team: TeamOrNeutral,
    pub pos: Vec2,
    pub facing: f64,
    pub radius: f64,
    pub hp: f64,
    pub mana: f64,
    pub shield: f64,
    /// Stats calculées pour ce tick (recalculées par buffs/leveling/economy).
    pub stats: Stats,
    pub dead: bool,
    pub subtype: Option<String>,
    pub anim: Option<String>,
    /// Ordre de déplacement courant (consommé par movement).
    pub move_target: Option<Vec2>,
    /// Chemin calculé par le pathfinding.
    pub path: Option<Vec<Vec2>>,
    /// Cible d'attaque courante (consommée par combat).
    pub attack_target: Option<EntityId>,
    pub champ: Option<ChampionState>,
    /// Données propres à chaque domaine, une structure par type.
    pub data: DataTable,
}

impl Entity {
    /// Entité avec les valeurs par défaut de `spawn` en TypeScript (`facing: 0, shield: 0, dead: false`).
    pub fn new(
        kind: EntityKind,
        team: TeamOrNeutral,
        pos: Vec2,
        radius: f64,
        hp: f64,
        mana: f64,
        stats: Stats,
    ) -> Self {
        Self {
            id: 0,
            kind,
            team,
            pos,
            facing: 0.0,
            radius,
            hp,
            mana,
            shield: 0.0,
            stats,
            dead: false,
            subtype: None,
            anim: None,
            move_target: None,
            path: None,
            attack_target: None,
            champ: None,
            data: DataTable::default(),
        }
    }
}

/// Commande joueur en attente d'être consommée par les systèmes.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedCommand {
    pub player_id: PlayerId,
    pub msg: ClientMsg,
}

// ------------------------------------------------------------------------------------------------
// Bus d'événements
// ------------------------------------------------------------------------------------------------

macro_rules! event_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// Discriminant de `GameEvent` (le champ `type` du JSON), pour abonner un écouteur.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum EventKind { $($variant),+ }

        impl EventKind {
            /// Nombre de types d'événements.
            pub const COUNT: usize = [$(EventKind::$variant),+].len();

            pub fn of(e: &GameEvent) -> Self {
                match e { $(GameEvent::$variant { .. } => EventKind::$variant),+ }
            }

            /// Valeur du champ `type` dans le protocole.
            pub const fn name(self) -> &'static str {
                match self { $(EventKind::$variant => $name),+ }
            }
        }
    };
}

event_kinds! {
    Damage => "damage",
    Heal => "heal",
    Shield => "shield",
    Death => "death",
    Respawn => "respawn",
    AutoAttack => "autoAttack",
    AbilityCast => "abilityCast",
    SummonerCast => "summonerCast",
    ProjectileSpawn => "projectileSpawn",
    ProjectileHit => "projectileHit",
    Dash => "dash",
    AreaEffect => "areaEffect",
    BuffApplied => "buffApplied",
    BuffRemoved => "buffRemoved",
    LevelUp => "levelUp",
    GoldGain => "goldGain",
    RecallStart => "recallStart",
    RecallEnd => "recallEnd",
    ItemBought => "itemBought",
    ItemSold => "itemSold",
    ItemUsed => "itemUsed",
    StructureDestroyed => "structureDestroyed",
    ObjectiveSpawn => "objectiveSpawn",
    ObjectiveTaken => "objectiveTaken",
    Announcement => "announcement",
    Ping => "ping",
    Chat => "chat",
}

/// Écouteur d'événement : reçoit le monde (pour réagir) et l'événement.
pub type Listener = Box<dyn FnMut(&mut World, &GameEvent) + Send>;

/// Poignée renvoyée par `EventBus::on` ; `EventBus::off` la désabonne (le `() => set.delete(fn)` du TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    kind: EventKind,
    id: u64,
}

struct ListenerSlot {
    id: u64,
    /// `None` pendant que l'écouteur s'exécute (il est sorti de son emplacement pour recevoir `&mut World`).
    f: Option<Listener>,
    removed: bool,
}

/// Bus d'événements de simulation. Chaque événement émis est aussi envoyé aux clients.
///
/// L'émission se fait par `World::emit` (les écouteurs ont besoin de `&mut World`).
pub struct EventBus {
    slots: Vec<Vec<ListenerSlot>>,
    /// Profondeur de distribution en cours par type : tant qu'elle est > 0 on ne retire ni ne déplace
    /// d'emplacement (une boucle de distribution les parcourt par index).
    depth: Vec<u32>,
    next_id: u64,
    /// Événements du tick courant, vidés après chaque snapshot.
    pub pending: Vec<GameEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            slots: (0..EventKind::COUNT).map(|_| Vec::new()).collect(),
            depth: vec![0; EventKind::COUNT],
            next_id: 0,
            pending: Vec::new(),
        }
    }

    /// Abonne `f` aux événements de type `kind`. Les écouteurs s'exécutent dans l'ordre d'enregistrement.
    /// Un écouteur enregistré pendant une distribution est appelé dès cette distribution (comme le
    /// `Set.forEach` du TS).
    pub fn on(
        &mut self,
        kind: EventKind,
        f: impl FnMut(&mut World, &GameEvent) + Send + 'static,
    ) -> Subscription {
        self.next_id += 1;
        let id = self.next_id;
        self.slots[kind as usize].push(ListenerSlot {
            id,
            f: Some(Box::new(f)),
            removed: false,
        });
        Subscription { kind, id }
    }

    /// Désabonne. Sans effet si déjà désabonné. Sûr depuis un écouteur (y compris lui-même).
    pub fn off(&mut self, sub: Subscription) {
        let k = sub.kind as usize;
        if self.depth[k] == 0 {
            self.slots[k].retain(|s| s.id != sub.id);
        } else if let Some(s) = self.slots[k].iter_mut().find(|s| s.id == sub.id) {
            s.removed = true;
            s.f = None;
        }
    }

    /// Ajoute l'événement aux `pending` sans appeler d'écouteur (voir `World::emit` pour le complet).
    pub fn queue(&mut self, e: GameEvent) {
        self.pending.push(e);
    }

    /// Renvoie les événements en attente et vide la file.
    pub fn drain(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.pending)
    }

    fn has_listeners(&self, kind: EventKind) -> bool {
        self.slots[kind as usize].iter().any(|s| !s.removed)
    }
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus")
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

// ------------------------------------------------------------------------------------------------
// Contextes et hooks
// ------------------------------------------------------------------------------------------------

/// Contexte mutable passé aux hooks de dégâts (avant mitigation). Voir combat/damage.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageContext {
    pub source: Option<EntityId>,
    /// Champion (ou entité) crédité : le propriétaire de la source si `source.data.owner` est défini.
    pub credit: Option<EntityId>,
    pub target: EntityId,
    /// Montant brut avant résistances ; un hook peut le modifier (0 = annulé).
    pub amount: f64,
    pub damage_type: DamageType,
    pub ability: Option<String>,
    pub is_auto_attack: bool,
    pub crit: bool,
    /// Pénétration additionnelle (flat) pour ce coup.
    pub flat_pen: f64,
    /// Pénétration additionnelle en pourcentage (0..1) pour ce coup.
    pub percent_pen: f64,
    /// Multiplicateur final appliqué après résistances (ex. réduction de dégâts 0.7).
    pub multiplier: f64,
}

impl DamageContext {
    /// Contexte neutre : pas de source, pas de crit, pas de pénétration, multiplicateur 1.
    pub fn new(target: EntityId, amount: f64, damage_type: DamageType) -> Self {
        Self {
            source: None,
            credit: None,
            target,
            amount,
            damage_type,
            ability: None,
            is_auto_attack: false,
            crit: false,
            flat_pen: 0.0,
            percent_pen: 0.0,
            multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealContext {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackContext {
    pub attacker: EntityId,
    pub target: EntityId,
    /// Dégâts physiques de base (AD) ; modifiable. Le crit multiplie ce montant.
    pub amount: f64,
    pub crit: bool,
    pub crit_multiplier: f64,
    /// Dégâts bonus ajoutés après le crit (on-hit), dans le même événement `damage`.
    pub bonus: f64,
    pub damage_type: DamageType,
}

/// Liste de hooks, exécutés dans l'ordre d'enregistrement.
///
/// Chaque hook est stocké dans un emplacement `Option` : pour appeler un hook qui a besoin de
/// `&mut World` alors qu'il est stocké dans ce même monde, `World::run_*` le **sort** de son emplacement
/// (`Option::take`), l'appelle, puis le remet. Un hook enregistré pendant la distribution est ajouté
/// à la fin et exécuté dans la même passe ; une distribution imbriquée (un hook qui inflige des dégâts,
/// qui redéclenchent les mêmes hooks) fonctionne et saute seulement le hook en cours d'exécution.
/// (Sortir toute la liste d'un bloc avec `mem::take` aurait rendu les distributions imbriquées muettes.)
pub struct HookList<F: ?Sized> {
    slots: Vec<Option<Box<F>>>,
}

impl<F: ?Sized> Default for HookList<F> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<F: ?Sized> HookList<F> {
    pub fn push(&mut self, f: Box<F>) {
        self.slots.push(Some(f));
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Hooks présents (hors hook en cours d'exécution), dans l'ordre.
    pub fn iter(&self) -> impl Iterator<Item = &F> {
        self.slots.iter().flatten().map(|b| &**b)
    }
}

impl<F: ?Sized> fmt::Debug for HookList<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HookList({})", self.slots.len())
    }
}

type Mutating<C> = dyn FnMut(&mut World, &mut C) + Send;
type MutatingDealt<C> = dyn FnMut(&mut World, &mut C, f64) + Send;
type PreventDeathFn = dyn FnMut(&mut World, EntityId, Option<EntityId>) -> bool + Send;
type BountyFn =
    dyn Fn(&World, EntityId, Option<EntityId>, &[EntityId]) -> Option<f64> + Send + Sync;
type RespawnTimeFn = dyn Fn(&World, EntityId) -> Option<f64> + Send + Sync;
type EntityPredicateFn = dyn Fn(&World, EntityId) -> bool + Send + Sync;
type TargetablePredicateFn = dyn Fn(&World, EntityId, EntityId) -> bool + Send + Sync;
type SelfStateFn = dyn Fn(&World, &Entity, &mut SelfState) + Send + Sync;
type EntitySnapshotFn = dyn Fn(&World, &Entity, &mut EntitySnapshot) + Send + Sync;
type ObjectiveTimersFn = dyn Fn(&World, Team) -> Option<BTreeMap<String, f64>> + Send + Sync;
/// Initialisation d'un système.
pub type InitFn = Box<dyn FnMut(&mut World) + Send>;
/// Mise à jour d'un système (`dt` en secondes).
pub type UpdateFn = Box<dyn FnMut(&mut World, f64) + Send>;

/// Points d'extension de la simulation : les systèmes s'y enregistrent dans leur `init` au lieu
/// d'éditer les systèmes d'un autre. Toutes les listes sont exécutées dans l'ordre d'enregistrement.
/// Les hooks « mutants » reçoivent `&mut World` ; les requêtes pures reçoivent `&World` (et sont `Fn`).
#[derive(Debug, Default)]
pub struct WorldHooks {
    /// Avant mitigation : modifier montant / multiplicateur / pénétration (Exhaust, fortification…).
    pub before_damage: HookList<Mutating<DamageContext>>,
    /// Après application (`dealt` = PV + bouclier retirés).
    pub after_damage: HookList<MutatingDealt<DamageContext>>,
    /// Modifie un soin (anti-soin, amplification).
    pub before_heal: HookList<Mutating<HealContext>>,
    /// `true` pour empêcher une mort (l'entité doit alors avoir hp > 0). Tous les hooks s'exécutent ;
    /// la mort est évitée si l'un d'eux répond `true` (voir `World::run_prevent_death`).
    pub prevent_death: HookList<PreventDeathFn>,
    /// Prime d'or d'un kill (victime, tueur, assistants). Le premier hook qui répond `Some` gagne.
    pub bounty: HookList<BountyFn>,
    /// Durée de mort d'un champion (s). Le premier hook qui répond `Some` gagne ; sinon formule par défaut.
    pub respawn_time: HookList<RespawnTimeFn>,
    /// `false` = l'entité ne peut pas se déplacer ce tick (root, stun, canalisation…).
    pub can_move: HookList<EntityPredicateFn>,
    /// `false` = l'entité ne peut pas auto-attaquer (stun, désarmement…).
    pub can_attack: HookList<EntityPredicateFn>,
    /// `false` = la cible (1er id) n'est pas ciblable par l'attaquant (2e id) : invulnérable, stase…
    pub is_targetable: HookList<TargetablePredicateFn>,
    /// Avant l'impact d'une auto-attaque : modifier montant / crit / bonus.
    pub before_attack_hit: HookList<Mutating<AttackContext>>,
    /// Après l'impact d'une auto-attaque (on-hit : buffs, cumuls…).
    pub after_attack_hit: HookList<MutatingDealt<AttackContext>>,
    /// Complète l'état privé envoyé au joueur (xpToNext, canShop, recalling…).
    pub self_state: HookList<SelfStateFn>,
    /// Complète le snapshot public d'une entité (buffs…). Doit rester déterministe (sert au delta).
    pub entity_snapshot: HookList<EntitySnapshotFn>,
    /// Timers d'objectifs connus d'une équipe (fusionnés dans `snapshot.objectiveTimers`).
    pub objective_timers: HookList<ObjectiveTimersFn>,
}

/// Parcourt une `HookList` mutante : chaque hook est sorti de son emplacement le temps de l'appel
/// (voir `HookList`). `$call` reçoit le hook sous le nom `$f`.
macro_rules! run_hooks {
    ($world:expr, $list:ident, |$f:ident| $call:expr) => {{
        let mut i = 0;
        while i < $world.hooks.$list.slots.len() {
            if let Some(mut $f) = $world.hooks.$list.slots[i].take() {
                $call;
                $world.hooks.$list.slots[i] = Some($f);
            }
            i += 1;
        }
    }};
}

// ------------------------------------------------------------------------------------------------
// Systèmes
// ------------------------------------------------------------------------------------------------

/// Un système de simulation. L'ordre d'exécution est celui de la tranche passée à `World::step`.
pub struct System {
    pub name: &'static str,
    /// Appelé une fois au démarrage de la partie (spawn des structures, abonnements au bus…).
    pub init: Option<InitFn>,
    pub update: UpdateFn,
}

impl System {
    pub fn new(name: &'static str, update: impl FnMut(&mut World, f64) + Send + 'static) -> Self {
        Self {
            name,
            init: None,
            update: Box::new(update),
        }
    }

    pub fn with_init(mut self, init: impl FnMut(&mut World) + Send + 'static) -> Self {
        self.init = Some(Box::new(init));
        self
    }
}

impl fmt::Debug for System {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "System({})", self.name)
    }
}

// ------------------------------------------------------------------------------------------------
// Monde
// ------------------------------------------------------------------------------------------------

/// Marque « emplacement vide » dans `index`.
const NO_SLOT: u32 = u32::MAX;

/// Monde de simulation.
///
/// Stockage des entités : `Vec<Option<Entity>>` dans l'ordre d'insertion + table `index` (id - 1 ->
/// position). Les ids sont séquentiels et jamais réutilisés, donc l'ordre d'insertion est l'ordre des
/// ids ; accès par id en O(1), itération déterministe sans hachage. Un retrait laisse un trou, compacté
/// quand les trous dominent (amorti O(1), l'ordre relatif est conservé).
#[derive(Debug)]
pub struct World {
    slots: Vec<Option<Entity>>,
    index: Vec<u32>,
    live: usize,
    pub events: EventBus,
    /// Commandes joueurs reçues depuis le dernier tick ; vidées en fin de tick.
    pub commands: Vec<QueuedCommand>,
    /// Points d'extension (voir `WorldHooks`).
    pub hooks: WorldHooks,
    pub tick: u64,
    /// Secondes de jeu.
    pub time: f64,
    pub winner: Option<Team>,
    /// Aléatoire semé de la partie : seule source d'aléa autorisée dans la simulation.
    pub rng: Rng,
    next_id: EntityId,
    players: HashMap<PlayerId, EntityId>,
}

impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            slots: Vec::new(),
            index: Vec::new(),
            live: 0,
            events: EventBus::new(),
            commands: Vec::new(),
            hooks: WorldHooks::default(),
            tick: 0,
            time: 0.0,
            winner: None,
            rng: Rng::new(seed),
            next_id: 1,
            players: HashMap::new(),
        }
    }

    /// Appelle l'`init` de chaque système, dans l'ordre.
    pub fn init(&mut self, systems: &mut [System]) {
        for s in systems {
            if let Some(init) = s.init.as_mut() {
                init(self);
            }
        }
    }

    /// Ajoute une entité et lui attribue un id (`entity.id` est écrasé). La position est copiée par
    /// construction (`Vec2` est `Copy`).
    pub fn spawn(&mut self, mut e: Entity) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        e.id = id;
        if let Some(c) = &e.champ {
            self.players.insert(c.player_id.clone(), id);
        }
        self.index.push(self.slots.len() as u32);
        self.slots.push(Some(e));
        self.live += 1;
        id
    }

    pub fn remove(&mut self, id: EntityId) {
        let Some(pos) = self.slot_of(id) else { return };
        if let Some(e) = self.slots[pos].take() {
            if let Some(c) = &e.champ
                && self.players.get(&c.player_id) == Some(&id)
            {
                self.players.remove(&c.player_id);
            }
            self.index[id as usize - 1] = NO_SLOT;
            self.live -= 1;
        }
        let holes = self.slots.len() - self.live;
        if holes > 64 && holes > self.live {
            self.compact();
        }
    }

    fn compact(&mut self) {
        let mut w = 0;
        for r in 0..self.slots.len() {
            if let Some(e) = self.slots[r].take() {
                self.index[e.id as usize - 1] = w as u32;
                self.slots[w] = Some(e);
                w += 1;
            }
        }
        self.slots.truncate(w);
    }

    fn slot_of(&self, id: EntityId) -> Option<usize> {
        let p = *self.index.get((id as usize).checked_sub(1)?)?;
        (p != NO_SLOT).then_some(p as usize)
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.slots.get(self.slot_of(id)?)?.as_ref()
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        let p = self.slot_of(id)?;
        self.slots.get_mut(p)?.as_mut()
    }

    /// `get` pour un `Option<EntityId>` (le `get(id | undefined)` du TS).
    pub fn get_opt(&self, id: Option<EntityId>) -> Option<&Entity> {
        self.get(id?)
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.get(id).is_some()
    }

    /// Nombre d'entités dans le monde (non retirées).
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn champion_of(&self, player_id: &str) -> Option<&Entity> {
        self.get(*self.players.get(player_id)?)
    }

    pub fn champion_id_of(&self, player_id: &str) -> Option<EntityId> {
        self.players.get(player_id).copied()
    }

    pub fn champion_of_mut(&mut self, player_id: &str) -> Option<&mut Entity> {
        let id = *self.players.get(player_id)?;
        self.get_mut(id)
    }

    /// Entités dans l'ordre d'insertion.
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.slots.iter().flatten()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.slots.iter_mut().flatten()
    }

    /// `query(kind)` du TS : entités d'un type (ou toutes), dans l'ordre d'insertion.
    pub fn query(&self, kind: Option<EntityKind>) -> impl Iterator<Item = &Entity> {
        self.iter()
            .filter(move |e| kind.is_none_or(|k| e.kind == k))
    }

    /// Remplit `out` (vidé d'abord) avec les ids d'un type, dans l'ordre d'insertion. Pour muter des
    /// entités pendant qu'on les parcourt sans allouer à chaque tick : réutiliser le même tampon.
    pub fn ids_into(&self, kind: Option<EntityKind>, out: &mut Vec<EntityId>) {
        out.clear();
        out.extend(self.query(kind).map(|e| e.id));
    }

    /// Un tick : avance le temps de `dt`, exécute les systèmes dans l'ordre donné, puis vide `commands`.
    pub fn step_dt(&mut self, systems: &mut [System], dt: f64) {
        self.tick += 1;
        self.time += dt;
        for s in systems {
            (s.update)(self, dt);
        }
        self.commands.clear();
    }

    /// `step_dt` avec le pas fixe `TICK_DT`.
    pub fn step(&mut self, systems: &mut [System]) {
        self.step_dt(systems, TICK_DT);
    }

    // ---- événements ----

    /// Émet un événement : il est ajouté à `events.pending`, puis les écouteurs de son type
    /// s'exécutent synchronement dans l'ordre d'enregistrement (comme en TS, un événement émis par un
    /// écouteur arrive donc après celui qui l'a causé).
    ///
    /// Chaque écouteur est sorti de son emplacement le temps de l'appel pour recevoir `&mut World` ;
    /// un écouteur ajouté pendant la distribution est donc conservé (et appelé dans la même passe), et
    /// une émission imbriquée du même type reste distribuée (elle saute seulement l'écouteur en cours).
    pub fn emit(&mut self, e: GameEvent) {
        let kind = EventKind::of(&e);
        if !self.events.has_listeners(kind) {
            self.events.pending.push(e);
            return;
        }
        // Un clone n'est payé que pour les événements écoutés : `pending` garde l'original.
        self.events.pending.push(e.clone());
        let k = kind as usize;
        self.events.depth[k] += 1;
        let mut i = 0;
        // `len()` relu à chaque tour : un écouteur ajouté pendant la distribution est visité.
        while i < self.events.slots[k].len() {
            let taken = {
                let s = &mut self.events.slots[k][i];
                if s.removed { None } else { s.f.take() }
            };
            if let Some(mut f) = taken {
                f(self, &e);
                let s = &mut self.events.slots[k][i];
                if !s.removed {
                    s.f = Some(f);
                }
            }
            i += 1;
        }
        self.events.depth[k] -= 1;
        if self.events.depth[k] == 0 {
            self.events.slots[k].retain(|s| !s.removed);
        }
    }

    // ---- distribution des hooks ----

    pub fn run_before_damage(&mut self, ctx: &mut DamageContext) {
        run_hooks!(self, before_damage, |f| f(self, ctx));
    }

    pub fn run_after_damage(&mut self, ctx: &mut DamageContext, dealt: f64) {
        run_hooks!(self, after_damage, |f| f(self, ctx, dealt));
    }

    pub fn run_before_heal(&mut self, ctx: &mut HealContext) {
        run_hooks!(self, before_heal, |f| f(self, ctx));
    }

    pub fn run_before_attack_hit(&mut self, ctx: &mut AttackContext) {
        run_hooks!(self, before_attack_hit, |f| f(self, ctx));
    }

    pub fn run_after_attack_hit(&mut self, ctx: &mut AttackContext, dealt: f64) {
        run_hooks!(self, after_attack_hit, |f| f(self, ctx, dealt));
    }

    /// Exécute **tous** les hooks `prevent_death` (pas de court-circuit, comme `damage.ts`) ; vrai si
    /// l'un d'eux a répondu vrai.
    pub fn run_prevent_death(&mut self, target: EntityId, killer: Option<EntityId>) -> bool {
        let mut prevented = false;
        run_hooks!(self, prevent_death, |f| {
            if f(self, target, killer) {
                prevented = true;
            }
        });
        prevented
    }

    /// Prime du premier hook qui répond (`None` : formule par défaut à la charge de l'appelant).
    pub fn hook_bounty(
        &self,
        victim: EntityId,
        killer: Option<EntityId>,
        assists: &[EntityId],
    ) -> Option<f64> {
        self.hooks
            .bounty
            .iter()
            .find_map(|f| f(self, victim, killer, assists))
    }

    pub fn hook_respawn_time(&self, victim: EntityId) -> Option<f64> {
        self.hooks.respawn_time.iter().find_map(|f| f(self, victim))
    }

    /// `world.allow(world.hooks.canMove, e)`.
    pub fn can_move(&self, e: EntityId) -> bool {
        self.hooks.can_move.iter().all(|f| f(self, e))
    }

    pub fn can_attack(&self, e: EntityId) -> bool {
        self.hooks.can_attack.iter().all(|f| f(self, e))
    }

    pub fn is_targetable(&self, target: EntityId, attacker: EntityId) -> bool {
        self.hooks
            .is_targetable
            .iter()
            .all(|f| f(self, target, attacker))
    }

    pub fn apply_self_state(&self, e: &Entity, s: &mut SelfState) {
        for f in self.hooks.self_state.iter() {
            f(self, e, s);
        }
    }

    pub fn apply_entity_snapshot(&self, e: &Entity, s: &mut EntitySnapshot) {
        for f in self.hooks.entity_snapshot.iter() {
            f(self, e, s);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Création d'un champion
// ------------------------------------------------------------------------------------------------

/// Paramètres de `create_champion` (`opts` du TS).
#[derive(Debug, Clone)]
pub struct CreateChampion {
    pub player_id: PlayerId,
    pub name: String,
    pub team: Team,
    pub champion_id: String,
    pub summoners: [SummonerId; 2],
    pub pos: Vec2,
}

/// Crée l'entité d'un champion. `base_stats` remplace `getChampion(id).baseStats` : les données de
/// champions ne sont pas encore portées, l'appelant les fournit.
pub fn create_champion(world: &mut World, opts: CreateChampion, base_stats: &Stats) -> EntityId {
    let mut e = Entity::new(
        EntityKind::Champion,
        match opts.team {
            Team::Blue => TeamOrNeutral::Blue,
            Team::Red => TeamOrNeutral::Red,
        },
        opts.pos,
        65.0,
        base_stats.max_hp,
        base_stats.max_mana,
        base_stats.clone(),
    );
    e.subtype = Some(opts.champion_id.clone());
    e.anim = Some("idle".to_string());
    e.champ = Some(ChampionState {
        player_id: opts.player_id,
        champion_id: opts.champion_id,
        name: opts.name,
        level: 1,
        xp: 0.0,
        skill_points: 1,
        ability_levels: AbilityMap {
            q: 0,
            w: 0,
            e: 0,
            r: 0,
        },
        cooldowns: Cooldowns {
            q: 0.0,
            w: 0.0,
            e: 0.0,
            r: 0.0,
            d: 0.0,
            f: 0.0,
        },
        summoners: SummonerMap {
            d: opts.summoners[0],
            f: opts.summoners[1],
        },
        gold: 500.0,
        items: vec![None; 6],
        kills: 0,
        deaths: 0,
        assists: 0,
        cs: 0,
        respawn_in: 0.0,
        damage_to_champions: 0.0,
    });
    world.spawn(e)
}
