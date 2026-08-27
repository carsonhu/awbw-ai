//! Mutable game state: units, property ownership, player funds, whose turn it
//! is. Designed to be cheap to clone for search and self-play — the map lives
//! behind an `Arc` and everything else is flat vectors of `Copy` records.

use std::sync::Arc;

use crate::co_data::CoData;
use crate::map::{Map, Pos};
use crate::types::{MoveType, TerrainKind, UnitType, Weather};

pub type PlayerId = u8;
pub type UnitId = u16;

/// Occupancy sentinel for "no unit here".
pub const NO_UNIT: UnitId = UnitId::MAX;

/// Capture counter a fresh property starts at; a capturing unit subtracts its
/// displayed HP each turn.
pub const CAPTURE_FULL: u8 = 20;

/// Transports hold at most two units (Lander, Cruiser, Carrier, Black Boat);
/// APC and T-Copter hold one.
pub const MAX_CARGO: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub id: UnitId,
    pub typ: UnitType,
    pub owner: PlayerId,
    pub pos: Pos,
    /// Internal HP on AWBW's 0..=100 scale; displayed HP is `ceil(hp100/10)`.
    pub hp100: u8,
    pub fuel: u8,
    pub ammo: u8,
    /// Already acted this turn.
    pub moved: bool,
    /// Dived sub or hidden stealth.
    pub hidden: bool,
    /// Set while riding inside a transport; such units are off the board.
    pub carried_by: Option<UnitId>,
    pub cargo: [UnitId; MAX_CARGO],
}

impl Unit {
    pub fn new(id: UnitId, typ: UnitType, owner: PlayerId, pos: Pos) -> Self {
        let stats = typ.stats();
        Unit {
            id,
            typ,
            owner,
            pos,
            hp100: 100,
            fuel: stats.max_fuel,
            ammo: stats.max_ammo,
            moved: false,
            hidden: false,
            carried_by: None,
            cargo: [NO_UNIT; MAX_CARGO],
        }
    }

    /// Displayed HP, 1..=10. This is what capture progress and the damage
    /// formula both use.
    #[inline]
    pub fn display_hp(&self) -> u8 {
        (self.hp100 + 9) / 10
    }

    #[inline]
    pub fn move_type(&self) -> MoveType {
        self.typ.stats().move_type
    }

    #[inline]
    pub fn cargo_iter(&self) -> impl Iterator<Item = UnitId> + '_ {
        self.cargo.iter().copied().filter(|&c| c != NO_UNIT)
    }

    #[inline]
    pub fn cargo_len(&self) -> usize {
        self.cargo_iter().count()
    }

    /// Fuel burnt at the start of the owner's turn. Dived subs and hidden
    /// stealths burn the higher rate.
    #[inline]
    pub fn fuel_upkeep(&self) -> u8 {
        let stats = self.typ.stats();
        if self.hidden {
            stats.fuel_per_turn_hidden
        } else {
            stats.fuel_per_turn
        }
    }

    /// Air and sea units crash when they run dry; land units merely strand.
    #[inline]
    pub fn crashes_without_fuel(&self) -> bool {
        matches!(
            self.move_type(),
            MoveType::Air | MoveType::Sea | MoveType::Lander
        )
    }
}

/// How many units a transport can carry, and what it accepts.
pub fn cargo_capacity(typ: UnitType) -> usize {
    match typ {
        UnitType::Apc | UnitType::TCopter => 1,
        UnitType::Lander | UnitType::BlackBoat | UnitType::Cruiser | UnitType::Carrier => 2,
        _ => 0,
    }
}

/// Whether `transport` accepts a unit of type `cargo`.
pub fn can_carry(transport: UnitType, cargo: UnitType) -> bool {
    match transport {
        // Foot soldiers only.
        UnitType::Apc | UnitType::TCopter | UnitType::BlackBoat => {
            matches!(cargo.stats().move_type, MoveType::Foot | MoveType::Boot)
        }
        // Any land unit.
        UnitType::Lander => matches!(
            cargo.stats().move_type,
            MoveType::Foot | MoveType::Boot | MoveType::Tread | MoveType::Tires
        ),
        // Copters only.
        UnitType::Cruiser => matches!(cargo, UnitType::BCopter | UnitType::TCopter),
        // Any air unit.
        UnitType::Carrier => cargo.stats().move_type == MoveType::Air,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Building {
    pub pos: Pos,
    pub kind: TerrainKind,
    pub owner: Option<PlayerId>,
    /// Counts down from `CAPTURE_FULL`; resets when capturing is interrupted.
    pub capture_remaining: u8,
}

impl Building {
    /// Whether this property can produce the given unit type for its owner.
    pub fn can_produce(&self, typ: UnitType) -> bool {
        let mt = typ.stats().move_type;
        match self.kind {
            TerrainKind::Base => matches!(
                mt,
                MoveType::Foot | MoveType::Boot | MoveType::Tread | MoveType::Tires | MoveType::Pipe
            ),
            TerrainKind::Airport => mt == MoveType::Air,
            TerrainKind::Port => matches!(mt, MoveType::Sea | MoveType::Lander),
            _ => false,
        }
    }

    /// Whether this property repairs and resupplies the given movement class.
    pub fn repairs(&self, mt: MoveType) -> bool {
        match self.kind {
            TerrainKind::City
            | TerrainKind::Base
            | TerrainKind::Hq
            | TerrainKind::ComTower
            | TerrainKind::Lab => matches!(
                mt,
                MoveType::Foot | MoveType::Boot | MoveType::Tread | MoveType::Tires | MoveType::Pipe
            ),
            TerrainKind::Airport => mt == MoveType::Air,
            TerrainKind::Port => matches!(mt, MoveType::Sea | MoveType::Lander),
            _ => false,
        }
    }
}

/// Which of a player's CO powers is running right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePower {
    #[default]
    None,
    Cop,
    Scop,
}

/// The power meter's units: displayed-HP damage priced in funds, x10 — what
/// AWBW's server records. One base star is [`STAR_CHARGE`] of these; dealing
/// 5 HP to an infantry banks 2,500 for the dealer (half rate) and 5,000 for
/// the victim. Decoded from the corpus in
/// `docs/log/2026-08-26-adder-powers-phase0.md`.
pub const STAR_CHARGE: u32 = 90_000;
/// Each activation raises every star by a fifth of its base cost...
pub const STAR_CHARGE_STEP: u32 = STAR_CHARGE / 5;
/// ...until ten activations in, where it settles at triple.
pub const STAR_COST_ESCALATIONS: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct Player {
    pub funds: u32,
    pub team: u8,
    pub eliminated: bool,
    /// Day-to-day ability. Defaults to the ability-free CO, which is what
    /// self-play uses; the replay harness sets the real one.
    pub co: &'static CoData,
    /// Power meter, in [`STAR_CHARGE`] units. Charges from combat, runs past
    /// the COP threshold toward the SCOP, and activation subtracts the cost
    /// and keeps the leftover.
    pub charge: u32,
    pub active_power: ActivePower,
    /// Lifetime activations, for the star-cost escalation.
    pub power_uses: u32,
}

impl Player {
    pub fn new(funds: u32, team: u8) -> Self {
        Player {
            funds,
            team,
            eliminated: false,
            co: &CoData::VANILLA,
            charge: 0,
            active_power: ActivePower::None,
            power_uses: 0,
        }
    }

    pub fn with_co(mut self, co: &'static CoData) -> Self {
        self.co = co;
        self
    }

    /// What one star costs this player now, after escalation.
    pub fn star_cost(&self) -> u32 {
        STAR_CHARGE + STAR_CHARGE_STEP * self.power_uses.min(STAR_COST_ESCALATIONS)
    }

    /// Full cost of the given power, or `None` if this CO lacks it.
    pub fn power_cost(&self, kind: ActivePower) -> Option<u32> {
        let stars = match kind {
            ActivePower::None => return None,
            ActivePower::Cop => self.co.cop_stars,
            ActivePower::Scop => self.co.scop_stars,
        };
        (stars >= 0).then(|| stars as u32 * self.star_cost())
    }

    /// How full the bar reads, 0..=1 against the COP threshold — the same
    /// normalisation the recorded games use, kept for the observation.
    pub fn charge_fraction(&self) -> f32 {
        match self.power_cost(ActivePower::Cop) {
            Some(cost) if cost > 0 => (self.charge as f32 / cost as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

/// Per-game settings that vary between AWBW game types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSettings {
    /// Funds per income property per day. 1000 is standard; High Funds games
    /// raise it.
    pub funds_per_property: u32,
    /// Properties one player must hold to win, if the game sets a limit.
    pub capture_limit: Option<u16>,
    /// AWBW's per-player unit cap.
    pub unit_limit: u16,
    pub fog: bool,
    /// HP restored per turn on a repairing property, on the 0..=100 scale.
    pub repair_hp100: u8,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            funds_per_property: 1000,
            capture_limit: None,
            unit_limit: 50,
            fog: false,
            repair_hp100: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    InProgress,
    /// Every surviving player is on this team.
    Winner(PlayerId),
    Draw,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub map: Arc<Map>,
    pub settings: GameSettings,
    /// Slot-indexed; `None` is a destroyed unit whose slot may be reused.
    units: Vec<Option<Unit>>,
    /// Tile -> unit id, `NO_UNIT` when empty. Carried units are not on it.
    occupancy: Vec<UnitId>,
    buildings: Vec<Building>,
    /// Tile -> index into `buildings`, `u16::MAX` when the tile has none.
    building_at: Vec<u16>,
    pub players: Vec<Player>,
    pub current: PlayerId,
    pub day: u16,
    pub weather: Weather,
}

const NO_BUILDING: u16 = u16::MAX;

impl GameState {
    /// A fresh game: properties take their owners from `owners`, keyed by the
    /// same order as `map.properties()`.
    pub fn new(
        map: Arc<Map>,
        settings: GameSettings,
        players: Vec<Player>,
        property_owners: &[Option<PlayerId>],
    ) -> Self {
        let tiles = map.tile_count();
        let mut buildings = Vec::with_capacity(map.properties().len());
        let mut building_at = vec![NO_BUILDING; tiles];
        for (i, seed) in map.properties().iter().enumerate() {
            building_at[map.index(seed.pos)] = buildings.len() as u16;
            buildings.push(Building {
                pos: seed.pos,
                kind: seed.kind,
                owner: property_owners.get(i).copied().flatten(),
                capture_remaining: CAPTURE_FULL,
            });
        }

        GameState {
            map,
            settings,
            units: Vec::new(),
            occupancy: vec![NO_UNIT; tiles],
            buildings,
            building_at,
            players,
            current: 0,
            day: 1,
            weather: Weather::Clear,
        }
    }

    // --- units -------------------------------------------------------------

    #[inline]
    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.get(id as usize).and_then(|u| u.as_ref())
    }

    #[inline]
    pub fn unit_mut(&mut self, id: UnitId) -> Option<&mut Unit> {
        self.units.get_mut(id as usize).and_then(|u| u.as_mut())
    }

    #[inline]
    pub fn unit_at(&self, pos: Pos) -> Option<&Unit> {
        match self.occupancy[self.map.index(pos)] {
            NO_UNIT => None,
            id => self.unit(id),
        }
    }

    #[inline]
    pub fn unit_id_at(&self, pos: Pos) -> Option<UnitId> {
        match self.occupancy[self.map.index(pos)] {
            NO_UNIT => None,
            id => Some(id),
        }
    }

    /// Raw occupancy by tile index, for hot loops that already have one.
    #[inline]
    pub fn occupancy_at(&self, index: usize) -> UnitId {
        self.occupancy[index]
    }

    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter_map(|u| u.as_ref())
    }

    pub fn units_of(&self, player: PlayerId) -> impl Iterator<Item = &Unit> {
        self.units().filter(move |u| u.owner == player)
    }

    /// Units a player owns that count against the unit cap, i.e. including
    /// those riding in transports.
    pub fn unit_count(&self, player: PlayerId) -> u16 {
        self.units_of(player).count() as u16
    }

    fn alloc_slot(&mut self) -> UnitId {
        match self.units.iter().position(|u| u.is_none()) {
            Some(slot) => slot as UnitId,
            None => {
                self.units.push(None);
                (self.units.len() - 1) as UnitId
            }
        }
    }

    /// Places a new unit on the board and returns its id.
    pub fn spawn(&mut self, typ: UnitType, owner: PlayerId, pos: Pos) -> UnitId {
        let id = self.alloc_slot();
        let unit = Unit::new(id, typ, owner, pos);
        self.units[id as usize] = Some(unit);
        self.occupancy[self.map.index(pos)] = id;
        id
    }

    /// Creates a unit that starts inside a transport, off the board.
    ///
    /// Loading an existing unit would need it to stand somewhere first, which a
    /// state being reconstructed from a saved snapshot has no room for.
    pub fn spawn_into(
        &mut self,
        typ: UnitType,
        owner: PlayerId,
        transport_id: UnitId,
    ) -> Option<UnitId> {
        let transport = self.unit(transport_id).copied()?;
        let slot = transport.cargo.iter().position(|&c| c == NO_UNIT)?;
        let id = self.alloc_slot();
        let mut unit = Unit::new(id, typ, owner, transport.pos);
        unit.carried_by = Some(transport_id);
        self.units[id as usize] = Some(unit);
        self.unit_mut(transport_id).unwrap().cargo[slot] = id;
        Some(id)
    }

    /// Removes a unit and everything it was carrying.
    pub fn destroy(&mut self, id: UnitId) {
        let Some(unit) = self.unit(id).copied() else {
            return;
        };
        for cargo in unit.cargo_iter().collect::<Vec<_>>() {
            self.units[cargo as usize] = None;
        }
        if unit.carried_by.is_none() {
            let tile = self.map.index(unit.pos);
            if self.occupancy[tile] == id {
                self.occupancy[tile] = NO_UNIT;
            }
            // A unit dying on a property it was capturing releases the counter.
            self.reset_capture_at(unit.pos);
        } else if let Some(transport) = unit.carried_by {
            if let Some(t) = self.unit_mut(transport) {
                for slot in t.cargo.iter_mut() {
                    if *slot == id {
                        *slot = NO_UNIT;
                    }
                }
            }
        }
        self.units[id as usize] = None;
    }

    /// Moves a unit between tiles, keeping occupancy in step.
    pub fn relocate(&mut self, id: UnitId, to: Pos) {
        let Some(unit) = self.unit(id).copied() else {
            return;
        };
        let from_tile = self.map.index(unit.pos);
        if self.occupancy[from_tile] == id {
            self.occupancy[from_tile] = NO_UNIT;
        }
        if unit.pos != to {
            // Walking off a property abandons any capture in progress.
            self.reset_capture_at(unit.pos);
        }
        self.occupancy[self.map.index(to)] = id;
        if let Some(u) = self.unit_mut(id) {
            u.pos = to;
        }
    }

    /// Loads `unit` into `transport`, taking it off the board.
    pub fn load_into(&mut self, unit_id: UnitId, transport_id: UnitId) -> bool {
        let Some(unit) = self.unit(unit_id).copied() else {
            return false;
        };
        let Some(transport) = self.unit(transport_id).copied() else {
            return false;
        };
        let Some(slot) = transport.cargo.iter().position(|&c| c == NO_UNIT) else {
            return false;
        };
        if transport.cargo_len() >= cargo_capacity(transport.typ) {
            return false;
        }

        let tile = self.map.index(unit.pos);
        if self.occupancy[tile] == unit_id {
            self.occupancy[tile] = NO_UNIT;
        }
        self.reset_capture_at(unit.pos);
        self.unit_mut(transport_id).unwrap().cargo[slot] = unit_id;
        let u = self.unit_mut(unit_id).unwrap();
        u.carried_by = Some(transport_id);
        u.pos = transport.pos;
        true
    }

    /// Puts a carried unit back on the board at `to`.
    pub fn unload_to(&mut self, transport_id: UnitId, cargo_id: UnitId, to: Pos) -> bool {
        let Some(transport) = self.unit(transport_id).copied() else {
            return false;
        };
        if !transport.cargo.contains(&cargo_id) {
            return false;
        }
        if self.occupancy[self.map.index(to)] != NO_UNIT {
            return false;
        }
        let t = self.unit_mut(transport_id).unwrap();
        for slot in t.cargo.iter_mut() {
            if *slot == cargo_id {
                *slot = NO_UNIT;
            }
        }
        let cargo = self.unit_mut(cargo_id).unwrap();
        cargo.carried_by = None;
        cargo.pos = to;
        // Unloaded units cannot act again this turn.
        cargo.moved = true;
        self.occupancy[self.map.index(to)] = cargo_id;
        true
    }

    // --- buildings ---------------------------------------------------------

    #[inline]
    pub fn building_at(&self, pos: Pos) -> Option<&Building> {
        match self.building_at[self.map.index(pos)] {
            NO_BUILDING => None,
            i => self.buildings.get(i as usize),
        }
    }

    #[inline]
    pub fn building_at_mut(&mut self, pos: Pos) -> Option<&mut Building> {
        match self.building_at[self.map.index(pos)] {
            NO_BUILDING => None,
            i => self.buildings.get_mut(i as usize),
        }
    }

    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    pub fn buildings_of(&self, player: PlayerId) -> impl Iterator<Item = &Building> {
        self.buildings.iter().filter(move |b| b.owner == Some(player))
    }

    pub fn property_count(&self, player: PlayerId) -> u16 {
        self.buildings_of(player)
            .filter(|b| b.kind.produces_income())
            .count() as u16
    }

    pub fn income(&self, player: PlayerId) -> u32 {
        let per_property =
            self.settings.funds_per_property + self.players[player as usize].co.property_fund_bonus;
        self.property_count(player) as u32 * per_property
    }

    /// The day-to-day ability of a player's CO.
    #[inline]
    pub fn co_of(&self, player: PlayerId) -> &'static CoData {
        self.players[player as usize].co
    }

    /// +10% attack per Com Tower the player owns, applied in the damage formula.
    pub fn com_tower_bonus(&self, player: PlayerId) -> i32 {
        self.buildings_of(player)
            .filter(|b| b.kind == TerrainKind::ComTower)
            .count() as i32
            * 10
    }

    /// The power running for `player` right now, if any.
    pub fn active_power(&self, player: PlayerId) -> ActivePower {
        self.players[player as usize].active_power
    }

    /// Movement every unit of `player` gains from a running power (Adder).
    pub fn power_move_bonus(&self, player: PlayerId) -> i32 {
        let p = &self.players[player as usize];
        match p.active_power {
            ActivePower::None => 0,
            ActivePower::Cop => p.co.cop_move_bonus as i32,
            ActivePower::Scop => p.co.scop_move_bonus as i32,
        }
    }

    /// Whether `player` may fire this power: the CO has it, nothing is
    /// already running, and the bar covers the cost.
    pub fn can_activate_power(&self, player: PlayerId, kind: ActivePower) -> bool {
        let p = &self.players[player as usize];
        p.active_power == ActivePower::None
            && p.power_cost(kind).is_some_and(|cost| p.charge >= cost)
    }

    /// Fires a power: the cost comes off the bar — the leftover stays, the
    /// recorded games show it kept to the digit — and future stars cost more.
    /// The effect runs through the opponent's turn and expires when this
    /// player's next one begins.
    pub fn activate_power(&mut self, player: PlayerId, kind: ActivePower) -> bool {
        if !self.can_activate_power(player, kind) {
            return false;
        }
        let p = &mut self.players[player as usize];
        let cost = p.power_cost(kind).expect("checked by can_activate_power");
        p.charge -= cost;
        p.active_power = kind;
        p.power_uses += 1;
        true
    }

    /// Banks combat charge, in [`STAR_CHARGE`] units. Nothing accrues while
    /// the player's own power runs — the corpus shows the bar frozen through
    /// the opponent's turn — and the bar stops at the SCOP cost (assumed;
    /// replay verification will check the cap).
    pub fn add_combat_charge(&mut self, player: PlayerId, units: u32) {
        let p = &mut self.players[player as usize];
        if p.active_power != ActivePower::None {
            return;
        }
        let cap = p
            .power_cost(ActivePower::Scop)
            .or_else(|| p.power_cost(ActivePower::Cop))
            .unwrap_or(0);
        p.charge = (p.charge + units).min(cap);
    }

    fn reset_capture_at(&mut self, pos: Pos) {
        if let Some(b) = self.building_at_mut(pos) {
            b.capture_remaining = CAPTURE_FULL;
        }
    }

    // --- players and turns -------------------------------------------------

    #[inline]
    pub fn player(&self, id: PlayerId) -> &Player {
        &self.players[id as usize]
    }

    #[inline]
    pub fn are_allied(&self, a: PlayerId, b: PlayerId) -> bool {
        a == b || self.players[a as usize].team == self.players[b as usize].team
    }

    #[inline]
    pub fn are_enemies(&self, a: PlayerId, b: PlayerId) -> bool {
        !self.are_allied(a, b)
    }

    /// Ends the current player's turn and runs the next player's turn-start
    /// bookkeeping: income, repair and resupply, then fuel upkeep.
    pub fn end_turn(&mut self) {
        let start = self.current;
        let count = self.players.len() as PlayerId;
        let mut next = self.current;
        loop {
            next = (next + 1) % count;
            if next <= start && next != start {
                // Wrapped past the last seat: a new day begins.
            }
            if !self.players[next as usize].eliminated || next == start {
                break;
            }
        }
        if next <= start {
            self.day += 1;
        }
        self.current = next;
        self.begin_turn();
    }

    /// Turn-start bookkeeping for `self.current`.
    pub fn begin_turn(&mut self) {
        let player = self.current;
        // A power fired last turn has now covered the opponent's whole turn:
        // it expires here, which is where the recorded games clear it.
        self.players[player as usize].active_power = ActivePower::None;
        self.players[player as usize].funds += self.income(player);

        let owned: Vec<(Pos, TerrainKind)> = self
            .buildings_of(player)
            .map(|b| (b.pos, b.kind))
            .collect();
        for (pos, _) in owned {
            let Some(id) = self.unit_id_at(pos) else {
                continue;
            };
            let Some(unit) = self.unit(id).copied() else {
                continue;
            };
            if unit.owner != player {
                continue;
            }
            let repairs = self
                .building_at(pos)
                .map(|b| b.repairs(unit.move_type()))
                .unwrap_or(false);
            if repairs {
                self.repair_and_resupply(id);
            }
        }

        self.apply_apc_supply(player);
        self.apply_fuel_upkeep(player);

        for unit in self.units.iter_mut().flatten() {
            if unit.owner == player {
                unit.moved = false;
            }
        }
    }

    /// Heals up to `repair_hp100` and tops off fuel and ammo.
    ///
    /// Healing is charged per displayed HP gained, and a player who cannot
    /// afford the full amount heals as much as they can pay for. Both the
    /// truncating division and the raw (not display-rounded) HP addition match
    /// DefendPeace's `HealUnitEvent.healAtCost`, so a unit at 5.5 HP repairs to
    /// 7.5, not to 8. Resupply is free regardless of funds.
    fn repair_and_resupply(&mut self, id: UnitId) {
        let Some(unit) = self.unit(id).copied() else {
            return;
        };
        let stats = unit.typ.stats();
        // Rachel's units mend an extra point, and pay for it like any other.
        let rate = self.settings.repair_hp100 + self.co_of(unit.owner).repair_bonus_hp100;
        let wanted = rate.min(100 - unit.hp100);

        let funds = self.players[unit.owner as usize].funds;
        let per_display_hp = stats.cost / 10;
        let healed = if per_display_hp == 0 {
            wanted
        } else {
            // Funds buy whole displayed HP, i.e. multiples of 10 on this scale.
            (((funds / per_display_hp) * 10) as u16).min(wanted as u16) as u8
        };
        // A fractional remainder below one displayed HP costs nothing.
        self.players[unit.owner as usize].funds -= (healed as u32 / 10) * per_display_hp;

        let u = self.unit_mut(id).unwrap();
        u.hp100 += healed;
        u.fuel = stats.max_fuel;
        u.ammo = stats.max_ammo;
    }

    /// APCs resupply adjacent allied units at the start of their owner's turn.
    fn apply_apc_supply(&mut self, player: PlayerId) {
        let apcs: Vec<Pos> = self
            .units_of(player)
            .filter(|u| u.typ == UnitType::Apc && u.carried_by.is_none())
            .map(|u| u.pos)
            .collect();
        for pos in apcs {
            let neighbors: Vec<Pos> = self.map.neighbors(pos).collect();
            for n in neighbors {
                let Some(id) = self.unit_id_at(n) else {
                    continue;
                };
                let Some(unit) = self.unit(id).copied() else {
                    continue;
                };
                if !self.are_allied(player, unit.owner) {
                    continue;
                }
                let stats = unit.typ.stats();
                let u = self.unit_mut(id).unwrap();
                u.fuel = stats.max_fuel;
                u.ammo = stats.max_ammo;
            }
        }
    }

    fn apply_fuel_upkeep(&mut self, player: PlayerId) {
        let co = self.co_of(player);
        let mut crashed = Vec::new();
        for unit in self.units.iter_mut().flatten() {
            if unit.owner != player {
                continue;
            }
            let mut upkeep = unit.fuel_upkeep();
            // Eagle's air units burn less fuel per day.
            if unit.move_type() == MoveType::Air {
                upkeep = upkeep.saturating_sub(co.air_fuel_decrease);
            }
            unit.fuel = unit.fuel.saturating_sub(upkeep);
            if unit.fuel == 0 && unit.crashes_without_fuel() {
                crashed.push(unit.id);
            }
        }
        for id in crashed {
            self.destroy(id);
        }
    }

    /// Marks a player out and clears their units; their properties revert to
    /// neutral unless a captor is given (an HQ capture hands them over).
    pub fn eliminate(&mut self, player: PlayerId, captor: Option<PlayerId>) {
        self.players[player as usize].eliminated = true;
        let ids: Vec<UnitId> = self.units_of(player).map(|u| u.id).collect();
        for id in ids {
            self.destroy(id);
        }
        for building in self.buildings.iter_mut() {
            if building.owner == Some(player) {
                building.owner = captor;
                building.capture_remaining = CAPTURE_FULL;
            }
        }
    }

    /// A player with neither units nor a way to build more is out.
    pub fn check_eliminations(&mut self) {
        let ids: Vec<PlayerId> = (0..self.players.len() as PlayerId).collect();
        for p in ids {
            if self.players[p as usize].eliminated {
                continue;
            }
            let has_units = self.units_of(p).next().is_some();
            let has_production = self
                .buildings_of(p)
                .any(|b| matches!(b.kind, TerrainKind::Base | TerrainKind::Airport | TerrainKind::Port));
            if !has_units && !has_production {
                self.eliminate(p, None);
            }
        }
    }

    pub fn outcome(&self) -> Outcome {
        let alive: Vec<PlayerId> = (0..self.players.len() as PlayerId)
            .filter(|&p| !self.players[p as usize].eliminated)
            .collect();
        match alive.len() {
            0 => Outcome::Draw,
            _ => {
                if let Some(limit) = self.settings.capture_limit {
                    for &p in &alive {
                        if self.property_count(p) >= limit {
                            return Outcome::Winner(p);
                        }
                    }
                }
                let team = self.players[alive[0] as usize].team;
                if alive.iter().all(|&p| self.players[p as usize].team == team) {
                    Outcome::Winner(alive[0])
                } else {
                    Outcome::InProgress
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn two_player_state() -> GameState {
        // 5x1: OS base, plain, neutral city, plain, BM base.
        let map = Arc::new(
            Map::from_awbw_ids(5, 1, &[39, 1, 34, 1, 44]).unwrap(),
        );
        let players = vec![
            Player::new(5000, 1),
            Player::new(5000, 2),
        ];
        // properties() is in row-major order: OS base, city, BM base.
        GameState::new(map, GameSettings::default(), players, &[Some(0), None, Some(1)])
    }

    #[test]
    fn power_meter_charges_activates_and_escalates() {
        let mut state = two_player_state();
        state.players[0].co = crate::co_data::co_by_name("Adder").unwrap();

        // The vanilla default has no powers, so its bar never fills.
        state.add_combat_charge(1, 50_000);
        assert_eq!(state.players[1].charge, 0);

        // Two base stars fire the COP; the leftover stays on the bar.
        state.add_combat_charge(0, 200_000);
        assert!(state.can_activate_power(0, ActivePower::Cop));
        assert!(!state.can_activate_power(0, ActivePower::Scop));
        assert!(state.activate_power(0, ActivePower::Cop));
        assert_eq!(state.players[0].charge, 20_000);
        assert_eq!(state.active_power(0), ActivePower::Cop);
        assert_eq!(state.power_move_bonus(0), 1);

        // Nothing accrues while the power runs, and nothing double-fires.
        state.add_combat_charge(0, 50_000);
        assert_eq!(state.players[0].charge, 20_000);
        assert!(!state.activate_power(0, ActivePower::Cop));

        // One use in, each star costs a fifth more: the COP is now 216,000.
        assert_eq!(state.players[0].power_cost(ActivePower::Cop), Some(216_000));

        // The power expires when its owner's next turn begins.
        state.current = 0;
        state.begin_turn();
        assert_eq!(state.active_power(0), ActivePower::None);

        // The bar stops at the SCOP cost — five escalated stars, 540,000 —
        // and the SCOP spends all of it.
        state.add_combat_charge(0, 10_000_000);
        assert_eq!(state.players[0].charge, 540_000);
        assert!(state.activate_power(0, ActivePower::Scop));
        assert_eq!(state.players[0].charge, 0);
        assert_eq!(state.power_move_bonus(0), 2);
    }

    #[test]
    fn spawn_and_destroy_track_occupancy() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        assert_eq!(state.unit_id_at(Pos::new(1, 0)), Some(id));
        assert_eq!(state.unit_count(0), 1);
        state.destroy(id);
        assert!(state.unit_at(Pos::new(1, 0)).is_none());
        assert_eq!(state.unit_count(0), 0);
    }

    #[test]
    fn destroyed_slots_are_reused() {
        let mut state = two_player_state();
        let a = state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        state.destroy(a);
        let b = state.spawn(UnitType::Mech, 0, Pos::new(3, 0));
        assert_eq!(a, b);
        assert_eq!(state.unit(b).unwrap().typ, UnitType::Mech);
    }

    #[test]
    fn transports_carry_and_release() {
        let mut state = two_player_state();
        let apc = state.spawn(UnitType::Apc, 0, Pos::new(1, 0));
        let inf = state.spawn(UnitType::Infantry, 0, Pos::new(3, 0));
        assert!(state.load_into(inf, apc));
        assert!(state.unit_at(Pos::new(3, 0)).is_none());
        assert_eq!(state.unit(inf).unwrap().carried_by, Some(apc));
        assert_eq!(state.unit(apc).unwrap().cargo_len(), 1);
        // Capacity of one.
        let inf2 = state.spawn(UnitType::Infantry, 0, Pos::new(3, 0));
        assert!(!state.load_into(inf2, apc));

        assert!(state.unload_to(apc, inf, Pos::new(0, 0)));
        assert_eq!(state.unit_id_at(Pos::new(0, 0)), Some(inf));
        assert_eq!(state.unit(apc).unwrap().cargo_len(), 0);
    }

    #[test]
    fn destroying_a_transport_kills_its_cargo() {
        let mut state = two_player_state();
        let lander = state.spawn(UnitType::Lander, 0, Pos::new(1, 0));
        let a = state.spawn(UnitType::Infantry, 0, Pos::new(3, 0));
        let b = state.spawn(UnitType::Mech, 0, Pos::new(0, 0));
        assert!(state.load_into(a, lander));
        assert!(state.load_into(b, lander));
        state.destroy(lander);
        assert!(state.unit(a).is_none());
        assert!(state.unit(b).is_none());
    }

    #[test]
    fn cargo_rules_match_awbw() {
        assert!(can_carry(UnitType::Apc, UnitType::Infantry));
        assert!(!can_carry(UnitType::Apc, UnitType::Tank));
        assert!(can_carry(UnitType::Lander, UnitType::Tank));
        assert!(!can_carry(UnitType::Lander, UnitType::Fighter));
        assert!(can_carry(UnitType::Cruiser, UnitType::BCopter));
        assert!(!can_carry(UnitType::Cruiser, UnitType::Fighter));
        assert!(can_carry(UnitType::Carrier, UnitType::Fighter));
        assert_eq!(cargo_capacity(UnitType::Apc), 1);
        assert_eq!(cargo_capacity(UnitType::Lander), 2);
        assert_eq!(cargo_capacity(UnitType::Tank), 0);
    }

    #[test]
    fn income_scales_with_properties() {
        let state = two_player_state();
        // Each side starts with one base.
        assert_eq!(state.income(0), 1000);
        assert_eq!(state.income(1), 1000);
    }

    #[test]
    fn turn_start_pays_income_and_clears_moved() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        state.unit_mut(id).unwrap().moved = true;
        let before = state.player(0).funds;
        state.current = 1;
        state.end_turn(); // back to player 0
        assert_eq!(state.current, 0);
        assert_eq!(state.player(0).funds, before + 1000);
        assert!(!state.unit(id).unwrap().moved);
    }

    #[test]
    fn units_repair_on_owned_properties() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(0, 0)); // OS base
        state.unit_mut(id).unwrap().hp100 = 50;
        state.players[0].funds = 5000;
        state.current = 0;
        state.begin_turn();
        assert_eq!(state.unit(id).unwrap().hp100, 70);
        // 2 HP of infantry at 100 funds each, on top of 1000 income.
        assert_eq!(state.player(0).funds, 5000 + 1000 - 200);
    }

    #[test]
    fn repair_adds_raw_hp_without_rounding_to_the_display_step() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        state.unit_mut(id).unwrap().hp100 = 55; // displays as 6
        state.current = 0;
        state.begin_turn();
        assert_eq!(state.unit(id).unwrap().hp100, 75); // not 80
    }

    #[test]
    fn a_player_who_cannot_pay_heals_partially() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        state.unit_mut(id).unwrap().hp100 = 50;
        // Tank costs 7000, so 700 per displayed HP. Income is 1000, so give
        // them nothing up front: they can afford exactly one HP.
        state.players[0].funds = 0;
        state.current = 0;
        state.begin_turn();
        assert_eq!(state.unit(id).unwrap().hp100, 60);
        assert_eq!(state.player(0).funds, 1000 - 700);
    }

    #[test]
    fn repair_caps_at_full_health_and_the_remainder_is_free() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        state.unit_mut(id).unwrap().hp100 = 95;
        state.players[0].funds = 5000;
        state.current = 0;
        state.begin_turn();
        assert_eq!(state.unit(id).unwrap().hp100, 100);
        // Half a displayed HP rounds down to no charge.
        assert_eq!(state.player(0).funds, 5000 + 1000);
    }

    #[test]
    fn resupply_happens_even_with_no_funds() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        {
            let u = state.unit_mut(id).unwrap();
            u.hp100 = 50;
            u.ammo = 0;
            u.fuel = 3;
        }
        state.players[0].funds = 0;
        state.settings.funds_per_property = 0;
        state.current = 0;
        state.begin_turn();
        let unit = state.unit(id).unwrap();
        assert_eq!(unit.hp100, 50, "no funds, no healing");
        assert_eq!(unit.ammo, UnitType::Tank.stats().max_ammo);
        assert_eq!(unit.fuel, UnitType::Tank.stats().max_fuel);
    }

    #[test]
    fn air_units_crash_when_fuel_runs_out() {
        let mut state = two_player_state();
        let id = state.spawn(UnitType::BCopter, 0, Pos::new(1, 0));
        state.unit_mut(id).unwrap().fuel = 2; // burns 2/turn
        state.current = 0;
        state.begin_turn();
        assert!(state.unit(id).is_none());
    }

    #[test]
    fn hq_capture_eliminates_and_transfers() {
        let mut state = two_player_state();
        state.spawn(UnitType::Infantry, 1, Pos::new(4, 0));
        state.eliminate(1, Some(0));
        assert!(state.player(1).eliminated);
        assert_eq!(state.unit_count(1), 0);
        assert_eq!(state.building_at(Pos::new(4, 0)).unwrap().owner, Some(0));
        assert_eq!(state.outcome(), Outcome::Winner(0));
    }

    #[test]
    fn playerless_of_units_and_production_is_eliminated() {
        let mut state = two_player_state();
        // Strip player 1's base so they have neither units nor production.
        state.building_at_mut(Pos::new(4, 0)).unwrap().owner = None;
        state.check_eliminations();
        assert!(state.player(1).eliminated);
    }
}
