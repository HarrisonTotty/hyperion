//! Drawing the primary parameters from the seed, one stream per parameter (plan 02, Design note
//! 2).
//!
//! Each parameter is drawn from `Stream::open(seed, tag, key)`, where `tag` is its
//! `galaxy.params.<name>` constant in [`tags`] and `key` is [`ObjectKey::galaxy()`], or
//! [`ObjectKey::galaxy_item`] with the item number for list parameters. No stream serves two
//! parameters, so adding or removing a parameter moves no other, and the order of the draws below
//! is immaterial. The laws and their ranges are those of P02.T5.a and are shared with the
//! builder's validation.
//!
//! Two kinds of draw are conditioned on another parameter's value, because a halo component's
//! stars cannot be younger than the event that put them in the halo. A satellite's star formation
//! quenches on infall, and the in-situ halo is disc heated by the last major merger, so both
//! formed before it. The in-situ and dominant components' age centres are therefore drawn on
//! `[max(10.5 Gyr, merger + 0.5 Gyr), 12.5 Gyr]` ([`heated_age_centre`]), and a lesser
//! progenitor's accretion time on `[6 Gyr, min(12 Gyr, its youngest stars' age)]`
//! ([`lesser_accretion`]). Each still takes one word from its own stream: removing the parameter
//! it depends on moves where that word maps, never the word.

use std::num::NonZeroU64;

use super::accretion::{FIRST_RECENT_PROGENITOR, Orbit};
use super::halo::HaloComponentKind;
use super::inputs::{
    ArmCount, HaloComponentInput, Inputs, LesserProgenitorInput, RecentProgenitorInput, Size,
};
use crate::Seed;
use crate::galaxy::imf::MassFunctionKind;
use crate::math;
use crate::rng::{DomainTag, ObjectKey, PowerLaw, Stream, Threshold, tags};
use crate::units::{LightYears, Radians};

/// How one scalar is drawn from its stream, and its range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Law {
    /// Uniform on `[lo, hi]`: [`Stream::uniform_in`], one word.
    Uniform { lo: f64, hi: f64 },
    /// Log-uniform on `[lo, hi]`: `lo × (hi ÷ lo)^u` for one uniform `u`, clamped.
    LogUniform { lo: f64, hi: f64 },
    /// A scatter in dex, normal with mean 0 and standard deviation `sigma`: two words.
    NormalDex { sigma: f64 },
}

/// A Box–Muller normal never lies further than 8.58 standard deviations from its mean
/// ([`Stream::standard_normal_pair`]), so a scatter beyond nine is not one the seed can draw.
const SCATTER_LIMIT_SIGMAS: f64 = 9.0;

impl Law {
    /// One draw from `stream`.
    pub(super) fn draw(self, stream: &mut Stream) -> f64 {
        match self {
            Self::Uniform { lo, hi } => stream.uniform_in(lo, hi),
            Self::LogUniform { lo, hi } => {
                (lo * math::exp(stream.uniform() * math::ln(hi / lo))).clamp(lo, hi)
            }
            Self::NormalDex { sigma } => stream.normal(0.0, sigma),
        }
    }

    /// The closed range every draw lies in, which the builder enforces.
    pub(super) fn bounds(self) -> (f64, f64) {
        match self {
            Self::Uniform { lo, hi } | Self::LogUniform { lo, hi } => (lo, hi),
            Self::NormalDex { sigma } => {
                let limit = SCATTER_LIMIT_SIGMAS * sigma;
                (-limit, limit)
            }
        }
    }

    /// The value a skipped draw takes in the order-independence test: the middle of the range.
    fn midpoint(self) -> f64 {
        let (lo, hi) = self.bounds();
        f64::midpoint(lo, hi)
    }
}

const GYR: f64 = 1e9;

// The laws of P02.T5.a's table, in its order. Masses in M☉, lengths in light-years, times in
// years, the pitch in degrees, scatters in dex.

pub(super) const STELLAR_MASS: Law = Law::LogUniform { lo: 3e10, hi: 1e11 };
pub(super) const SHARE_THICK: Law = Law::Uniform { lo: 0.08, hi: 0.14 };
pub(super) const SHARE_BULGE_BAR: Law = Law::Uniform { lo: 0.20, hi: 0.35 };
pub(super) const SHARE_BAR_OF_BULGE: Law = Law::Uniform { lo: 0.30, hi: 0.40 };
pub(super) const SHARE_NUCLEAR_DISC: Law = Law::Uniform {
    lo: 0.010,
    hi: 0.025,
};
pub(super) const SHARE_HALO: Law = Law::Uniform {
    lo: 0.007,
    hi: 0.014,
};
pub(super) const SFH_TIMESCALE: Law = Law::Uniform {
    lo: 5.0 * GYR,
    hi: 9.0 * GYR,
};
pub(super) const THIN_LENGTH_SCATTER: Law = Law::NormalDex { sigma: 0.05 };
pub(super) const THIN_MEAN_HEIGHT: Law = Law::Uniform {
    lo: 850.0,
    hi: 1_150.0,
};
// The brainstorm's 130–200 ly gives way to 225–345 ly, 285 ly with the same relative width (plan
// 02, ruling 3 of 2026-09-22), so that the young disc's height agrees with the brainstorm's own
// 5 km/s floor on its vertical dispersion: in the Milky Way fixture's potential of version 10, at
// three thin scale lengths, the bottom of the range, 225 ly, gave 4.91 km/s, and 285 ly gave 6.16,
// where 130–200 ly gave 2–3.5. Other galaxies' potentials meet the floor there only from 170–423
// ly (median 283), so 225 ly misses it in 95% of them and their drawn height in half; and the
// dispersion falls outward, so every young disc misses it in its outer parts. Since the profiles
// are solved at the Sun's radius (plan 02, P02.T12.a) the fixture's 285 ly gave 4.34 km/s there,
// and the fixture takes 335 ly (5.06 km/s). The floor is plan 08's (P08.T2.c), not this range's
// (plan 02, Risks, R23). 285 ly is an
// effective height Σ ÷ 2ρ₀ of 87 pc, where the youngest measured cohorts are: Bovy's (2017, MNRAS
// 470, 1360, Table 1) A dwarfs have z_d = 37–56 pc in sech²(Z ÷ 2z_d), whose effective height is
// 2z_d, 75–110 pc. 130–200 ly is 40–60 pc, the molecular gas's rather than a stellar cohort's.
pub(super) const YOUNG_HEIGHT: Law = Law::Uniform {
    lo: 225.0,
    hi: 345.0,
};
pub(super) const THICK_LENGTH_RATIO: Law = Law::Uniform { lo: 0.7, hi: 0.9 };
pub(super) const THICK_HEIGHT_RATIO: Law = Law::Uniform { lo: 2.7, hi: 3.3 };
pub(super) const BULGE_LENGTH_SCATTER: Law = Law::NormalDex { sigma: 0.06 };
pub(super) const BULGE_B_OVER_A: Law = Law::Uniform { lo: 0.5, hi: 0.7 };
pub(super) const BULGE_C_OVER_A: Law = Law::Uniform { lo: 0.3, hi: 0.4 };
pub(super) const BULGE_BOXINESS: Law = Law::Uniform { lo: 3.0, hi: 4.0 };
pub(super) const BAR_LENGTH_SCATTER: Law = Law::NormalDex { sigma: 0.05 };
pub(super) const BAR_WIDTH_RATIO: Law = Law::Uniform { lo: 0.08, hi: 0.12 };
pub(super) const BAR_HEIGHT: Law = Law::Uniform {
    lo: 500.0,
    hi: 700.0,
};
pub(super) const BAR_COROTATION_RATIO: Law = Law::Uniform { lo: 1.0, hi: 1.4 };
pub(super) const NUCLEAR_LENGTH_SCATTER: Law = Law::NormalDex { sigma: 0.04 };
pub(super) const NUCLEAR_HEIGHT_RATIO: Law = Law::Uniform { lo: 0.3, hi: 0.5 };
pub(super) const NUCLEAR_CLUSTER_MASS_SCATTER: Law = Law::NormalDex { sigma: 0.2 };
pub(super) const ARMS_PITCH_DEGREES: Law = Law::Uniform { lo: 10.0, hi: 18.0 };
pub(super) const ARMS_YOUNG_WIDTH: Law = Law::Uniform {
    lo: 250.0,
    hi: 500.0,
};
pub(super) const ARMS_YOUNG_FRACTION: Law = Law::Uniform { lo: 0.7, hi: 0.9 };
pub(super) const ARMS_OLD_AMPLITUDE: Law = Law::Uniform { lo: 0.10, hi: 0.30 };
// The brainstorm's 0.10–0.20 times 7 ÷ 4, the factor ruling 1 of 2026-09-22 found to carry plan
// 07's gas field's column at the Sun's radius, 7.9 M☉ pc⁻², to McKee, Parravano and Hollenbach's
// (2015, ApJ 814, 13) measured 13.7 ± 1.6, with `GasDiscParams::HEIGHT` raised by the same factor
// so that the mid-plane gas density, which the in-plane extinction reads, does not move. Once plan
// 07 drew its warm ionised layer by its own density (its ruling 19) the Milky Way fixture needed
// only 24% to reach the column, 1.6 times its "about 15%", and the range was kept: it is the spread
// of other galaxies, whose gas fractions the Milky Way's column does not measure, and 24% lies
// inside it. Over 2,000 seeds it leaves the neutral disc at least 0.53 of the gas (plan 07's
// `tests/gas_statistics.rs`).
pub(super) const GAS_MASS_FRACTION: Law = Law::Uniform {
    lo: 0.175,
    hi: 0.350,
};
pub(super) const GAS_LENGTH_RATIO: Law = Law::Uniform { lo: 1.5, hi: 2.0 };
pub(super) const DARK_F_STAR: Law = Law::LogUniform { lo: 0.12, hi: 0.45 };
pub(super) const DARK_CONCENTRATION_SCATTER: Law = Law::NormalDex { sigma: 0.11 };
pub(super) const BH_SCATTER: Law = Law::NormalDex { sigma: 0.38 };
pub(super) const METALLICITY_GRADIENT: Law = Law::Uniform {
    lo: -0.07,
    hi: -0.04,
};

// The halo's components ("Streams and accreted structure", table, and P02.T5.a). Cores,
// flattenings and the lesser progenitors' [Fe/H] are this plan's provisional ranges (Risks, R10).

pub(super) const HALO_IN_SITU_SHARE: Law = Law::Uniform { lo: 0.15, hi: 0.30 };
pub(super) const HALO_IN_SITU_FLATTENING: Law = Law::Uniform { lo: 0.45, hi: 0.55 };
pub(super) const HALO_IN_SITU_CORE: Law = Law::Uniform {
    lo: 1_500.0,
    hi: 3_000.0,
};
pub(super) const HALO_DOMINANT_SHARE: Law = Law::Uniform { lo: 0.35, hi: 0.60 };
pub(super) const HALO_DOMINANT_FLATTENING: Law = Law::Uniform { lo: 0.6, hi: 0.8 };
pub(super) const HALO_DOMINANT_CORE: Law = Law::Uniform {
    lo: 2_000.0,
    hi: 5_000.0,
};
/// The dominant merger's break, ly: 16–28 kpc, where star counts measure it (Deason, Belokurov and
/// Evans 2011, 27 kpc; Xue et al. 2015, 18; Pila-Díez et al. 2015, about 20; Medina et al. 2024,
/// 18 to 24 as the flattening is taken), rounded to 52,000–91,000 ly as the brainstorm has it
/// ("Streams and accreted structure").
pub(super) const HALO_DOMINANT_BREAK_RADIUS: Law = Law::Uniform {
    lo: 52_000.0,
    hi: 91_000.0,
};
/// How much the dominant merger's slope steepens beyond its break: 1.5–2.5, the measured outer
/// slopes (3.8–4.85) less the inner (2.1–2.5) in the same sources.
pub(super) const HALO_DOMINANT_BREAK_STEEPENING: Law = Law::Uniform { lo: 1.5, hi: 2.5 };
pub(super) const HALO_LESSER_SHARE_TOTAL: Law = Law::Uniform { lo: 0.10, hi: 0.25 };
pub(super) const HALO_LESSER_FLATTENING: Law = Law::Uniform { lo: 0.6, hi: 1.0 };
pub(super) const HALO_DEBRIS_SHARE: Law = Law::Uniform { lo: 0.08, hi: 0.15 };
pub(super) const HALO_DEBRIS_SLOPE: Law = Law::Uniform { lo: 4.0, hi: 4.5 };
pub(super) const HALO_DEBRIS_CORE: Law = Law::Uniform {
    lo: 3_000.0,
    hi: 5_000.0,
};
pub(super) const HALO_DISCRETE_SHARE: Law = Law::Uniform { lo: 0.02, hi: 0.15 };
/// The smooth components' inner slope, 2.2–2.8 (brainstorm, "Streams and accreted structure";
/// Deason, Belokurov and Evans 2011, 2.3; Pila-Díez et al. 2015, 2.50; Iorio et al. 2018, 2.96 as
/// a single power law; Xue et al. 2015, 2.1 ± 0.3). The globular-born debris stays steeper,
/// [`HALO_DEBRIS_SLOPE`].
pub(super) const HALO_COMPONENT_SLOPE: Law = Law::Uniform { lo: 2.2, hi: 2.8 };
/// Half the width of a halo component's age range, which is one gigayear.
pub(super) const HALO_AGE_HALF_WIDTH: f64 = 0.5 * GYR;
/// The centre of a halo component's one-gigayear age range, so that the range lies in 10–13 Gyr.
///
/// The globular-born debris and the lesser progenitors take it as it is; the in-situ and dominant
/// components take [`heated_age_centre`].
pub(super) const HALO_COMPONENT_AGE_CENTRE: Law = Law::Uniform {
    lo: HALO_AGE_CENTRE_RANGE.0,
    hi: HALO_AGE_CENTRE_RANGE.1,
};
/// The ends of [`HALO_COMPONENT_AGE_CENTRE`], years.
const HALO_AGE_CENTRE_RANGE: (f64, f64) = (10.5 * GYR, 12.5 * GYR);

/// The age centre of the in-situ or the dominant halo component, given the last major merger
/// `merger` (years ago): [`HALO_COMPONENT_AGE_CENTRE`] with its lower end raised so that the
/// component's youngest stars, half a gigayear younger than the centre, are no younger than the
/// merger.
///
/// The dominant component is the merger's own debris, whose star formation quenched when it fell
/// in; the in-situ component is the early disc that the merger heated (brainstorm, "Streams and
/// accreted structure"). The merger lies in 6–11 Gyr, so the range is never empty.
pub(super) fn heated_age_centre(merger: f64) -> Law {
    let (lo, hi) = HALO_AGE_CENTRE_RANGE;
    Law::Uniform {
        lo: lo.max(merger + HALO_AGE_HALF_WIDTH),
        hi,
    }
}
pub(super) const HALO_LESSER_FEH: Law = Law::Uniform { lo: -2.0, hi: -1.0 };
/// The globular-born debris is spherical: no parameter draws its axis ratio.
pub(super) const DEBRIS_FLATTENING: f64 = 1.0;
/// A stick-breaking fraction.
const HALO_LESSER_SPLIT: Law = Law::Uniform { lo: 0.0, hi: 1.0 };

/// The fewest and most lesser old progenitors.
pub(super) const HALO_LESSER_COUNT: (usize, usize) = (2, 5);

// The accretion history ("Streams and accreted structure"; P02.T5.a). The progenitors' times of
// the lesser ones and every orbit are provisional ranges that plan 10 revalidates.

pub(super) const LAST_MAJOR_MERGER: Law = Law::Uniform {
    lo: 6.0 * GYR,
    hi: 11.0 * GYR,
};
pub(super) const GLOBULAR_COUNT_SCATTER: Law = Law::NormalDex { sigma: 0.2 };
/// The mean number of recent progenitors.
pub(super) const RECENT_PROGENITOR_MEAN: f64 = 8.0;
/// The slope of the recent progenitors' stellar mass function, M^−1.45.
const RECENT_MASS_SLOPE: f64 = 1.45;
/// The recent progenitors' stellar masses, M☉: 10⁵–10⁹·⁵ (the upper end is `10^9.5`).
pub(super) const RECENT_MASS: (f64, f64) = (1e5, 3_162_277_660.168_379_5);
/// The ends of a lesser progenitor's accretion time, years ago, before [`lesser_accretion`]
/// lowers the upper end to its youngest stars' age.
const LESSER_ACCRETED_RANGE: (f64, f64) = (6.0 * GYR, 12.0 * GYR);
pub(super) const RECENT_ACCRETED: Law = Law::Uniform {
    lo: 0.0,
    hi: 6.0 * GYR,
};

/// A lesser progenitor's accretion time (years ago), given the centre of its halo component's
/// age range: uniform on 6–12 Gyr with the upper end lowered to the component's youngest stars,
/// half a gigayear younger than the centre, because a satellite's star formation quenches when it
/// falls in. The centre lies in 10.5–12.5 Gyr, so the range is at least 6–10 Gyr.
pub(super) fn lesser_accretion(age_centre: f64) -> Law {
    let (lo, hi) = LESSER_ACCRETED_RANGE;
    Law::Uniform {
        lo,
        hi: hi.min(age_centre - HALO_AGE_HALF_WIDTH),
    }
}

pub(super) const ORBIT_APOCENTRE: Law = Law::LogUniform {
    lo: 20_000.0,
    hi: 200_000.0,
};
/// The pericentre over the apocentre of every orbit but the dominant merger's.
const ORBIT_PERICENTRE_RATIO: Law = Law::Uniform { lo: 0.05, hi: 0.6 };
/// The dominant merger's orbital eccentricity, `(apocentre − pericentre) ÷ (apocentre +
/// pericentre)`.
///
/// Its debris is on strongly radial orbits (brainstorm, "Streams and accreted structure"), like
/// the Gaia Sausage's, whose anisotropy is β ≈ 0.9. Belokurov et al. (2018, MNRAS 478, 611, §3
/// and Figure 5) find that satellites of more than 10¹⁰ M☉ accreted 8–11 Gyr ago end on orbits
/// whose eccentricities peak above 0.7 without a disc and above 0.9 once the growing disc is
/// included, and trace the Sausage's anisotropy to that radialisation. The range 0.85–0.95 is
/// centred on 0.9. Naidu et al. (2021, ApJ 923, 92) fit the merger's orbit at infall, with a
/// circularity of 0.5 before dynamical friction radialises it, so they are not used for the
/// eccentricity of the debris.
pub(super) const DOMINANT_ECCENTRICITY: Law = Law::Uniform { lo: 0.85, hi: 0.95 };

/// The pericentre over the apocentre of an orbit of eccentricity `e`: `(1 − e) ÷ (1 + e)`.
///
/// It falls monotonically with `e` in floating point too, since each operation is correctly
/// rounded, so the builder can check a pericentre against the ratios at the ends of
/// [`DOMINANT_ECCENTRICITY`] exactly.
pub(super) fn pericentre_ratio(e: f64) -> f64 {
    (1.0 - e) / (1.0 + e)
}

/// How an orbit's second draw sets its pericentre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrbitShape {
    /// The pericentre is 0.05–0.6 of the apocentre ([`ORBIT_PERICENTRE_RATIO`]).
    Broad,
    /// The eccentricity is 0.85–0.95 ([`DOMINANT_ECCENTRICITY`]): the dominant merger.
    Radial,
}

/// Two arms or four, at equal odds: a mark below the threshold of ½ gives two.
const TWO_ARMS: f64 = 0.5;

/// One draw by one parameter: its tag, item number and value, for the order-independence test.
pub(super) type Draw = (DomainTag, u64, f64);

/// Opens streams for one seed. In the order-independence test it can skip one tag, giving that
/// parameter its range's midpoint, and log every draw.
struct Drawer<'a> {
    seed: Seed,
    skip: Option<DomainTag>,
    log: Option<&'a mut Vec<Draw>>,
}

impl Drawer<'_> {
    fn stream(&self, tag: DomainTag, item: u64) -> Stream {
        Stream::open(self.seed, tag, ObjectKey::galaxy_item(item))
    }

    fn record(&mut self, tag: DomainTag, item: u64, value: f64) -> f64 {
        if let Some(log) = self.log.as_deref_mut() {
            log.push((tag, item, value));
        }
        value
    }

    fn skipped(&self, tag: DomainTag) -> bool {
        self.skip == Some(tag)
    }

    /// Item `item` of a list parameter drawn by `law`.
    fn item(&mut self, tag: DomainTag, item: u64, law: Law) -> f64 {
        let value = if self.skipped(tag) {
            law.midpoint()
        } else {
            law.draw(&mut self.stream(tag, item))
        };
        self.record(tag, item, value)
    }

    /// A scalar parameter drawn by `law`, on `ObjectKey::galaxy()`.
    fn scalar(&mut self, tag: DomainTag, law: Law) -> f64 {
        self.item(tag, 0, law)
    }

    /// A progenitor's orbit: apocentre, pericentre, inclination, node and phase, in that order,
    /// from its one stream. The second word is the pericentre's ratio to the apocentre for a
    /// broad orbit and the eccentricity for a radial one, and is logged as drawn.
    fn orbit(&mut self, number: u64, shape: OrbitShape) -> Orbit {
        let tag = tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_ORBIT;
        let two_pi = 2.0 * core::f64::consts::PI;
        let second = match shape {
            OrbitShape::Broad => ORBIT_PERICENTRE_RATIO,
            OrbitShape::Radial => DOMINANT_ECCENTRICITY,
        };
        let [apocentre, drawn, cos_inclination, node, phase] = if self.skipped(tag) {
            [
                ORBIT_APOCENTRE.midpoint(),
                second.midpoint(),
                0.0,
                0.5 * two_pi,
                0.5 * two_pi,
            ]
        } else {
            let mut stream = self.stream(tag, number);
            [
                ORBIT_APOCENTRE.draw(&mut stream),
                second.draw(&mut stream),
                stream.uniform_in(-1.0, 1.0),
                stream.uniform_in(0.0, two_pi),
                stream.uniform_in(0.0, two_pi),
            ]
        };
        for value in [apocentre, drawn, cos_inclination, node, phase] {
            self.record(tag, number, value);
        }
        let ratio = match shape {
            OrbitShape::Broad => drawn,
            OrbitShape::Radial => pericentre_ratio(drawn),
        };
        Orbit::new(
            LightYears::new(apocentre),
            LightYears::new(ratio * apocentre),
            Radians::new(math::acos(cos_inclination)),
            Radians::new(node),
            Radians::new(phase),
        )
    }

    /// The centre of a halo component's age range, by its item number, drawn by `law`.
    fn halo_age(&mut self, item: u64, law: Law) -> f64 {
        self.item(tags::GALAXY_PARAMS_HALO_COMPONENT_AGE, item, law)
    }

    fn lesser_progenitors(&mut self) -> Vec<LesserProgenitorInput> {
        let count_tag = tags::GALAXY_PARAMS_HALO_LESSER_COUNT;
        let (fewest, most) = HALO_LESSER_COUNT;
        let span = NonZeroU64::new(u64::try_from(most - fewest + 1).expect("a small count"))
            .expect("the count range is not empty");
        let count = if self.skipped(count_tag) {
            3
        } else {
            let extra = self.stream(count_tag, 0).below(span);
            fewest + usize::try_from(extra).expect("below 4")
        };
        self.record(
            count_tag,
            0,
            f64::from(u32::try_from(count).expect("at most 5")),
        );
        // Stick-breaking: lesser progenitor n takes a fraction v_n of what the ones before it
        // left, and the last takes the remainder.
        let mut remaining = 1.0;
        (1..=count)
            .map(|n| {
                let number = u8::try_from(n).expect("at most five lesser progenitors");
                let item = u64::from(number);
                let weight = if n < count {
                    let v = self.item(
                        tags::GALAXY_PARAMS_HALO_LESSER_SPLIT,
                        item,
                        HALO_LESSER_SPLIT,
                    );
                    let weight = remaining * v;
                    remaining -= weight;
                    weight
                } else {
                    remaining
                };
                let component = 1 + item;
                let flattening = self.item(
                    tags::GALAXY_PARAMS_HALO_LESSER_FLATTENING,
                    item,
                    HALO_LESSER_FLATTENING,
                );
                let slope = self.item(
                    tags::GALAXY_PARAMS_HALO_COMPONENT_SLOPE,
                    component,
                    HALO_COMPONENT_SLOPE,
                );
                let age_centre = self.halo_age(component, HALO_COMPONENT_AGE_CENTRE);
                LesserProgenitorInput {
                    weight,
                    flattening,
                    slope,
                    age_centre,
                    feh_mean: self.item(
                        tags::GALAXY_PARAMS_HALO_COMPONENT_FEH,
                        component,
                        HALO_LESSER_FEH,
                    ),
                    accreted: self.item(
                        tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME,
                        item,
                        lesser_accretion(age_centre),
                    ),
                    orbit: self.orbit(item, OrbitShape::Broad),
                }
            })
            .collect()
    }

    fn recent_progenitors(&mut self) -> Vec<RecentProgenitorInput> {
        let count_tag = tags::GALAXY_PARAMS_ACCRETION_RECENT_COUNT;
        let count = if self.skipped(count_tag) {
            0
        } else {
            self.stream(count_tag, 0).poisson(RECENT_PROGENITOR_MEAN)
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a Poisson count of mean 8 is far below 2^53"
        )]
        self.record(count_tag, 0, count as f64);
        let masses = PowerLaw::new(RECENT_MASS_SLOPE, RECENT_MASS.0, RECENT_MASS.1)
            .expect("fixed limits are ordered and positive");
        let mass_tag = tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_MASS;
        (0..count)
            .map(|j| {
                let number = FIRST_RECENT_PROGENITOR + j;
                let mass = if self.skipped(mass_tag) {
                    RECENT_MASS.0
                } else {
                    self.stream(mass_tag, number).power_law(&masses)
                };
                self.record(mass_tag, number, mass);
                RecentProgenitorInput {
                    mass,
                    accreted: self.item(
                        tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME,
                        number,
                        RECENT_ACCRETED,
                    ),
                    orbit: self.orbit(number, OrbitShape::Broad),
                }
            })
            .collect()
    }

    fn arm_count(&mut self) -> ArmCount {
        let tag = tags::GALAXY_PARAMS_ARMS_COUNT;
        let two = if self.skipped(tag) {
            true
        } else {
            self.stream(tag, 0)
                .decide(Threshold::from_probability(TWO_ARMS))
        };
        self.record(tag, 0, if two { 2.0 } else { 4.0 });
        if two { ArmCount::Two } else { ArmCount::Four }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one line per parameter of the table, in the table's order"
    )]
    fn inputs(&mut self, mass_function: MassFunctionKind) -> Inputs {
        let coupled = |scatter| Size::Coupled { scatter };
        // Drawn first because the in-situ and dominant components' ages are conditioned on it.
        let last_major_merger = self.scalar(
            tags::GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER,
            LAST_MAJOR_MERGER,
        );
        let heated = heated_age_centre(last_major_merger);
        Inputs {
            mass_function,
            stellar_mass: self.scalar(tags::GALAXY_PARAMS_STELLAR_MASS, STELLAR_MASS),
            share_thick: self.scalar(tags::GALAXY_PARAMS_SHARE_THICK, SHARE_THICK),
            share_bulge_bar: self.scalar(tags::GALAXY_PARAMS_SHARE_BULGE_BAR, SHARE_BULGE_BAR),
            share_bar_of_bulge: self
                .scalar(tags::GALAXY_PARAMS_SHARE_BAR_OF_BULGE, SHARE_BAR_OF_BULGE),
            share_nuclear_disc: self
                .scalar(tags::GALAXY_PARAMS_SHARE_NUCLEAR_DISC, SHARE_NUCLEAR_DISC),
            share_halo: self.scalar(tags::GALAXY_PARAMS_SHARE_HALO, SHARE_HALO),
            sfh_timescale: self.scalar(tags::GALAXY_PARAMS_SFH_TIMESCALE, SFH_TIMESCALE),
            thin_length: coupled(
                self.scalar(tags::GALAXY_PARAMS_THIN_LENGTH_SCATTER, THIN_LENGTH_SCATTER),
            ),
            thin_mean_height: self.scalar(tags::GALAXY_PARAMS_THIN_MEAN_HEIGHT, THIN_MEAN_HEIGHT),
            young_height: self.scalar(tags::GALAXY_PARAMS_YOUNG_HEIGHT, YOUNG_HEIGHT),
            thick_length_ratio: self
                .scalar(tags::GALAXY_PARAMS_THICK_LENGTH_RATIO, THICK_LENGTH_RATIO),
            thick_height_ratio: self
                .scalar(tags::GALAXY_PARAMS_THICK_HEIGHT_RATIO, THICK_HEIGHT_RATIO),
            bulge_length: coupled(self.scalar(
                tags::GALAXY_PARAMS_BULGE_LENGTH_SCATTER,
                BULGE_LENGTH_SCATTER,
            )),
            bulge_b_over_a: self.scalar(tags::GALAXY_PARAMS_BULGE_B_OVER_A, BULGE_B_OVER_A),
            bulge_c_over_a: self.scalar(tags::GALAXY_PARAMS_BULGE_C_OVER_A, BULGE_C_OVER_A),
            bulge_boxiness: self.scalar(tags::GALAXY_PARAMS_BULGE_BOXINESS, BULGE_BOXINESS),
            bar_length: coupled(
                self.scalar(tags::GALAXY_PARAMS_BAR_LENGTH_SCATTER, BAR_LENGTH_SCATTER),
            ),
            bar_width_ratio: self.scalar(tags::GALAXY_PARAMS_BAR_WIDTH_RATIO, BAR_WIDTH_RATIO),
            bar_height: self.scalar(tags::GALAXY_PARAMS_BAR_HEIGHT, BAR_HEIGHT),
            bar_corotation_ratio: self.scalar(
                tags::GALAXY_PARAMS_BAR_COROTATION_RATIO,
                BAR_COROTATION_RATIO,
            ),
            nuclear_length: coupled(self.scalar(
                tags::GALAXY_PARAMS_NUCLEAR_LENGTH_SCATTER,
                NUCLEAR_LENGTH_SCATTER,
            )),
            nuclear_height_ratio: self.scalar(
                tags::GALAXY_PARAMS_NUCLEAR_HEIGHT_RATIO,
                NUCLEAR_HEIGHT_RATIO,
            ),
            nuclear_cluster_mass_scatter: self.scalar(
                tags::GALAXY_PARAMS_NUCLEAR_CLUSTER_MASS_SCATTER,
                NUCLEAR_CLUSTER_MASS_SCATTER,
            ),
            arm_count: self.arm_count(),
            arm_pitch: self.scalar(tags::GALAXY_PARAMS_ARMS_PITCH, ARMS_PITCH_DEGREES),
            arm_young_width: self.scalar(tags::GALAXY_PARAMS_ARMS_YOUNG_WIDTH, ARMS_YOUNG_WIDTH),
            arm_young_fraction: self
                .scalar(tags::GALAXY_PARAMS_ARMS_YOUNG_FRACTION, ARMS_YOUNG_FRACTION),
            arm_old_amplitude: self
                .scalar(tags::GALAXY_PARAMS_ARMS_OLD_AMPLITUDE, ARMS_OLD_AMPLITUDE),
            gas_mass_fraction: self
                .scalar(tags::GALAXY_PARAMS_GAS_MASS_FRACTION, GAS_MASS_FRACTION),
            gas_length_ratio: self.scalar(tags::GALAXY_PARAMS_GAS_LENGTH_RATIO, GAS_LENGTH_RATIO),
            dark_f_star: self.scalar(tags::GALAXY_PARAMS_DARK_F_STAR, DARK_F_STAR),
            dark_concentration_scatter: self.scalar(
                tags::GALAXY_PARAMS_DARK_CONCENTRATION_SCATTER,
                DARK_CONCENTRATION_SCATTER,
            ),
            bh_scatter: self.scalar(tags::GALAXY_PARAMS_BH_SCATTER, BH_SCATTER),
            metallicity_gradient: self.scalar(
                tags::GALAXY_PARAMS_METALLICITY_GRADIENT,
                METALLICITY_GRADIENT,
            ),
            halo_in_situ: HaloComponentInput {
                share: self.scalar(tags::GALAXY_PARAMS_HALO_IN_SITU_SHARE, HALO_IN_SITU_SHARE),
                flattening: self.scalar(
                    tags::GALAXY_PARAMS_HALO_IN_SITU_FLATTENING,
                    HALO_IN_SITU_FLATTENING,
                ),
                core: self.scalar(tags::GALAXY_PARAMS_HALO_IN_SITU_CORE, HALO_IN_SITU_CORE),
                slope: self.item(
                    tags::GALAXY_PARAMS_HALO_COMPONENT_SLOPE,
                    0,
                    HALO_COMPONENT_SLOPE,
                ),
                age_centre: self.halo_age(0, heated),
            },
            halo_dominant: HaloComponentInput {
                share: self.scalar(tags::GALAXY_PARAMS_HALO_DOMINANT_SHARE, HALO_DOMINANT_SHARE),
                flattening: self.scalar(
                    tags::GALAXY_PARAMS_HALO_DOMINANT_FLATTENING,
                    HALO_DOMINANT_FLATTENING,
                ),
                core: self.scalar(tags::GALAXY_PARAMS_HALO_DOMINANT_CORE, HALO_DOMINANT_CORE),
                slope: self.item(
                    tags::GALAXY_PARAMS_HALO_COMPONENT_SLOPE,
                    1,
                    HALO_COMPONENT_SLOPE,
                ),
                age_centre: self.halo_age(1, heated),
            },
            halo_dominant_break_radius: self.scalar(
                tags::GALAXY_PARAMS_HALO_DOMINANT_BREAK_RADIUS,
                HALO_DOMINANT_BREAK_RADIUS,
            ),
            halo_dominant_break_steepening: self.scalar(
                tags::GALAXY_PARAMS_HALO_DOMINANT_BREAK_STEEPENING,
                HALO_DOMINANT_BREAK_STEEPENING,
            ),
            halo_lesser_share_total: self.scalar(
                tags::GALAXY_PARAMS_HALO_LESSER_SHARE_TOTAL,
                HALO_LESSER_SHARE_TOTAL,
            ),
            halo_lesser: self.lesser_progenitors(),
            halo_debris: HaloComponentInput {
                share: self.scalar(tags::GALAXY_PARAMS_HALO_DEBRIS_SHARE, HALO_DEBRIS_SHARE),
                flattening: DEBRIS_FLATTENING,
                core: self.scalar(tags::GALAXY_PARAMS_HALO_DEBRIS_CORE, HALO_DEBRIS_CORE),
                slope: self.scalar(tags::GALAXY_PARAMS_HALO_DEBRIS_SLOPE, HALO_DEBRIS_SLOPE),
                age_centre: self.halo_age(
                    HaloComponentKind::GlobularDebris.item(),
                    HALO_COMPONENT_AGE_CENTRE,
                ),
            },
            halo_discrete_share: self
                .scalar(tags::GALAXY_PARAMS_HALO_DISCRETE_SHARE, HALO_DISCRETE_SHARE),
            last_major_merger,
            dominant_orbit: self.orbit(0, OrbitShape::Radial),
            recent: self.recent_progenitors(),
            globular_count_scatter: self.scalar(
                tags::GALAXY_PARAMS_ACCRETION_GLOBULAR_COUNT_SCATTER,
                GLOBULAR_COUNT_SCATTER,
            ),
        }
    }
}

/// Every primary parameter drawn from `seed`.
pub(super) fn draw_inputs(seed: Seed, mass_function: MassFunctionKind) -> Inputs {
    Drawer {
        seed,
        skip: None,
        log: None,
    }
    .inputs(mass_function)
}

/// Every primary parameter drawn from `seed` with `skip`'s parameter at its range's middle, and
/// the log of every draw.
#[cfg(test)]
pub(super) fn draw_logged(
    seed: Seed,
    mass_function: MassFunctionKind,
    skip: Option<DomainTag>,
) -> (Inputs, Vec<Draw>) {
    let mut log = Vec::new();
    let inputs = Drawer {
        seed,
        skip,
        log: Some(&mut log),
    }
    .inputs(mass_function);
    (inputs, log)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, chi_square_gof, ks_one_sample, normal_cdf, poisson_cdf, poisson_pmf,
    };

    use super::*;

    const SEEDS: u64 = 10_000;

    fn seed(n: u64) -> Seed {
        Seed::new(0x9a1a_0000_0000_0000 ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15))
    }

    fn plan_02_tags() -> Vec<DomainTag> {
        tags::ALL
            .iter()
            .copied()
            .filter(|t| t.name().starts_with("galaxy.params."))
            .collect()
    }

    /// Every draw of a log, keyed by tag, item and its place among that key's draws.
    fn keyed(log: &[Draw]) -> BTreeMap<(&'static str, u64, usize), f64> {
        let mut places: BTreeMap<(&'static str, u64), usize> = BTreeMap::new();
        let mut keyed = BTreeMap::new();
        for &(tag, item, value) in log {
            let place = places.entry((tag.name(), item)).or_insert(0);
            keyed.insert((tag.name(), item, *place), value);
            *place += 1;
        }
        keyed
    }

    impl Law {
        /// The distribution function of a draw.
        fn cdf(self, x: f64) -> f64 {
            match self {
                Self::Uniform { lo, hi } => ((x - lo) / (hi - lo)).clamp(0.0, 1.0),
                Self::LogUniform { lo, hi } => {
                    (math::ln(x / lo) / math::ln(hi / lo)).clamp(0.0, 1.0)
                }
                Self::NormalDex { sigma } => normal_cdf(x / sigma),
            }
        }
    }

    /// The parameters with a continuous law, by tag and item, for the marginal tests.
    #[expect(clippy::too_many_lines, reason = "one entry per parameter")]
    fn continuous_laws() -> Vec<(DomainTag, u64, Law)> {
        use tags as t;
        vec![
            (t::GALAXY_PARAMS_STELLAR_MASS, 0, STELLAR_MASS),
            (t::GALAXY_PARAMS_SHARE_THICK, 0, SHARE_THICK),
            (t::GALAXY_PARAMS_SHARE_BULGE_BAR, 0, SHARE_BULGE_BAR),
            (t::GALAXY_PARAMS_SHARE_BAR_OF_BULGE, 0, SHARE_BAR_OF_BULGE),
            (t::GALAXY_PARAMS_SHARE_NUCLEAR_DISC, 0, SHARE_NUCLEAR_DISC),
            (t::GALAXY_PARAMS_SHARE_HALO, 0, SHARE_HALO),
            (t::GALAXY_PARAMS_SFH_TIMESCALE, 0, SFH_TIMESCALE),
            (t::GALAXY_PARAMS_THIN_LENGTH_SCATTER, 0, THIN_LENGTH_SCATTER),
            (t::GALAXY_PARAMS_THIN_MEAN_HEIGHT, 0, THIN_MEAN_HEIGHT),
            (t::GALAXY_PARAMS_YOUNG_HEIGHT, 0, YOUNG_HEIGHT),
            (t::GALAXY_PARAMS_THICK_LENGTH_RATIO, 0, THICK_LENGTH_RATIO),
            (t::GALAXY_PARAMS_THICK_HEIGHT_RATIO, 0, THICK_HEIGHT_RATIO),
            (
                t::GALAXY_PARAMS_BULGE_LENGTH_SCATTER,
                0,
                BULGE_LENGTH_SCATTER,
            ),
            (t::GALAXY_PARAMS_BULGE_B_OVER_A, 0, BULGE_B_OVER_A),
            (t::GALAXY_PARAMS_BULGE_C_OVER_A, 0, BULGE_C_OVER_A),
            (t::GALAXY_PARAMS_BULGE_BOXINESS, 0, BULGE_BOXINESS),
            (t::GALAXY_PARAMS_BAR_LENGTH_SCATTER, 0, BAR_LENGTH_SCATTER),
            (t::GALAXY_PARAMS_BAR_WIDTH_RATIO, 0, BAR_WIDTH_RATIO),
            (t::GALAXY_PARAMS_BAR_HEIGHT, 0, BAR_HEIGHT),
            (
                t::GALAXY_PARAMS_BAR_COROTATION_RATIO,
                0,
                BAR_COROTATION_RATIO,
            ),
            (
                t::GALAXY_PARAMS_NUCLEAR_LENGTH_SCATTER,
                0,
                NUCLEAR_LENGTH_SCATTER,
            ),
            (
                t::GALAXY_PARAMS_NUCLEAR_HEIGHT_RATIO,
                0,
                NUCLEAR_HEIGHT_RATIO,
            ),
            (
                t::GALAXY_PARAMS_NUCLEAR_CLUSTER_MASS_SCATTER,
                0,
                NUCLEAR_CLUSTER_MASS_SCATTER,
            ),
            (t::GALAXY_PARAMS_ARMS_PITCH, 0, ARMS_PITCH_DEGREES),
            (t::GALAXY_PARAMS_ARMS_YOUNG_WIDTH, 0, ARMS_YOUNG_WIDTH),
            (t::GALAXY_PARAMS_ARMS_YOUNG_FRACTION, 0, ARMS_YOUNG_FRACTION),
            (t::GALAXY_PARAMS_ARMS_OLD_AMPLITUDE, 0, ARMS_OLD_AMPLITUDE),
            (t::GALAXY_PARAMS_GAS_MASS_FRACTION, 0, GAS_MASS_FRACTION),
            (t::GALAXY_PARAMS_GAS_LENGTH_RATIO, 0, GAS_LENGTH_RATIO),
            (t::GALAXY_PARAMS_DARK_F_STAR, 0, DARK_F_STAR),
            (
                t::GALAXY_PARAMS_DARK_CONCENTRATION_SCATTER,
                0,
                DARK_CONCENTRATION_SCATTER,
            ),
            (t::GALAXY_PARAMS_BH_SCATTER, 0, BH_SCATTER),
            (
                t::GALAXY_PARAMS_METALLICITY_GRADIENT,
                0,
                METALLICITY_GRADIENT,
            ),
            (t::GALAXY_PARAMS_HALO_IN_SITU_SHARE, 0, HALO_IN_SITU_SHARE),
            (
                t::GALAXY_PARAMS_HALO_IN_SITU_FLATTENING,
                0,
                HALO_IN_SITU_FLATTENING,
            ),
            (t::GALAXY_PARAMS_HALO_IN_SITU_CORE, 0, HALO_IN_SITU_CORE),
            (t::GALAXY_PARAMS_HALO_DOMINANT_SHARE, 0, HALO_DOMINANT_SHARE),
            (
                t::GALAXY_PARAMS_HALO_DOMINANT_FLATTENING,
                0,
                HALO_DOMINANT_FLATTENING,
            ),
            (t::GALAXY_PARAMS_HALO_DOMINANT_CORE, 0, HALO_DOMINANT_CORE),
            (
                t::GALAXY_PARAMS_HALO_DOMINANT_BREAK_RADIUS,
                0,
                HALO_DOMINANT_BREAK_RADIUS,
            ),
            (
                t::GALAXY_PARAMS_HALO_DOMINANT_BREAK_STEEPENING,
                0,
                HALO_DOMINANT_BREAK_STEEPENING,
            ),
            (
                t::GALAXY_PARAMS_HALO_LESSER_SHARE_TOTAL,
                0,
                HALO_LESSER_SHARE_TOTAL,
            ),
            (t::GALAXY_PARAMS_HALO_DEBRIS_SHARE, 0, HALO_DEBRIS_SHARE),
            (t::GALAXY_PARAMS_HALO_DEBRIS_SLOPE, 0, HALO_DEBRIS_SLOPE),
            (t::GALAXY_PARAMS_HALO_DEBRIS_CORE, 0, HALO_DEBRIS_CORE),
            (t::GALAXY_PARAMS_HALO_DISCRETE_SHARE, 0, HALO_DISCRETE_SHARE),
            (
                t::GALAXY_PARAMS_HALO_COMPONENT_SLOPE,
                1,
                HALO_COMPONENT_SLOPE,
            ),
            (
                t::GALAXY_PARAMS_HALO_COMPONENT_AGE,
                7,
                HALO_COMPONENT_AGE_CENTRE,
            ),
            (t::GALAXY_PARAMS_HALO_COMPONENT_FEH, 2, HALO_LESSER_FEH),
            (t::GALAXY_PARAMS_HALO_LESSER_SPLIT, 1, HALO_LESSER_SPLIT),
            (
                t::GALAXY_PARAMS_HALO_LESSER_FLATTENING,
                1,
                HALO_LESSER_FLATTENING,
            ),
            (
                t::GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER,
                0,
                LAST_MAJOR_MERGER,
            ),
            (
                t::GALAXY_PARAMS_ACCRETION_GLOBULAR_COUNT_SCATTER,
                0,
                GLOBULAR_COUNT_SCATTER,
            ),
        ]
    }

    fn logs() -> Vec<BTreeMap<(&'static str, u64, usize), f64>> {
        (0..SEEDS)
            .map(|n| keyed(&draw_logged(seed(n), MassFunctionKind::Kroupa, None).1))
            .collect()
    }

    /// Kolmogorov–Smirnov of every continuous parameter over 10⁴ seeds, chi-square of the counts.
    #[test]
    fn marginal_distributions_follow_their_laws() {
        let logs = logs();
        for (tag, item, law) in continuous_laws() {
            let mut sample: Vec<f64> = logs
                .iter()
                .filter_map(|log| log.get(&(tag.name(), item, 0)).copied())
                .collect();
            assert!(
                sample.len() > 1_000,
                "{}: {} draws",
                tag.name(),
                sample.len()
            );
            let (lo, hi) = law.bounds();
            assert!(
                sample.iter().all(|x| (lo..=hi).contains(x)),
                "{}",
                tag.name()
            );
            let ks = ks_one_sample(&mut sample, |x| law.cdf(x));
            assert_p_value(tag.name(), ks.p_value, ALPHA);
        }
        let orbit = tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_ORBIT.name();
        let mut apocentres: Vec<f64> = logs.iter().map(|l| l[&(orbit, 0, 0)]).collect();
        let ks = ks_one_sample(&mut apocentres, |x| ORBIT_APOCENTRE.cdf(x));
        assert_p_value("orbit apocentre", ks.p_value, ALPHA);
        let mut cos_inclinations: Vec<f64> = logs.iter().map(|l| l[&(orbit, 0, 2)]).collect();
        let ks = ks_one_sample(&mut cos_inclinations, |x| (0.5 * x + 0.5).clamp(0.0, 1.0));
        assert_p_value("orbit inclination", ks.p_value, ALPHA);
        let masses = PowerLaw::new(RECENT_MASS_SLOPE, RECENT_MASS.0, RECENT_MASS.1).unwrap();
        let mass = tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_MASS.name();
        let mut recent: Vec<f64> = logs
            .iter()
            .filter_map(|l| l.get(&(mass, FIRST_RECENT_PROGENITOR, 0)).copied())
            .collect();
        let ks = ks_one_sample(&mut recent, |x| masses.cdf(x));
        assert_p_value("recent progenitor mass", ks.p_value, ALPHA);
        let mut eccentricities: Vec<f64> = logs.iter().map(|l| l[&(orbit, 0, 1)]).collect();
        let ks = ks_one_sample(&mut eccentricities, |x| DOMINANT_ECCENTRICITY.cdf(x));
        assert_p_value("dominant eccentricity", ks.p_value, ALPHA);
        conditioned_draws_follow_their_laws(&logs);

        let count = |log: &BTreeMap<_, f64>, tag: DomainTag| log[&(tag.name(), 0, 0)];
        let n = logs.len();
        #[expect(clippy::cast_precision_loss, reason = "10^4 seeds")]
        let total = n as f64;
        let histogram = |values: &mut dyn Iterator<Item = f64>, bins: usize| {
            let mut h = vec![0_u64; bins];
            for v in values {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "whole numbers below the bin count"
                )]
                let i = (v as usize).min(bins - 1);
                h[i] += 1;
            }
            h
        };
        let arms = histogram(
            &mut logs
                .iter()
                .map(|l| count(l, tags::GALAXY_PARAMS_ARMS_COUNT) / 2.0 - 1.0),
            2,
        );
        assert_p_value(
            "arm count",
            chi_square_gof(&arms, &[total / 2.0, total / 2.0]).p_value,
            ALPHA,
        );
        let lesser = histogram(
            &mut logs
                .iter()
                .map(|l| count(l, tags::GALAXY_PARAMS_HALO_LESSER_COUNT) - 2.0),
            4,
        );
        assert_p_value(
            "lesser count",
            chi_square_gof(&lesser, &[total / 4.0; 4]).p_value,
            ALPHA,
        );
        let recent_counts = histogram(
            &mut logs
                .iter()
                .map(|l| count(l, tags::GALAXY_PARAMS_ACCRETION_RECENT_COUNT)),
            25,
        );
        let mut expected: Vec<f64> = (0..24)
            .map(|k| total * poisson_pmf(k, RECENT_PROGENITOR_MEAN))
            .collect();
        expected.push(total * (1.0 - poisson_cdf(23, RECENT_PROGENITOR_MEAN)));
        assert_p_value(
            "recent count",
            chi_square_gof(&recent_counts, &expected).p_value,
            ALPHA,
        );
    }

    /// The conditioned draws are uniform on their conditional ranges: each one's distribution
    /// function under the law it was drawn from, given the value it depends on, is uniform on
    /// `[0, 1]` (the probability integral transform), by Kolmogorov–Smirnov.
    fn conditioned_draws_follow_their_laws(logs: &[BTreeMap<(&'static str, u64, usize), f64>]) {
        let age = tags::GALAXY_PARAMS_HALO_COMPONENT_AGE.name();
        let time = tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME.name();
        let merger = tags::GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER.name();
        for item in [0, 1] {
            let mut transformed: Vec<f64> = logs
                .iter()
                .map(|l| heated_age_centre(l[&(merger, 0, 0)]).cdf(l[&(age, item, 0)]))
                .collect();
            let ks = ks_one_sample(&mut transformed, |x| x.clamp(0.0, 1.0));
            assert_p_value("heated age centre", ks.p_value, ALPHA);
        }
        // Lesser progenitor 1, whose halo component is item 2.
        let mut transformed: Vec<f64> = logs
            .iter()
            .map(|l| lesser_accretion(l[&(age, 2, 0)]).cdf(l[&(time, 1, 0)]))
            .collect();
        let ks = ks_one_sample(&mut transformed, |x| x.clamp(0.0, 1.0));
        assert_p_value("lesser accretion", ks.p_value, ALPHA);
    }

    /// The draws whose laws are conditioned on the parameter `tag`: skipping `tag` moves where
    /// their words map, never the words, so the order-independence test compares their
    /// transformed values instead.
    fn conditioned_on(tag: DomainTag) -> Vec<(DomainTag, u64)> {
        if tag == tags::GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER {
            vec![
                (tags::GALAXY_PARAMS_HALO_COMPONENT_AGE, 0),
                (tags::GALAXY_PARAMS_HALO_COMPONENT_AGE, 1),
            ]
        } else if tag == tags::GALAXY_PARAMS_HALO_COMPONENT_AGE {
            (1..=5)
                .map(|n| (tags::GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME, n))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// The conditional law a conditioned draw of `log` was drawn from.
    fn conditional_law(
        log: &BTreeMap<(&'static str, u64, usize), f64>,
        tag: &str,
        item: u64,
    ) -> Law {
        if tag == tags::GALAXY_PARAMS_HALO_COMPONENT_AGE.name() {
            let merger = tags::GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER.name();
            heated_age_centre(log[&(merger, 0, 0)])
        } else {
            let age = tags::GALAXY_PARAMS_HALO_COMPONENT_AGE.name();
            lesser_accretion(log[&(age, item + 1, 0)])
        }
    }

    /// No component's youngest stars are younger than the event that put them in the halo, over
    /// 10⁴ seeds: the in-situ and dominant components' against the last major merger, each lesser
    /// progenitor's against its accretion.
    #[test]
    fn halo_stars_predate_their_accretion_over_ten_thousand_seeds() {
        for n in 0..SEEDS {
            let i = draw_inputs(seed(n), MassFunctionKind::Kroupa);
            let merger = i.last_major_merger;
            assert!(
                (6.0 * GYR..=11.0 * GYR).contains(&merger),
                "seed {n}: {merger}"
            );
            for (what, centre) in [
                ("in situ", i.halo_in_situ.age_centre),
                ("dominant", i.halo_dominant.age_centre),
            ] {
                let youngest = centre - HALO_AGE_HALF_WIDTH;
                assert!(
                    youngest >= merger,
                    "seed {n}, {what}: {youngest} < {merger}"
                );
                assert!(centre <= 12.5 * GYR, "seed {n}, {what}: {centre}");
            }
            for lesser in &i.halo_lesser {
                let youngest = lesser.age_centre - HALO_AGE_HALF_WIDTH;
                assert!(youngest >= 10.0 * GYR, "seed {n}: {youngest}");
                assert!(
                    (6.0 * GYR..=youngest.min(12.0 * GYR)).contains(&lesser.accreted),
                    "seed {n}: accreted {} with stars from {youngest}",
                    lesser.accreted
                );
            }
        }
    }

    /// The dominant merger's eccentricity lies in 0.85–0.95 over 10⁴ seeds, and every other
    /// orbit keeps the broad range.
    #[test]
    fn the_dominant_merger_is_on_a_radial_orbit_over_ten_thousand_seeds() {
        for n in 0..SEEDS {
            let i = draw_inputs(seed(n), MassFunctionKind::Kroupa);
            let orbit = i.dominant_orbit;
            let (apocentre, pericentre) = (orbit.apocentre().value(), orbit.pericentre().value());
            let e = (apocentre - pericentre) / (apocentre + pericentre);
            assert!(
                (0.85 - 1e-12..=0.95 + 1e-12).contains(&e),
                "seed {n}: eccentricity {e}"
            );
            let others = i
                .halo_lesser
                .iter()
                .map(|l| l.orbit)
                .chain(i.recent.iter().map(|r| r.orbit));
            for orbit in others {
                let ratio = orbit.pericentre() / orbit.apocentre();
                assert!(
                    (0.05 - 1e-12..=0.6 + 1e-12).contains(&ratio),
                    "seed {n}: {ratio}"
                );
            }
        }
    }

    /// Every registered `galaxy.params.*` tag draws something, and nothing else is drawn.
    #[test]
    fn every_parameter_tag_is_used() {
        let registered: BTreeSet<&str> = plan_02_tags().iter().map(|t| t.name()).collect();
        let mut used = BTreeSet::new();
        for n in 0..64 {
            let (_, log) = draw_logged(seed(n), MassFunctionKind::Kroupa, None);
            used.extend(log.iter().map(|(t, _, _)| t.name()));
        }
        assert_eq!(used, registered);
        for tag in plan_02_tags() {
            assert_eq!(tag.scope(), crate::rng::TagScope::Galaxy, "{}", tag.name());
        }
    }

    /// Plan 02's order-independence test: drawing without any one parameter changes no other.
    ///
    /// A draw conditioned on the skipped parameter maps the same word onto a different range,
    /// so for it the distribution function under its conditional law is compared instead.
    #[test]
    fn removing_one_draw_moves_no_other() {
        for n in 0..4 {
            let full = keyed(&draw_logged(seed(n), MassFunctionKind::Kroupa, None).1);
            for skipped in plan_02_tags() {
                let without =
                    keyed(&draw_logged(seed(n), MassFunctionKind::Kroupa, Some(skipped)).1);
                let conditioned = conditioned_on(skipped);
                let mut compared = 0;
                for (key, value) in &without {
                    if key.0 == skipped.name() {
                        continue;
                    }
                    let Some(original) = full.get(key) else {
                        continue;
                    };
                    if conditioned
                        .iter()
                        .any(|&(t, item)| t.name() == key.0 && item == key.1)
                    {
                        let before = conditional_law(&full, key.0, key.1).cdf(*original);
                        let after = conditional_law(&without, key.0, key.1).cdf(*value);
                        assert!(
                            (before - after).abs() < 1e-9,
                            "without {} the word of {key:?} moved: {before} to {after}",
                            skipped.name()
                        );
                    } else {
                        assert!(
                            value.total_cmp(original).is_eq(),
                            "without {} the draw {key:?} moved",
                            skipped.name()
                        );
                    }
                    compared += 1;
                }
                assert!(
                    compared > 50,
                    "without {}: {compared} compared",
                    skipped.name()
                );
            }
        }
    }

    /// Each parameter's value is the same whatever was drawn before it.
    #[test]
    fn scalar_draws_are_order_independent() {
        let laws = continuous_laws();
        assert_order_independent(&laws, |&(tag, item, law)| {
            law.draw(&mut Stream::open(
                seed(1),
                tag,
                ObjectKey::galaxy_item(item),
            ))
            .to_string()
        });
        let (_, log) = draw_logged(seed(1), MassFunctionKind::Kroupa, None);
        for (tag, item, law) in laws.iter().copied().filter(|l| l.1 == 0) {
            let alone = law.draw(&mut Stream::open(seed(1), tag, ObjectKey::galaxy()));
            let logged = log.iter().find(|d| d.0 == tag && d.1 == item).unwrap().2;
            assert!(alone.total_cmp(&logged).is_eq(), "{}", tag.name());
        }
    }

    /// The same seed draws the same inputs, and the mass function takes no part in the draws.
    #[test]
    fn inputs_are_a_pure_function_of_the_seed() {
        let a = draw_inputs(seed(5), MassFunctionKind::Kroupa);
        let b = draw_inputs(seed(5), MassFunctionKind::Kroupa);
        assert_eq!(a, b);
        let mut c = draw_inputs(seed(5), MassFunctionKind::Chabrier);
        c.mass_function = MassFunctionKind::Kroupa;
        assert_eq!(a, c);
        assert_ne!(a, draw_inputs(seed(6), MassFunctionKind::Kroupa));
    }
}
