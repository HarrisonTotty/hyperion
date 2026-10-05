//! The Milky Way fixture's sky tables, built once for every test of the crate that reads them: a
//! full build of the luminosity tables takes a minute or more.

use std::sync::OnceLock;

use crate::galaxy::features::centre::testing::milky_way_galaxy;

use super::envelope::BrightnessEnvelope;
use super::luminosity::{BuildOptions, LuminosityTables, REFERENCE_TIME};

/// The fixture's luminosity tables, at the reference time.
pub(crate) fn milky_way_tables() -> &'static LuminosityTables {
    static TABLES: OnceLock<LuminosityTables> = OnceLock::new();
    TABLES.get_or_init(|| LuminosityTables::build(milky_way_galaxy()))
}

/// The fixture's brightness envelope: the fitted table, which every galaxy shares, so it costs a
/// copy of the table and builds no galaxy.
pub(crate) fn milky_way_envelope() -> &'static BrightnessEnvelope {
    static ENVELOPE: OnceLock<BrightnessEnvelope> = OnceLock::new();
    ENVELOPE.get_or_init(BrightnessEnvelope::fitted)
}

/// Tables of the fixture built for no component, for tests that need a [`super::census::SkyContext`]
/// but read no table: the census's own skips read only the envelope.
pub(crate) fn milky_way_dark_tables() -> &'static LuminosityTables {
    static TABLES: OnceLock<LuminosityTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        LuminosityTables::build_with(
            milky_way_galaxy(),
            REFERENCE_TIME,
            &[],
            BuildOptions::STANDARD,
        )
    })
}
