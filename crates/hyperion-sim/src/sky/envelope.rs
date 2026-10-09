//! The brightness envelope: the brightest absolute V magnitude any star of at most a given mass
//! reaches over a range of ages, which bounds a system's light before its stars are generated
//! (rendering plan R06, Design note 8).
//!
//! The census skips a candidate whose system could not pass the cut at the cell's least distance
//! with no extinction ([`BrightnessEnvelope::mass_floor`], read by
//! [`generate_cell_where`](crate::galaxy::placement::generate_cell_where)), and the caps read the
//! envelope's brightest star per layer. Both must bound every star the census can meet, so the
//! envelope takes the extreme of every source of spread the tracks have:
//!
//! - **Masses.** [`MASS_NODES`] nodes even in ln m over 0.0124–150 M☉, with the layers' band edges
//!   among them. Each node's brightest magnitude per age bin is made a running minimum over mass
//!   (the brightest of every lighter star too), and a mass between two nodes reads the heavier
//!   node, so the envelope is a bound wherever magnitude falls with mass between nodes and is
//!   otherwise off by at most the difference the dense slow test measures, under the
//!   [`MARGIN_MAG`] the envelope is brightened by.
//! - **Composition and draws.** The tracks at [`FE_H_NODES`], spanning the metallicities the
//!   components draw (the tracks clamp Z to 10⁻⁴–0.03), each at the Reimers η of [`ETA_DRAWS`],
//!   from the physical extreme η = 0 up (plan 06, design note 7), the one draw that moves a
//!   living star's light. The envelope is the brightest of all of them, so it does not depend on
//!   the component.
//! - **Ages.** [`AGE_BINS`] bins even in log age from 10⁴ years to [`MAX_AGE_YEARS`], with one bin
//!   from 0 to 10⁴; a track's phase is cut at its segments' ends and knots and into
//!   [`SAMPLES_PER_PHASE`] parts, and each part's brightest of its ends and three interior points
//!   is entered in every age bin it overlaps.
//!
//! Dark states (protostars, white dwarfs and the rest of [`super::photometry`]'s rules) enter
//! nothing. The envelope bounds this generator's stars, not nature's: its extremes, M<sub>V</sub>
//! −12.85 at 150 M☉ and about −5.7 for old stars of at most 1 M☉, most likely come from the
//! tracks' super-Eddington excursions after the main sequence at low Z and from the η = 0 draws'
//! late giants, and they move when the tracks do.
//!
//! The envelope depends on no galaxy, so it is built offline, by `hyperion-fit`'s task
//! `sky_envelope` from [`raw_node`] and [`BrightnessEnvelope::assemble`], and checked in as
//! [`tables::sky_envelope`](crate::tables::sky_envelope), each magnitude in integer
//! millimagnitudes rounded brighter ([`to_millimag`]) so that it stays a bound (decided
//! 2026-10-03, `decision-r06-tables.md`). [`BrightnessEnvelope::build`] reads that table. A change
//! to the tracks that would leave it stale fails the fit's sim fingerprint in `just fit-check`
//! (probe nodes at \[Fe/H\] −2.5 and 0), and, for any track, the slow comparison with
//! [`BrightnessEnvelope::build_with`] and fit-check's byte-for-byte rerun.
//!
//! Plan 11's binary evolution moves mass within a pair and rejuvenates or merges stars:
//! [`max_star_mass`] says how massive a star a system can hold, twice its primary's initial mass
//! (R06.T16.b), and a system whose pairs may have interacted is bounded over every age from zero
//! to its own, since a main-sequence merger or accretor shines as a younger star of its new mass
//! (the census's `flux_bound` gives the reasoning for the other pair-evolved states). A layer whose
//! every system is single ([`always_single`]: the free-floating brown dwarfs) holds no pair, so its
//! heaviest star is its primary ([`max_star_mass_in`], R06.T8.f).
//!
//! The cell floor asks about every age at once, so the envelope also keeps each node's brightest
//! magnitude over every age bin, and [`BrightnessEnvelope::mass_floor`] reads that one value a node
//! for the question (R06.T8.f), bit for bit what a scan of the bins gives.

use crate::galaxy::Galaxy;
use crate::galaxy::fields::ComponentId;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::SystemKind;
use crate::id::Layer;
use crate::math;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::sse::{MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track};
use crate::stellar::substellar;
use crate::stellar::{Composition, StarState};
use crate::tables::sky_envelope;
use crate::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};

use super::photometry::absolute_v_of_state;

/// The mass nodes of the envelope, even in ln m over the brown dwarfs' and stars' ranges.
pub const MASS_NODES: usize = 192;

/// The \[Fe/H\] at which the envelope's tracks are built: from the tracks' metal-poor clamp (Z =
/// 10⁻⁴, \[Fe/H\] ≈ −2.30, which −2.5 reads) to their metal-rich one (Z = 0.03, +0.18), every
/// quarter dex. The tracks read nothing of a composition beyond its clamped Z and its helium
/// excess, which the provisional `tables::helium` (P06.T17, the identity) leaves without effect,
/// so these cover every star. When plan 15's P15.T7 fits that table, a helium-rich star reaches
/// its giant phases earlier, and the fitted envelope (`hyperion-fit`'s `sky_envelope`) must be
/// rebuilt with helium-excess nodes.
pub const FE_H_NODES: [f64; 12] = [
    -2.5, -2.25, -2.0, -1.75, -1.5, -1.25, -1.0, -0.75, -0.5, -0.25, 0.0, 0.18,
];

/// The standard-normal draws of Reimers η (η = 0.5 + 0.07 z, plan 06's design note 7) at which
/// the envelope's tracks are built: the physical extreme η = 0, no Reimers wind (z = −0.5 ÷ 0.07),
/// where a giant's tip is brightest, then ±3.5σ, the median and +7σ. A star of z above 7 (one in
/// 10¹²) is outside what the dense test checks.
pub const ETA_DRAWS: [f64; 5] = [-0.5 / 0.07, -3.5, 0.0, 3.5, 7.0];

/// The equal parts each phase of a track is cut into, besides its knots.
pub const SAMPLES_PER_PHASE: u32 = 32;

/// The age bins: one from 0 to 10⁴ years, then this many even in log age to [`MAX_AGE_YEARS`].
pub const AGE_BINS: usize = 192;

/// The oldest age the envelope covers, years: older than any star of any galaxy.
pub const MAX_AGE_YEARS: f64 = 1.5e10;

/// The heaviest mass, M☉, the cooling fits are read at for the node at 0.1 M☉, here and in the
/// phase envelope ([`super::phase`]).
pub(crate) const COOLING_TOP_MSUN: f64 = 0.099_999;

/// The age bins either side of a bin that [`AGE_SPREAD_FACTOR`] spans.
#[must_use]
fn age_spread_bins() -> usize {
    #[expect(clippy::cast_precision_loss, reason = "192 bins")]
    let width = math::ln(MAX_AGE_YEARS / FIRST_AGE_YEARS) / AGE_BINS as f64;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a handful of bins"
    )]
    let bins = (math::ln(AGE_SPREAD_FACTOR) / width).ceil() as usize;
    bins
}

/// The youngest age of the log-age bins, years.
const FIRST_AGE_YEARS: f64 = 1e4;

/// The factor in age either side of a bin over which each node's brightest star is spread: a
/// lighter or more metal-rich star than a node reaches the node's bright phases later, a heavier or
/// more metal-poor one earlier, by up to the lifetime's change between nodes (some 13% a mass
/// node, and up to about 30% a metallicity node), so a node's bright phases are taken to hold over
/// ages within this factor of when the node has them. The dense slow test found single nodes
/// missing metal-poor giants by up to 5.6 mag without it.
pub const AGE_SPREAD_FACTOR: f64 = 1.6;

/// How much the envelope is brightened, mag, to bound the stars between its mass nodes, its
/// samples and its compositions (Design note 8; the dense slow test measures what is used).
pub const MARGIN_MAG: f64 = 0.3;

/// The mass of the heaviest star `primary_initial`'s system can hold, M☉: twice the primary's
/// initial mass, and at most the tracks' top, [`MAX_INITIAL_MASS`] (150 M☉) (Design note 8;
/// R06.T16.b).
///
/// Plan 11's engine evolves only pairs of two stars (P11.T11), whose mass can only move between
/// its two members or leave the pair, and no companion is drawn heavier than its primary. So a
/// merger or an accretor holds at most m₁ + m₂ ≤ 2 m₁. A merger product heavier than 150 M☉
/// follows the 150 M☉ track (`track_mass`). A main-sequence accretor above it takes the closed
/// forms extrapolated past their range (BSE's formulae hold to 100 M☉, Hurley, Tout and Pols
/// 2002, §1), and the top node bounds it through the 150 M☉ track's later phases and the margin.
/// A NaN mass reads 150 M☉, the brightest node, which skips nothing.
#[must_use]
pub fn max_star_mass(primary_initial: SolarMasses) -> SolarMasses {
    SolarMasses::new((2.0 * primary_initial.value()).min(MAX_INITIAL_MASS.value()))
}

/// Whether every system of `layer` is single: a free-floating brown dwarf or a rogue planet, which
/// plan 11's multiplicity is not applied to (`stellar::system::grid_multiplicity` makes each a
/// forced single; plan 13's Risks, "Brown dwarfs are single"). A test holds the census's records
/// to it, layer by layer.
#[must_use]
pub const fn always_single(layer: Layer) -> bool {
    match SystemKind::of_layer(layer) {
        SystemKind::Stellar => false,
        SystemKind::BrownDwarf | SystemKind::RoguePlanet => true,
    }
}

/// The mass of the heaviest star a system of `layer` with a primary of `primary_initial` can
/// hold, M☉: the primary's own where every system of the layer is single ([`always_single`]), and
/// [`max_star_mass`] otherwise, since a pair can merge or feed one star (R06.T8.f).
#[must_use]
pub fn max_star_mass_in(layer: Layer, primary_initial: SolarMasses) -> SolarMasses {
    if always_single(layer) {
        primary_initial
    } else {
        max_star_mass(primary_initial)
    }
}

/// The brightest absolute V magnitude stars of at most a given mass reach over a range of ages
/// (see the [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct BrightnessEnvelope {
    /// The nodes' masses, M☉, ascending.
    masses: Vec<f64>,
    /// Per node, per age bin, the brightest M<sub>V</sub> of this node and every lighter one,
    /// margin included; +∞ where none shines.
    brightest: Vec<[f64; AGE_BINS + 1]>,
    /// Per node, its row's brightest over every age bin, folded as [`brightest`](Self::brightest)
    /// folds a range: what the cell floor's question, every age at once, reads (R06.T8.f). Like
    /// each bin, it never rises with mass.
    brightest_ever: Vec<f64>,
}

impl BrightnessEnvelope {
    /// The envelope for the stars of `galaxy`, read from the fitted table
    /// [`tables::sky_envelope`](crate::tables::sky_envelope) at no cost.
    ///
    /// The envelope reads nothing of the galaxy's but what every galaxy shares, the tracks, so
    /// `hyperion-fit`'s task `sky_envelope` builds it once from [`build_with`](Self::build_with)
    /// at [`FE_H_NODES`] and [`SAMPLES_PER_PHASE`] and stores it in integer millimagnitudes,
    /// each rounded brighter ([`to_millimag`]), so the table bounds whatever the build bounds
    /// (decided 2026-10-03, `decision-r06-tables.md`).
    #[must_use]
    pub fn build(_galaxy: &Galaxy) -> Self {
        // The tracks are the generator's, not the galaxy's; the parameter keeps the server's
        // cache keyed as the luminosity tables are, should a galaxy ever carry its own physics.
        Self::fitted()
    }

    /// The envelope of the fitted table, which [`build`](Self::build) returns for every galaxy.
    #[must_use]
    pub(crate) fn fitted() -> Self {
        Self::from_rows(
            sky_envelope::MASSES.to_vec(),
            sky_envelope::BRIGHTEST_MMAG
                .iter()
                .map(|row| row.map(from_millimag))
                .collect(),
        )
    }

    /// The envelope of `masses` and their rows, with each row's brightest over every age.
    ///
    /// # Panics
    ///
    /// If a node's brightest over every age is brighter than the next node's, which the running
    /// minimum over mass of [`assemble`](Self::assemble) and of the fitted table rules out, and on
    /// which [`mass_floor`](Self::mass_floor)'s reading of one value a node rests.
    #[must_use]
    fn from_rows(masses: Vec<f64>, brightest: Vec<[f64; AGE_BINS + 1]>) -> Self {
        let brightest_ever: Vec<f64> = brightest
            .iter()
            .map(|row| row.iter().fold(f64::INFINITY, |a, &b| a.min(b)))
            .collect();
        assert!(
            brightest_ever
                .windows(2)
                .all(|w| w[1].partial_cmp(&w[0]) != Some(core::cmp::Ordering::Greater)),
            "an envelope never fades with mass"
        );
        Self {
            masses,
            brightest,
            brightest_ever,
        }
    }

    /// The envelope built from the tracks at the metallicities `fe_h` with `samples_per_phase`
    /// parts per phase: [`raw_node`] at every node of [`mass_nodes`], then
    /// [`assemble`](Self::assemble). At [`FE_H_NODES`] and [`SAMPLES_PER_PHASE`] it is the fitted
    /// table's generator: 9,240 tracks, some 13 CPU-seconds in a release build.
    #[doc(hidden)]
    #[must_use]
    pub fn build_with(fe_h: &[f64], samples_per_phase: u32) -> Self {
        let masses = mass_nodes();
        let raw = masses
            .iter()
            .map(|&m| raw_node(m, fe_h, samples_per_phase))
            .collect();
        Self::assemble(masses, raw)
    }

    /// The envelope from each node's own brightest magnitudes, `raw[j]` being [`raw_node`] at
    /// `masses[j]`: each node's are spread over ages within [`AGE_SPREAD_FACTOR`], made a running
    /// minimum over mass and brightened by [`MARGIN_MAG`]. Nodes may be built in any order or in
    /// parallel; this reads them in mass order.
    ///
    /// # Panics
    ///
    /// If `masses` and `raw` differ in length, or `masses` is empty or not increasing.
    #[doc(hidden)]
    #[must_use]
    pub fn assemble(masses: Vec<f64>, raw: Vec<[f64; AGE_BINS + 1]>) -> Self {
        assert_eq!(masses.len(), raw.len(), "one row per mass node");
        assert!(
            !masses.is_empty() && masses.windows(2).all(|w| w[0] < w[1]),
            "the mass nodes increase"
        );
        let mut brightest = raw;
        let spread = age_spread_bins();
        for bins in &mut brightest {
            let narrow = *bins;
            for (k, bin) in bins.iter_mut().enumerate() {
                // The young bin, 0–10⁴ years, takes only its neighbour.
                let lo = k.saturating_sub(spread).max(usize::from(k > 0));
                let hi = (k + spread).min(AGE_BINS);
                *bin = narrow[lo..=hi].iter().fold(f64::INFINITY, |a, &b| a.min(b));
            }
        }
        for j in 1..brightest.len() {
            let lighter = brightest[j - 1];
            for (bin, &v) in brightest[j].iter_mut().zip(&lighter) {
                *bin = bin.min(v);
            }
        }
        for bins in &mut brightest {
            for v in bins.iter_mut() {
                *v -= MARGIN_MAG;
            }
        }
        Self::from_rows(masses, brightest)
    }

    /// The nodes' masses, M☉, ascending, and per node and age bin the brightest M<sub>V</sub>,
    /// margin included (+∞ where none shines): what the fitted table stores.
    #[doc(hidden)]
    #[must_use]
    pub fn rows(&self) -> (&[f64], &[[f64; AGE_BINS + 1]]) {
        (&self.masses, &self.brightest)
    }

    /// The brightest absolute V magnitude, margin included, of any star of at most
    /// `mass_at_most` aged within `ages` (years, inclusive), or `None` if none shines in V.
    ///
    /// `layer` and `component` say whose stars are asked about: the envelope is the same for all
    /// (see the [module](self) documentation), and the arguments keep the caller's question
    /// whole.
    #[must_use]
    pub fn brightest(
        &self,
        _layer: Layer,
        _component: ComponentId,
        mass_at_most: SolarMasses,
        ages: (Years, Years),
    ) -> Option<Magnitudes> {
        let node = self.node_at_or_above(mass_at_most.value());
        let (lo, hi) = age_bin_range(ages)?;
        let v = self.brightest[node][lo..=hi]
            .iter()
            .fold(f64::INFINITY, |a, &b| a.min(b));
        v.is_finite().then(|| Magnitudes::new(v))
    }

    /// The least primary initial mass, M☉, of `layer`'s band whose system could hold a star as
    /// bright as `faintest` (absolute V) at ages within `ages`: every lighter primary's system is
    /// fainter, so a census skips it. The band's upper edge where no primary of the band can, and
    /// its lower edge where every one can.
    ///
    /// A system's brightest star is at most [`max_star_mass_in`] of its layer and primary: twice
    /// its mass where a pair can merge or feed one star, and its own where the layer's systems are
    /// all single, as the brown dwarfs' are. A primary can hold a star as bright as `faintest` when
    /// a star of up to that mass can be, and the floor is the lightest such primary.
    ///
    /// Asked about every age at once, as the census's cell floor asks, it reads one value a node,
    /// its brightest over every age bin, and gives bit for bit what a scan of the bins gives
    /// (R06.T8.f): the bisection is the same, and so is each of its answers.
    #[must_use]
    pub fn mass_floor(
        &self,
        layer: Layer,
        component: ComponentId,
        faintest: Magnitudes,
        ages: (Years, Years),
    ) -> SolarMasses {
        let band = MassBand::from(layer);
        let star_mass = |m1: f64| max_star_mass_in(layer, SolarMasses::new(m1)).value();
        if age_bin_range(ages) != Some((0, AGE_BINS)) {
            return bisect_floor(band, |m1| {
                self.brightest(layer, component, SolarMasses::new(star_mass(m1)), ages)
                    .is_some_and(|v| v.value() <= faintest.value())
            });
        }
        // Every age: a node passes when its brightest ever is finite and at most `faintest`, and
        // since that never rises with mass, the nodes that pass are those from `first` on. A
        // primary passes when the node at or above its star mass does, so when the node below
        // `first` is lighter than that mass: the scan's answer at every step of the bisection.
        let first = self
            .brightest_ever
            .partition_point(|&v| !(v.is_finite() && v <= faintest.value()));
        match first {
            0 => bisect_floor(band, |_| true),
            f if f >= self.masses.len() => bisect_floor(band, |_| false),
            f => {
                let below = self.masses[f - 1];
                bisect_floor(band, |m1| below < star_mass(m1))
            }
        }
    }

    /// [`mass_floor`](Self::mass_floor) by its scan of the age bins alone, whatever the ages: the
    /// reading it is held to bit for bit.
    #[cfg(test)]
    #[must_use]
    fn mass_floor_by_scan(
        &self,
        layer: Layer,
        component: ComponentId,
        faintest: Magnitudes,
        ages: (Years, Years),
    ) -> SolarMasses {
        bisect_floor(MassBand::from(layer), |m1| {
            self.brightest(
                layer,
                component,
                max_star_mass_in(layer, SolarMasses::new(m1)),
                ages,
            )
            .is_some_and(|v| v.value() <= faintest.value())
        })
    }

    /// The bytes the envelope owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        (self.masses.capacity() + self.brightest_ever.capacity()) * size_of::<f64>()
            + self.brightest.capacity() * size_of::<[f64; AGE_BINS + 1]>()
    }

    /// The first node at or above `m`, or the last.
    #[must_use]
    fn node_at_or_above(&self, m: f64) -> usize {
        self.masses
            .partition_point(|&node| node < m)
            .min(self.masses.len() - 1)
    }
}

/// The least primary mass of `band` that `passes`, by bisection in log mass, as
/// [`BrightnessEnvelope::mass_floor`] defines it: the band's lower edge if it passes, its upper
/// edge if that does not, and otherwise the last mass found not to pass after 48 halvings.
/// `passes` is monotone in the primary's mass, the envelope being a running minimum over mass and
/// a system's heaviest star rising with its primary.
#[must_use]
fn bisect_floor(band: MassBand, passes: impl Fn(f64) -> bool) -> SolarMasses {
    let (lo, hi) = (band.lo(), band.hi());
    if passes(lo) {
        return SolarMasses::new(lo);
    }
    if !passes(hi) {
        return SolarMasses::new(hi);
    }
    let (mut below, mut above) = (lo, hi);
    for _ in 0..48 {
        let mid = (below * above).sqrt();
        if passes(mid) {
            above = mid;
        } else {
            below = mid;
        }
    }
    SolarMasses::new(below)
}

/// The envelope's mass nodes: even in ln m over 0.0124–150 M☉, with every band edge added.
#[doc(hidden)]
#[must_use]
pub fn mass_nodes() -> Vec<f64> {
    let (lo, hi) = (MassBand::BrownDwarf.lo(), MassBand::E.hi());
    let (ln_lo, ln_hi) = (math::ln(lo), math::ln(hi));
    #[expect(
        clippy::cast_precision_loss,
        reason = "the node count, 192, is exact in f64"
    )]
    let steps = (MASS_NODES - 1) as f64;
    let mut masses: Vec<f64> = (0..MASS_NODES)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "k < 192")]
            let k = k as f64;
            if k >= steps {
                hi
            } else {
                math::exp(ln_lo + (ln_hi - ln_lo) * k / steps)
            }
        })
        .collect();
    for band in MassBand::ALL {
        masses.push(band.lo());
        masses.push(band.hi());
    }
    masses.push(MIN_INITIAL_MASS.value());
    masses.sort_by(f64::total_cmp);
    masses.dedup();
    masses
}

/// One mass node's own brightest absolute V magnitude per age bin, before the spread over ages,
/// the running minimum over mass and the margin ([`BrightnessEnvelope::assemble`]): the brightest
/// of the tracks of mass `m` at the metallicities `fe_h` and every draw of [`ETA_DRAWS`], each
/// phase cut into `samples_per_phase` parts, and of the cooling fits at and below 0.1 M☉; +∞ in
/// a bin where none shines. A pure function of its arguments, which the fit builds node by node
/// in parallel.
#[doc(hidden)]
#[must_use]
pub fn raw_node(m: f64, fe_h: &[f64], samples_per_phase: u32) -> [f64; AGE_BINS + 1] {
    let mut bins = [f64::INFINITY; AGE_BINS + 1];
    for &z in fe_h {
        let composition = Composition::from_fe_h(Dex::new(z), HeliumExcess::ZERO);
        if m <= MIN_INITIAL_MASS.value() {
            // At 0.1 M☉ itself the tracks and the cooling fits meet, and stars just below it
            // follow the fits, which are brighter there.
            enter_cooling(m.min(COOLING_TOP_MSUN), &composition, &mut bins);
            if m < MIN_INITIAL_MASS.value() {
                continue;
            }
        }
        for &eta in &ETA_DRAWS {
            let draws = StarDraws::from_parts(StarDrawsParts {
                eta: StandardNormal::new(eta).expect("finite"),
                ..StarDrawsParts::MEDIAN
            });
            let track = Track::to_age(
                SolarMasses::new(m),
                &composition,
                &draws,
                Years::new(MAX_AGE_YEARS),
            );
            enter_track(&track, samples_per_phase, &mut bins);
        }
    }
    bins
}

/// The fitted table's value for `v`, an envelope's magnitude: `v` in integer millimagnitudes
/// rounded toward −∞, and lowered further should [`from_millimag`] of it not be at most `v`, so
/// the table is never fainter than the build and stays a bound; `None` for +∞, a bin where none
/// shines, which the table writes as its `DARK`.
///
/// # Panics
///
/// If `v` is NaN, −∞, or beyond ±2 × 10⁶ mag.
#[doc(hidden)]
#[must_use]
pub fn to_millimag(v: f64) -> Option<i32> {
    if v == f64::INFINITY {
        return None;
    }
    assert!(v.is_finite() && v.abs() < 2e6, "an envelope magnitude: {v}");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "|v| < 2e6 mag, so |1000 v| < 2^31, and floor makes it an integer"
    )]
    let mut k = (v * 1000.0).floor() as i32;
    while from_millimag(k) > v {
        k -= 1;
    }
    Some(k)
}

/// The magnitude of a fitted table's value `k`, millimagnitudes, +∞ for its `DARK`.
#[doc(hidden)]
#[must_use]
pub fn from_millimag(k: i32) -> f64 {
    if k == sky_envelope::DARK {
        f64::INFINITY
    } else {
        f64::from(k) / 1000.0
    }
}

/// The bin holding `age`, years.
#[must_use]
fn age_bin(age: f64) -> usize {
    if age.is_nan() || age < FIRST_AGE_YEARS {
        return 0;
    }
    let x = math::ln(age / FIRST_AGE_YEARS) / math::ln(MAX_AGE_YEARS / FIRST_AGE_YEARS);
    #[expect(
        clippy::cast_precision_loss,
        reason = "the bin count, 192, is exact in f64"
    )]
    let bins = AGE_BINS as f64;
    let k = (x * bins).floor();
    if k >= bins {
        return AGE_BINS;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "k lies in [0, 192)"
    )]
    let k = k.max(0.0) as usize;
    k + 1
}

/// The bins `ages` overlaps, or `None` for an empty or wholly negative range.
#[must_use]
fn age_bin_range((lo, hi): (Years, Years)) -> Option<(usize, usize)> {
    let (lo, hi) = (lo.value().max(0.0), hi.value());
    (hi >= lo && hi >= 0.0).then(|| (age_bin(lo), age_bin(hi)))
}

/// Enters `state`'s magnitude in `bins` from `a` to `b` years.
fn enter(bins: &mut [f64; AGE_BINS + 1], a: f64, b: f64, v: f64) {
    for bin in &mut bins[age_bin(a)..=age_bin(b)] {
        *bin = bin.min(v);
    }
}

#[must_use]
fn magnitude(state: &StarState) -> f64 {
    absolute_v_of_state(state).map_or(f64::INFINITY, Magnitudes::value)
}

/// Enters a track's life in `bins`: each part's brightest of five points over the part
/// ([`track_parts`]).
fn enter_track(track: &Track, samples_per_phase: u32, bins: &mut [f64; AGE_BINS + 1]) {
    for (a, b, v) in track_parts(track, samples_per_phase) {
        enter(bins, a, b, v);
    }
}

/// A track's life up to [`MAX_AGE_YEARS`] cut into parts, each `(from, to, v)`.
///
/// `from` and `to` are ages, years, and `v` the brightest absolute V of five points over the
/// part, its ends read just inside it in its own segment. Each phase is cut at its knots and into
/// `samples_per_phase` equal parts; a part where none of the five shines is left out. Shared by
/// this envelope and the phase envelope ([`super::phase`]), so the two read a track alike.
#[must_use]
pub(crate) fn track_parts(track: &Track, samples_per_phase: u32) -> Vec<(f64, f64, f64)> {
    let built = track.built_until().value().min(MAX_AGE_YEARS);
    let mut parts = Vec::new();
    let mut cuts = Vec::new();
    for (start, end, knots) in track.segment_ages() {
        let end = end.min(built);
        if end <= start || end.is_nan() {
            continue;
        }
        cuts.clear();
        cuts.extend(
            (0..=samples_per_phase)
                .map(|k| start + (end - start) * f64::from(k) / f64::from(samples_per_phase)),
        );
        cuts.extend(knots.filter(|&age| age > start && age < end));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let v = [0.0, 0.25, 0.5, 0.75, 1.0]
                .iter()
                .map(|f| {
                    // Each end is read just inside the part, in its own segment.
                    let t = (a + (b - a) * f).clamp(a + (b - a) * 1e-9, b - (b - a) * 1e-9);
                    magnitude(&track.state_at(Years::new(t)))
                })
                .fold(f64::INFINITY, f64::min);
            if v.is_finite() {
                parts.push((a, b, v));
            }
        }
    }
    parts
}

/// Enters an object below 0.1 M☉, on plan 06's cooling fits, in `bins`: its magnitude falls with
/// age, so each bin takes it at its young edge.
fn enter_cooling(mass: f64, composition: &Composition, bins: &mut [f64; AGE_BINS + 1]) {
    let state_at = |age: f64| {
        substellar::cooling(SolarMasses::new(mass), Years::new(age), composition)
            .expect("a mass of 0.01-0.1 M_sun at a finite positive age is inside the fits")
    };
    for (k, bin) in bins.iter_mut().enumerate() {
        let young = if k == 0 {
            1.0
        } else {
            #[expect(clippy::cast_precision_loss, reason = "k ≤ 192")]
            let x = (k - 1) as f64 / AGE_BINS as f64;
            FIRST_AGE_YEARS * math::exp(x * math::ln(MAX_AGE_YEARS / FIRST_AGE_YEARS))
        };
        *bin = bin.min(magnitude(&state_at(young)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;

    /// The populations' reference metallicities lie inside the envelope's span (a check that the
    /// span covers what the components are centred on).
    #[must_use]
    fn covers_reference_metallicities() -> bool {
        let (lo, hi) = (FE_H_NODES[0], FE_H_NODES[FE_H_NODES.len() - 1]);
        crate::galaxy::POPULATIONS
            .iter()
            .map(|&p| crate::galaxy::fates::reference_fe_h(p).value())
            .all(|z| (lo..=hi).contains(&z))
    }

    fn envelope() -> &'static BrightnessEnvelope {
        crate::sky::testing::milky_way_envelope()
    }

    fn any_component() -> ComponentId {
        milky_way_galaxy()
            .fields()
            .component_ids()
            .next()
            .expect("a component")
    }

    fn years(lo: f64, hi: f64) -> (Years, Years) {
        (Years::new(lo), Years::new(hi))
    }

    #[test]
    fn the_envelope_brightens_with_mass_and_bounds_the_sun() {
        let e = envelope();
        let c = any_component();
        let sun = e
            .brightest(Layer::C, c, SolarMasses::new(1.0), years(4.5e9, 4.7e9))
            .expect("the Sun shines");
        // The Sun is M_V 4.8 today; the envelope is brighter by its margin at least.
        assert!(sun.value() < 4.8 - MARGIN_MAG + 0.05, "{sun:?}");
        let mut previous = f64::INFINITY;
        for m in [0.2, 0.5, 1.0, 2.0, 5.0, 20.0, 100.0] {
            let v = e
                .brightest(Layer::E, c, SolarMasses::new(m), years(1e6, 1e10))
                .expect("shines")
                .value();
            assert!(v <= previous, "{m}: {v} after {previous}");
            previous = v;
        }
        // Massive stars reach about M_V −10 at the top.
        assert!(previous < -9.0, "{previous}");
    }

    #[test]
    fn the_mass_floor_rises_as_the_cut_brightens() {
        let e = envelope();
        let c = any_component();
        let ages = years(1e8, 1e10);
        let mut previous = 0.0;
        for faintest in [10.0, 5.0, 2.0, 0.0, -2.0] {
            let floor = e
                .mass_floor(Layer::C, c, Magnitudes::new(faintest), ages)
                .value();
            assert!(floor >= previous, "{faintest}: {floor} below {previous}");
            assert!((MassBand::C.lo()..=MassBand::C.hi()).contains(&floor));
            previous = floor;
        }
        // Every primary of layer A shines brighter than V 20 at some age; none passes −15.
        assert!(
            (e.mass_floor(Layer::A, c, Magnitudes::new(20.0), ages)
                .value()
                - MassBand::A.lo())
            .abs()
                < 1e-12
        );
        assert!(
            (e.mass_floor(Layer::A, c, Magnitudes::new(-15.0), ages)
                .value()
                - MassBand::A.hi())
            .abs()
                < 1e-12
        );
    }

    #[test]
    fn an_empty_age_range_has_no_brightest_star() {
        let e = envelope();
        assert_eq!(
            e.brightest(
                Layer::A,
                any_component(),
                SolarMasses::new(0.4),
                years(-10.0, -1.0)
            ),
            None
        );
    }

    #[test]
    fn the_span_of_metallicities_covers_every_population() {
        assert!(covers_reference_metallicities());
    }

    /// The fitted table sits on the build's own mass nodes, bit for bit, and is a running
    /// minimum over mass, so a heavier node is never fainter and a dark bin has only dark lighter
    /// nodes.
    #[test]
    fn the_fitted_table_is_on_the_mass_nodes_and_falls_with_mass() {
        let e = BrightnessEnvelope::fitted();
        let (masses, rows) = e.rows();
        let nodes = mass_nodes();
        assert_eq!(masses.len(), nodes.len());
        assert!(
            masses
                .iter()
                .zip(&nodes)
                .all(|(a, b)| a.total_cmp(b).is_eq()),
            "the table's masses are mass_nodes()"
        );
        assert_eq!(rows.len(), masses.len());
        for pair in rows.windows(2) {
            for (heavier, lighter) in pair[1].iter().zip(&pair[0]) {
                assert!(heavier <= lighter, "{heavier} after {lighter}");
            }
        }
        assert!(rows.iter().flatten().all(|v| !v.is_nan()));
    }

    /// `to_millimag` rounds brighter, by under a millimagnitude plus a rounding, and
    /// `from_millimag` reads it back; +∞ is the table's `DARK`.
    #[test]
    fn millimagnitudes_round_brighter() {
        for v in [
            -12.85,
            -0.0005,
            0.0,
            0.001,
            4.83,
            7.0 / 3.0,
            16.663,
            29.999_999_9,
        ] {
            let k = to_millimag(v).expect("finite");
            let stored = from_millimag(k);
            assert!(stored <= v, "{stored} above {v}");
            assert!(v - stored < 0.001 + 1e-12, "{stored} far below {v}");
        }
        assert_eq!(to_millimag(f64::INFINITY), None);
        assert_eq!(to_millimag(4.83), Some(4830));
        assert!(from_millimag(sky_envelope::DARK).is_infinite());
    }

    /// A raw node is the same whatever was built before it, so the fit may build the nodes in any
    /// order or in parallel before assembling them in mass order (the slow
    /// `the_fitted_envelope_is_the_build_rounded_brighter` compares the result with the build).
    #[test]
    fn raw_nodes_are_order_independent() {
        hyperion_testkit::order::assert_order_independent(&[0.05, 0.1, 0.3, 1.0], |&m| {
            raw_node(m, &[0.0], 2)
        });
    }

    #[test]
    fn max_star_mass_is_twice_the_primary_up_to_the_tracks_top() {
        assert_eq!(
            max_star_mass(SolarMasses::new(0.638)),
            SolarMasses::new(1.276)
        );
        assert_eq!(max_star_mass(SolarMasses::new(3.0)), SolarMasses::new(6.0));
        assert_eq!(
            max_star_mass(SolarMasses::new(75.0)),
            SolarMasses::new(150.0)
        );
        assert_eq!(max_star_mass(SolarMasses::new(120.0)), MAX_INITIAL_MASS);
        assert_eq!(max_star_mass(SolarMasses::new(150.0)), MAX_INITIAL_MASS);
    }

    /// The floor inverts the envelope at [`max_star_mass`]: a primary just above it could hold a
    /// star of twice its mass as bright as the faintest listable, and one just below could not.
    /// For some cuts the primary's own mass would not pass there, so the floor is lower than that
    /// mass alone would make it.
    #[test]
    fn the_mass_floor_admits_every_primary_whose_merger_could_pass() {
        let e = envelope();
        let c = any_component();
        let ages = years(0.0, MAX_AGE_YEARS);
        let shines = |m: f64, faintest: f64| {
            e.brightest(Layer::A, c, SolarMasses::new(m), ages)
                .is_some_and(|v| v.value() <= faintest)
        };
        let (mut inside, mut lowered) = (0, 0);
        for faintest in [9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0] {
            let floor = e
                .mass_floor(Layer::A, c, Magnitudes::new(faintest), ages)
                .value();
            if floor <= MassBand::A.lo() || floor >= MassBand::A.hi() {
                continue;
            }
            inside += 1;
            let above = max_star_mass(SolarMasses::new(floor * (1.0 + 1e-9))).value();
            let below = max_star_mass(SolarMasses::new(floor * (1.0 - 1e-6))).value();
            assert!(shines(above, faintest), "{faintest}: above {floor}");
            assert!(!shines(below, faintest), "{faintest}: below {floor}");
            lowered += usize::from(!shines(floor * (1.0 + 1e-9), faintest));
        }
        assert!(inside > 0, "no cut puts A's floor inside its band");
        assert!(lowered > 0, "the widened mass never lowers A's floor");
    }

    /// The floor over every age reads one value a node and gives the scan's answer bit for bit,
    /// at 10⁴ random queries over every layer (R06.T8.f), among them faintest magnitudes on the
    /// nodes' own values, where a comparison could tip.
    #[test]
    fn the_floor_over_every_age_is_the_scan_bit_for_bit() {
        let e = envelope();
        let c = any_component();
        let every_age = years(0.0, MAX_AGE_YEARS);
        let layers = [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
        ];
        let (masses, _) = e.rows();
        let mut u = crate::sky::testing::uniforms(0x7a8f);
        let mut inside = 0;
        for k in 0..10_000_usize {
            let layer = layers[k % layers.len()];
            let faintest = if k % 7 == 0 {
                // A node's brightest ever, exactly.
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss,
                    reason = "an index below the node count, some 200"
                )]
                let j = (u.next().expect("endless") * masses.len() as f64) as usize;
                e.brightest_ever[j.min(masses.len() - 1)]
            } else {
                -16.0 + 40.0 * u.next().expect("endless")
            };
            let faintest = Magnitudes::new(faintest);
            let fast = e.mass_floor(layer, c, faintest, every_age);
            let scan = e.mass_floor_by_scan(layer, c, faintest, every_age);
            assert_eq!(
                hyperion_testkit::float::bits(fast.value()),
                hyperion_testkit::float::bits(scan.value()),
                "{layer:?} at {faintest:?}: {fast:?} against {scan:?}"
            );
            let band = MassBand::from(layer);
            inside += usize::from(fast.value() > band.lo() && fast.value() < band.hi());
        }
        // Most queries bisect, so the comparison is made at every step of many bisections.
        assert!(inside > 1_000, "{inside} floors inside their bands");
        // A narrower range of ages takes the scan itself.
        let some_ages = years(1e8, 1e10);
        let f = Magnitudes::new(3.0);
        assert_eq!(
            e.mass_floor(Layer::C, c, f, some_ages),
            e.mass_floor_by_scan(Layer::C, c, f, some_ages)
        );
    }

    #[test]
    fn a_systems_heaviest_star_is_its_primary_where_every_system_is_single() {
        let m = SolarMasses::new(0.05);
        assert!(always_single(Layer::BrownDwarf) && always_single(Layer::RoguePlanet));
        assert_eq!(max_star_mass_in(Layer::BrownDwarf, m), m);
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            assert!(!always_single(layer), "{layer:?}");
            let m1 = SolarMasses::new(MassBand::from(layer).lo());
            assert_eq!(max_star_mass_in(layer, m1), max_star_mass(m1), "{layer:?}");
        }
    }

    /// The brown dwarfs' floor inverts the envelope at each primary's own mass, which is higher
    /// for some cuts than the floor at twice it would be (R06.T16.b's open item, closed in
    /// R06.T8.f): their systems are single, so no merger or accretor of twice the mass exists.
    #[test]
    fn the_brown_dwarfs_floor_reads_each_primarys_own_mass() {
        let e = envelope();
        let c = any_component();
        let ages = years(0.0, MAX_AGE_YEARS);
        let band = MassBand::BrownDwarf;
        let shines = |m: f64, faintest: f64| {
            e.brightest(Layer::BrownDwarf, c, SolarMasses::new(m), ages)
                .is_some_and(|v| v.value() <= faintest)
        };
        let mut raised = 0;
        for faintest in [20.0, 18.0, 16.0, 14.0, 12.0, 10.0, 8.0] {
            let floor = e
                .mass_floor(Layer::BrownDwarf, c, Magnitudes::new(faintest), ages)
                .value();
            if floor <= band.lo() || floor >= band.hi() {
                continue;
            }
            assert!(
                shines(floor * (1.0 + 1e-9), faintest),
                "{faintest}: {floor}"
            );
            assert!(
                !shines(floor * (1.0 - 1e-6), faintest),
                "{faintest}: {floor}"
            );
            // Read at twice each primary's mass, as before R06.T8.f, the floor would be lower.
            let doubled = bisect_floor(band, |m1| {
                e.brightest(
                    Layer::BrownDwarf,
                    c,
                    max_star_mass(SolarMasses::new(m1)),
                    ages,
                )
                .is_some_and(|v| v.value() <= faintest)
            })
            .value();
            assert!(doubled <= floor, "{faintest}: {doubled} above {floor}");
            raised += usize::from(doubled < floor);
        }
        assert!(raised > 0, "the primary's own mass never raises the floor");
    }
}
