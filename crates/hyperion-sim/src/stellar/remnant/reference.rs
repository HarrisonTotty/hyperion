//! The kick law's reference population, its score quantiles, and the observables the kick-law
//! tests assert (plan 06, P06.T19.b and T19.d).
//!
//! The ordinary kick's rank is `F_x(x)`, the distribution of Mandel and Müller's score over a
//! reference population ([`KickRankTable`]). That population ([`ReferencePopulation`]) is a
//! Kroupa sample of 8–150 M☉ primaries at Z = 0.02 (slope 2.3 above 1 M☉; Kroupa 2001, MNRAS 322,
//! 231), each built into a star with no ID ([`StarDraws::from_parts`]) from the stream
//! `stellar.reference` of its sample number ([`tags::STELLAR_REFERENCE`]) and evolved to its death
//! on the generator's own track. The iron-core collapses of single and wind-stripped progenitors
//! that leave a neutron star are its members, and [`score_quantiles`] sorts their scores into the
//! 257 quantiles at ranks i ÷ 256 of [`tables::kick_rank`](crate::tables::kick_rank). The scorer
//! works on any range of sample numbers ([`ReferencePopulation::scores`]) and the quantile step on
//! any list of scores ([`quantiles_of`]), so that plan 15's P15.T5.a can run it in chunks.
//!
//! [`kick_observables`] draws the same kind of sample, with the provisional companion-stripped
//! mark (design note 11) and every kick draw, and measures the six figures of T19.d that pin the
//! law against its sources. It is pure, and plan 15's P15.T5.b calls it too.
//!
//! Tracks stop at 100 M☉ until P06.T14, so a star of 100–150 M☉ is evolved as one of 100 M☉, as
//! [`StarModel`](crate::stellar::system::StarModel) does.

use core::ops::Range;

use super::{
    CompactRemnant, Death, DeathKind, KickDraws, KickLaw, KickLawParams, KickMode, KickRankTable,
    NatalKick, ProgenitorAtDeath, RemnantKind, StandardKickLaw, Stripping, ordinary_score,
};
use crate::math;
use crate::rng::{ObjectKey, PowerLaw, Seed, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::draws::{
    REDRAW_TRIES, StandardNormal, StarDraws, StarDrawsParts, UnitUniform, isotropic,
};
use crate::stellar::sse::{self, MAX_INITIAL_MASS, TrackOptions};
use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};
use crate::units::{KilometresPerSecond, SolarMasses};

/// The number of quantiles of the score's distribution: ranks i ÷ 256 for i = 0–256.
pub const QUANTILES: usize = 257;

/// The initial masses of the reference population and the test sample, M☉: 8–150.
pub const MASS_RANGE: (f64, f64) = (8.0, 150.0);

/// Kroupa's (2001) slope above 1 M☉: dN/dm ∝ m^−2.3.
pub const KROUPA_HIGH_MASS_SLOPE: f64 = 2.3;

/// The most sample numbers [`score_quantiles`] scores at a time.
const CHUNK: u64 = 1_024;

// The words of each sample's stream (the layout `tags::STELLAR_REFERENCE` documents).
const WORD_ETA: u64 = 1;
const WORD_REMNANT_TYPE: u64 = 3;
const WORD_REMNANT_FALLBACK: u64 = 4;
const WORD_REMNANT_MASS: u64 = 5;
const WORD_KICK_SCORE: u64 = 7;
const WORD_STRIPPED: u64 = 23;
const WORD_KICK_MODE: u64 = 24;
const WORD_KICK_LOW: u64 = 25;
const WORD_KICK_DIRECTION: u64 = 31;
const WORD_SEPARATION: u64 = 33;

/// One star of the reference sample: its initial mass, its draws, and the rank of the test-only
/// toy binary's separation.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceStar {
    initial_mass: SolarMasses,
    draws: StarDraws,
    separation_rank: UnitUniform,
}

impl ReferenceStar {
    /// The initial mass, M☉, in [`MASS_RANGE`].
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.initial_mass
    }

    /// The star's draws: Reimers η, the remnant's and the kick's, the stripped mark, and every
    /// other draw at its median.
    #[must_use]
    pub const fn draws(&self) -> &StarDraws {
        &self.draws
    }

    /// The rank of the toy binary's separation on its log-uniform range.
    #[must_use]
    pub const fn separation_rank(&self) -> UnitUniform {
        self.separation_rank
    }
}

/// The population `F_x` is tabulated over (P06.T19.b): Kroupa primaries of 8–150 M☉ at Z = 0.02,
/// whose iron-core collapses of single and wind-stripped progenitors leave a neutron star.
///
/// # Examples
///
/// The scorer and the quantile step, as plan 15 runs them in chunks: the scores of two chunks,
/// joined in sample order, are those of the whole range.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::stellar::remnant::reference::{ReferencePopulation, quantiles_of};
///
/// let population = ReferencePopulation::default();
/// let seed = Seed::new(6);
/// let mut scores = population.scores(seed, 0..500);
/// scores.extend(population.scores(seed, 500..1_000));
/// assert_eq!(scores, population.scores(seed, 0..1_000));
/// let quantiles = quantiles_of(&mut scores);
/// assert!(quantiles.windows(2).all(|q| q[0] < q[1]));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferencePopulation {
    score_scatter: f64,
}

impl Default for ReferencePopulation {
    fn default() -> Self {
        Self::new(KickLawParams::default().score_scatter)
    }
}

impl ReferencePopulation {
    /// The population whose scores take the factor ξ of relative scatter `score_scatter`
    /// ([`KickLawParams::score_scatter`]).
    #[must_use]
    pub const fn new(score_scatter: f64) -> Self {
        Self { score_scatter }
    }

    /// The composition of every star: solar, Z = 0.02.
    #[must_use]
    pub const fn composition(&self) -> Composition {
        Composition::SOLAR
    }

    /// Sample star `i` of the universe of `seed`, from its stream `stellar.reference`.
    ///
    /// # Panics
    ///
    /// Never: the mass function over 8–150 M☉ is a valid power law, and every draw is finite.
    #[must_use]
    pub fn star(&self, seed: Seed, i: u64) -> ReferenceStar {
        let mut s = Stream::open(seed, tags::STELLAR_REFERENCE, ObjectKey::galaxy_item(i));
        let imf = PowerLaw::new(KROUPA_HIGH_MASS_SLOPE, MASS_RANGE.0, MASS_RANGE.1)
            .expect("Kroupa's high-mass slope over 8–150 M☉ is a valid power law");
        let initial_mass = SolarMasses::new(s.power_law(&imf));
        let normal = |s: &mut Stream| {
            StandardNormal::new(s.standard_normal()).expect("a Box–Muller normal is finite")
        };
        let at = |s: &mut Stream, word: u64| s.seek(word);
        at(&mut s, WORD_ETA);
        let eta = normal(&mut s);
        at(&mut s, WORD_REMNANT_TYPE);
        let remnant_type = s.mark();
        at(&mut s, WORD_REMNANT_FALLBACK);
        let remnant_fallback = s.mark();
        at(&mut s, WORD_REMNANT_MASS);
        let remnant_mass = normal(&mut s);
        at(&mut s, WORD_KICK_SCORE);
        let kick_score: [StandardNormal; REDRAW_TRIES] = core::array::from_fn(|_| normal(&mut s));
        at(&mut s, WORD_STRIPPED);
        let stripped = s.mark();
        at(&mut s, WORD_KICK_MODE);
        let kick_mode = s.mark();
        at(&mut s, WORD_KICK_LOW);
        let kick_low: [StandardNormal; 3] = core::array::from_fn(|_| normal(&mut s));
        at(&mut s, WORD_KICK_DIRECTION);
        let kick_direction = isotropic(&mut s);
        at(&mut s, WORD_SEPARATION);
        let separation_rank = UnitUniform::new(s.uniform_open())
            .expect("an open uniform lies strictly inside (0, 1)");
        ReferenceStar {
            initial_mass,
            draws: StarDraws::from_parts(StarDrawsParts {
                eta,
                stripped,
                remnant_type,
                remnant_fallback,
                remnant_mass,
                kick_score,
                kick_mode,
                kick_low,
                kick_direction,
                ..StarDrawsParts::MEDIAN
            }),
            separation_rank,
        }
    }

    /// The death and remnant of `star` on the generator's track, built in full (at 100 M☉ above
    /// it, until P06.T14).
    #[must_use]
    pub fn fate(&self, star: &ReferenceStar) -> (Death, CompactRemnant) {
        let m0 = if star.initial_mass > MAX_INITIAL_MASS {
            MAX_INITIAL_MASS
        } else {
            star.initial_mass
        };
        let fate = sse::fate_of(
            m0,
            &self.composition(),
            &star.draws,
            TrackOptions::default(),
        );
        (fate.death, fate.remnant)
    }

    /// The ordinary score of sample star `i`, or `None` unless it is a member: an iron core's
    /// collapse to a neutron star. Its progenitor is single or wind-stripped, since the track
    /// never strips by a companion and the stripped mark is not read here.
    #[must_use]
    pub fn score(&self, seed: Seed, i: u64) -> Option<f64> {
        let star = self.star(seed, i);
        let (death, remnant) = self.fate(&star);
        let member = matches!(death.kind(), DeathKind::CoreCollapse { .. })
            && remnant.kind() == RemnantKind::NeutronStar;
        member.then(|| {
            let params = KickLawParams {
                score_scatter: self.score_scatter,
                ..KickLawParams::default()
            };
            let xi = params.score_factor(star.draws.kick_score());
            ordinary_score(death.progenitor().co_core_mass(), remnant.mass(), xi)
        })
    }

    /// The scores of the members among sample stars `items`, in sample order: the per-chunk
    /// scorer.
    #[must_use]
    pub fn scores(&self, seed: Seed, items: Range<u64>) -> Vec<f64> {
        items.filter_map(|i| self.score(seed, i)).collect()
    }
}

/// The quantiles of `scores` at ranks i ÷ 256, i = 0–256, sorting `scores` in place: the quantile
/// step.
///
/// Quantile p is the order statistic at position p (n − 1), linear between the two neighbours, so
/// the first is the least score and the last the greatest. A quantile that would not be above the
/// one before it (only where scores tie) is moved up to the next double, so that the table is
/// strictly increasing.
///
/// # Panics
///
/// If `scores` holds fewer than two values or a value that is not finite.
#[must_use]
pub fn quantiles_of(scores: &mut [f64]) -> [f64; QUANTILES] {
    assert!(
        scores.len() >= 2 && scores.iter().all(|x| x.is_finite()),
        "quantiles need at least two finite scores"
    );
    scores.sort_by(f64::total_cmp);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample of up to 2^53 scores is exact in an f64"
    )]
    let top = (scores.len() - 1) as f64;
    let mut quantiles = [0.0; QUANTILES];
    for (i, q) in quantiles.iter_mut().enumerate() {
        #[expect(
            clippy::cast_precision_loss,
            reason = "i is at most 256, exact in an f64"
        )]
        let h = i as f64 / (QUANTILES - 1) as f64 * top;
        let below = h.floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "0 <= h <= n - 1, an index of the slice"
        )]
        let k = below as usize;
        let x = if k + 1 < scores.len() {
            math::mul_add(h - below, scores[k + 1] - scores[k], scores[k])
        } else {
            scores[k]
        };
        *q = x;
    }
    for i in 1..QUANTILES {
        if quantiles[i] <= quantiles[i - 1] {
            quantiles[i] = quantiles[i - 1].next_up();
        }
    }
    quantiles
}

/// The 257 quantiles of the ordinary score over the first `n` members of `pop` in the universe
/// of `seed`, at ranks i ÷ 256 (P06.T19.b): the members of sample stars 0, 1, 2, … in order, scored
/// by [`ReferencePopulation::scores`] in chunks and cut at the n-th, then [`quantiles_of`].
///
/// The committed table, [`tables::kick_rank`](crate::tables::kick_rank), is this with n = 10⁶;
/// plan 15's P15.T5.a calls it with 10⁷. Each member costs one full track of a massive star, about
/// a millisecond, and about three sample stars in five are members.
///
/// # Panics
///
/// If `n` is below 2.
#[must_use]
pub fn score_quantiles(pop: &ReferencePopulation, n: u64, seed: Seed) -> [f64; QUANTILES] {
    let n = usize::try_from(n).expect("a sample that fits in memory has a usize length");
    assert!(n >= 2, "quantiles need at least two scores");
    let mut scores = Vec::with_capacity(n);
    let mut start = 0;
    while scores.len() < n {
        // About three sample stars in five are members, so twice the scores still wanted, within
        // one chunk; the result is the first n members whatever the chunks.
        let wanted = u64::try_from(n - scores.len()).expect("a usize fits in 64 bits");
        let chunk = (2 * wanted).clamp(16, CHUNK);
        scores.extend(pop.scores(seed, start..start + chunk));
        start += chunk;
    }
    scores.truncate(n);
    quantiles_of(&mut scores)
}

/// One kick of the test sample: the star, its death with the stripped mark applied, its remnant,
/// and its kick.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledKick {
    star: ReferenceStar,
    death: Death,
    remnant: CompactRemnant,
    kick: Option<NatalKick>,
}

impl SampledKick {
    /// The sample star.
    #[must_use]
    pub const fn star(&self) -> &ReferenceStar {
        &self.star
    }

    /// Its death, with the provisional stripped mark applied
    /// ([`StandardKickLaw::with_stripped_mark`]).
    #[must_use]
    pub const fn death(&self) -> Death {
        self.death
    }

    /// Its remnant.
    #[must_use]
    pub const fn remnant(&self) -> CompactRemnant {
        self.remnant
    }

    /// Its kick, or `None` where it left nothing.
    #[must_use]
    pub const fn kick(&self) -> Option<NatalKick> {
        self.kick
    }

    /// The kick's speed, km/s; zero where there is none.
    #[must_use]
    pub fn speed_km_s(&self) -> f64 {
        self.kick
            .map_or(0.0, |k| KilometresPerSecond::from(k.speed()).value())
    }
}

/// Sample star `i` of the universe of `seed` with its kick under `law`, as a single star's model
/// draws it ([`StandardKickLaw::natal_kick`]).
#[must_use]
pub fn sampled_kick(law: &StandardKickLaw, seed: Seed, i: u64) -> SampledKick {
    let pop = ReferencePopulation::new(law.params().score_scatter);
    let star = pop.star(seed, i);
    let (death, remnant) = pop.fate(&star);
    let death = law.with_stripped_mark(death, star.draws());
    let kick = law.natal_kick(&death, &remnant, star.draws());
    SampledKick {
        star,
        death,
        remnant,
        kick,
    }
}

/// The six figures of the kick-law tests (P06.T19.d), measured on one sample.
///
/// Shares are fractions in [0, 1]; each test names the default it pins in its own documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct KickObservables {
    /// Test 1: ln(v ÷ km s⁻¹) of every ordinary-mode neutron star of a single or wind-stripped
    /// iron-core progenitor, the reference population's own, sorted ascending.
    pub reference_ln_speeds: Vec<f64>,
    /// Test 1: the mean and standard deviation of ln(v ÷ km s⁻¹) over
    /// [`KickObservables::reference_ln_speeds`].
    pub reference_ln_moments: (f64, f64),
    /// Test 1, reported: the mean and standard deviation of ln(v ÷ km s⁻¹) over every
    /// ordinary-mode neutron star, those of companion-stripped progenitors above the ramp
    /// included, and their number.
    pub ordinary_ln_moments: (f64, f64, u64),
    /// Test 2: the share of isolated pulsars whose speed projected on the sky is under 50 km/s,
    /// averaged over isotropic viewing directions: every ordinary-mode neutron star, and the
    /// low-mode ones of progenitors no companion stripped (the electron captures of the single
    /// star's window).
    pub isolated_slow_share: f64,
    /// Test 3: the low mode's share of all neutron stars.
    pub low_mode_share: f64,
    /// Test 4: the share of neutron stars under 20, 50 and 100 km/s, judging the low-mode ones by
    /// a pair recoil of a third of their kick.
    pub retention: [f64; 3],
    /// Test 5: the share of the toy's surviving double neutron stars with e < 0.3, and the number
    /// of pairs that survived out of those tried.
    pub double_neutron_stars: (f64, u64, u64),
    /// Test 5, reported: the same toy with the progenitor's whole helium core as the exploding
    /// star's mass, a star no companion stripped further: the share of survivors with e < 0.3,
    /// and their number.
    pub double_neutron_stars_helium_core: (f64, u64),
    /// Test 6: the share of black holes with no kick.
    pub black_holes_unkicked: f64,
    /// Test 6: among black holes under 12 M☉, the share with no kick and the share above
    /// 100 km/s, and their number.
    pub light_black_holes: (f64, f64, u64),
    /// Counts: neutron stars and black holes in the sample.
    pub counts: (u64, u64),
}

/// The retention thresholds of test 4, km/s.
pub const RETENTION_SPEEDS_KM_S: [f64; 3] = [20.0, 50.0, 100.0];

/// The sky-projected speed test 2 counts under, km/s (Willcox et al. 2021).
pub const SLOW_TRANSVERSE_KM_S: f64 = 50.0;

/// The toy binary's neutron star, M☉ (test 5).
pub const TOY_NEUTRON_STAR: SolarMasses = SolarMasses::new(1.4);

/// The toy binary's separations, R☉, log-uniform between (test 5).
pub const TOY_SEPARATIONS_RSUN: (f64, f64) = (1.0, 10.0);

/// The black holes test 6 calls light, M☉: under 12 (Nagarajan and El-Badry 2025).
pub const LIGHT_BLACK_HOLE: SolarMasses = SolarMasses::new(12.0);

/// The six figures of T19.d over `n` stars of the Kroupa sample of 8–150 M☉ at Z = 0.02 in the
/// universe of `seed` (sample stars 0 to n − 1), under the law of `params` and `table`, with the
/// provisional companion-stripped mark.
///
/// Test 5's toy binary (Hills 1983; Brandt and Podsiadlowski 1995): each neutron star of the
/// sample is taken as the second-born of a circular pair with a 1.4 M☉ neutron star, as if a
/// companion had stripped it, at a separation log-uniform over 1–10 R☉. Its progenitor is then an
/// ultra-stripped star whose helium envelope the neutron star took, so the mass it holds before
/// the explosion is its carbon–oxygen core (Tauris, Langer and Podsiadlowski 2015, MNRAS 451,
/// 2123, find ultra-stripped supernovae eject ≲ 0.2 M☉); it loses all but the remnant at once and
/// takes this law's kick for a companion-stripped star.
///
/// # Panics
///
/// If the sample holds no neutron star of a single or wind-stripped iron-core progenitor.
#[must_use]
pub fn kick_observables(
    params: &KickLawParams,
    table: &KickRankTable,
    n: u64,
    seed: Seed,
) -> KickObservables {
    let law = StandardKickLaw::new(*params, table.clone());
    let mut reference = Vec::new();
    let mut ordinary = Vec::new();
    let mut isolated = Tally::default();
    let (mut neutron_stars, mut low) = (0_u64, 0_u64);
    let mut retained = [0_u64; 3];
    let (mut pairs, mut survivors, mut circular) = (0_u64, 0_u64, 0_u64);
    let (mut helium_survivors, mut helium_circular) = (0_u64, 0_u64);
    let (mut black_holes, mut unkicked) = (0_u64, 0_u64);
    let (mut light, mut light_unkicked, mut light_fast) = (0_u64, 0_u64, 0_u64);
    for i in 0..n {
        let s = sampled_kick(&law, seed, i);
        let Some(kick) = s.kick else { continue };
        let v = s.speed_km_s();
        let stripped = s.death.progenitor().stripping() == Stripping::Companion;
        match s.remnant.kind() {
            RemnantKind::NeutronStar => {
                neutron_stars += 1;
                let is_low = kick.mode() == KickMode::Low;
                low += u64::from(is_low);
                if kick.mode() == KickMode::Ordinary {
                    ordinary.push(math::ln(v));
                    if !stripped {
                        reference.push(math::ln(v));
                    }
                }
                if kick.mode() == KickMode::Ordinary || (is_low && !stripped) {
                    isolated.add(slow_on_the_sky(v));
                }
                let judged = if is_low { v / 3.0 } else { v };
                for (count, limit) in retained.iter_mut().zip(RETENTION_SPEEDS_KM_S) {
                    *count += u64::from(judged < limit);
                }
                pairs += 1;
                if let Some(e) = toy_binary(&law, &s, ToyStar::UltraStripped) {
                    survivors += 1;
                    circular += u64::from(e < 0.3);
                }
                if let Some(e) = toy_binary(&law, &s, ToyStar::HeliumCore) {
                    helium_survivors += 1;
                    helium_circular += u64::from(e < 0.3);
                }
            }
            RemnantKind::BlackHole => {
                black_holes += 1;
                let none = kick.mode() == KickMode::FallbackNone;
                unkicked += u64::from(none);
                if s.remnant.mass() < LIGHT_BLACK_HOLE {
                    light += 1;
                    light_unkicked += u64::from(none);
                    light_fast += u64::from(v > 100.0);
                }
            }
            RemnantKind::WhiteDwarf | RemnantKind::None => {}
        }
    }
    assert!(
        !reference.is_empty(),
        "the sample of {n} stars holds no reference neutron star"
    );
    reference.sort_by(f64::total_cmp);
    let (om, os) = moments(&ordinary);
    KickObservables {
        reference_ln_moments: moments(&reference),
        reference_ln_speeds: reference,
        ordinary_ln_moments: (om, os, count(ordinary.len())),
        isolated_slow_share: isolated.mean(),
        low_mode_share: share(low, neutron_stars),
        retention: retained.map(|r| share(r, neutron_stars)),
        double_neutron_stars: (share(circular, survivors), survivors, pairs),
        double_neutron_stars_helium_core: (
            share(helium_circular, helium_survivors),
            helium_survivors,
        ),
        black_holes_unkicked: share(unkicked, black_holes),
        light_black_holes: (
            share(light_unkicked, light),
            share(light_fast, light),
            light,
        ),
        counts: (neutron_stars, black_holes),
    }
}

/// The chance that a speed of `v` km/s, seen from an isotropic direction, is under
/// [`SLOW_TRANSVERSE_KM_S`] on the sky: 1 for v at or below it, else 1 − √(1 − (50 ÷ v)²), since
/// the projection is v sin θ with cos θ uniform.
#[must_use]
fn slow_on_the_sky(v: f64) -> f64 {
    if v <= SLOW_TRANSVERSE_KM_S {
        1.0
    } else {
        let s = SLOW_TRANSVERSE_KM_S / v;
        1.0 - (1.0 - s * s).sqrt()
    }
}

/// What test 5's exploding star holds just before its supernova.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToyStar {
    /// Its carbon–oxygen core: an ultra-stripped star, whose helium envelope the neutron star took.
    UltraStripped,
    /// Its whole helium core: a star no companion stripped further.
    HeliumCore,
}

/// The eccentricity of test 5's toy binary after the supernova of `sampled`, whose exploding star
/// holds the mass `star` says, or `None` if the pair is unbound.
///
/// Before: a circular orbit of separation a, the neutron star at the origin and the exploding
/// star at a x̂, moving at the circular speed along ŷ relative to it. At once the exploding star
/// drops to the remnant's mass and its velocity gains the kick, taken along the orbit's axes.
#[must_use]
fn toy_binary(law: &StandardKickLaw, sampled: &SampledKick, star: ToyStar) -> Option<f64> {
    let progenitor = sampled.death.progenitor();
    let stripped = ProgenitorAtDeath::new(
        progenitor.co_core_mass(),
        progenitor.helium_core_mass(),
        progenitor.envelope_mass(),
        Stripping::Companion,
    );
    let channel = super::CollapseChannel::of(sampled.death.kind())?;
    let kick = law.kick(
        channel,
        &stripped,
        &sampled.remnant,
        &KickDraws::of(sampled.star.draws()),
    );
    let (lo, hi) = TOY_SEPARATIONS_RSUN;
    let rank = sampled.star.separation_rank.value();
    let separation = lo * math::exp(rank * math::ln(hi / lo)) * SOLAR_RADIUS_M;
    let exploding = match star {
        ToyStar::UltraStripped => progenitor.co_core_mass(),
        ToyStar::HeliumCore => progenitor.helium_core_mass(),
    };
    let mu_before = GM_SUN * (TOY_NEUTRON_STAR.value() + exploding.value());
    let mu_after = GM_SUN * (TOY_NEUTRON_STAR.value() + sampled.remnant.mass().value());
    let [kx, ky, kz] = kick.velocity();
    let (vx, vy, vz) = (kx, (mu_before / separation).sqrt() + ky, kz);
    let energy = 0.5 * (vx * vx + vy * vy + vz * vz) - mu_after / separation;
    if energy >= 0.0 {
        return None;
    }
    let axis = -mu_after / (2.0 * energy);
    // h = |a x̂ × v| = a √(v_y² + v_z²).
    let h2 = separation * separation * (vy * vy + vz * vz);
    Some((1.0 - h2 / (mu_after * axis)).max(0.0).sqrt())
}

/// A running mean.
#[derive(Debug, Default)]
struct Tally {
    sum: f64,
    n: u64,
}

impl Tally {
    fn add(&mut self, x: f64) {
        self.sum += x;
        self.n += 1;
    }

    fn mean(&self) -> f64 {
        share_f(self.sum, self.n)
    }
}

/// `part ÷ whole` as a share, 0 for an empty whole.
fn share(part: u64, whole: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample's count is far below 2^53"
    )]
    let part = part as f64;
    share_f(part, whole)
}

/// `sum ÷ n`, 0 for n = 0.
fn share_f(sum: f64, n: u64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample's count is far below 2^53"
    )]
    let n = n as f64;
    sum / n
}

/// A slice's length as a count.
fn count(len: usize) -> u64 {
    u64::try_from(len).expect("a slice length fits in 64 bits")
}

/// The mean and the sample standard deviation of `xs`, summed in order; zeros for fewer than two.
fn moments(xs: &[f64]) -> (f64, f64) {
    if xs.len() < 2 {
        return (xs.first().copied().unwrap_or(0.0), 0.0);
    }
    let n = count(xs.len());
    let mean = share_f(xs.iter().sum(), n);
    let var = share_f(xs.iter().map(|x| (x - mean) * (x - mean)).sum(), n - 1);
    (mean, var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_are_the_order_statistics_linear_between() {
        let mut scores: Vec<f64> = (0..=512).rev().map(f64::from).collect();
        let q = quantiles_of(&mut scores);
        for (i, x) in q.iter().enumerate() {
            assert!((x - 2.0 * f64::from(u32::try_from(i).unwrap())).abs() < 1e-12);
        }
        let mut ties = vec![1.0; 10];
        ties.push(2.0);
        let q = quantiles_of(&mut ties);
        assert!(q.windows(2).all(|w| w[0] < w[1]), "ties are moved apart");
        assert!((q[256] - 2.0).abs() < 1e-15);
    }

    #[test]
    fn a_sample_star_depends_on_its_seed_and_number_alone() {
        let pop = ReferencePopulation::default();
        let a = pop.star(Seed::new(3), 17);
        assert_eq!(a, pop.star(Seed::new(3), 17));
        assert_ne!(a, pop.star(Seed::new(3), 18));
        assert_ne!(a, pop.star(Seed::new(4), 17));
        let m = a.initial_mass().value();
        assert!((MASS_RANGE.0..MASS_RANGE.1).contains(&m));
        // The draws the population does not use stay at their medians.
        assert_eq!(a.draws().rotation(), StarDraws::median().rotation());
        assert_ne!(a.draws().eta(), StarDraws::median().eta());
    }

    #[test]
    fn the_sky_projection_counts_slow_speeds() {
        assert!((slow_on_the_sky(30.0) - 1.0).abs() < 1e-15);
        assert!((slow_on_the_sky(100.0) - (1.0 - 0.75_f64.sqrt())).abs() < 1e-15);
        assert!(slow_on_the_sky(1_000.0) < 0.002);
    }

    #[test]
    fn the_chunked_scorer_is_the_whole_ranges() {
        let pop = ReferencePopulation::default();
        let seed = Seed::new(0x0619_b000_0000_0001);
        let mut joined = pop.scores(seed, 0..12);
        joined.extend(pop.scores(seed, 12..24));
        assert_eq!(joined, pop.scores(seed, 0..24));
        assert!(
            !joined.is_empty(),
            "a dozen massive stars leave a neutron star"
        );
    }
}
