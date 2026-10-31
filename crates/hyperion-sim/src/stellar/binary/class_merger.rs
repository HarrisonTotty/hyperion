//! The engine's merger of an evolved donor with its companion, as the stellar merger class's
//! evolved donors read it (plan 11, P11.T8.c's channels, at 26): the common envelope the engine's
//! collision of a giant-like star starts (BSE sections 2.7.1–2.7.4, their table 2), through the
//! engine's own helpers, so that the class and the engine agree bit for bit.
//!
//! The pair is the engine's at the merger: each star on its own track at its initial mass, the
//! donor the primary, on the input's orbit. Nothing is stepped: the engine is built at the
//! merger's age and asked only what the envelope leaves ([`envelope_outcome`], then
//! [`coalesced_product`] and [`Engine::product_member`]), as `Engine::common_envelope` asks it.

use std::sync::Arc;

use crate::stellar::StarState;
use crate::units::Years;
use crate::units::consts::SOLAR_RADIUS_M;

use super::BinaryInput;
use super::common_envelope::{
    CoalescedProduct, EnvelopeOutcome, Side, coalesced_product, envelope_binding, envelope_outcome,
};
use super::evolve::{Engine, LiveOrbit, own_members};
use super::timeline::Context;

/// The star the merger of `input`'s pair at `age` leaves (P11.T8.c's evolved donors): the
/// common envelope around its primary, a giant-like donor, and its companion on the input's
/// orbit, resolved as the engine resolves one, with the engine's run to `until` (years) for the
/// product's track; `None` where the envelope leaves the cores apart, nothing is left, or a star
/// has no structure then.
///
/// The engine's own `common_envelope` reads the same helpers at the same structures, so a pair
/// the engine merges this way leaves the same star bit for bit.
#[must_use]
pub(crate) fn evolved_donor_merger(
    input: &BinaryInput,
    age: Years,
    until: Years,
) -> Option<StarState> {
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
    let product = engine.envelope_merger(0)?;
    let member = engine.product_member(product);
    (!member.is_gone()).then(|| member.state_at(&ctx, 0, age))
}

impl Engine {
    /// What a common envelope around member `d`, a giant-like donor, and its companion leaves if
    /// their cores coalesce, as [`Engine::common_envelope`] would resolve it now, with nothing
    /// changed; `None` if they survive it or a star has no structure.
    #[must_use]
    fn envelope_merger(&self, d: usize) -> Option<CoalescedProduct> {
        let o = 1 - d;
        let (md, td) = self.current(d);
        let (mo, to) = self.current(o);
        let sd = self.structure(d, self.age, md, td)?;
        let so = self.structure(o, self.age, mo, to)?;
        let a_i = self.orbit.as_ref().map_or(0.0, |orbit| orbit.a);
        let (s1, s2) = (Side::of(&sd), Side::of(&so));
        let params = *self.ctx.params();
        let binding = envelope_binding(&s1, &s2, params.lambda);
        match envelope_outcome(s1, s2, binding, a_i, &params) {
            EnvelopeOutcome::Survives { .. } => None,
            EnvelopeOutcome::Coalesces(remains) => {
                Some(coalesced_product(&remains, self.ctx.giant_exponent()))
            }
        }
    }
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

    /// The helper leaves the star the engine's own common envelope leaves, bit for bit: an engine
    /// built at the same age on the same pair, its `common_envelope` resolved with every side
    /// effect, holds the helper's star in the primary's place, and where the envelope leaves the
    /// cores apart the helper has none.
    #[test]
    fn the_helper_s_merger_is_the_engine_s_common_envelope_s() {
        let mut merged = 0;
        // Donors from the Hertzsprung gap to the early asymptotic giant branch beside light and
        // heavy companions: seven envelopes merge the pair, the last leaves the cores apart.
        let cases = [
            (1.3, 0.25, Phase::HertzsprungGap),
            (1.3, 0.25, Phase::FirstGiantBranch),
            (2.5, 0.3, Phase::FirstGiantBranch),
            (5.0, 1.0, Phase::CoreHeliumBurning),
            (1.3, 1.0, Phase::FirstGiantBranch),
            (12.0, 3.0, Phase::HertzsprungGap),
            (2.0, 1.8, Phase::EarlyAgb),
            (5.0, 4.0, Phase::EarlyAgb),
        ];
        for (m1, m2, phase) in cases {
            let (input, age) = pair_at_lobe(m1, m2, phase);
            let until = Years::new(age + 2.0e3);
            let helper = evolved_donor_merger(&input, Years::new(age), until);
            let ctx = Arc::new(Context::of(&input));
            let members = own_members(&input, until.value(), None);
            let orbit = input.orbit();
            let live = LiveOrbit::new(
                age,
                orbit.semi_major_axis().value() / SOLAR_RADIUS_M,
                0.0,
                *orbit.orientation(),
                orbit.mean_anomaly_at_epoch(),
            );
            let mut engine = Engine::new(Arc::clone(&ctx), age, until.value(), members, live, None);
            engine.common_envelope(0);
            let engine_merged = engine.kind == crate::stellar::binary::SegmentKind::Merged;
            println!("{m1} + {m2}, {phase:?} at {age:e}: merged {engine_merged}");
            assert_eq!(helper.is_some(), engine_merged);
            if let Some(star) = helper {
                assert_eq!(star, engine.members[0].state_at(&ctx, 0, age));
                merged += 1;
            }
        }
        assert_eq!(merged, cases.len() - 1);
    }
}
