//! The engine's merger of an evolved donor with its companion, as the stellar merger class's
//! evolved donors read it (plan 11, P11.T8.c's channels, at 26): the onset of Roche-lobe overflow
//! from a donor at its lobe, which the engine's own stability test sends to a common envelope
//! (BSE sections 2.6.1 and 2.7.1–2.7.4, their table 2), run by the engine itself, so that the
//! class and the engine agree bit for bit.
//!
//! The pair is the engine's at the merger: each star on its own track at its initial mass, the
//! donor the primary, on the input's circular orbit at the lobe. The engine is built at the
//! merger's age and asked for one event, its `roche_onset`, which carries both stars, tests the
//! transfer's stability and, for an unstable giant, resolves the common envelope through the
//! helpers the engine's every common envelope calls (`envelope_outcome`, `coalesced_product`,
//! `Engine::product_member`). A pair is the class's only if that leaves one star: stable
//! transfer, cores that survive the envelope and an envelope that leaves nothing are none.

use std::sync::Arc;

use crate::stellar::sse::giant_radius_exponent;
use crate::stellar::{Composition, StarState};
use crate::units::Years;
use crate::units::consts::SOLAR_RADIUS_M;

use super::BinaryInput;
use super::evolve::{Engine, LiveOrbit, own_members};
use super::rlof::{CODE_Q, GAP_Q, giant_critical_ratio};
use super::star::Kind;
use super::timeline::{Context, SegmentKind};

/// The star the onset of transfer from `input`'s primary at `age` leaves, run by the engine as
/// [`onset_merger`] runs it, with each star's track built to `until` (years); `None` unless the
/// engine merges the pair there into one star.
#[must_use]
pub(crate) fn evolved_donor_merger(
    input: &BinaryInput,
    age: Years,
    until: Years,
) -> Option<StarState> {
    let (ctx, engine) = onset_merger(input, age, until)?;
    Some(engine.members[0].state_at(&ctx, 0, age.value()))
}

/// Whether the onset of transfer from `input`'s primary at `age` merges the pair into one star:
/// [`evolved_donor_merger`]'s test, by the same [`onset_merger`], without reading the star.
#[must_use]
pub(crate) fn evolved_donor_merges(input: &BinaryInput, age: Years, until: Years) -> bool {
    onset_merger(input, age, until).is_some()
}

/// The largest mass ratio, companion over donor's mass now, at which the onset of transfer from
/// `donor` is unstable in the engine (`Engine::stability`): the inverse of its critical ratio for
/// a Hertzsprung-gap donor (`GAP_Q`), a core-helium-burning one (`CODE_Q`) or a giant-branch and
/// asymptotic giant-branch one ([`giant_critical_ratio`] of its core's share, at `composition`'s
/// giant radius exponent); `None` for a donor of another kind. A giant whose lobe falls inside its
/// core is unstable at any ratio, which this leaves out, so a companion drawn inside it is always
/// sent to a common envelope and the engine's test still decides.
#[must_use]
pub(crate) fn unstable_companion_ratio(
    donor: &StarState,
    composition: &Composition,
) -> Option<f64> {
    let md = donor.mass().value();
    let critical = match Kind::of(donor.phase(), md) {
        Kind::HertzsprungGap => GAP_Q,
        Kind::CoreHeliumBurning => CODE_Q,
        Kind::GiantBranch | Kind::EarlyAgb | Kind::PulsingAgb => giant_critical_ratio(
            donor.core_mass().value() / md,
            giant_radius_exponent(composition),
        ),
        Kind::LowMassMainSequence
        | Kind::MainSequence
        | Kind::HeliumMainSequence
        | Kind::HeliumGap
        | Kind::HeliumGiant
        | Kind::HeliumWhiteDwarf
        | Kind::CarbonOxygenWhiteDwarf
        | Kind::OxygenNeonWhiteDwarf
        | Kind::NeutronStar
        | Kind::BlackHole
        | Kind::Massless => return None,
    };
    Some(1.0 / critical)
}

/// The engine of `input`'s pair after the onset of transfer from its primary at `age`, and its
/// context, if the onset merges the pair into one star: built at `age`, each star on its own track
/// to `until` (years), on the input's orbit, and run through its `roche_onset` alone.
#[must_use]
fn onset_merger(input: &BinaryInput, age: Years, until: Years) -> Option<(Arc<Context>, Engine)> {
    let (ctx, mut engine) = engine_at(input, age, until);
    engine.roche_onset(0);
    (engine.kind == SegmentKind::Merged && !engine.members[0].is_gone()).then_some((ctx, engine))
}

/// The engine of `input`'s pair built at `age`, each star on its own track to `until`, and its
/// context.
#[must_use]
fn engine_at(input: &BinaryInput, age: Years, until: Years) -> (Arc<Context>, Engine) {
    let (age, until) = (age.value(), until.value().max(age.value()));
    let ctx = Arc::new(Context::of(input));
    let members = own_members(input, until, None);
    let orbit = input.orbit();
    let live = LiveOrbit::new(
        age,
        orbit.semi_major_axis().value() / SOLAR_RADIUS_M,
        orbit.eccentricity().value(),
        *orbit.orientation(),
        orbit.mean_anomaly_at_epoch(),
    );
    let engine = Engine::new(Arc::clone(&ctx), age, until, members, live, None);
    (ctx, engine)
}

/// The star `input`'s pair leaves if, at `age`, its secondary transfers dynamically and the pair
/// merges (`Engine::merge_dynamically`), the engine built as [`onset_merger`] builds it, each star
/// on its own track to `until` (years): the tests' reach of a merger the grid's pairs seldom come
/// to, a low-mass main-sequence donor onto a Hertzsprung-gap accretor.
#[cfg(test)]
#[must_use]
pub(crate) fn secondary_dynamical_merger(
    input: &BinaryInput,
    age: Years,
    until: Years,
) -> StarState {
    let (ctx, mut engine) = engine_at(input, age, until);
    engine.merge_dynamically(1);
    engine.members[0].state_at(&ctx, 0, age.value())
}

/// The kinds of the segments the engine records at the onset of transfer from `input`'s primary
/// at `age`, and the kind it is in after: for an evolved donor the class keeps, a detached
/// history, a common envelope, then the merged star.
#[cfg(test)]
#[must_use]
pub(crate) fn onset_segment_kinds(
    input: &BinaryInput,
    age: Years,
    until: Years,
) -> (Vec<SegmentKind>, SegmentKind) {
    let (_, mut engine) = engine_at(input, age, until);
    engine.roche_onset(0);
    let kinds = engine
        .segments
        .iter()
        .map(super::timeline::Segment::kind)
        .collect();
    (kinds, engine.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::Track;
    use crate::stellar::{Composition, Phase};
    use crate::units::{GravitationalParameter, Metres, Radians, SolarMasses};

    /// A donor of `m1` M☉ halfway through `phase` beside a main-sequence companion of `m2` M☉,
    /// the donor filling its lobe on a circular orbit, solar, at the median draws; and its age.
    fn pair_at_lobe(m1: f64, m2: f64, phase: Phase) -> (BinaryInput, f64) {
        let comp = Composition::SOLAR;
        let draws = [StarDraws::median(), StarDraws::median()];
        let full = Track::to_age(SolarMasses::new(m1), &comp, &draws[0], Years::new(2.0e10));
        let age = full
            .age_in_phase(phase, 0.5)
            .expect("the track passes the phase");
        let donor = full.state_at(Years::new(age));
        assert_eq!(donor.phase(), phase);
        let lobe = roche_lobe_radius(m1 / m2, Metres::new(1.0)).value();
        let a = Metres::new(donor.radius().value() * SOLAR_RADIUS_M / lobe);
        let orbit = KeplerElements::from_semi_major_axis(
            a,
            GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
            Eccentricity::CIRCULAR,
            Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap();
        let input = BinaryInput::new(
            SolarMasses::new(m1),
            SolarMasses::new(m2),
            comp,
            orbit,
            draws,
            Years::new(age),
        )
        .unwrap();
        (input, age)
    }

    /// Under the version's rules and every 26 hold, for donors from the Hertzsprung gap to the
    /// early asymptotic giant branch beside light and heavy companions: the merge test and the
    /// star agree; a pair kept went through the engine's own common envelope at the onset, after a
    /// detached history, into the merged star; a companion inside [`unstable_companion_ratio`]
    /// always reaches the envelope; and the outcomes are pinned.
    #[test]
    fn the_class_s_merger_is_the_engine_s_onset_s() {
        use crate::stellar::rules::StellarRules;
        // (donor, companion, phase, whether the engine merges them).
        let cases = [
            (1.3, 0.25, Phase::HertzsprungGap, true),
            (1.3, 0.25, Phase::FirstGiantBranch, true),
            (2.5, 0.3, Phase::FirstGiantBranch, true),
            (5.0, 1.0, Phase::CoreHeliumBurning, true),
            (1.3, 1.0, Phase::FirstGiantBranch, true),
            (12.0, 3.0, Phase::HertzsprungGap, false),
            (2.0, 1.8, Phase::EarlyAgb, true),
            (5.0, 4.0, Phase::EarlyAgb, false),
        ];
        let version = StellarRules::of_version(crate::GENERATOR_VERSION);
        for rules in [version, StellarRules::every_26_hold()] {
            for (m1, m2, phase, merges) in cases {
                let (input, age) = pair_at_lobe(m1, m2, phase);
                let input = input.with_rules(rules);
                let (at, until) = (Years::new(age), Years::new(age + 2.0e3));
                let star = evolved_donor_merger(&input, at, until);
                let (kinds, after) = onset_segment_kinds(&input, at, until);
                println!("{m1} + {m2}, {phase:?}: {kinds:?} then {after:?}");
                assert_eq!(evolved_donor_merges(&input, at, until), star.is_some());
                assert_eq!(star.is_some(), merges, "{m1} + {m2}, {phase:?}");
                let enveloped = kinds.contains(&SegmentKind::CommonEnvelope);
                if star.is_some() {
                    assert!(enveloped, "{kinds:?}");
                    assert_eq!(kinds[0], SegmentKind::Detached);
                    assert_eq!(after, SegmentKind::Merged);
                }
                let donor = Track::to_age(
                    SolarMasses::new(m1),
                    &Composition::SOLAR,
                    &StarDraws::median(),
                    at,
                )
                .state_at(at);
                let ratio = unstable_companion_ratio(&donor, &Composition::SOLAR)
                    .expect("an evolved donor has a critical ratio");
                if m2 < ratio * donor.mass().value() {
                    assert!(enveloped, "{m1} + {m2}, {phase:?}: {kinds:?}");
                }
            }
        }
    }
}
