pub mod action;
pub mod ai;
pub mod building;
pub mod city;
pub mod civilization;
pub mod dynamic;
pub mod engine;
pub mod id;
pub mod mapgen;
pub mod pathfinding;
pub mod protocol;
pub mod scripting;
pub mod tech;
pub mod tile;
pub mod turn;
pub mod types;
pub mod unit;
pub mod unit_type;
pub mod world;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
