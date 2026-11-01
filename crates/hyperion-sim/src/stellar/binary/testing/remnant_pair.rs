//! A main-sequence donor at its Roche lobe beside a white dwarf or a neutron star, run through the
//! engine from the onset of its transfer (plan 11's test helpers): the single pairs P11.T4's engine
//! fixes measure a transfer on, built directly, since the grid's pairs come to such a state only
//! after a supernova or a common envelope.

use std::sync::Arc;

use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::binary::evolve::{Engine, LiveOrbit, roche_lobe};
use crate::stellar::binary::star::{Member, Path, cooling_origin};
use crate::stellar::binary::timeline::Context;
use crate::stellar::binary::{BinaryInput, BinaryTimeline};
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
use crate::stellar::{Composition, Phase};
use crate::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};

/// The pair's age at the onset, years: the remnant was born half of it before.
pub(crate) const ONSET_AGE: f64 = 1.0e9;

/// The timeline of a solar main-sequence donor of `donor` M☉ at fractional age `tau`, filling its
/// Roche lobe on a circular orbit about a remnant of `phase` and `accretor` M☉, under `rules`,
/// from [`ONSET_AGE`] for `years`; and the orbit's semi-major axis at the onset, R☉.
///
/// The pair starts in stable transfer from the donor ([`Engine::roche_onset`]); its remnant was
/// born at half the onset's age, a white dwarf with the cooling origin of its mass.
///
/// # Panics
///
/// If `donor` or `accretor` lies outside the stars' 0.08–150 M☉ (the input's masses), or the
/// donor has no radius.
#[must_use]
pub(crate) fn donor_onto_remnant(
    donor: f64,
    tau: f64,
    phase: Phase,
    accretor: f64,
    rules: StellarRules,
    years: f64,
) -> (BinaryTimeline, f64) {
    let comp = Composition::SOLAR;
    let placeholder = KeplerElements::from_semi_major_axis(
        Metres::new(1.0e10),
        GravitationalParameter::from_solar_masses(SolarMasses::new(donor + accretor)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).expect("zero angles"),
        Radians::ZERO,
    )
    .expect("a bound orbit");
    let input = BinaryInput::new(
        SolarMasses::new(donor),
        SolarMasses::new(accretor),
        comp,
        placeholder,
        [StarDraws::median(), StarDraws::median()],
        Years::new(ONSET_AGE + years),
    )
    .expect("a pair of stars' masses")
    .with_rules(rules);
    let ctx = Arc::new(Context::of(&input));
    let star = Member::MainSequence {
        helium: false,
        mass: Path::starting(ONSET_AGE, donor),
        tau: Path::starting(ONSET_AGE, tau),
    };
    let remnant = Member::Remnant {
        phase,
        birth: 0.5 * ONSET_AGE,
        origin: cooling_origin(&ctx, phase, accretor, None),
        mass: Path::starting(ONSET_AGE, accretor),
    };
    let radius = star
        .radius(&ctx, 0, ONSET_AGE, donor, tau)
        .expect("a main-sequence donor has a radius");
    let a = radius / roche_lobe(donor, accretor, 1.0);
    let orbit = LiveOrbit::new(
        ONSET_AGE,
        a,
        0.0,
        *placeholder.orientation(),
        placeholder.mean_anomaly_at_epoch(),
    );
    let mut engine = Engine::new(
        ctx,
        ONSET_AGE,
        ONSET_AGE + years,
        [star, remnant],
        orbit,
        None,
    );
    engine.roche_onset(0);
    engine.run();
    (engine.finish(), a)
}
