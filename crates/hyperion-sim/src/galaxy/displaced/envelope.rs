//! A galaxy's band-E single stars' lifetimes over initial mass and metallicity (plan 09, P09.T37's
//! cold cost): a bracket on a star's death age far tighter than the shared lifetime bracket
//! ([`LifetimeBracket`](super::marks::LifetimeBracket)), which spans Z from 10⁻⁴ to 0.03 with a
//! tenth's widening, for the core-collapse candidates of the features' lists, whose metallicity
//! is their feature's.
//!
//! A band-E star's lifetime under a galaxy's stellar rules is nearly a function of its initial
//! mass and metallicity alone: at the median draws and at others it differs by none at all above
//! 9 M☉ and by about 1% at 8 M☉ (measured over 200 stars' draws at [Fe/H] 0 and −0.5 under every
//! 27 hold), where a star in the companion-stripped window with its mark set lives otherwise and
//! takes no bracket here. The envelope holds the median draws' lifetime, under the galaxy's
//! stellar rules, on [`MASSES`] log-spaced initial masses over band E and [`FE_H_NODES`]
//! metallicities a tenth of a dex apart, each metallicity's column made when first read (some
//! 30 ms). A star's bracket is the bilinear interpolation of the logarithm of its neighbours'
//! lifetimes, widened by [`MARGIN`], which a test holds over random masses, metallicities and draws
//! ([`tests`]); outside the grid, or with a helium excess, it gives none.

use std::sync::OnceLock;

use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand};
use crate::math;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::fates::{STRIPPED_WINDOW, stripped_mark_matters};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::lifetime_under;
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

/// The envelope's initial masses, log-spaced over band E, 8–150 M☉: some 0.03 in ln m apart.
pub(crate) const MASSES: usize = 97;

/// The envelope's metallicities: [Fe/H] from [`FE_H_LOWEST`] a tenth of a dex apart.
pub(crate) const FE_H_NODES: usize = 31;

/// The lowest metallicity the envelope holds, dex.
const FE_H_LOWEST: f64 = -2.4;

/// The metallicities' spacing, dex.
const FE_H_STEP: f64 = 0.1;

/// The relative widening of an interpolated lifetime into a bracket: some four times the largest
/// error the slow test meets over 2 × 10⁴ random stars, 0.69% ([`tests`]).
pub(crate) const MARGIN: f64 = 0.03;

/// A galaxy's band-E single stars' lifetimes over initial mass and metallicity (module
/// documentation), under the stellar rules `rules`, each metallicity's column made when first read.
#[derive(Debug)]
pub(crate) struct LifetimeEnvelope {
    rules: StellarRules,
    columns: Vec<OnceLock<Box<[f64]>>>,
}

impl LifetimeEnvelope {
    /// An envelope of no column yet, under `rules`.
    #[must_use]
    pub(crate) fn new(rules: StellarRules) -> Self {
        Self {
            rules,
            columns: (0..FE_H_NODES).map(|_| OnceLock::new()).collect(),
        }
    }

    /// A bracket on the lifetime, years, of a star of initial mass `m0`, `composition` and the
    /// draws `draws` gives, under the envelope's rules: `None` outside the grid, with a helium
    /// excess, or in the companion-stripped window with the mark set, where the exact tier
    /// decides.
    #[must_use]
    pub(crate) fn bracket(
        &self,
        m0: SolarMasses,
        composition: &Composition,
        draws: impl FnOnce() -> StarDraws,
    ) -> Option<(Years, Years)> {
        let (lightest, heaviest) = (MASS_BAND_EDGES[MassBand::E.index()], 150.0);
        let mass = m0.value();
        if !(lightest..=heaviest).contains(&mass) || composition.helium_excess().value() > 0.0 {
            return None;
        }
        // The star's place on the grid, in node units.
        let at_fe_h = (composition.fe_h().value() - FE_H_LOWEST) / FE_H_STEP;
        #[expect(
            clippy::cast_precision_loss,
            reason = "the node counts are small and exact"
        )]
        let (fe_h_top, mass_top) = ((FE_H_NODES - 1) as f64, (MASSES - 1) as f64);
        if !(0.0..=fe_h_top).contains(&at_fe_h) {
            return None;
        }
        if mass <= STRIPPED_WINDOW.1 && stripped_mark_matters(m0, composition, &draws()) {
            return None;
        }
        let at_mass = math::ln(mass / lightest) / math::ln(heaviest / lightest) * mass_top;
        let (row, col) = (cell(at_mass, MASSES), cell(at_fe_h, FE_H_NODES));
        let (along_mass, along_fe_h) = (at_mass - index(row), at_fe_h - index(col));
        let (left, right) = (self.column(col), self.column(col + 1));
        let along = |column: &[f64]| column[row] + along_mass * (column[row + 1] - column[row]);
        let ln_life = along(left) + along_fe_h * (along(right) - along(left));
        let life = math::exp(ln_life);
        Some((
            Years::new(life / (1.0 + MARGIN)),
            Years::new(life * (1.0 + MARGIN)),
        ))
    }

    /// Column `j`'s logarithms of the median draws' lifetimes, years, at each mass node.
    fn column(&self, j: usize) -> &[f64] {
        self.columns[j].get_or_init(|| {
            let composition = Composition::from_fe_h(
                Dex::new(FE_H_LOWEST + FE_H_STEP * index(j)),
                HeliumExcess::ZERO,
            );
            let draws = StarDraws::median();
            let (lo, hi) = (MASS_BAND_EDGES[MassBand::E.index()], 150.0);
            #[expect(
                clippy::cast_precision_loss,
                reason = "the node count is small and exact"
            )]
            let m_top = (MASSES - 1) as f64;
            (0..MASSES)
                .map(|i| {
                    let m = lo * math::exp(math::ln(hi / lo) * index(i) / m_top);
                    let m = SolarMasses::new(m.min(hi));
                    math::ln(lifetime_under(m, &composition, &draws, self.rules).value())
                })
                .collect()
        })
    }
}

/// The cell of a grid of `nodes` holding the coordinate `x`, in node units: the last cell for the
/// grid's last node.
#[must_use]
fn cell(x: f64, nodes: usize) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "x lies in [0, nodes − 1], which a usize holds"
    )]
    let k = x.floor() as usize;
    k.min(nodes - 2)
}

/// `k` as an `f64`, exact for any node index.
#[must_use]
fn index(k: usize) -> f64 {
    f64::from(u32::try_from(k).expect("a node index fits in 32 bits"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::placement::CellKey;
    use crate::id::{BodyId, Layer};
    use crate::rng::{ObjectKey, Stream, tags};

    /// The envelope holds the exact lifetime of `n` stars of random band-E masses (log-uniform),
    /// metallicities over its grid and draws, under the version's rules and every 27 hold's, and
    /// gives the largest relative error of its interpolation it met.
    fn holds(n: u32) -> f64 {
        let key = CellKey::new(Layer::E, [0, 0, 0]).unwrap();
        let seed = Seed::new(0x0937_0000_0000_0e17);
        let mut stream = Stream::open(seed, tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let mut worst: f64 = 0.0;
        for rules in [
            StellarRules::of_version(crate::GENERATOR_VERSION),
            StellarRules::every_27_hold(),
        ] {
            let envelope = LifetimeEnvelope::new(rules);
            for k in 0..n {
                let m0 =
                    SolarMasses::new(8.0 * math::exp(stream.uniform() * math::ln(150.0 / 8.0)));
                #[expect(clippy::cast_precision_loss, reason = "a small exact count")]
                let span = (FE_H_NODES - 1) as f64 * FE_H_STEP;
                let fe_h = FE_H_LOWEST + stream.uniform() * span;
                let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                let draws = StarDraws::for_star(seed, BodyId::new(key.candidate_id(k).unwrap(), 0));
                let Some((lo, hi)) = envelope.bracket(m0, &composition, || draws.clone()) else {
                    continue;
                };
                let life = lifetime_under(m0, &composition, &draws, rules).value();
                assert!(
                    (lo.value()..=hi.value()).contains(&life),
                    "{m0:?} at [Fe/H] {fe_h}: {life} yr outside {lo:?}–{hi:?}"
                );
                let mid = (lo.value() * hi.value()).sqrt();
                worst = worst.max((life / mid - 1.0).abs());
            }
        }
        worst
    }

    /// Over 300 random stars under each set of rules, the envelope holds each one's lifetime, its
    /// interpolation within a third of its margin.
    #[test]
    fn the_envelope_holds_each_star_s_lifetime() {
        let worst = holds(300);
        eprintln!("the interpolation's worst error: {worst:.5}");
        assert!(worst < MARGIN / 3.0, "{worst}");
    }

    /// The same over 10⁴ stars under each set of rules: 0.69% at the worst (pod, 2026-11-03).
    #[test]
    #[ignore = "slow: 2 × 10⁴ lifetimes, some 10 s"]
    fn the_envelope_holds_each_star_s_lifetime_over_ten_thousand_stars() {
        let worst = holds(10_000);
        eprintln!("the interpolation's worst error: {worst:.5}");
        assert!(worst < MARGIN / 3.0, "{worst}");
    }
}
