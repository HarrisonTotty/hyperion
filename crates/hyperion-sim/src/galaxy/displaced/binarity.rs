//! The binarity seam: the one stripped share the class quadrature and plan 11's systems both read
//! (plan 08, P08.T8.a).
//!
//! A progenitor whose envelope a companion strips dies with a lighter core and, below a
//! carbon–oxygen core of 3 M☉, may take the kick law's low mode (plan 06, design note 11), so the
//! share of stripped primaries sets how many remnants stay near their birth sites. It is plan 11's
//! [`stellar::multiplicity::stripped_share`](crate::stellar::multiplicity::stripped_share): the
//! primary's multiple fraction times the probability that its innermost companion's periastron is
//! close enough to interact before the primary's core collapses, over plan 11's period,
//! mass-ratio and eccentricity laws ([`MultiplicityModel::default_v1`]).
//!
//! The interacting periastron is plan 11's provisional 10 au
//! ([`PROVISIONAL_INTERACTING_PERIASTRON`]) until P11.T4.a's `can_interact` threshold replaces it
//! by way of P11.T1.d, which repoints the closure here and nowhere else: the class table
//! (P08.T9) and [`kick_bins`](super::kick_bins) read this function, and nothing else of plan 11.

use crate::stellar::Composition;
use crate::stellar::multiplicity::{
    MultiplicityModel, PROVISIONAL_INTERACTING_PERIASTRON, stripped_share as multiplicity_share,
};
use crate::units::{Metres, SolarMasses};

/// The interacting periastron the seam passes plan 11's share: the largest periastron at which a
/// pair of primary `m1`, mass ratio `q` and composition `comp` interacts before the primary's
/// core collapse. For now plan 11's provisional 10 au for every pair
/// ([`PROVISIONAL_INTERACTING_PERIASTRON`]); P11.T1.d supplies P11.T4.a's threshold here.
#[must_use]
pub fn interacting_periastron(_m1: SolarMasses, _q: f64, _comp: &Composition) -> Metres {
    PROVISIONAL_INTERACTING_PERIASTRON
}

/// The share of primaries of initial mass `m` and composition `comp` whose envelope a companion
/// strips before they die: plan 11's
/// [`stripped_share`](crate::stellar::multiplicity::stripped_share) with its model
/// ([`MultiplicityModel::default_v1`]) and [`interacting_periastron`] (module documentation).
///
/// # Panics
///
/// If `m` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripped_share;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::SolarMasses;
///
/// // Most massive stars have a close companion (Sana et al. 2012), so the share is large.
/// let share = stripped_share(SolarMasses::new(20.0), &Composition::SOLAR);
/// assert!((0.3..0.9).contains(&share), "{share}");
/// ```
#[must_use]
pub fn stripped_share(m: SolarMasses, comp: &Composition) -> f64 {
    multiplicity_share(
        &MultiplicityModel::default_v1(),
        m,
        comp,
        interacting_periastron,
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::math;
    use crate::units::{Dex, HeliumExcess};

    /// P08.T8.a: the seam is plan 11's function with plan 11's model and threshold, bit for bit,
    /// at 33 masses across layer E's band and at three metallicities.
    #[test]
    fn the_seam_is_plan_elevens_share() {
        let model = MultiplicityModel::default_v1();
        for fe_h in [-1.5, 0.0, 0.3] {
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for i in 0..33 {
                let m =
                    SolarMasses::new(8.0 * math::exp(f64::from(i) / 32.0 * math::ln(150.0 / 8.0)));
                let direct = multiplicity_share(&model, m, &comp, |_, _, _| {
                    PROVISIONAL_INTERACTING_PERIASTRON
                });
                let share = stripped_share(m, &comp);
                assert_same_bits(share, direct);
                assert!((0.0..=1.0).contains(&share), "{share} at {m:?}");
            }
        }
    }
}
