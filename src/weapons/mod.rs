//! Weapons module
//!
//! Handles weapon tag-based combat calculations and status effects.

pub mod tags;

pub use tags::{
    WeaponTagCalculator,
    DamageResult,
    StatusEffect,
    StatusEffectType,
};
