//! The Milky Way fixture's sky tables, built once for every test of the crate that reads them: a
//! full build of the luminosity tables takes a minute or more.

use std::sync::{Arc, OnceLock};

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::testing::milky_way_galaxy;
use crate::galaxy::gas::modifiers::NoModifiers;
use crate::galaxy::gas::noise::NoiseCache;
use crate::observe::Observer;
use crate::time::UniverseTime;

use super::census::{CellOffsets, NoSkyCellCache, SkyContext};
use super::dgl::Illumination;
use super::envelope::BrightnessEnvelope;
use super::luminosity::LuminosityTables;
use super::phase::PhaseEnvelope;

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

/// The phase envelope: the fitted table, which depends on no galaxy, as the census reads it
/// ([`PhaseEnvelope::shared`]).
pub(crate) fn phase_envelope() -> &'static PhaseEnvelope {
    PhaseEnvelope::shared()
}

/// The fixture's bounds on how far a cell's stars lie from their barycentres.
pub(crate) fn milky_way_offsets() -> &'static CellOffsets {
    static OFFSETS: OnceLock<CellOffsets> = OnceLock::new();
    OFFSETS.get_or_init(|| CellOffsets::build(milky_way_galaxy()))
}

/// The fixture's galaxy built with its kinematic tables, so that its systems move: built once for
/// the tests of the census in motion (R06.T8.f, R06.T8.j) and of its cell cache (R06.T8.h). Its
/// parameters are the fixture's, so [`milky_way_offsets`] serve it.
pub(crate) fn moving_galaxy() -> &'static Galaxy {
    static MOVING: OnceLock<Galaxy> = OnceLock::new();
    MOVING.get_or_init(|| milky_way_galaxy().clone().with_full_potential())
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

/// The Sun's place in the fixture, ly, as the sim's sky tests stand.
pub(crate) const SUN_LY: [f64; 3] = [0.0, 26_000.0, 68.0];

/// The observer at the Sun's place in the fixture, at the epoch.
pub(crate) fn sun_observer() -> Observer {
    let at = GalacticPosition::from_light_years(SUN_LY).expect("the Sun is in the cube");
    Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock")
}

/// A job's context on the fixture's tables, with a fresh noise cache.
pub(crate) fn milky_way_context() -> SkyContext<'static> {
    SkyContext {
        tables: milky_way_tables(),
        envelope: milky_way_envelope(),
        offsets: milky_way_offsets(),
        noise: NoiseCache::with_capacity(1 << 16),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: &NoModifiers,
    }
}

/// The illumination of the fixture's sky at the Sun's place at the epoch (R06.T9.g): built once
/// for every test that lights a band near the Sun.
pub(crate) fn sun_illumination() -> &'static Arc<Illumination> {
    static ILLUMINATION: OnceLock<Arc<Illumination>> = OnceLock::new();
    ILLUMINATION.get_or_init(|| {
        Arc::new(Illumination::march(
            milky_way_galaxy(),
            &mut milky_way_context(),
            &sun_observer(),
        ))
    })
}
