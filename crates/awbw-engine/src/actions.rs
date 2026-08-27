//! The action set, legality checking, and application.
//!
//! One action is one order: move a unit, move-and-shoot, capture, build, and so
//! on. A turn is a variable-length sequence of these ending in `EndTurn`, which
//! is what makes the game tractable as an RL environment — each env step picks
//! one order from a masked, enumerable set rather than composing a whole turn.

use crate::combat::{self, DamageSpread};
use crate::map::Pos;
use crate::movement::Reach;
use crate::rng::Rng;
use crate::state::{
    can_carry, cargo_capacity, GameState, PlayerId, UnitId, CAPTURE_FULL, MAX_CARGO,
};
use crate::types::{TerrainKind, UnitType};
use crate::vision::Vision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Move to `dest` (possibly staying put) and end the unit's turn.
    Move { unit: UnitId, dest: Pos },
    /// Move to `dest`, then attack the unit at `target`.
    Attack {
        unit: UnitId,
        dest: Pos,
        target: Pos,
    },
    /// Move to `dest` and capture the property there.
    Capture { unit: UnitId, dest: Pos },
    /// Produce a unit at an owned, empty production property.
    Build { at: Pos, typ: UnitType },
    /// Move onto an allied transport at `dest` and board it.
    Load { unit: UnitId, dest: Pos },
    /// Drop `cargo` onto an adjacent tile.
    ///
    /// AWBW departs from the cartridge here: a transport "may unload at any
    /// point in their turn, even if they have already moved, and doing so does
    /// not end the unit's turn either, effectively making unloading a free
    /// action". So this carries no destination — the transport unloads from
    /// wherever it stands, and moving is a separate order.
    Unload {
        transport: UnitId,
        cargo: UnitId,
        drop_at: Pos,
    },
    /// Merge into a damaged friendly unit of the same type at `dest`.
    Join { unit: UnitId, dest: Pos },
    /// Move an APC to `dest` and resupply every adjacent ally.
    Supply { unit: UnitId, dest: Pos },
    EndTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    NoSuchUnit,
    NotYourUnit,
    AlreadyMoved,
    Unreachable,
    Occupied,
    NoTarget,
    OutOfRange,
    CannotAttackThat,
    NotCapturable,
    NotAProductionSite,
    CannotProduceThat,
    NotEnoughFunds,
    UnitLimitReached,
    NoRoom,
    CannotCarryThat,
    CannotJoinThat,
    NotATransport,
    NotASupplier,
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ActionError {}

/// What happened when an action resolved, for logging and reward shaping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionReport {
    pub damage_dealt: u8,
    pub damage_taken: u8,
    pub defender_destroyed: bool,
    pub attacker_destroyed: bool,
    pub property_captured: bool,
    pub unit_built: Option<UnitId>,
    /// The move stopped early because it ran into a unit the mover could not
    /// see. Always false without fog.
    pub ambushed: bool,
}

/// The mutable context an action needs: the state plus the luck source.
pub struct Engine {
    pub state: GameState,
    pub rng: Rng,
    reach: Reach,
    /// What the player to move can see. Rebuilt whenever the board changes;
    /// without fog it is simply everything.
    vision: Vision,
    /// Scratch buffers reused by `legal_actions_into`.
    movable: Vec<UnitId>,
    sites: Vec<Pos>,
}

impl Engine {
    pub fn new(state: GameState, seed: u64) -> Self {
        let mut engine = Engine {
            state,
            rng: Rng::new(seed),
            reach: Reach::new(),
            vision: Vision::new(),
            movable: Vec::new(),
            sites: Vec::new(),
        };
        engine.refresh_vision();
        engine
    }

    /// Recomputes the moving player's view. Cheap without fog, and needed after
    /// anything that moves, builds or destroys a unit.
    pub fn refresh_vision(&mut self) {
        let player = self.state.current;
        self.vision.compute(&self.state, player);
    }

    /// What the player to move can see.
    pub fn vision(&self) -> &Vision {
        &self.vision
    }

    /// Reachability for a unit, from behind its owner's fog.
    fn compute_reach(&mut self, unit_id: UnitId) {
        if self.state.settings.fog {
            let (reach, state, vision) = (&mut self.reach, &self.state, &self.vision);
            reach.compute_with_vision(state, unit_id, vision);
        } else {
            self.reach.compute(&self.state, unit_id);
        }
    }

    /// What a unit costs this player, after their CO's price multiplier
    /// (Colin builds at 80%, Kanbei at 120%).
    pub fn unit_cost(&self, player: PlayerId, typ: UnitType) -> u32 {
        let co = self.state.co_of(player);
        typ.stats().cost * co.price_multiplier_pct / 100
    }

    /// A unit's firing range for its owner's CO.
    fn range_of(&self, player: PlayerId, typ: UnitType) -> (u32, u32) {
        combat::effective_range(self.state.co_of(player), typ)
    }

    // --- legality ----------------------------------------------------------

    /// Checks an action without applying it.
    pub fn check(&mut self, action: Action) -> Result<(), ActionError> {
        match action {
            Action::EndTurn => Ok(()),
            Action::Build { at, typ } => self.check_build(at, typ),
            Action::Move { unit, dest } => {
                self.check_move(unit, dest)?;
                if self.state.unit_id_at(dest).is_some_and(|o| o != unit) {
                    return Err(ActionError::Occupied);
                }
                Ok(())
            }
            Action::Attack { unit, dest, target } => self.check_attack(unit, dest, target),
            Action::Capture { unit, dest } => self.check_capture(unit, dest),
            Action::Load { unit, dest } => self.check_load(unit, dest),
            Action::Unload {
                transport,
                cargo,
                drop_at,
            } => self.check_unload(transport, cargo, drop_at),
            Action::Join { unit, dest } => self.check_join(unit, dest),
            Action::Supply { unit, dest } => self.check_supply(unit, dest),
        }
    }

    /// Shared preamble: the unit is ours, hasn't acted, and `dest` is in range.
    fn check_move(&mut self, unit_id: UnitId, dest: Pos) -> Result<(), ActionError> {
        let unit = self.state.unit(unit_id).ok_or(ActionError::NoSuchUnit)?;
        if unit.owner != self.state.current {
            return Err(ActionError::NotYourUnit);
        }
        if unit.moved || unit.carried_by.is_some() {
            return Err(ActionError::AlreadyMoved);
        }
        self.compute_reach(unit_id);
        if !self.reach.can_reach(&self.state, dest) {
            return Err(ActionError::Unreachable);
        }
        Ok(())
    }

    fn check_attack(&mut self, unit_id: UnitId, dest: Pos, target: Pos) -> Result<(), ActionError> {
        self.check_move(unit_id, dest)?;
        if self.state.unit_id_at(dest).is_some_and(|o| o != unit_id) {
            return Err(ActionError::Occupied);
        }
        let unit = *self.state.unit(unit_id).unwrap();

        // Indirects must fire from where they started.
        if unit.typ.is_indirect() && dest != unit.pos {
            return Err(ActionError::OutOfRange);
        }
        let (min_range, max_range) = self.range_of(unit.owner, unit.typ);
        let distance = dest.distance(target);
        if distance < min_range || distance > max_range {
            return Err(ActionError::OutOfRange);
        }

        let defender = self.state.unit_at(target).ok_or(ActionError::NoTarget)?;
        if !self.state.are_enemies(unit.owner, defender.owner) {
            return Err(ActionError::NoTarget);
        }
        // You cannot shoot what you have not found.
        if !self.vision.sees_unit(&self.state, defender) {
            return Err(ActionError::NoTarget);
        }
        if defender.hidden && !combat::can_target_hidden(unit.typ, defender.typ) {
            return Err(ActionError::CannotAttackThat);
        }
        if combat::base_percentage(unit.typ, defender.typ, unit.ammo).is_none() {
            return Err(ActionError::CannotAttackThat);
        }
        Ok(())
    }

    fn check_capture(&mut self, unit_id: UnitId, dest: Pos) -> Result<(), ActionError> {
        self.check_move(unit_id, dest)?;
        if self.state.unit_id_at(dest).is_some_and(|o| o != unit_id) {
            return Err(ActionError::Occupied);
        }
        let unit = *self.state.unit(unit_id).unwrap();
        if !matches!(unit.typ, UnitType::Infantry | UnitType::Mech) {
            return Err(ActionError::NotCapturable);
        }
        let building = self.state.building_at(dest).ok_or(ActionError::NotCapturable)?;
        if building.owner == Some(unit.owner) {
            return Err(ActionError::NotCapturable);
        }
        Ok(())
    }

    fn check_build(&self, at: Pos, typ: UnitType) -> Result<(), ActionError> {
        let player = self.state.current;
        let building = self
            .state
            .building_at(at)
            .ok_or(ActionError::NotAProductionSite)?;
        if building.owner != Some(player) {
            return Err(ActionError::NotAProductionSite);
        }
        if !building.can_produce(typ) {
            return Err(ActionError::CannotProduceThat);
        }
        if self.state.unit_id_at(at).is_some() {
            return Err(ActionError::Occupied);
        }
        if self.state.player(player).funds < self.unit_cost(player, typ) {
            return Err(ActionError::NotEnoughFunds);
        }
        if self.state.unit_count(player) >= self.state.settings.unit_limit {
            return Err(ActionError::UnitLimitReached);
        }
        Ok(())
    }

    fn check_load(&mut self, unit_id: UnitId, dest: Pos) -> Result<(), ActionError> {
        self.check_move(unit_id, dest)?;
        let unit = *self.state.unit(unit_id).unwrap();
        let transport_id = self.state.unit_id_at(dest).ok_or(ActionError::NotATransport)?;
        let transport = *self.state.unit(transport_id).unwrap();
        if !self.state.are_allied(unit.owner, transport.owner) {
            return Err(ActionError::NotATransport);
        }
        if !can_carry(transport.typ, unit.typ) {
            return Err(ActionError::CannotCarryThat);
        }
        if transport.cargo_len() >= cargo_capacity(transport.typ) {
            return Err(ActionError::NoRoom);
        }
        Ok(())
    }

    fn check_unload(
        &mut self,
        transport_id: UnitId,
        cargo_id: UnitId,
        drop_at: Pos,
    ) -> Result<(), ActionError> {
        let transport = *self.state.unit(transport_id).ok_or(ActionError::NoSuchUnit)?;
        if transport.owner != self.state.current {
            return Err(ActionError::NotYourUnit);
        }
        // Deliberately no `moved` check: unloading is free in AWBW.
        if transport.carried_by.is_some() {
            return Err(ActionError::AlreadyMoved);
        }
        if !transport.cargo.contains(&cargo_id) {
            return Err(ActionError::NotATransport);
        }
        if transport.pos.distance(drop_at) != 1 {
            return Err(ActionError::OutOfRange);
        }
        if self.state.unit_id_at(drop_at).is_some() {
            return Err(ActionError::Occupied);
        }
        let cargo = self.state.unit(cargo_id).ok_or(ActionError::NoSuchUnit)?;
        // The passenger must be able to stand where it is dropped.
        self.state
            .map
            .terrain_at(drop_at)
            .move_cost(self.state.weather, cargo.move_type())
            .ok_or(ActionError::Unreachable)?;
        Ok(())
    }

    fn check_join(&mut self, unit_id: UnitId, dest: Pos) -> Result<(), ActionError> {
        self.check_move(unit_id, dest)?;
        let unit = *self.state.unit(unit_id).unwrap();
        let other_id = self.state.unit_id_at(dest).ok_or(ActionError::CannotJoinThat)?;
        if other_id == unit_id {
            return Err(ActionError::CannotJoinThat);
        }
        let other = *self.state.unit(other_id).unwrap();
        if other.owner != unit.owner || other.typ != unit.typ {
            return Err(ActionError::CannotJoinThat);
        }
        if other.hp100 >= 100 {
            return Err(ActionError::CannotJoinThat);
        }
        // Neither side may be carrying anything: AWBW has nowhere to put it.
        if unit.cargo_len() > 0 || other.cargo_len() > 0 {
            return Err(ActionError::CannotJoinThat);
        }
        Ok(())
    }

    fn check_supply(&mut self, unit_id: UnitId, dest: Pos) -> Result<(), ActionError> {
        self.check_move(unit_id, dest)?;
        if self.state.unit_id_at(dest).is_some_and(|o| o != unit_id) {
            return Err(ActionError::Occupied);
        }
        let unit = *self.state.unit(unit_id).unwrap();
        if unit.typ != UnitType::Apc {
            return Err(ActionError::NotASupplier);
        }
        Ok(())
    }

    // --- application -------------------------------------------------------

    /// Validates and applies an action, advancing the game.
    pub fn apply(&mut self, action: Action) -> Result<ActionReport, ActionError> {
        self.check(action)?;
        let mut report = ActionReport::default();

        match action {
            Action::EndTurn => {
                self.state.end_turn();
            }
            Action::Move { unit, dest } => {
                let stop = self.ambush_stop(unit, dest);
                report.ambushed = stop != dest;
                self.state.relocate(unit, stop);
                self.spend_move(unit, stop);
                self.state.unit_mut(unit).unwrap().moved = true;
            }
            Action::Attack { unit, dest, target } => {
                self.state.relocate(unit, dest);
                self.spend_move(unit, dest);
                self.state.unit_mut(unit).unwrap().moved = true;
                report = self.resolve_battle(unit, target);
            }
            Action::Capture { unit, dest } => {
                self.state.relocate(unit, dest);
                self.spend_move(unit, dest);
                self.state.unit_mut(unit).unwrap().moved = true;
                report.property_captured = self.resolve_capture(unit, dest);
            }
            Action::Build { at, typ } => {
                let player = self.state.current;
                let cost = self.unit_cost(player, typ);
                self.state.players[player as usize].funds -= cost;
                let id = self.state.spawn(typ, player, at);
                // Fresh units cannot act on the turn they are built.
                self.state.unit_mut(id).unwrap().moved = true;
                report.unit_built = Some(id);
            }
            Action::Load { unit, dest } => {
                let transport = self.state.unit_id_at(dest).unwrap();
                self.spend_move(unit, dest);
                self.state.load_into(unit, transport);
                self.state.unit_mut(unit).unwrap().moved = true;
            }
            Action::Unload {
                transport,
                cargo,
                drop_at,
            } => {
                // Free action: the transport keeps whatever move it had left.
                self.state.unload_to(transport, cargo, drop_at);
            }
            Action::Join { unit, dest } => {
                let other = self.state.unit_id_at(dest).unwrap();
                self.resolve_join(unit, other);
            }
            Action::Supply { unit, dest } => {
                self.state.relocate(unit, dest);
                self.spend_move(unit, dest);
                self.state.unit_mut(unit).unwrap().moved = true;
                self.resolve_supply(unit, dest);
            }
        }

        self.state.check_eliminations();
        self.refresh_vision();
        Ok(report)
    }

    /// Where a move actually ends.
    ///
    /// Under fog the planned route may run through a tile holding an enemy the
    /// mover never saw. AWBW halts the unit on the last tile before it, which
    /// is what makes hiding in woods a trap rather than a formality.
    fn ambush_stop(&mut self, unit_id: UnitId, dest: Pos) -> Pos {
        if !self.state.settings.fog {
            return dest;
        }
        let Some(owner) = self.state.unit(unit_id).map(|u| u.owner) else {
            return dest;
        };
        let path = self.reach.path_to(&self.state, dest);
        for (i, &step) in path.iter().enumerate().skip(1) {
            let Some(other) = self.state.unit_at(step) else {
                continue;
            };
            if self.state.are_enemies(owner, other.owner) {
                return path[i - 1];
            }
        }
        dest
    }

    /// Burns the fuel a move cost. `self.reach` still holds the unit's map
    /// because `check` just computed it.
    fn spend_move(&mut self, unit_id: UnitId, dest: Pos) {
        let cost = self.reach.cost_to(&self.state, dest).unwrap_or(0);
        if let Some(unit) = self.state.unit_mut(unit_id) {
            unit.fuel = unit.fuel.saturating_sub(cost);
        }
    }

    /// Resolves an attack and its counterattack, rolling luck for each.
    fn resolve_battle(&mut self, attacker_id: UnitId, target: Pos) -> ActionReport {
        let mut report = ActionReport::default();
        let Some(defender_id) = self.state.unit_id_at(target) else {
            return report;
        };
        let attacker = *self.state.unit(attacker_id).unwrap();
        let defender = *self.state.unit(defender_id).unwrap();

        let Some((pct, weapon)) =
            combat::base_percentage(attacker.typ, defender.typ, attacker.ammo)
        else {
            return report;
        };
        if weapon == combat::Weapon::Primary {
            self.state.unit_mut(attacker_id).unwrap().ammo -= 1;
        }

        let damage = self.roll_damage(pct, attacker, defender, target);
        report.damage_dealt = damage as u8;

        let defender_hp = defender.hp100 as i32 - damage;
        self.charge_meters(&attacker, &defender, defender_hp);
        if defender_hp <= 0 {
            report.defender_destroyed = true;
            self.state.destroy(defender_id);
            return report;
        }
        self.state.unit_mut(defender_id).unwrap().hp100 = defender_hp as u8;

        // Counterattack: only between two direct-combat units in range.
        if attacker.typ.is_indirect() || defender.typ.is_indirect() {
            return report;
        }
        let defender = *self.state.unit(defender_id).unwrap();
        let Some((counter_pct, counter_weapon)) =
            combat::base_percentage(defender.typ, attacker.typ, defender.ammo)
        else {
            return report;
        };
        if counter_weapon == combat::Weapon::Primary {
            self.state.unit_mut(defender_id).unwrap().ammo -= 1;
        }
        let attacker = *self.state.unit(attacker_id).unwrap();
        let counter = self.roll_damage(counter_pct, defender, attacker, attacker.pos);
        report.damage_taken = counter as u8;

        let attacker_hp = attacker.hp100 as i32 - counter;
        self.charge_meters(&defender, &attacker, attacker_hp);
        if attacker_hp <= 0 {
            report.attacker_destroyed = true;
            self.state.destroy(attacker_id);
        } else {
            self.state.unit_mut(attacker_id).unwrap().hp100 = attacker_hp as u8;
        }
        report
    }

    /// Banks power charge for one strike: `victim` fell from full health to
    /// `hp_after`. Both meters fill on the *displayed*-HP damage priced in
    /// the victim's cost — full rate for the side that took it, half for the
    /// side that dealt it. (5 HP off an infantry: 5,000 units and 2,500.)
    fn charge_meters(
        &mut self,
        dealer: &crate::state::Unit,
        victim: &crate::state::Unit,
        hp_after: i32,
    ) {
        let dhp = combat::display_hp(victim.hp100 as i32) - combat::display_hp(hp_after.max(0));
        let value = dhp.max(0) as u32 * victim.typ.stats().cost;
        self.state.add_combat_charge(victim.owner, value);
        self.state.add_combat_charge(dealer.owner, value / 2);
    }

    /// One luck roll of AWBW's damage formula for a concrete pairing.
    fn roll_damage(
        &mut self,
        pct: i32,
        attacker: crate::state::Unit,
        defender: crate::state::Unit,
        defender_pos: Pos,
    ) -> i32 {
        let terrain = self.state.map.terrain_at(defender_pos);
        let terrain_defense = combat::effective_terrain_defense(defender.move_type(), terrain);
        let attacker_co = self.state.co_of(attacker.owner);
        let defender_co = self.state.co_of(defender.owner);
        let attacker_terrain = self.state.map.terrain_at(attacker.pos);

        // Luck is the attacking CO's range. Note the wide-luck COs (Nell, Flak,
        // Jugger) are banned from Global League play, so no recorded game in the
        // corpus exercises this path -- it is implemented from the CO table but
        // unverified.
        let good_luck = self.rng.roll_inclusive(attacker_co.luck_good_max.max(0) as u32) as i32;
        let bad_luck = self.rng.roll_inclusive(attacker_co.luck_bad_max.max(0) as u32) as i32;

        combat::damage_roll(
            pct,
            attacker.hp100 as i32,
            defender.hp100 as i32,
            terrain_defense,
            combat::co_modifiers(
                attacker_co,
                attacker.typ,
                attacker_terrain,
                self.state.active_power(attacker.owner),
            ),
            combat::co_modifiers(
                defender_co,
                defender.typ,
                terrain,
                self.state.active_power(defender.owner),
            ),
            self.state.com_tower_bonus(attacker.owner),
            good_luck,
            bad_luck,
        )
    }

    /// Applies capture progress; returns whether the property changed hands.
    fn resolve_capture(&mut self, unit_id: UnitId, pos: Pos) -> bool {
        let unit = *self.state.unit(unit_id).unwrap();
        let owner = unit.owner;
        // Capture advances by displayed HP, scaled by the CO: Sami takes
        // properties half again as fast.
        let rate = self.state.co_of(owner).capture_multiplier_pct;
        let progress = (unit.display_hp() as u32 * rate / 100).min(255) as u8;

        let Some(building) = self.state.building_at_mut(pos) else {
            return false;
        };
        building.capture_remaining = building.capture_remaining.saturating_sub(progress);
        if building.capture_remaining > 0 {
            return false;
        }

        let kind = building.kind;
        let previous = building.owner;
        building.owner = Some(owner);
        building.capture_remaining = CAPTURE_FULL;

        // Taking an HQ wipes out its owner and hands over everything they held.
        if kind == TerrainKind::Hq {
            if let Some(loser) = previous {
                self.state.eliminate(loser, Some(owner));
            }
        }
        true
    }

    fn resolve_join(&mut self, unit_id: UnitId, other_id: UnitId) {
        let unit = *self.state.unit(unit_id).unwrap();
        let other = *self.state.unit(other_id).unwrap();
        let stats = unit.typ.stats();

        let combined = unit.hp100 as u16 + other.hp100 as u16;
        // HP above 10 is refunded at the unit's per-HP value.
        let overflow = combined.saturating_sub(100);
        if overflow > 0 {
            let display_overflow = (overflow as u32 + 9) / 10;
            self.state.players[unit.owner as usize].funds += display_overflow * (stats.cost / 10);
        }

        let target = self.state.unit_mut(other_id).unwrap();
        target.hp100 = combined.min(100) as u8;
        target.fuel = (unit.fuel as u16 + other.fuel as u16).min(stats.max_fuel as u16) as u8;
        target.ammo = (unit.ammo as u16 + other.ammo as u16).min(stats.max_ammo as u16) as u8;
        target.moved = true;

        self.state.destroy(unit_id);
    }

    fn resolve_supply(&mut self, unit_id: UnitId, pos: Pos) {
        let owner = self.state.unit(unit_id).unwrap().owner;
        let neighbors: Vec<Pos> = self.state.map.neighbors(pos).collect();
        for n in neighbors {
            let Some(id) = self.state.unit_id_at(n) else {
                continue;
            };
            let Some(other) = self.state.unit(id).copied() else {
                continue;
            };
            if !self.state.are_allied(owner, other.owner) {
                continue;
            }
            let stats = other.typ.stats();
            let u = self.state.unit_mut(id).unwrap();
            u.fuel = stats.max_fuel;
            u.ammo = stats.max_ammo;
        }
    }

    // --- enumeration -------------------------------------------------------

    /// Every legal action for the player to move, for action masking.
    ///
    /// `EndTurn` is always included and always last, so a policy can never be
    /// left with an empty mask.
    pub fn legal_actions(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        self.legal_actions_into(&mut out);
        out
    }

    /// As [`Engine::legal_actions`], but refills a caller-owned buffer. Self-play
    /// calls this once per micro-step, so it must not allocate.
    pub fn legal_actions_into(&mut self, out: &mut Vec<Action>) {
        out.clear();
        let player = self.state.current;

        // Taken from `self` so the loop can hold `&mut self.reach`, and handed
        // back at the end to keep its capacity.
        let mut unit_ids = std::mem::take(&mut self.movable);
        unit_ids.clear();
        unit_ids.extend(
            self.state
                .units_of(player)
                .filter(|u| !u.moved && u.carried_by.is_none())
                .map(|u| u.id),
        );

        for &unit_id in unit_ids.iter() {
            let unit = *self.state.unit(unit_id).unwrap();
            self.compute_reach(unit_id);
            self.push_unit_actions(unit_id, unit, out);
            self.push_unloads(out, unit_id, unit);
        }

        // Transports that have already moved can still unload.
        let loaded: Vec<UnitId> = self
            .state
            .units_of(player)
            .filter(|u| u.moved && u.carried_by.is_none() && u.cargo_len() > 0)
            .map(|u| u.id)
            .collect();
        for unit_id in loaded {
            let unit = *self.state.unit(unit_id).unwrap();
            self.push_unloads(out, unit_id, unit);
        }

        unit_ids.clear();
        self.movable = unit_ids;

        // Production. Buildings are few, so this scan stays cheap.
        let mut sites = std::mem::take(&mut self.sites);
        sites.clear();
        sites.extend(
            self.state
                .buildings_of(player)
                .filter(|b| {
                    matches!(b.kind, TerrainKind::Base | TerrainKind::Airport | TerrainKind::Port)
                })
                .map(|b| b.pos),
        );
        for &at in sites.iter() {
            if self.state.unit_id_at(at).is_some() {
                continue;
            }
            for typ in UnitType::ALL {
                if self.check_build(at, typ).is_ok() {
                    out.push(Action::Build { at, typ });
                }
            }
        }
        sites.clear();
        self.sites = sites;

        out.push(Action::EndTurn);
    }

    /// Every order one unit can give, assuming `self.reach` already holds its
    /// movement map.
    fn push_unit_actions(&self, unit_id: UnitId, unit: crate::state::Unit, out: &mut Vec<Action>) {
        let player = unit.owner;
        for dest in self.reach.reachable(&self.state) {
            let occupant = self.state.unit_id_at(dest);
            let free = occupant.is_none() || occupant == Some(unit_id);

            if free {
                out.push(Action::Move { unit: unit_id, dest });
                self.push_attacks(out, unit_id, unit, dest);

                if matches!(unit.typ, UnitType::Infantry | UnitType::Mech)
                    && self
                        .state
                        .building_at(dest)
                        .is_some_and(|b| b.owner != Some(player))
                {
                    out.push(Action::Capture { unit: unit_id, dest });
                }
                if unit.typ == UnitType::Apc {
                    out.push(Action::Supply { unit: unit_id, dest });
                }
            } else if let Some(other_id) = occupant {
                let other = *self.state.unit(other_id).unwrap();
                if other.owner == player
                    && other.typ == unit.typ
                    && other.hp100 < 100
                    && unit.cargo_len() == 0
                    && other.cargo_len() == 0
                {
                    out.push(Action::Join { unit: unit_id, dest });
                }
                if self.state.are_allied(player, other.owner)
                    && can_carry(other.typ, unit.typ)
                    && other.cargo_len() < cargo_capacity(other.typ)
                {
                    out.push(Action::Load { unit: unit_id, dest });
                }
            }
        }
    }

    /// Orders available to a single unit.
    ///
    /// This is the cheap path for a factorized policy that picks *which unit*
    /// first and *what it does* second: it costs one reachability search
    /// instead of one per unit, which is the difference between O(n) and O(n^2)
    /// work per turn.
    pub fn legal_actions_for(&mut self, unit_id: UnitId, out: &mut Vec<Action>) {
        out.clear();
        let Some(unit) = self.state.unit(unit_id).copied() else {
            return;
        };
        if unit.owner != self.state.current || unit.moved || unit.carried_by.is_some() {
            return;
        }
        self.compute_reach(unit_id);
        self.push_unit_actions(unit_id, unit, out);
        self.push_unloads(out, unit_id, unit);
    }

    /// Whether this tile is an owned production property that can afford at
    /// least one unit, i.e. a tile that can act without holding a unit.
    pub fn can_build_anything(&self, at: Pos) -> bool {
        UnitType::ALL
            .into_iter()
            .any(|typ| self.check_build(at, typ).is_ok())
    }

    /// Every order available from one tile, whether that means the unit
    /// standing on it or the property under it.
    ///
    /// This is the staged counterpart to `legal_actions_into`: a policy that
    /// picks a tile first only ever needs this much of the action set, which is
    /// one reachability search rather than one per unit.
    pub fn legal_actions_at(&mut self, at: Pos, out: &mut Vec<Action>) {
        out.clear();
        if let Some(unit) = self.state.unit_id_at(at) {
            self.legal_actions_for(unit, out);
            // Unloading costs a transport nothing, so a transport that has
            // already moved still has this to offer.
            if let Some(u) = self.state.unit(unit).copied() {
                if u.owner == self.state.current && u.moved {
                    self.push_unloads(out, unit, u);
                }
            }
            return;
        }
        for typ in UnitType::ALL {
            if self.check_build(at, typ).is_ok() {
                out.push(Action::Build { at, typ });
            }
        }
    }

    /// Units belonging to the player to move that still have an order left.
    pub fn movable_units(&self) -> impl Iterator<Item = UnitId> + '_ {
        let player = self.state.current;
        self.state
            .units_of(player)
            .filter(|u| !u.moved && u.carried_by.is_none())
            .map(|u| u.id)
    }

    /// Enumerates shots from `dest` by walking the weapon's range diamond.
    ///
    /// Scanning the diamond rather than every unit on the map matters: a direct
    /// unit looks at 4 tiles instead of the whole army, and this runs once per
    /// reachable tile per unit per step.
    fn push_attacks(&self, out: &mut Vec<Action>, unit_id: UnitId, unit: crate::state::Unit, dest: Pos) {
        if unit.typ.is_indirect() && dest != unit.pos {
            return;
        }
        let (min_range, max_range) = self.range_of(unit.owner, unit.typ);
        let (min, max) = (min_range as i32, max_range as i32);
        for dy in -max..=max {
            let span = max - dy.abs();
            for dx in -span..=span {
                let distance = dx.abs() + dy.abs();
                if distance < min || distance == 0 {
                    continue;
                }
                let (tx, ty) = (dest.x as i32 + dx, dest.y as i32 + dy);
                if !self.state.map.contains(tx, ty) {
                    continue;
                }
                let target = Pos::new(tx as u8, ty as u8);
                let Some(other) = self.state.unit_at(target) else {
                    continue;
                };
                if !self.state.are_enemies(unit.owner, other.owner) {
                    continue;
                }
                if !self.vision.sees_unit(&self.state, other) {
                    continue;
                }
                if other.hidden && !combat::can_target_hidden(unit.typ, other.typ) {
                    continue;
                }
                if combat::base_percentage(unit.typ, other.typ, unit.ammo).is_none() {
                    continue;
                }
                out.push(Action::Attack {
                    unit: unit_id,
                    dest,
                    target,
                });
            }
        }
    }

    /// Unloads available from where a transport currently stands.
    fn push_unloads(&self, out: &mut Vec<Action>, unit_id: UnitId, unit: crate::state::Unit) {
        if unit.cargo_len() == 0 || unit.carried_by.is_some() {
            return;
        }
        for slot in 0..MAX_CARGO {
            let cargo_id = unit.cargo[slot];
            let Some(cargo) = self.state.unit(cargo_id) else {
                continue;
            };
            for drop_at in self.state.map.neighbors(unit.pos) {
                if self.state.unit_id_at(drop_at).is_some() {
                    continue;
                }
                if self
                    .state
                    .map
                    .terrain_at(drop_at)
                    .move_cost(self.state.weather, cargo.move_type())
                    .is_none()
                {
                    continue;
                }
                out.push(Action::Unload {
                    transport: unit_id,
                    cargo: cargo_id,
                    drop_at,
                });
            }
        }
    }

    /// Expected-damage preview for a candidate attack, without rolling.
    pub fn preview_damage(&self, attacker_id: UnitId, target: Pos) -> Option<DamageSpread> {
        let defender_hp = self.state.unit_at(target)?.hp100 as i32;
        self.preview_damage_at(attacker_id, target, defender_hp)
    }

    /// Damage a defender would deal back, given where the attacker will be
    /// standing and how much health the defender will have left.
    ///
    /// The plain previews look the defender up by tile, which cannot answer
    /// "what happens to me if I move *there* and shoot": nothing stands on the
    /// destination yet. Both units are named explicitly here instead.
    pub fn preview_counter(
        &self,
        defender_id: UnitId,
        defender_hp100: i32,
        attacker_id: UnitId,
        attacker_pos: Pos,
    ) -> Option<DamageSpread> {
        let defender = self.state.unit(defender_id)?;
        let attacker = self.state.unit(attacker_id)?;
        // Indirect units never counter, and nothing counters indirect fire.
        if defender.typ.is_indirect() || attacker.typ.is_indirect() {
            return None;
        }
        let (pct, _) = combat::base_percentage(defender.typ, attacker.typ, defender.ammo)?;
        let terrain = self.state.map.terrain_at(attacker_pos);
        let defender_co = self.state.co_of(defender.owner);
        let attacker_co = self.state.co_of(attacker.owner);
        Some(combat::damage_spread(
            pct,
            defender_hp100,
            attacker.hp100 as i32,
            combat::effective_terrain_defense(attacker.move_type(), terrain),
            combat::co_modifiers(
                defender_co,
                defender.typ,
                self.state.map.terrain_at(defender.pos),
                self.state.active_power(defender.owner),
            ),
            combat::co_modifiers(
                attacker_co,
                attacker.typ,
                terrain,
                self.state.active_power(attacker.owner),
            ),
            self.state.com_tower_bonus(defender.owner),
            defender_co.luck_good_max,
            defender_co.luck_bad_max,
        ))
    }

    /// As [`Engine::preview_damage`], but for a hypothetical defender HP.
    ///
    /// Terrain cover scales with the defender's displayed HP, so a wounded
    /// defender takes more damage than a healthy one on the same tile; callers
    /// reasoning about a unit whose exact HP they do not know need to ask about
    /// both ends of the range.
    pub fn preview_damage_at(
        &self,
        attacker_id: UnitId,
        target: Pos,
        defender_hp100: i32,
    ) -> Option<DamageSpread> {
        let attacker = self.state.unit(attacker_id)?;
        let defender = self.state.unit_at(target)?;
        let (pct, _) = combat::base_percentage(attacker.typ, defender.typ, attacker.ammo)?;
        let terrain = self.state.map.terrain_at(target);
        let attacker_co = self.state.co_of(attacker.owner);
        let defender_co = self.state.co_of(defender.owner);
        let attacker_terrain = self.state.map.terrain_at(attacker.pos);
        Some(combat::damage_spread(
            pct,
            attacker.hp100 as i32,
            defender_hp100,
            combat::effective_terrain_defense(defender.move_type(), terrain),
            combat::co_modifiers(
                attacker_co,
                attacker.typ,
                attacker_terrain,
                self.state.active_power(attacker.owner),
            ),
            combat::co_modifiers(
                defender_co,
                defender.typ,
                terrain,
                self.state.active_power(defender.owner),
            ),
            self.state.com_tower_bonus(attacker.owner),
            attacker_co.luck_good_max,
            attacker_co.luck_bad_max,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::state::{GameSettings, GameState, Outcome, Player};
    use crate::types::TerrainKind;
    use std::sync::Arc;

    fn engine(width: u8, height: u8, kinds: Vec<TerrainKind>) -> Engine {
        let map = Arc::new(Map::from_kinds(width, height, kinds).unwrap());
        let props = map.properties().len();
        let players = vec![
            Player::new(10_000, 1),
            Player::new(10_000, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &vec![None; props]);
        Engine::new(state, 12345)
    }

    fn plains(w: u8, h: u8) -> Engine {
        engine(w, h, vec![TerrainKind::Plain; w as usize * h as usize])
    }

    #[test]
    fn move_marks_the_unit_and_burns_fuel() {
        let mut e = plains(9, 1);
        let id = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        let fuel = e.state.unit(id).unwrap().fuel;
        e.apply(Action::Move { unit: id, dest: Pos::new(3, 0) }).unwrap();
        let unit = e.state.unit(id).unwrap();
        assert_eq!(unit.pos, Pos::new(3, 0));
        assert!(unit.moved);
        assert_eq!(unit.fuel, fuel - 3);
        // A second order is refused.
        assert_eq!(
            e.apply(Action::Move { unit: id, dest: Pos::new(4, 0) }),
            Err(ActionError::AlreadyMoved)
        );
    }

    #[test]
    fn cannot_move_another_players_unit() {
        let mut e = plains(5, 1);
        let id = e.state.spawn(UnitType::Tank, 1, Pos::new(0, 0));
        assert_eq!(
            e.apply(Action::Move { unit: id, dest: Pos::new(1, 0) }),
            Err(ActionError::NotYourUnit)
        );
    }

    #[test]
    fn attack_damages_and_draws_a_counter() {
        let mut e = plains(5, 1);
        let attacker = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        let defender = e.state.spawn(UnitType::Tank, 1, Pos::new(3, 0));
        let report = e
            .apply(Action::Attack {
                unit: attacker,
                dest: Pos::new(2, 0),
                target: Pos::new(3, 0),
            })
            .unwrap();
        assert!(report.damage_dealt > 0);
        assert!(report.damage_taken > 0, "a surviving direct unit counters");
        assert!(e.state.unit(defender).unwrap().hp100 < 100);
        assert!(e.state.unit(attacker).unwrap().hp100 < 100);
        // Both spent a round of primary ammo.
        assert_eq!(e.state.unit(attacker).unwrap().ammo, UnitType::Tank.stats().max_ammo - 1);
    }

    #[test]
    fn combat_charges_both_power_meters() {
        use crate::state::ActivePower;
        let mut e = plains(5, 1);
        e.state.players[0].co = crate::co_data::co_by_name("Adder").unwrap();
        e.state.players[1].co = crate::co_data::co_by_name("Adder").unwrap();
        let attacker = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Tank, 1, Pos::new(3, 0));
        let report = e
            .apply(Action::Attack {
                unit: attacker,
                dest: Pos::new(2, 0),
                target: Pos::new(3, 0),
            })
            .unwrap();

        // Displayed-HP damage, priced in the victim's cost: full rate to the
        // side that took it, half to the side that dealt it.
        let dealt = 10 - combat::display_hp(100 - report.damage_dealt as i32) as u32;
        let taken = 10 - combat::display_hp(100 - report.damage_taken as i32) as u32;
        let cost = UnitType::Tank.stats().cost;
        assert_eq!(e.state.players[1].charge, dealt * cost + taken * cost / 2);
        assert_eq!(e.state.players[0].charge, taken * cost + dealt * cost / 2);
        assert_eq!(e.state.active_power(0), ActivePower::None);
    }

    #[test]
    fn indirect_units_must_hold_still_and_take_no_counter() {
        let mut e = plains(9, 1);
        let arty = e.state.spawn(UnitType::Artillery, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Tank, 1, Pos::new(2, 0));

        // Cannot move and fire in the same order.
        assert_eq!(
            e.check(Action::Attack {
                unit: arty,
                dest: Pos::new(1, 0),
                target: Pos::new(2, 0)
            }),
            Err(ActionError::OutOfRange)
        );
        // Adjacent targets are inside the minimum range, so also illegal.
        assert!(e
            .check(Action::Attack {
                unit: arty,
                dest: Pos::new(0, 0),
                target: Pos::new(1, 0)
            })
            .is_err());

        let report = e
            .apply(Action::Attack {
                unit: arty,
                dest: Pos::new(0, 0),
                target: Pos::new(2, 0),
            })
            .unwrap();
        assert!(report.damage_dealt > 0);
        assert_eq!(report.damage_taken, 0);
    }

    #[test]
    fn killing_blow_prevents_the_counter() {
        let mut e = plains(5, 1);
        let attacker = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        let defender = e.state.spawn(UnitType::Infantry, 1, Pos::new(1, 0));
        e.state.unit_mut(defender).unwrap().hp100 = 10;
        let report = e
            .apply(Action::Attack {
                unit: attacker,
                dest: Pos::new(0, 0),
                target: Pos::new(1, 0),
            })
            .unwrap();
        assert!(report.defender_destroyed);
        assert_eq!(report.damage_taken, 0);
        assert!(e.state.unit(defender).is_none());
        assert_eq!(e.state.unit(attacker).unwrap().hp100, 100);
    }

    #[test]
    fn capture_takes_two_turns_at_full_health() {
        let mut kinds = vec![TerrainKind::Plain; 5];
        kinds[2] = TerrainKind::City;
        let mut e = engine(5, 1, kinds);
        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));

        let report = e.apply(Action::Capture { unit: inf, dest: Pos::new(2, 0) }).unwrap();
        assert!(!report.property_captured);
        assert_eq!(e.state.building_at(Pos::new(2, 0)).unwrap().capture_remaining, 10);

        e.state.unit_mut(inf).unwrap().moved = false;
        let report = e.apply(Action::Capture { unit: inf, dest: Pos::new(2, 0) }).unwrap();
        assert!(report.property_captured);
        assert_eq!(e.state.building_at(Pos::new(2, 0)).unwrap().owner, Some(0));
    }

    #[test]
    fn damaged_units_capture_more_slowly() {
        let mut kinds = vec![TerrainKind::Plain; 3];
        kinds[1] = TerrainKind::City;
        let mut e = engine(3, 1, kinds);
        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        e.state.unit_mut(inf).unwrap().hp100 = 55; // displays as 6
        e.apply(Action::Capture { unit: inf, dest: Pos::new(1, 0) }).unwrap();
        assert_eq!(e.state.building_at(Pos::new(1, 0)).unwrap().capture_remaining, 14);
    }

    #[test]
    fn walking_off_a_property_resets_its_capture() {
        let mut kinds = vec![TerrainKind::Plain; 5];
        kinds[2] = TerrainKind::City;
        let mut e = engine(5, 1, kinds);
        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(2, 0));
        e.apply(Action::Capture { unit: inf, dest: Pos::new(2, 0) }).unwrap();
        assert_eq!(e.state.building_at(Pos::new(2, 0)).unwrap().capture_remaining, 10);

        e.state.unit_mut(inf).unwrap().moved = false;
        e.apply(Action::Move { unit: inf, dest: Pos::new(3, 0) }).unwrap();
        assert_eq!(
            e.state.building_at(Pos::new(2, 0)).unwrap().capture_remaining,
            CAPTURE_FULL
        );
    }

    #[test]
    fn capturing_an_hq_wins_the_game() {
        let mut kinds = vec![TerrainKind::Plain; 3];
        kinds[1] = TerrainKind::Hq;
        let map = Arc::new(Map::from_kinds(3, 1, kinds).unwrap());
        let players = vec![
            Player::new(0, 1),
            Player::new(0, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(1)]);
        let mut e = Engine::new(state, 1);

        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Tank, 1, Pos::new(2, 0));
        e.apply(Action::Capture { unit: inf, dest: Pos::new(1, 0) }).unwrap();
        e.state.unit_mut(inf).unwrap().moved = false;
        let report = e.apply(Action::Capture { unit: inf, dest: Pos::new(1, 0) }).unwrap();

        assert!(report.property_captured);
        assert!(e.state.player(1).eliminated);
        assert_eq!(e.state.unit_count(1), 0, "losing the HQ loses the army");
        assert_eq!(e.state.outcome(), Outcome::Winner(0));
    }

    #[test]
    fn build_costs_funds_and_produces_a_spent_unit() {
        let mut kinds = vec![TerrainKind::Plain; 3];
        kinds[0] = TerrainKind::Base;
        let map = Arc::new(Map::from_kinds(3, 1, kinds).unwrap());
        let players = vec![
            Player::new(8000, 1),
            Player::new(0, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(0)]);
        let mut e = Engine::new(state, 1);

        let report = e.apply(Action::Build { at: Pos::new(0, 0), typ: UnitType::Tank }).unwrap();
        let id = report.unit_built.unwrap();
        assert_eq!(e.state.player(0).funds, 1000);
        assert!(e.state.unit(id).unwrap().moved);

        // Too expensive now, and the tile is taken.
        assert_eq!(
            e.check(Action::Build { at: Pos::new(0, 0), typ: UnitType::Tank }),
            Err(ActionError::Occupied)
        );
    }

    #[test]
    fn bases_refuse_air_and_sea_units() {
        let mut kinds = vec![TerrainKind::Plain; 3];
        kinds[0] = TerrainKind::Base;
        let map = Arc::new(Map::from_kinds(3, 1, kinds).unwrap());
        let players = vec![
            Player::new(99_000, 1),
            Player::new(0, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(0)]);
        let mut e = Engine::new(state, 1);
        assert_eq!(
            e.check(Action::Build { at: Pos::new(0, 0), typ: UnitType::Fighter }),
            Err(ActionError::CannotProduceThat)
        );
        assert!(e.check(Action::Build { at: Pos::new(0, 0), typ: UnitType::Infantry }).is_ok());
    }

    #[test]
    fn load_and_unload_moves_passengers() {
        let mut e = plains(9, 1);
        let apc = e.state.spawn(UnitType::Apc, 0, Pos::new(4, 0));
        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(2, 0));

        e.apply(Action::Load { unit: inf, dest: Pos::new(4, 0) }).unwrap();
        assert_eq!(e.state.unit(inf).unwrap().carried_by, Some(apc));
        assert!(e.state.unit_at(Pos::new(2, 0)).is_none());

        // The transport drives off, having already spent its move loading.
        e.state.unit_mut(apc).unwrap().moved = false;
        e.apply(Action::Move { unit: apc, dest: Pos::new(7, 0) }).unwrap();
        assert!(e.state.unit(apc).unwrap().moved);

        // Unloading is free in AWBW: a transport may do it after moving.
        e.apply(Action::Unload {
            transport: apc,
            cargo: inf,
            drop_at: Pos::new(8, 0),
        })
        .unwrap();
        assert_eq!(e.state.unit(inf).unwrap().pos, Pos::new(8, 0));
        assert!(e.state.unit(inf).unwrap().moved, "unloaded units cannot act");
        assert_eq!(e.state.unit(apc).unwrap().cargo_len(), 0);
    }

    #[test]
    fn unloading_is_free_and_does_not_spend_the_transports_turn() {
        // "transports may unload at any point in their turn, even if they have
        // already moved, and doing so does not end the unit's turn either" --
        // the AWBW wiki. The cartridge behaves differently.
        let mut e = plains(9, 1);
        let apc = e.state.spawn(UnitType::Apc, 0, Pos::new(4, 0));
        let inf = e.state.spawn(UnitType::Infantry, 0, Pos::new(5, 0));
        e.state.load_into(inf, apc);
        e.state.unit_mut(inf).unwrap().moved = false;

        // Unload first...
        e.apply(Action::Unload { transport: apc, cargo: inf, drop_at: Pos::new(3, 0) })
            .unwrap();
        assert_eq!(e.state.unit(inf).unwrap().pos, Pos::new(3, 0));
        assert!(!e.state.unit(apc).unwrap().moved, "unloading is free");

        // ...and the transport can still move afterwards.
        assert!(e.apply(Action::Move { unit: apc, dest: Pos::new(7, 0) }).is_ok());
    }

    #[test]
    fn join_merges_hp_and_refunds_overflow() {
        let mut e = plains(5, 1);
        let a = e.state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        let b = e.state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        e.state.unit_mut(a).unwrap().hp100 = 60;
        e.state.unit_mut(b).unwrap().hp100 = 60;
        let funds = e.state.player(0).funds;

        e.apply(Action::Join { unit: a, dest: Pos::new(1, 0) }).unwrap();
        assert!(e.state.unit(a).is_none());
        assert_eq!(e.state.unit(b).unwrap().hp100, 100);
        // 2 HP over the cap, refunded at 100 funds each.
        assert_eq!(e.state.player(0).funds, funds + 200);
    }

    #[test]
    fn join_refuses_full_health_and_mismatched_types() {
        let mut e = plains(5, 1);
        let a = e.state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        assert_eq!(
            e.check(Action::Join { unit: a, dest: Pos::new(1, 0) }),
            Err(ActionError::CannotJoinThat)
        );
        let mech = e.state.spawn(UnitType::Mech, 0, Pos::new(2, 0));
        e.state.unit_mut(mech).unwrap().hp100 = 50;
        assert_eq!(
            e.check(Action::Join { unit: a, dest: Pos::new(2, 0) }),
            Err(ActionError::CannotJoinThat)
        );
    }

    #[test]
    fn apc_supply_refills_neighbors() {
        let mut e = plains(5, 1);
        let apc = e.state.spawn(UnitType::Apc, 0, Pos::new(0, 0));
        let tank = e.state.spawn(UnitType::Tank, 0, Pos::new(2, 0));
        e.state.unit_mut(tank).unwrap().ammo = 0;
        e.state.unit_mut(tank).unwrap().fuel = 5;

        e.apply(Action::Supply { unit: apc, dest: Pos::new(1, 0) }).unwrap();
        let tank = e.state.unit(tank).unwrap();
        assert_eq!(tank.ammo, UnitType::Tank.stats().max_ammo);
        assert_eq!(tank.fuel, UnitType::Tank.stats().max_fuel);
    }

    #[test]
    fn legal_actions_are_never_empty_and_always_check_out() {
        let mut kinds = vec![TerrainKind::Plain; 25];
        kinds[0] = TerrainKind::Base;
        kinds[12] = TerrainKind::City;
        let map = Arc::new(Map::from_kinds(5, 5, kinds).unwrap());
        let players = vec![
            Player::new(20_000, 1),
            Player::new(20_000, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(0), None]);
        let mut e = Engine::new(state, 99);
        e.state.spawn(UnitType::Infantry, 0, Pos::new(2, 2));
        e.state.spawn(UnitType::Artillery, 0, Pos::new(1, 1));
        e.state.spawn(UnitType::Tank, 1, Pos::new(3, 3));

        let actions = e.legal_actions();
        assert!(actions.len() > 10);
        assert_eq!(actions.last(), Some(&Action::EndTurn));
        // Everything enumerated must survive a fresh legality check.
        for action in &actions {
            assert!(e.check(*action).is_ok(), "illegal action enumerated: {action:?}");
        }
        // The capture and a build are both in there.
        assert!(actions.iter().any(|a| matches!(a, Action::Capture { .. })));
        assert!(actions.iter().any(|a| matches!(a, Action::Build { .. })));
    }

    #[test]
    fn per_unit_enumeration_agrees_with_the_flat_list() {
        let mut kinds = vec![TerrainKind::Plain; 25];
        kinds[0] = TerrainKind::Base;
        kinds[12] = TerrainKind::City;
        let map = Arc::new(Map::from_kinds(5, 5, kinds).unwrap());
        let players = vec![
            Player::new(20_000, 1),
            Player::new(20_000, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(0), None]);
        let mut e = Engine::new(state, 5);
        e.state.spawn(UnitType::Infantry, 0, Pos::new(2, 2));
        e.state.spawn(UnitType::Artillery, 0, Pos::new(1, 1));
        let apc = e.state.spawn(UnitType::Apc, 0, Pos::new(0, 2));
        let rider = e.state.spawn(UnitType::Mech, 0, Pos::new(0, 3));
        e.state.load_into(rider, apc);
        e.state.spawn(UnitType::Tank, 1, Pos::new(3, 3));

        let flat = e.legal_actions();
        let mut per_unit = Vec::new();
        let mut buffer = Vec::new();
        let units: Vec<UnitId> = e.movable_units().collect();
        for unit in units {
            e.legal_actions_for(unit, &mut buffer);
            per_unit.extend(buffer.iter().copied());
        }

        let mut expected: Vec<Action> = flat
            .iter()
            .copied()
            .filter(|a| !matches!(a, Action::Build { .. } | Action::EndTurn))
            .collect();
        expected.sort_by_key(|a| format!("{a:?}"));
        per_unit.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(per_unit, expected);
        assert!(!expected.is_empty());
    }

    fn fog_engine(width: u8, height: u8, kinds: Vec<TerrainKind>) -> Engine {
        let map = Arc::new(Map::from_kinds(width, height, kinds).unwrap());
        let props = map.properties().len();
        let players = vec![Player::new(10_000, 1), Player::new(10_000, 2)];
        let settings = GameSettings { fog: true, ..GameSettings::default() };
        let state = GameState::new(map, settings, players, &vec![None; props]);
        Engine::new(state, 7)
    }

    #[test]
    fn an_unseen_enemy_ambushes_a_passing_unit() {
        // A tank sees three tiles, so an enemy five tiles down the corridor is
        // beyond its knowledge and does not appear on the route it plans.
        let mut e = fog_engine(9, 1, vec![TerrainKind::Plain; 9]);
        let tank = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(5, 0));
        e.refresh_vision();

        // The hidden unit does not block planning...
        assert!(e.check(Action::Move { unit: tank, dest: Pos::new(6, 0) }).is_ok());
        // ...but the tank is stopped on the last tile before it.
        let report = e.apply(Action::Move { unit: tank, dest: Pos::new(6, 0) }).unwrap();
        assert!(report.ambushed);
        assert_eq!(e.state.unit(tank).unwrap().pos, Pos::new(4, 0));
    }

    #[test]
    fn a_visible_enemy_blocks_instead_of_ambushing() {
        let mut e = fog_engine(9, 1, vec![TerrainKind::Plain; 9]);
        let tank = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(2, 0));
        e.refresh_vision();
        // Tank vision is 3, so this one is in plain sight and blocks the route.
        assert_eq!(
            e.check(Action::Move { unit: tank, dest: Pos::new(6, 0) }),
            Err(ActionError::Unreachable)
        );
    }

    #[test]
    fn units_you_cannot_see_cannot_be_shot() {
        let mut kinds = vec![TerrainKind::Plain; 9];
        kinds[5] = TerrainKind::Wood;
        let mut e = fog_engine(9, 1, kinds);
        let arty = e.state.spawn(UnitType::Artillery, 0, Pos::new(3, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(5, 0));
        e.refresh_vision();

        // In range (2 tiles) but concealed by the woods.
        assert_eq!(
            e.check(Action::Attack { unit: arty, dest: Pos::new(3, 0), target: Pos::new(5, 0) }),
            Err(ActionError::NoTarget)
        );
        assert!(!e
            .legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Attack { .. })));

        // Put a spotter next to the woods and the shot opens up.
        e.state.spawn(UnitType::Infantry, 0, Pos::new(4, 0));
        e.refresh_vision();
        assert!(e
            .check(Action::Attack { unit: arty, dest: Pos::new(3, 0), target: Pos::new(5, 0) })
            .is_ok());
    }

    #[test]
    fn without_fog_nothing_is_hidden_or_ambushed() {
        let mut kinds = vec![TerrainKind::Plain; 9];
        kinds[4] = TerrainKind::Wood;
        let mut e = engine(9, 1, kinds);
        let tank = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(4, 0));
        // The enemy is visible, so it blocks rather than ambushes.
        assert_eq!(
            e.check(Action::Move { unit: tank, dest: Pos::new(6, 0) }),
            Err(ActionError::Unreachable)
        );
        let report = e.apply(Action::Move { unit: tank, dest: Pos::new(3, 0) }).unwrap();
        assert!(!report.ambushed);
    }

    #[test]
    fn a_random_fog_game_terminates_without_panicking() {
        let mut kinds = vec![TerrainKind::Plain; 49];
        kinds[0] = TerrainKind::Base;
        kinds[24] = TerrainKind::City;
        kinds[48] = TerrainKind::Base;
        for i in [10, 17, 31, 38] {
            kinds[i] = TerrainKind::Wood;
        }
        let map = Arc::new(Map::from_kinds(7, 7, kinds).unwrap());
        let players = vec![Player::new(10_000, 1), Player::new(10_000, 2)];
        let settings = GameSettings { fog: true, ..GameSettings::default() };
        let state = GameState::new(map, settings, players, &[Some(0), None, Some(1)]);
        let mut e = Engine::new(state, 4242);
        e.state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(5, 6));
        e.refresh_vision();

        let mut rng = Rng::new(11);
        for _ in 0..3000 {
            if e.state.outcome() != Outcome::InProgress {
                break;
            }
            let actions = e.legal_actions();
            let pick = actions[rng.roll_inclusive(actions.len() as u32 - 1) as usize];
            e.apply(pick).expect("enumerated actions must apply");
        }
    }

    #[test]
    fn a_random_game_terminates_without_panicking() {
        let mut kinds = vec![TerrainKind::Plain; 49];
        kinds[0] = TerrainKind::Base;
        kinds[24] = TerrainKind::City;
        kinds[48] = TerrainKind::Base;
        let map = Arc::new(Map::from_kinds(7, 7, kinds).unwrap());
        let players = vec![
            Player::new(10_000, 1),
            Player::new(10_000, 2),
        ];
        let state = GameState::new(map, GameSettings::default(), players, &[Some(0), None, Some(1)]);
        let mut e = Engine::new(state, 2024);
        e.state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(5, 6));

        let mut rng = Rng::new(7);
        for _ in 0..5000 {
            if e.state.outcome() != Outcome::InProgress {
                break;
            }
            let actions = e.legal_actions();
            let pick = actions[rng.roll_inclusive(actions.len() as u32 - 1) as usize];
            e.apply(pick).expect("enumerated actions must apply");
        }
    }

    #[test]
    fn preview_matches_the_rolled_range() {
        let mut e = plains(5, 1);
        let attacker = e.state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        e.state.spawn(UnitType::Infantry, 1, Pos::new(1, 0));
        let spread = e.preview_damage(attacker, Pos::new(1, 0)).unwrap();
        assert!(spread.min <= spread.max);
        let report = e
            .apply(Action::Attack {
                unit: attacker,
                dest: Pos::new(0, 0),
                target: Pos::new(1, 0),
            })
            .unwrap();
        assert!((report.damage_dealt as i32) >= spread.min);
        assert!((report.damage_dealt as i32) <= spread.max);
    }
}
