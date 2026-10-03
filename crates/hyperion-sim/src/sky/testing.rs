//! The Milky Way fixture's sky tables, built once for every test of the crate that reads them: a
//! full build takes a minute or more.

use std::sync::OnceLock;

use crate::galaxy::features::centre::testing::milky_way_galaxy;
use crate::time::UniverseTime;

use super::envelope::BrightnessEnvelope;
use super::luminosity::LuminosityTables;

/// The fixture's luminosity tables at the epoch.
pub(crate) fn milky_way_tables() -> &'static LuminosityTables {
    static TABLES: OnceLock<LuminosityTables> = OnceLock::new();
    TABLES.get_or_init(|| LuminosityTables::build(milky_way_galaxy(), UniverseTime::EPOCH))
}

/// The fixture's brightness envelope.
pub(crate) fn milky_way_envelope() -> &'static BrightnessEnvelope {
    static ENVELOPE: OnceLock<BrightnessEnvelope> = OnceLock::new();
    ENVELOPE.get_or_init(|| BrightnessEnvelope::build(milky_way_galaxy()))
}
