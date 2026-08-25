//! The normalized replay format produced by `tools/prepare_replay.py`.
//!
//! Everything AWBW-specific and messy — zip, gzip, PHP serialization, the map
//! API — is handled on the Python side, so this is a plain flat schema.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Replay {
    pub game_id: i64,
    pub name: Option<String>,
    pub map_id: i64,
    pub map_name: Option<String>,
    pub width: u32,
    pub height: u32,
    /// Row-major `[y][x]` AWBW terrain ids.
    pub terrain: Vec<Vec<u16>>,
    pub fog: bool,
    pub funds_per_property: u32,
    pub starting_funds: u32,
    pub capture_limit: Option<u16>,
    pub weather: String,
    pub use_powers: bool,
    pub players: Vec<PlayerInfo>,
    pub turns: Vec<Turn>,
    /// Snapshots with no matching action record; those turns cannot be checked.
    #[serde(default)]
    pub unmatched_turns: usize,
}

#[derive(Debug, Deserialize)]
pub struct PlayerInfo {
    pub id: i64,
    pub order: i64,
    pub country: String,
    pub team: String,
    #[serde(default)]
    pub co_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Turn {
    pub day: u16,
    pub active: i64,
    /// Keyed by player id rendered as a string.
    pub funds: HashMap<String, i64>,
    pub eliminated: HashMap<String, bool>,
    #[serde(default)]
    pub co_power_on: HashMap<String, String>,
    pub units: Vec<UnitRec>,
    pub buildings: Vec<BuildingRec>,
    /// Raw AWBW action payloads, in the order they were played.
    pub actions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitRec {
    pub id: i64,
    #[serde(rename = "type")]
    pub typ: String,
    pub player: i64,
    pub x: u8,
    pub y: u8,
    /// AWBW's 0..=100 internal HP scale (its stored value times ten).
    pub hp100: i32,
    pub fuel: i32,
    pub ammo: i32,
    pub moved: bool,
    #[serde(default)]
    pub capture: i32,
    #[serde(default)]
    pub carried: bool,
    #[serde(default)]
    pub sub_dive: bool,
    #[serde(default)]
    pub cargo: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingRec {
    pub x: u8,
    pub y: u8,
    pub terrain_id: u16,
    pub capture: u8,
}

/// AWBW wraps many action payloads in a visibility map keyed by `"global"` (no
/// fog) or by player id (fog). Unwraps to the single meaningful value.
///
/// In fog games the player who could not see the action gets an entry too, but
/// it is an empty string rather than null, so "first non-null" picks the blind
/// seat's blank and loses the payload. Only a populated object or a non-empty
/// array counts as the real view.
pub fn unwrap_vision(value: &serde_json::Value) -> Option<&serde_json::Value> {
    fn is_meaningful(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(map) => !map.is_empty(),
            serde_json::Value::Array(items) => !items.is_empty(),
            serde_json::Value::Null => false,
            serde_json::Value::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    let obj = value.as_object()?;
    if let Some(global) = obj.get("global").filter(|v| is_meaningful(v)) {
        return Some(global);
    }
    obj.values().find(|v| is_meaningful(v))
}
