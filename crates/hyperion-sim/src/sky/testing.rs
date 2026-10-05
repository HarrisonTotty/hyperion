//! The Milky Way fixture's sky tables, built once for every test of the crate that reads them: a
//! full build of the luminosity tables takes a minute or more.

use std::sync::OnceLock;

use crate::galaxy::features::centre::testing::milky_way_galaxy;

use super::census::CellOffsets;
use super::envelope::BrightnessEnvelope;
use super::luminosity::LuminosityTables;

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

/// The fixture's bounds on how far a cell's stars lie from their barycentres.
pub(crate) fn milky_way_offsets() -> &'static CellOffsets {
    static OFFSETS: OnceLock<CellOffsets> = OnceLock::new();
    OFFSETS.get_or_init(|| CellOffsets::build(milky_way_galaxy()))
}

/// A fixed stream of uniform deviates in [0, 1), for the sky tests' random queries: `SplitMix64`
/// from `seed` (Vigna 2015, `splitmix64.c`: the golden gamma, then Stafford's 2011 Mix13), its top
/// 53 bits a deviate.
pub(crate) fn uniforms(seed: u64) -> impl Iterator<Item = f64> {
    let mut state = seed;
    std::iter::repeat_with(move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        #[expect(clippy::cast_precision_loss, reason = "53 bits, exact in f64")]
        let u = (z >> 11) as f64 / (1_u64 << 53) as f64;
        u
    })
}

/// Tables of the fixture built for no component, for tests that need a [`super::census::SkyContext`]
/// but read no table: the census's own skips read only the envelope.
pub(crate) fn milky_way_dark_tables() -> &'static LuminosityTables {
    static TABLES: OnceLock<LuminosityTables> = OnceLock::new();
    TABLES.get_or_init(|| LuminosityTables::dark(milky_way_galaxy()))
}
