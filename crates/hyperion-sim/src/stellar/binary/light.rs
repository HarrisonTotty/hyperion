//! The census's bound on a pair's light (plan 11, P11.T17; rendering plan R06's ask B,
//! `decision-r06-census-cost.md` §7): before a system is generated, whether a pair of two stars
//! can hold a star bright enough to list, from fitted tables alone.
//!
//! The interface is fixed here, in P11.T17.a, and is unchanged by T17.b and T17.c (decided
//! 2026-10-07, `decision-p11-t16-hierarchy-bound.md` §5). [`pair_light_bound`] takes the two
//! stars' initial masses as P11.T16's `hierarchy_bound` lists them, bit for bit, and the pair's
//! drawn periastron, and answers `None` where the pair cannot be bounded, so that R06.T8.g
//! generates its record, or one of four verdicts:
//!
//! - [`PairLight::Detached`]: the pair cannot have interacted by the window's end, by the
//!   closed-form, conservative test below. Each star is its own single-star model.
//! - [`PairLight::Unchanged`]: each star of the pair living in the window is its own single-star
//!   model, and no product of the pair lives in it. The census treats it as `Detached`.
//! - [`PairLight::Remnants`]: no star or product of the pair lives in the window.
//! - [`PairLight::Bright`]: a living star may depart from its own single-star model, or a product
//!   may live. Each such star and product is no brighter than the magnitude, margin included; the
//!   census bounds each living star of the pair by the brighter of its own bound and it.
//!
//! T17.a answers `Detached`; T17.c adds `Unchanged`, `Remnants` and `Bright` from T17.b's
//! tables (below).
//!
//! # `Detached`
//!
//! A pair is two single stars wherever [`can_interact`](super::can_interact) passes it over
//! (design note 7): the engine runs no step of it before that age, and a run to a later age shows
//! no interaction before it (P11.T4.j's build-age contract), its stars of at least 0.1 M☉
//! accreting no wind while on their own tracks. The bound answers `Detached` only where that test
//! fails for **every** orbit
//! of periastron at least `periastron_min`, every eccentricity, every η and every mass of the
//! intervals given, with each star's largest radius R̂ᵢ from the reach table
//! ([`largest_radius_bound`](super::largest_radius_bound)) at the window's end, which bounds the
//! radius the engine reads. Writing `r_p` for the periastron, a pair is `Detached` when:
//!
//! 1. **The lobe test fails** (Eggleton 1983): R̂ᵢ < `f_L`(qᵢ,min) `periastron_min` for both stars,
//!    with qᵢ,min the least ratio mᵢ ÷ mⱼ of the intervals. The engine's test reaches at `r_p` if
//!    Rᵢ ≥ `f_L`(qᵢ) `r_p`, so it reaches at no `r_p` ≥ `periastron_min`.
//! 2. **P11.T4.j's coarse decay test fails** at `periastron_min`, every term at its largest over
//!    the eccentricity e. The engine's test reads the semi-latus rectum p₀ = `r_p` (1 + e) and the
//!    critical `p_c` = `r_c` (1 + e), `r_c` = maxᵢ Rᵢ ÷ `f_L`(qᵢ); divided by (1 + e)⁵ throughout:
//!    - the spins' reservoir takes a share Φ I ÷ (μ (1 + e)² `r_c`^{3/2} √`r_p`) of the orbit's
//!      angular momentum, with Φ = f₂(e) ÷ f₅(e) (Hut 1981; BSE equation 34) and I = Σ kᵢ mᵢ Rᵢ²
//!      (BSE equation 35's coarse bound). Since Φ ÷ (1 + e)² ≤ 1 and `r_c` ≥ Rᵢ ÷ `f_L`(qᵢ),
//!      the share is at most c ÷ √`r_p`, c = Σᵢ kᵢ √R̂ᵢ `f_L`(qᵢ,max)^{3/2} (1 + qᵢ,max),
//!      whichever star sets `r_c`, with kᵢ the core's k′₃ = 0.21 where the table says a star may
//!      have left its main sequence and the envelope's k′₂ = 0.1 elsewhere;
//!    - magnetic braking (BSE equation 50) at the equilibrium spin carries Φ³, and Φ³ ÷ (1 + e)⁵
//!      is at most [`BRAKING_ECCENTRICITY_FACTOR`], its value as e → 1; its integral is at most
//!      Σ R̂ᵢ³ over the whole age, of each star above BSE's 0.35 M☉ floor, with M ÷ μ at its
//!      largest over the intervals;
//!    - gravitational radiation (BSE equation 48) carries (1 + 7e²/8)(1 + e), so its term
//!      divided by (1 + e)⁵ is at most its value at e = 0, 10 β′ m₁ m₂ M `r_p` over the age.
//!
//!    The engine's test reaches if (`r_p` (1 − x)²)⁵ − 1.25 (B + W) ≤ `r_c`⁵, x the reservoir's
//!    share. With the bounds above that is at most (√`r_p` − c)¹⁰ − 1.25 (B̂ + Ŵ `r_p`) ≤
//!    `r̂_c`⁵, whose left side grows with `r_p` wherever its derivative at `periastron_min` is
//!    non-negative (that derivative itself grows with `r_p`), which the bound checks. So a pair that fails it at
//!    `periastron_min` fails it at every `r_p` beyond. A pair whose reservoir could take the whole
//!    orbit (c ≥ √`periastron_min`) is not `Detached`.
//!
//! 3. **No star can have died suddenly by the window's end** (a core's collapse, an electron
//!    capture or a pair instability, the reach table's earliest collapse of each star's cell). A
//!    kick can leave an eccentric orbit whose periastron the drawn one does not bound, and the
//!    engine runs such a pair from zero age when it can interact by the end of the clock window
//!    or holds a remnant by then (`run_pairs`): a later transfer onto the remnant would erode the
//!    companion before the window's end. The pre-test's build-age contract, and P11.T4.j's
//!    zero-miss gate, cover interactions before the pair's first supernova only. So such a pair is
//!    `None` (`decision-p11-t16-hierarchy-bound.md` §5). A white dwarf's formation never brings
//!    the periastron in: under isotropic, kick-free mass loss at any rate the periastron does not
//!    decrease (Veras et al. 2011, MNRAS 417, 2104, equation 21), the engine models no white
//!    dwarf's kick, and its stars of at least 0.1 M☉ accrete no wind on their own tracks. So the
//!    test at the drawn periastron, with both stars' largest radii, covers it. (Real white dwarfs
//!    may take kicks of about 1 km s⁻¹, El-Badry and Rix 2018, MNRAS 480, 4884, which this
//!    bound, like the engine, leaves out.)
//! 4. **A pair with a star below 0.1 M☉ cannot interact by +H** (R06.T8.g's science check,
//!    adopted 2026-10-08). Such a star follows P06.T13's cooling fits as a member that carries
//!    mass, and in a pair the engine steps it accretes its companion's wind (BSE equation 6) even
//!    while the pair is detached: some 10⁻³ M☉ at most, about 0.15–0.2 mag in V at 0.08–0.1 M☉
//!    (Baraffe et al. 2015 at 5 Gyr, with Pecaut and Mamajek 2013's BC<sub>V</sub>; R06.T8.g's
//!    check had 0.05–0.1) and more just above the hydrogen-burning limit, which no bound here
//!    proves inside a margin. `run_pairs` runs a pair that holds a remnant or can interact by the
//!    pair's age at +H, the end of the clock window, which may lie after the window's end. A pair
//!    it runs for its remnant alone is still one detached segment of its own models
//!    ([`evolve`](super::evolve)'s pre-test gate), whose cooling member accretes nothing; the
//!    engine steps the pair only if it can interact by +H. The window's ages are light-time ages,
//!    each the age at an emission time of the source horizon (−(H + L) to +H), so the age at +H
//!    lies at most [`RUN_HORIZON_PAST_WINDOW_YEARS`], 2H + L, past the window's end. So such a
//!    pair is `Detached` only if items 1–3 also hold there. Otherwise it is read from the tables,
//!    whose walk counts the accretion as a departure and whose cooling floor bounds its V in every
//!    bin where a sample of the cell's neighbourhood departed. A bin where none did stays DARK,
//!    and reads `Unchanged`: the samples are run to 1.5 × 10¹⁰ years, so they are stepped at least
//!    as often as a pair run to its +H, apart from draws the fit leaves out (η below −3.5σ, e above
//!    0.95; plan 11's Risks, "P11.T17.b as built").
//!
//! Every term is monotone in the R̂ᵢ, the masses and the age, so later ages and smaller periastra
//! only take pairs out of `Detached`. The engine's own span starts at the first arrival, after
//! zero age, so the whole age bounds it. The test is the engine's at version 21, before P11.T4.l's
//! wind-spin term and its passed-over collapses: its slow test runs again when they land.
//!
//! # `Unchanged`, `Remnants` and `Bright` (P11.T17.c)
//!
//! Past `Detached` the bound reads P11.T17.b's tables ([`pair_light`](super::pair_light)), which
//! were sampled through the engine and are certified statistically
//! (`decision-p11-t16-hierarchy-bound.md` §5). The table is the layer's whose band holds the
//! heavier star's initial mass (C 0.75–2.5, D 2.5–8, E 8–150 M☉, C's at 2.5 M☉), and the cell the
//! pair's ([`PairLightTable::cell_of`]). Every age bin from the window's start to its end is read
//! through the readers that apply the cooling floor below 0.1 M☉
//! ([`PairLightTable::living_cmag_for`], [`PairLightTable::changed_cmag_for`]), E's read margin
//! and smear already in their values (the orchestrator's ruling of 2026-10-08: T17.b's read-side
//! widenings stand at version 21, and the version-22 refit stores them):
//!
//! - `Remnants` where the living value is DARK in every bin: neither star's own model nor any
//!   departing star or product lives there;
//! - `Unchanged` where the changed value is DARK in every bin;
//! - `None` where a bin's changed value is not DARK and the cell's own samples hold only one to
//!   three living departing stars or products there (count class at most
//!   [`THIN_DEPARTING_CLASS`]): a rare channel of the cell, whose brightest is poorly sampled, is
//!   within the margin of `Unchanged`'s boundary;
//! - otherwise `Bright(M)`, M the brightest changed value over the bins, margin included. A
//!   departing star with no V (a protostar, or one cooler than the photometry's tables) gives the
//!   faintest V the table stores, +20.45 ([`FAINTEST_CMAG`]): it has none to list. A bin whose
//!   changed value comes only from the cell's neighbours (its own samples hold none) is as
//!   consistent with `Unchanged` as with `Bright`, and reads `Bright`.
//!
//! The tables answer `None` outside them: a heavier star outside 0.75–150 M☉, a periastron below
//! 1 R☉, a window ending past 1.5 × 10¹⁰ years, a layer whose table is a placeholder, or masses
//! given as intervals ([`pair_light_bound_over`] reads the tables for exact masses only).
//!
//! **The held floor** (P11.T17.c's follow-up, 2026-10-08, after R06.T8.g's slow test found a
//! bulge pair of layer D whose stars shone at M<sub>V</sub> 1.3 and 2.5 at 8.7 Gyr where the
//! tables read `Remnants`). A timeline that reaches the engine's cap on segments is left as it
//! stands, its last segment stretched to the age asked, and a star the binary carries on its main
//! sequence there, hydrogen or helium, is held at its last mass and fractional age for ever. The
//! engine reaches the cap where an accretor so carried is fed at its main sequence's end, each
//! step landing short of that end once the accretion has lowered its fractional age (plan 11's
//! Risks: finding F15 for a helium accretor, and the same for a hydrogen one), and in P11.T4.g's
//! swell and strip cycle (finding C2). About 3 × 10⁻⁵ of the census's pairs do, too few for the
//! tables' 72 samples a cell to hold, and a held gainer reaches M<sub>V</sub> −7.6. So where the
//! closed form does not call the pair `Detached` at the window's end, each bin from
//! [`pair_light::held_floor`]'s first one reads its living and changed values as no fainter than
//! the floor: the brightest hydrogen or helium main-sequence star of at most the pair's total
//! mass, from the engine's own models, margin included. A pair the closed form calls `Detached`
//! there has not been stepped by then, so it holds no such star, and reads the tables as they
//! are. `Remnants` is then answered only by such pairs, and `Unchanged` only before any star of the
//! pair can have ended its main sequence. The floor goes when the version-22 batch's engine work
//! removes the cap's causes.
//!
//! **White dwarfs are dark.** The tables hold no remnant's light, so all three verdicts hold only
//! while white dwarfs have no V (R06's ask A4). A white dwarf the binary makes can be far younger
//! and hotter than either star's own, or exist where the star's own model is still a star (a
//! stripped donor's helium white dwarf). When A4 lands the tables are refitted with a white-dwarf
//! column (plan 11's Risks, "P11.T17.b as built").
//!
//! **The order, and inclusion monotonicity.** The tables' `Remnants` is asked first, then
//! `Detached`, then the tables' other answers. So each verdict only loosens as the window widens
//! (R06.T8.h's warm cache rests on it, `decision-r06-t8h-warm.md` §5): for windows A ⊆ A′, the
//! verdict over A′ is no tighter than over A, in the census's order `Remnants` < `Detached` =
//! `Unchanged` < `Bright(M)`, looser as M brightens, < `None`. `Remnants` over A′ holds over A,
//! since its bins are A′'s; `Detached` reads only the window's end, and only shrinks with it; a
//! wider window's M is the brightest over more bins, and the thin rule reads each bin alone. The
//! held floor is read only where the closed form fails at the window's end, which it does at every
//! later end: if it fails at A′'s end and holds at A's, A reads the tables as they are and answers
//! `Remnants` or `Detached`, no looser than A′'s verdict, which is not `Detached`.
//! Asked after `Detached`, `Remnants` over A′ would follow `Detached` over A, whose stars take
//! their own bounds, which may not be dark. A test holds the order over random nested windows.

use std::ops::RangeInclusive;

use crate::orbit::roche_lobe_radius;
use crate::stellar::Composition;
use crate::stellar::sse::{CORE_GYRATION, ENVELOPE_GYRATION, MIN_INITIAL_MASS};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M};
use crate::units::{Magnitudes, Metres, SolarMasses, Years};

use super::detached::{
    DECAY_SAFETY, GRAVITATIONAL_WAVE_RATE, MAGNETIC_BRAKING, MAGNETIC_BRAKING_FLOOR,
};
use super::pair_light::{
    self, DARK_CMAG, FAINTEST_CMAG, LAYERS as TABLE_LAYERS, PairLightTable, from_cmag,
};
use super::reach::{ReachBound, ReachTable, fe_h_of, has_helium_excess};
use super::star::{G, positive};

/// What a pair of two stars can hold in a window of ages, as the census reads it (plan 11,
/// P11.T17; `decision-p11-t16-hierarchy-bound.md` §5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PairLight {
    /// The pair cannot have interacted by the window's end: each star is its own single-star
    /// model, and takes its own bound.
    Detached,
    /// Each star of the pair living in the window is its own single-star model, and no product of
    /// the pair lives in it: the census treats it as `Detached` (P11.T17.c, from the tables,
    /// which hold no remnant's light, so while white dwarfs have no V, R06's ask A4).
    Unchanged,
    /// No star or product of the pair lives in the window (P11.T17.c, from the tables, while
    /// white dwarfs have no V, R06's ask A4).
    Remnants,
    /// A living star may depart from its own single-star model, or a product may live: each such
    /// star and product is no brighter than this absolute V magnitude at any age in the window,
    /// margin included. A star that does not depart is its own model; the census bounds each
    /// living star of the pair by the brighter of its own bound and this (P11.T17.c, from the
    /// tables, while white dwarfs have no V, R06's ask A4).
    Bright(Magnitudes),
}

/// The share of magnetic braking's term that eccentricity can add, at most, against a circular
/// orbit of the same periastron: Φ³ ÷ (1 + e)⁵, with Φ = f₂(e) ÷ f₅(e) (Hut 1981, A&A 99, 126),
/// has its supremum on [0, 1) as e → 1, where Φ = 14.4375 ÷ 4.375 = 3.3, so 3.3³ ÷ 32 =
/// 1.123 031 25, written here rounded up.
pub(crate) const BRAKING_ECCENTRICITY_FACTOR: f64 = 1.123_032;

/// How far past its window's end a pair's age at +H, the end of the clock window, can lie, years:
/// 2H + L = 264,144 years, the span of the source horizon, −(H + L) to +H
/// ([`SourceHorizon`](crate::time::SourceHorizon)).
///
/// The window's ages are light-time ages, each the pair's age at an emission time in the source
/// horizon, so its end is no earlier than the age at −(H + L), and the age at +H no later than
/// the end plus this. The module's item 4 reads it.
pub const RUN_HORIZON_PAST_WINDOW_YEARS: f64 = {
    let seconds = 2 * CLOCK_WINDOW_H.seconds() + LIGHT_CROSSING_L.seconds();
    #[expect(
        clippy::cast_precision_loss,
        reason = "8.3 × 10¹² s, below 2⁵³, is exact in an f64"
    )]
    let seconds = seconds as f64;
    seconds / SECONDS_PER_JULIAN_YEAR
};

/// The largest count class ([`pair_light::count_class`]) of a cell's own departing samples at an
/// age bin that the bound treats as too thin to bound (the module's `None`): class 2, one to three
/// of the cell's 72 samples.
///
/// Set by P11.T17.c, and recorded with its `None` share in plan 11's Risks.
pub(crate) const THIN_DEPARTING_CLASS: u8 = 2;

/// The bound on the pair of stars of initial masses `a` and `b`, of drawn periastron
/// `periastron_min`, of `composition`, over the window of their `ages` (plan 11, P11.T17): `None`
/// where the pair cannot be bounded, so that the census generates its record.
///
/// The masses are the two stars' initial masses as P11.T16's `hierarchy_bound` lists them, bit for
/// bit, in either order, which gives the same answer bit for bit; the periastron is the pair's
/// drawn one, and any orbit of a periastron at least it is bounded too. The ages are the record's
/// light-time ages: the age at +H lies at most [`RUN_HORIZON_PAST_WINDOW_YEARS`] past their end.
///
/// The verdicts are asked in an order that makes a wider window never give a tighter answer
/// (R06.T8.h's warm cache rests on it):
/// 1. [`PairLight::Remnants`], where P11.T17.b's tables ([`pair_light`](super::pair_light)) say
///    that nothing of the pair lives in any age bin of the window;
/// 2. [`PairLight::Detached`], by T17.a's closed form at the window's end: neither star's largest
///    radius from the reach table can reach the other's lobe, nor the orbit decay into reach, on
///    any orbit of the drawn periastron or wider, and no star may have died suddenly by then. A
///    pair with a star below 0.1 M☉, which accretes its companion's wind wherever the engine steps
///    the pair, must also pass that test [`RUN_HORIZON_PAST_WINDOW_YEARS`] later, at the latest
///    +H;
/// 3. from the tables again, [`PairLight::Unchanged`] where no departing star or product lives in
///    any bin, `None` where a bin's departing stars are too few among its cell's own samples to
///    bound (one to three of 72), and otherwise [`PairLight::Bright`] at the brightest departing
///    star or product of the bins, margin included.
///
/// Where the closed form fails at the window's end, the tables are read through the held floor
/// (the `pair_light` module's): a capped timeline may hold a main-sequence star of the pair for
/// ever, so from the first age any star of the pair can end
/// its main sequence, every bin holds a living, departing star as bright as such a star can be.
///
/// The tables cover a heavier star of 0.75–150 M☉, a periastron of at least 1 R☉ and a window
/// ending by 1.5 × 10¹⁰ years, for exact masses; outside them a pair is `Detached` or `None`.
/// `None` also answers a mass below 0.08 M☉ or not finite, a periastron that is not a positive
/// number, a window that ends past 10^10.2 years, starts after it ends or is not a number, and a
/// composition with a helium excess. The bound reads fitted tables only, and no stream.
///
/// # Panics
///
/// As [`PairLightTable::generator`], if a committed pair-light table cannot be read: a broken
/// invariant, which `hyperion-fit check` and the tests rule out.
///
/// # Examples
///
/// The census asks whether a solar-mass star and its 0.6 M☉ companion, 9 Gyr old, can still be two
/// single stars: 2,000 au apart at periastron they can; 0.04 au apart they may already have
/// interacted, and the tables say a star of theirs may then depart from its own model, or a
/// merger's product live, and how bright it can be.
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::{PairLight, pair_light_bound};
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{Metres, SolarMasses, Years};
///
/// let (sun, dwarf) = (SolarMasses::new(1.0), SolarMasses::new(0.6));
/// let au = METRES_PER_AU;
/// let ages = Years::new(9.0e9)..=Years::new(9.0e9);
/// let wide = pair_light_bound(sun, dwarf, Metres::new(2_000.0 * au), &Composition::SOLAR,
///     ages.clone());
/// assert_eq!(wide, Some(PairLight::Detached));
/// let close = pair_light_bound(sun, dwarf, Metres::new(0.04 * au), &Composition::SOLAR, ages);
/// assert!(matches!(close, Some(PairLight::Bright(m)) if m.value() > -10.0));
/// ```
#[must_use]
pub fn pair_light_bound(
    a: SolarMasses,
    b: SolarMasses,
    periastron_min: Metres,
    composition: &Composition,
    ages: RangeInclusive<Years>,
) -> Option<PairLight> {
    pair_light_bound_over([a..=a, b..=b], periastron_min, composition, ages)
}

/// [`pair_light_bound`] over every pair whose initial masses lie in `masses`, each interval its
/// ends included: what it answers holds for each such pair. An interval whose start lies above
/// its end is `None`. The tables are read for exact masses only, so a pair of intervals of some
/// width is `Detached` or `None`. The census passes exact masses
/// (`decision-p11-t16-hierarchy-bound.md` rejected an interval bound); the intervals let the
/// tests check that `Detached` only shrinks as they widen.
#[must_use]
pub(crate) fn pair_light_bound_over(
    masses: [RangeInclusive<SolarMasses>; 2],
    periastron_min: Metres,
    composition: &Composition,
    ages: RangeInclusive<Years>,
) -> Option<PairLight> {
    bound_with(masses, periastron_min, composition, ages, Tables::Read)
}

/// Whether a bound reads P11.T17.b's tables, or the closed form alone (for the tests of
/// `Detached`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tables {
    Read,
    #[cfg(test)]
    Skip,
}

/// [`pair_light_bound_over`], reading the tables or not.
#[must_use]
fn bound_with(
    masses: [RangeInclusive<SolarMasses>; 2],
    periastron_min: Metres,
    composition: &Composition,
    ages: RangeInclusive<Years>,
    tables: Tables,
) -> Option<PairLight> {
    let (start, end) = (ages.start().value(), ages.end().value());
    if start.is_nan() || end.is_nan() || start > end || has_helium_excess(composition) {
        return None;
    }
    let periastron_rsun = periastron_min.value() / SOLAR_RADIUS_M;
    if !positive(periastron_rsun) {
        return None;
    }
    let mut ends = masses.map(|m| [m.start().value(), m.end().value()]);
    if ends
        .iter()
        .any(|&[lo, hi]| !(lo > 0.0 && hi >= lo && hi.is_finite()))
    {
        return None;
    }
    // A fixed order, so that the answer is the same bit for bit whichever star is given first.
    ends.sort_by(|x, y| x[0].total_cmp(&y[0]).then(x[1].total_cmp(&y[1])));
    let fe_h = fe_h_of(composition);
    // The reach table bounds no window past its last age, and the tables reach less far.
    let until = end.max(0.0);
    let table = ReachTable::generator();
    let reach = [
        table.bound(ends[0][0], ends[0][1], fe_h, until)?,
        table.bound(ends[1][0], ends[1][1], fe_h, until)?,
    ];
    let exact = ends.iter().all(|&[lo, hi]| lo.total_cmp(&hi).is_eq());
    let detached = detached(ends, reach, periastron_rsun, fe_h, until);
    let read = if exact && tables == Tables::Read {
        // A pair that cannot have interacted by the window's end holds no capped timeline's
        // star in it (the module's held floor).
        let held = if detached {
            HeldStars::Impossible
        } else {
            HeldStars::Possible
        };
        tabled(
            ends[1][0],
            ends[0][0],
            fe_h,
            periastron_rsun,
            [start, end],
            held,
        )
    } else {
        None
    };
    // `Remnants` first, so that a wider window never answers tighter (the module's order).
    if read == Some(PairLight::Remnants) {
        return read;
    }
    if detached {
        return Some(PairLight::Detached);
    }
    read
}

/// Whether a capped timeline's held star may live in a pair's window (the module's held floor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeldStars {
    /// The pair may have interacted by the window's end, so the tables are read with the held
    /// floor.
    Possible,
    /// The pair cannot have interacted by the window's end (T17.a's `Detached`), so the engine has
    /// stepped none of it there, and the tables are read as they are.
    Impossible,
}

/// Whether the pair of initial masses `ends` (M☉, each `[lo, hi]`, the lighter's `lo` first), of
/// largest radii bounded by `reach` at the window's end `until_years`, metallicity coordinate
/// `fe_h` and drawn periastron `periastron_rsun` (R☉), is `Detached` (the module's items 1–4).
#[must_use]
fn detached(
    ends: [[f64; 2]; 2],
    reach: [ReachBound; 2],
    periastron_rsun: f64,
    fe_h: f64,
    until_years: f64,
) -> bool {
    // 3. A star that may have exploded by the window's end may have kicked the orbit in.
    if reach.iter().any(ReachBound::may_have_collapsed) {
        return false;
    }
    if !passed_over(ends, reach, periastron_rsun, until_years) {
        return false;
    }
    // 4. A star below 0.1 M☉ accretes its companion's wind in a pair the engine steps, which it
    // may do if the pair can interact by +H.
    if ends[0][0] >= MIN_INITIAL_MASS.value() {
        return true;
    }
    let horizon = until_years + RUN_HORIZON_PAST_WINDOW_YEARS;
    let table = ReachTable::generator();
    let (Some(lighter), Some(heavier)) = (
        table.bound(ends[0][0], ends[0][1], fe_h, horizon),
        table.bound(ends[1][0], ends[1][1], fe_h, horizon),
    ) else {
        return false;
    };
    let at_horizon = [lighter, heavier];
    !at_horizon.iter().any(ReachBound::may_have_collapsed)
        && passed_over(ends, at_horizon, periastron_rsun, horizon)
}

/// What P11.T17.b's tables say of the pair of initial masses `heavier_msun` and `lighter_msun`
/// (M☉), metallicity coordinate `fe_h` and drawn periastron `periastron_rsun` (R☉) over the window
/// `[start, end]` (years), with the held floor where `held` says a held star is possible:
/// `Remnants`, `Unchanged`, `Bright`, or `None` outside the tables or for a thin bin (see the
/// module's documentation).
#[must_use]
fn tabled(
    heavier_msun: f64,
    lighter_msun: f64,
    fe_h: f64,
    periastron_rsun: f64,
    [start, end]: [f64; 2],
    held: HeldStars,
) -> Option<PairLight> {
    let (table, cell) = TABLE_LAYERS.iter().find_map(|&layer| {
        let table = PairLightTable::generator(layer)?;
        let cell = table.cell_of(heavier_msun, lighter_msun, fe_h, periastron_rsun)?;
        Some((table, cell))
    })?;
    let (first, last) = (pair_light::age_bin(start)?, pair_light::age_bin(end)?);
    // An empty range would read as `Remnants`: refuse it, should rounding ever put the start's
    // bin past the end's.
    if first > last {
        return None;
    }
    let fe_cell = table.grid().cell_parts(cell)[2];
    let (held_from, floor) = match held {
        HeldStars::Possible => {
            let floor = pair_light::held_floor(fe_cell, heavier_msun + lighter_msun)?;
            (floor.first_bin(), floor.cmag())
        }
        HeldStars::Impossible => (pair_light::AGE_BINS, DARK_CMAG),
    };
    let mut remnants = true;
    let mut brightest = DARK_CMAG;
    let mut thin = false;
    for bin in first..=last {
        // A capped timeline's held star may live at every bin from `held_from` (the module's).
        let floor = if bin >= held_from { floor } else { DARK_CMAG };
        remnants &= table.living_cmag_for(cell, bin, lighter_msun).min(floor) == DARK_CMAG;
        let changed = table.changed_cmag_for(cell, bin, lighter_msun).min(floor);
        if changed != DARK_CMAG {
            // A departing star with no V (UNSEEN) reads the faintest V stored.
            brightest = brightest.min(changed.min(FAINTEST_CMAG));
            thin |= (1..=THIN_DEPARTING_CLASS).contains(&table.departing_class(cell, bin));
        }
    }
    if remnants {
        Some(PairLight::Remnants)
    } else if brightest == DARK_CMAG {
        Some(PairLight::Unchanged)
    } else if thin {
        None
    } else {
        Some(PairLight::Bright(Magnitudes::new(from_cmag(brightest))))
    }
}

/// Whether every pair of initial masses in `ends` (M☉, each `[lo, hi]`), of largest radii bounded
/// by `reach` at `until_years`, on any orbit of periastron at least `periastron_rsun` (R☉), fails
/// `can_interact`'s lobe and coarse decay tests by `until_years` (the module's items 1 and 2).
#[must_use]
fn passed_over(
    ends: [[f64; 2]; 2],
    reach: [ReachBound; 2],
    periastron_rsun: f64,
    until_years: f64,
) -> bool {
    let p = periastron_rsun;
    let radius = reach.map(|r| r.radius_rsun());
    // The least and greatest ratio mᵢ ÷ mⱼ, and Eggleton's lobe fraction at each: it grows with q.
    let q_min = [ends[0][0] / ends[1][1], ends[1][0] / ends[0][1]];
    let q_max = [ends[0][1] / ends[1][0], ends[1][1] / ends[0][0]];
    if q_min
        .iter()
        .chain(&q_max)
        .any(|&q| !(positive(q) && q.is_finite()))
    {
        return false;
    }
    let f_min = q_min.map(lobe_fraction);
    let f_max = q_max.map(lobe_fraction);
    // 1. The lobe test at the least periastron.
    if (0..2).any(|i| radius[i] >= f_min[i] * p) {
        return false;
    }
    let r_c = (0..2).map(|i| radius[i] / f_min[i]).fold(0.0, f64::max);
    // 2. The reservoir's share is at most c ÷ √r_p.
    let c = (0..2).fold(0.0, |sum, i| {
        let k = if reach[i].may_hold_core() {
            CORE_GYRATION
        } else {
            ENVELOPE_GYRATION
        };
        sum + k * radius[i].sqrt() * f_max[i] * f_max[i].sqrt() * (1.0 + q_max[i])
    });
    let s = p.sqrt() - c;
    if !positive(s) {
        return false;
    }
    // Braking: M ÷ μ = 2 + q + 1/q is largest at an end of the range of q = m₁ ÷ m₂.
    let m_over_mu = [q_min[0], q_max[0]]
        .iter()
        .map(|&q| 2.0 + q + 1.0 / q)
        .fold(0.0, f64::max);
    let braking_rate = 10.0 * MAGNETIC_BRAKING * G * m_over_mu * BRAKING_ECCENTRICITY_FACTOR;
    let braking = (0..2)
        .filter(|&i| ends[i][1] > MAGNETIC_BRAKING_FLOOR)
        .fold(0.0, |sum, i| sum + radius[i] * radius[i] * radius[i])
        * until_years;
    // Gravitational radiation: its term is `radiation` × r_p.
    let [m1, m2] = [ends[0][1], ends[1][1]];
    let radiation = 10.0 * GRAVITATIONAL_WAVE_RATE * m1 * m2 * (m1 + m2) * until_years;
    // The left side grows with r_p from here on only if its derivative is non-negative here.
    let s9 = crate::math::powi(s, 9);
    if 5.0 * s9 < DECAY_SAFETY * radiation * p.sqrt() {
        return false;
    }
    s9 * s - DECAY_SAFETY * (braking_rate * braking + radiation * p) > crate::math::powi(r_c, 5)
}

/// Eggleton's (1983) Roche-lobe radius over the separation for a star of mass ratio `q` = its
/// mass ÷ its companion's, as the engine reads it.
#[must_use]
fn lobe_fraction(q: f64) -> f64 {
    roche_lobe_radius(q, Metres::new(1.0)).value()
}

#[cfg(test)]
mod tests {
    use super::super::detached::{hut_f2, hut_f5};
    use super::*;

    use crate::units::consts::METRES_PER_AU as AU_M;

    fn solar(a: f64, b: f64, periastron_au: f64, age: f64) -> Option<PairLight> {
        pair_light_bound(
            SolarMasses::new(a),
            SolarMasses::new(b),
            Metres::new(periastron_au * AU_M),
            &Composition::SOLAR,
            Years::new(age)..=Years::new(age),
        )
    }

    /// Whether T17.a's closed form alone calls the pair of [`solar`] `Detached`, the tables unread.
    fn closed(a: f64, b: f64, periastron_au: f64, age: f64) -> bool {
        closed_over([a, a], [b, b], periastron_au, age)
    }

    /// [`closed`] over the mass intervals `a` and `b` (M☉).
    fn closed_over(a: [f64; 2], b: [f64; 2], periastron_au: f64, age: f64) -> bool {
        bound_with(
            [
                SolarMasses::new(a[0])..=SolarMasses::new(a[1]),
                SolarMasses::new(b[0])..=SolarMasses::new(b[1]),
            ],
            Metres::new(periastron_au * AU_M),
            &Composition::SOLAR,
            Years::new(age)..=Years::new(age),
            Tables::Skip,
        ) == Some(PairLight::Detached)
    }

    /// Φ³ ÷ (1 + e)⁵ never exceeds the factor, Φ ÷ (1 + e)² never exceeds 1, and
    /// (1 + 7e²/8) ÷ (1 + e)⁴ never exceeds 1, on a fine grid of e over [0, 1].
    #[test]
    fn the_eccentricity_factors_are_their_suprema() {
        let mut largest_braking: f64 = 0.0;
        for n in 0..=100_000_u32 {
            let e = f64::from(n) / 100_000.0;
            let e2 = e * e;
            let phi = hut_f2(e2) / hut_f5(e2);
            let one_e = 1.0 + e;
            let braking = phi * phi * phi / crate::math::powi(one_e, 5);
            largest_braking = largest_braking.max(braking);
            assert!(phi / (one_e * one_e) <= 1.0 + 1e-15, "e {e}");
            assert!(
                (1.0 + 0.875 * e2) / crate::math::powi(one_e, 4) <= 1.0 + 1e-15,
                "e {e}"
            );
        }
        assert!(
            largest_braking <= BRAKING_ECCENTRICITY_FACTOR,
            "{largest_braking}"
        );
        assert!(BRAKING_ECCENTRICITY_FACTOR - largest_braking < 1e-5);
    }

    #[test]
    fn a_wide_pair_is_detached_and_a_close_one_is_not() {
        assert_eq!(solar(1.0, 0.6, 2_000.0, 9.0e9), Some(PairLight::Detached));
        assert!(!closed(1.0, 0.6, 0.04, 9.0e9));
        assert_ne!(solar(1.0, 0.6, 0.04, 9.0e9), Some(PairLight::Detached));
        // Young, a 1 au periastron is far outside two main-sequence stars' lobes; at 14 Gyr the
        // solar-mass star's giant branch reaches it.
        assert_eq!(solar(1.0, 0.6, 1.0, 1.0e9), Some(PairLight::Detached));
        assert!(!closed(1.0, 0.6, 1.0, 1.4e10));
        assert_ne!(solar(1.0, 0.6, 1.0, 1.4e10), Some(PairLight::Detached));
        // The order of the masses does not matter.
        for (p, age) in [(5.0, 1.0e9), (0.3, 3.0e9), (40.0, 1.3e10)] {
            assert_eq!(solar(1.0, 0.6, p, age), solar(0.6, 1.0, p, age));
        }
    }

    /// A wider orbit, an earlier age and narrower mass intervals only bring a pair into
    /// `Detached`.
    #[test]
    fn detached_grows_with_the_periastron_and_shrinks_with_age_and_width() {
        let periastra: Vec<f64> = (0..60)
            .map(|k| 0.005 * crate::math::powi(1.25, k))
            .collect();
        let ages: Vec<f64> = (0..40).map(|k| 1.0e6 * crate::math::powi(1.3, k)).collect();
        for &(a, b) in &[
            (1.0, 0.6),
            (3.0, 2.5),
            (12.0, 4.0),
            (0.4, 0.1),
            (60.0, 55.0),
        ] {
            for &age in ages.iter().filter(|&&t| t < 1.5e10) {
                let detached: Vec<bool> = periastra.iter().map(|&p| closed(a, b, p, age)).collect();
                let first = detached.iter().position(|&d| d).unwrap_or(detached.len());
                assert!(
                    detached[first..].iter().all(|&d| d),
                    "{a} + {b} at {age} yr: {detached:?}"
                );
            }
            for &p in &periastra {
                let detached: Vec<bool> = ages
                    .iter()
                    .filter(|&&t| t < 1.5e10)
                    .map(|&t| closed(a, b, p, t))
                    .collect();
                let last = detached.iter().rposition(|&d| d).map_or(0, |k| k + 1);
                assert!(detached[..last].iter().all(|&d| d), "{a} + {b} at {p} au");
            }
        }
        // An interval holding a pair that is not `Detached` is not `Detached`.
        let at = |lo: f64, hi: f64| {
            pair_light_bound_over(
                [
                    SolarMasses::new(lo)..=SolarMasses::new(hi),
                    SolarMasses::new(0.6)..=SolarMasses::new(0.6),
                ],
                Metres::new(AU_M),
                &Composition::SOLAR,
                Years::new(6.0e9)..=Years::new(6.0e9),
            )
        };
        assert_eq!(at(0.9, 0.9), Some(PairLight::Detached));
        assert!(!closed(1.3, 0.6, 1.0, 6.0e9));
        assert!(!closed_over([0.9, 1.3], [0.6, 0.6], 1.0, 6.0e9));
        // The tables are read for exact masses only.
        assert_eq!(at(0.9, 1.3), None);
    }

    /// The census's order of the verdicts, tightest first (R06.T8.h's inclusion monotonicity,
    /// the module's order): `Remnants`, then `Detached` and `Unchanged` alike, then `Bright`,
    /// looser as its magnitude brightens, then none.
    fn looseness(verdict: Option<PairLight>) -> (u8, f64) {
        match verdict {
            Some(PairLight::Remnants) => (0, 0.0),
            Some(PairLight::Detached | PairLight::Unchanged) => (1, 0.0),
            Some(PairLight::Bright(m)) => (2, -m.value()),
            None => (3, 0.0),
        }
    }

    /// The verdicts' names, in [`verdict_index`]'s order.
    const VERDICTS: [&str; 5] = ["Detached", "Unchanged", "Remnants", "Bright", "none"];

    /// The index of `verdict` in [`VERDICTS`].
    fn verdict_index(verdict: Option<PairLight>) -> usize {
        match verdict {
            Some(PairLight::Detached) => 0,
            Some(PairLight::Unchanged) => 1,
            Some(PairLight::Remnants) => 2,
            Some(PairLight::Bright(_)) => 3,
            None => 4,
        }
    }

    /// A value log-uniform over `lo`–`hi` at the uniform variate `u`.
    fn log_uniform(u: f64, lo: f64, hi: f64) -> f64 {
        lo * crate::math::exp(u * crate::math::ln(hi / lo))
    }

    /// A window of ages, years, ending at `end`, of the kind `kind` (mod 4): a point; up to
    /// 1,000 years wide, as the census's windows before a drift are; up to 3 × 10⁵ years, the
    /// light-crossing bound's reach; up to 0.5 dex, wider than any the census asks.
    fn window_ending(mix: &mut Mix, end: f64, kind: usize) -> [f64; 2] {
        let start = match kind % 4 {
            0 => end,
            1 => end - 1.0e3 * mix.unit(),
            2 => end - 3.0e5 * mix.unit(),
            _ => end * crate::math::exp10(-0.5 * mix.unit()),
        };
        [start, end]
    }

    /// Asserts that the pair's verdict over `outer` is no tighter than over a window inside it
    /// drawn from `mix` (a point for a quarter of them), and returns the verdict over `outer`.
    fn assert_no_tighter(
        [a, b]: [SolarMasses; 2],
        periastron: Metres,
        comp: &Composition,
        outer: [f64; 2],
        mix: &mut Mix,
    ) -> Option<PairLight> {
        let [start, end] = outer;
        let (x, y) = (mix.unit(), mix.unit());
        let at = |f: f64| (start + (end - start) * f).min(end);
        let inner = if mix.unit() < 0.25 {
            [at(x), at(x)]
        } else {
            [at(x.min(y)), at(x.max(y))]
        };
        let years = |[s, e]: [f64; 2]| Years::new(s)..=Years::new(e);
        let wide = pair_light_bound(a, b, periastron, comp, years(outer));
        let narrow = pair_light_bound(a, b, periastron, comp, years(inner));
        assert!(
            looseness(wide) >= looseness(narrow),
            "{} + {} M_sun, periastron {:.4e} R_sun, [Fe/H] {:.3}: {wide:?} over {outer:?} is \
             tighter than {narrow:?} over {inner:?}",
            a.value(),
            b.value(),
            periastron.value() / SOLAR_RADIUS_M,
            comp.fe_h().value()
        );
        wide
    }

    /// Each verdict only loosens as its window widens (R06.T8.h's ruling,
    /// `decision-r06-t8h-warm.md` §5, which its warm cache's bit-identical reply rests on): over
    /// 10⁴ random pairs, a fifth with a star below 0.1 M☉, and 600 of the generator's own pairs of
    /// C, D and E about their records' ages, the verdict over a window is no tighter, in the
    /// census's order ([`looseness`]), than over any window inside it. Every verdict occurs.
    #[test]
    fn each_verdict_only_loosens_as_the_window_widens() {
        let mut mix = Mix(SEED ^ 0x0001_7c0c);
        let mut seen = [0_u32; 5];
        for i in 0..10_000_usize {
            let heavier = if i % 4 == 0 {
                log_uniform(mix.unit(), 0.08, 150.0)
            } else {
                log_uniform(mix.unit(), 0.75, 150.0)
            };
            let lighter = if i % 5 == 0 {
                (0.08 + 0.02 * mix.unit()).min(heavier)
            } else {
                log_uniform(mix.unit(), 0.08, heavier)
            };
            let periastron = Metres::new(log_uniform(mix.unit(), 0.5, 1.0e7) * SOLAR_RADIUS_M);
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(-2.5 + 2.8 * mix.unit()),
                crate::units::HeliumExcess::ZERO,
            );
            let end = log_uniform(mix.unit(), 1.0e4, 1.5e10);
            let outer = window_ending(&mut mix, end, i);
            let masses = [SolarMasses::new(heavier), SolarMasses::new(lighter)];
            let wide = assert_no_tighter(masses, periastron, &comp, outer, &mut mix);
            seen[verdict_index(wide)] += 1;
        }
        let galaxy = milky_way();
        for (salt, layer) in (0x0c_u64..).zip([Layer::C, Layer::D, Layer::E]) {
            for (i, (input, age)) in generated_pairs(&galaxy, layer, 200, SEED ^ salt)
                .iter()
                .enumerate()
            {
                let outer = window_ending(&mut mix, *age, i);
                let p = input.orbit().periapsis();
                let wide =
                    assert_no_tighter(input.masses(), p, input.composition(), outer, &mut mix);
                seen[verdict_index(wide)] += 1;
            }
        }
        // Across the age at which the closed form first fails, where the held floor starts to be
        // read: a neutron star and a white dwarf 1,000 au apart are `Detached` at 10 Myr, before
        // the primary may have collapsed, and `Bright` at the floor over a window to 1 Gyr.
        let years = |s: f64, e: f64| Years::new(s)..=Years::new(e);
        let (a, b) = (SolarMasses::new(10.0), SolarMasses::new(8.0));
        let p = Metres::new(1.0e3 * AU_M);
        let narrow = pair_light_bound(a, b, p, &Composition::SOLAR, years(1.0e7, 1.0e7));
        let wide = pair_light_bound(a, b, p, &Composition::SOLAR, years(1.0e7, 1.0e9));
        assert_eq!(narrow, Some(PairLight::Detached));
        assert!(matches!(wide, Some(PairLight::Bright(_))), "{wide:?}");
        assert!(looseness(wide) >= looseness(narrow));
        println!("verdicts over the outer windows: {seen:?} ({VERDICTS:?})");
        assert!(
            seen.iter().all(|&n| n > 0),
            "every verdict occurs: {seen:?}"
        );
    }

    /// The tables answer P11.T17.c's three verdicts: `Remnants` for two white dwarfs, asked before
    /// `Detached`; `Unchanged` for a close pair too young for any star of it to have left its main
    /// sequence; and `Bright` for an Algol, which bounds its gainer as the engine evolves it. A
    /// pair whose star collapsed, which the closed form leaves, was `Remnants` until the held floor
    /// (P11.T17.c's follow-up): its kick may have brought the orbit into an interaction, so a
    /// capped timeline may hold a star of it, and it is `Bright` at the floor.
    #[test]
    fn the_tables_answer_remnants_unchanged_and_bright() {
        // Two white dwarfs 10⁴ au apart: the closed form holds, but `Remnants` is asked first.
        assert!(closed(3.0, 2.8, 1.0e4, 5.0e9));
        assert_eq!(solar(3.0, 2.8, 1.0e4, 5.0e9), Some(PairLight::Remnants));
        assert_eq!(solar(3.0, 2.8, 1.0e4, 1.0e8), Some(PairLight::Detached));
        // A neutron star and a white dwarf 1,000 au apart: the primary collapsed, so the closed
        // form cannot say, and nothing departs in the tables, but the held floor binds.
        assert!(!closed(10.0, 8.0, 1.0e3, 1.0e9));
        let table = PairLightTable::generator(Layer::E).expect("E's fitted table");
        let p_rsun = 1.0e3 * AU_M / SOLAR_RADIUS_M;
        let cell = table
            .cell_of(10.0, 8.0, fe_h_of(&Composition::SOLAR), p_rsun)
            .expect("inside E's table");
        let floor =
            pair_light::held_floor(table.grid().cell_parts(cell)[2], 18.0).expect("a pair's mass");
        assert!(floor.first_bin() <= pair_light::age_bin(1.0e9).expect("a bin"));
        assert_eq!(
            solar(10.0, 8.0, 1.0e3, 1.0e9),
            Some(PairLight::Bright(Magnitudes::new(from_cmag(floor.cmag()))))
        );
        // Two B stars 0.03 au apart at 0.24 Myr: the closed form cannot pass them over, and
        // neither has reached its main sequence's end, the held floor's first bin.
        assert!(!closed(4.6, 4.4, 0.03, 2.4e5));
        assert_eq!(solar(4.6, 4.4, 0.03, 2.4e5), Some(PairLight::Unchanged));
        // An Algol-like pair of Hurley, Tout and Pols's (2002, section 3.1) masses on `evolve`'s
        // example's 3-day orbit, at 500 Myr: its gainer departs from its own model, no brighter
        // than M.
        let input = super::super::tests::pair(2.9, 0.9, 3.0, 0.0, 0.02);
        let [m1, m2] = input.masses();
        let p = input.orbit().periapsis();
        let age = Years::new(5.0e8)..=Years::new(5.0e8);
        let algol = pair_light_bound(m1, m2, p, &Composition::SOLAR, age.clone());
        assert_eq!(algol, pair_light_bound(m2, m1, p, &Composition::SOLAR, age));
        let held = held(
            &input,
            5.0e8 + CLOCK_WINDOW_H.as_julian_years_f64(),
            [5.0e8, 5.0e8],
        );
        assert!(
            held.departing && held.departing_mag < UNSEEN_MAG,
            "{held:?}"
        );
        assert!(
            matches!(algol, Some(PairLight::Bright(m)) if m.value() <= held.departing_mag),
            "{algol:?} against {held:?}"
        );
    }

    /// The centre of `table`'s `cell`: its heavier star's mass (M☉, geometric), its lighter's
    /// (M☉, from the mass ratio's middle, at least 0.08 M☉), its metallicity coordinate (dex) and
    /// its periastron (R☉, geometric; the open bin's lower edge × 10).
    fn centre(table: &PairLightTable, cell: usize) -> [f64; 4] {
        let grid = table.grid();
        let [m, q, f, p] = grid.cell_parts(cell);
        let (m_lo, m_hi) = grid.mass_edges_msun(m);
        let (q_lo, q_hi) = grid.q_edges(q);
        let (f_lo, f_hi) = grid.fe_h_edges(f);
        let (p_lo, p_hi) = grid.periastron_edges_rsun(p);
        let heavier = (m_lo * m_hi).sqrt();
        let periastron = if p + 1 == grid.periastron_cells() {
            10.0 * p_lo
        } else {
            (p_lo * p_hi).sqrt()
        };
        [
            heavier,
            (heavier * f64::midpoint(q_lo, q_hi)).max(pair_light::LOWEST_COMPANION_MSUN),
            f64::midpoint(f_lo, f_hi),
            periastron,
        ]
    }

    /// A bin whose changed value comes from one to three of the cell's own departing samples is
    /// too thin to bound, and the tables answer `None` there; with none of its own, or four or
    /// more, the bin is `Bright` at its changed value. The tables are read as they are, without
    /// the held floor, which [`a_capped_timelines_held_stars_are_bounded`] tests.
    #[test]
    fn a_thin_bin_is_not_bounded() {
        let mut found = [0_u32; 3];
        for layer in TABLE_LAYERS {
            let table = PairLightTable::generator(layer).expect("a fitted table");
            for cell in (0..table.grid().cells()).step_by(7) {
                let [heavier, lighter, fe_h, p] = centre(table, cell);
                // A low-mass cell's first mass ratios hold no star of 0.08 M☉ or more.
                if table.cell_of(heavier, lighter, fe_h, p) != Some(cell) {
                    continue;
                }
                for bin in 1..pair_light::AGE_BINS {
                    let changed = table.changed_cmag_for(cell, bin, lighter);
                    if changed == DARK_CMAG {
                        continue;
                    }
                    let class = table.departing_class(cell, bin);
                    let kind = match class {
                        0 => 0,
                        1..=THIN_DEPARTING_CLASS => 1,
                        _ => 2,
                    };
                    let age = (pair_light::age_edge_years(bin)
                        * pair_light::age_edge_years(bin + 1))
                    .sqrt();
                    let verdict =
                        tabled(heavier, lighter, fe_h, p, [age, age], HeldStars::Impossible);
                    if kind == 1 {
                        assert_eq!(verdict, None, "{layer:?} cell {cell} bin {bin}");
                    } else {
                        let m = from_cmag(changed.min(FAINTEST_CMAG));
                        assert_eq!(
                            verdict,
                            Some(PairLight::Bright(Magnitudes::new(m))),
                            "{layer:?} cell {cell} bin {bin}, class {class}"
                        );
                    }
                    found[kind] += 1;
                }
            }
        }
        assert!(found.iter().all(|&n| n > 0), "each kind of bin: {found:?}");
    }

    /// A pair with a star below 0.1 M☉ is `Detached` only where the closed form also holds
    /// [`RUN_HORIZON_PAST_WINDOW_YEARS`] past its window's end, the latest its age at +H can be,
    /// since a pair the engine steps lets that star accrete its companion's wind (the module's
    /// item 4); a pair of heavier stars is read at its window's end alone.
    #[test]
    fn a_light_members_pair_is_detached_only_if_it_cannot_interact_by_plus_h() {
        assert!((RUN_HORIZON_PAST_WINDOW_YEARS - 264_144.0).abs() < 1e-9);
        let fe_h = 0.0;
        let table = ReachTable::generator();
        let reach = |ends: [[f64; 2]; 2], t: f64| {
            [0, 1].map(|i| {
                table
                    .bound(ends[i][0], ends[i][1], fe_h, t)
                    .expect("inside the table")
            })
        };
        let mut boundaries = 0;
        for (light, companion, p) in [
            (0.09, 1.0, 3.0),
            (0.085, 1.2, 10.0),
            (0.09, 0.6, 0.02),
            (0.099, 2.0, 20.0),
        ] {
            let p_rsun = p * AU_M / SOLAR_RADIUS_M;
            let ends = [[light, light], [companion, companion]];
            let alone = |t: f64| passed_over(ends, reach(ends, t), p_rsun, t);
            // The latest age, to a year, at which the closed form alone holds at the window's end.
            let (mut lo, mut hi) = (1.0e5, 1.5e10);
            if !alone(lo) || alone(hi) {
                continue;
            }
            while hi - lo > 1.0 {
                let mid = f64::midpoint(lo, hi);
                if alone(mid) { lo = mid } else { hi = mid }
            }
            boundaries += 1;
            for t in [lo - 3.0e5, lo - 1.0e5, lo] {
                let held = detached(ends, reach(ends, t), p_rsun, fe_h, t);
                assert_eq!(
                    held,
                    t + RUN_HORIZON_PAST_WINDOW_YEARS <= lo,
                    "{ends:?} at {t}"
                );
            }
            // A star of 0.1 M☉ accretes nothing on its own track: its pair is read at its end.
            let heavy = [[0.1, 0.1], [companion, companion]];
            let at = |t: f64| detached(heavy, reach(heavy, t), p_rsun, fe_h, t);
            assert_eq!(
                at(lo - 1.0e5),
                passed_over(heavy, reach(heavy, lo - 1.0e5), p_rsun, lo - 1.0e5)
            );
        }
        assert!(
            boundaries >= 3,
            "the pairs reach their boundaries: {boundaries}"
        );
    }

    /// The reach table's reader and the bound are pinned bit for bit (the determinism audit): the
    /// bound's radius and flags at every 61st mass node, each node of `FE_H_NODES` and every 13th
    /// age edge, edges included, and the verdicts of 256 fixed queries.
    #[test]
    fn the_reach_bound_and_the_verdicts_are_pinned() {
        use super::super::reach::{
            AGE_BINS, FE_H_NODES, MASS_CELLS, MASS_SUBDIVISIONS, age_edge_years, mass_node_msun,
        };
        use crate::stellar::binary::largest_radius_bound;
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        let mut w = GoldenWriter::new();
        w.header(crate::GENERATOR_VERSION.get());
        for n in (0..=MASS_CELLS * MASS_SUBDIVISIONS).step_by(61) {
            let m = SolarMasses::new(mass_node_msun(n));
            for &fe_h in &FE_H_NODES {
                let comp = Composition::from_fe_h(
                    crate::units::Dex::new(fe_h),
                    crate::units::HeliumExcess::ZERO,
                );
                for k in (0..AGE_BINS).step_by(13) {
                    let age = age_edge_years(k);
                    let label = format!("mass node {n} [Fe/H] {fe_h:.3} age bin {k}");
                    match largest_radius_bound(m..=m, &comp, Years::new(age)) {
                        Some(bound) => {
                            w.f64(&label, bound.radius_rsun());
                            w.line(&format!(
                                "  core {} collapsed {}",
                                bound.may_hold_core(),
                                bound.may_have_collapsed()
                            ));
                        }
                        None => w.line(&format!("{label}: none")),
                    }
                }
            }
        }
        let mut mix = Mix(SEED ^ 0x0001_9014);
        let log_uniform =
            |u: f64, lo: f64, hi: f64| lo * crate::math::exp(u * crate::math::ln(hi / lo));
        for i in 0..256_u32 {
            let a = log_uniform(mix.unit(), 0.08, 150.0);
            let b = log_uniform(mix.unit(), 0.08, a);
            let p = log_uniform(mix.unit(), 1.0, 1.0e5);
            let age = log_uniform(mix.unit(), 1.0e5, 1.4e10);
            let fe_h = -2.5 + 2.8 * mix.unit();
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(fe_h),
                crate::units::HeliumExcess::ZERO,
            );
            let verdict = pair_light_bound(
                SolarMasses::new(a),
                SolarMasses::new(b),
                Metres::new(p * SOLAR_RADIUS_M),
                &comp,
                Years::new(age)..=Years::new(age),
            );
            w.line(&format!(
                "query {i:03}: {a:.6} + {b:.6} M☉, periastron {p:.6e} R☉, age {age:.6e} yr, \
                 [Fe/H] {fe_h:.4}: {verdict:?}"
            ));
            if let Some(PairLight::Bright(m)) = verdict {
                w.f64(&format!("query {i:03}: M_V"), m.value());
            }
        }
        golden!("stellar/pair_light", w.as_str());
    }

    /// The bound is the same bit for bit whichever star is given first, over 10⁴ random queries.
    #[test]
    fn the_order_of_the_stars_does_not_matter() {
        let mut mix = Mix(SEED ^ 0x05ed);
        let log_uniform =
            |u: f64, lo: f64, hi: f64| lo * crate::math::exp(u * crate::math::ln(hi / lo));
        let mut detached = 0_u32;
        for _ in 0..10_000 {
            let a = SolarMasses::new(log_uniform(mix.unit(), 0.08, 150.0));
            let b = SolarMasses::new(log_uniform(mix.unit(), 0.08, 150.0));
            let p = Metres::new(log_uniform(mix.unit(), 1.0, 1.0e5) * SOLAR_RADIUS_M);
            let age = Years::new(log_uniform(mix.unit(), 1.0e5, 1.4e10));
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(-2.5 + 2.8 * mix.unit()),
                crate::units::HeliumExcess::ZERO,
            );
            let one_way = pair_light_bound(a, b, p, &comp, age..=age);
            assert_eq!(one_way, pair_light_bound(b, a, p, &comp, age..=age));
            detached += u32::from(one_way == Some(PairLight::Detached));
        }
        assert!(detached > 1_000, "the sample reaches Detached: {detached}");
    }

    /// The bound gives each query the same answer bit for bit, whatever was asked before it
    /// (the sim-determinism skill's order independence: the census's warm cache asks it, and its
    /// tables and cooling and held floors are filled lazily): 150 queries over layers C, D and E,
    /// a third with a star below 0.1 M☉, and four that answer `Remnants` and `Unchanged`,
    /// answering every verdict.
    #[test]
    fn the_bound_does_not_depend_on_what_was_asked_before() {
        use hyperion_testkit::order::assert_order_independent;
        let mut mix = Mix(SEED ^ 0x0de7);
        let bands = [(0.75, 2.5), (2.5, 8.0), (8.0, 150.0)];
        let queries: Vec<(f64, f64, f64, f64, f64)> = (0..150_usize)
            .map(|i| {
                let (lo, hi) = bands[i % 3];
                let heavier = log_uniform(mix.unit(), lo, hi);
                let lighter = if i % 3 == 1 {
                    0.08 + 0.02 * mix.unit()
                } else {
                    log_uniform(mix.unit(), 0.1, heavier)
                };
                let p = log_uniform(mix.unit(), 1.0, 1.0e6);
                let age = log_uniform(mix.unit(), 1.0e5, 1.4e10);
                (heavier, lighter, p, age, -2.5 + 2.8 * mix.unit())
            })
            .chain([
                // Pairs of white dwarfs on wide orbits, which only `Remnants` answers, and a young
                // close pair, which `Unchanged` does (the held floor makes both rare at random).
                (3.0, 2.8, 2.15e6, 5.0e9, 0.0),
                (6.0, 5.0, 1.0e6, 2.0e9, -0.5),
                (4.0, 3.5, 5.0e5, 9.0e9, 0.2),
                (4.6, 4.4, 6.45, 2.4e5, 0.0),
            ])
            .collect();
        let verdict = |&(a, b, p, age, fe_h): &(f64, f64, f64, f64, f64)| {
            let comp = Composition::from_fe_h(
                crate::units::Dex::new(fe_h),
                crate::units::HeliumExcess::ZERO,
            );
            let v = pair_light_bound(
                SolarMasses::new(a),
                SolarMasses::new(b),
                Metres::new(p * SOLAR_RADIUS_M),
                &comp,
                Years::new(age)..=Years::new(age),
            );
            // `Bright`'s magnitude is k ÷ 100 for a stored integer k, never NaN or −0, so its
            // equality is its bits'.
            let m = match v {
                Some(PairLight::Bright(m)) => m.value(),
                _ => 0.0,
            };
            (verdict_index(v), m)
        };
        let mut seen = [0_u32; 5];
        for q in &queries {
            seen[verdict(q).0] += 1;
        }
        assert!(seen.iter().all(|&n| n > 0), "every verdict: {seen:?}");
        assert_order_independent(&queries, verdict);
    }

    /// A pair one of whose stars may have died suddenly by the window's end is not `Detached`,
    /// since a kick can bring its periastron in, and is read from the tables, which sample the
    /// kicks; a white dwarf's birth, which only widens the orbit, is no bar.
    #[test]
    fn a_pair_whose_star_may_have_exploded_is_not_detached() {
        assert_eq!(solar(20.0, 1.0, 1.0e4, 1.0e6), Some(PairLight::Detached));
        assert!(!closed(20.0, 1.0, 1.0e4, 5.0e7));
        assert!(!closed(1.0, 20.0, 1.0e4, 5.0e7));
        assert_ne!(solar(20.0, 1.0, 1.0e4, 5.0e7), Some(PairLight::Detached));
        assert_eq!(
            solar(20.0, 1.0, 1.0e4, 5.0e7),
            solar(1.0, 20.0, 1.0e4, 5.0e7)
        );
        // The Sun's white dwarf by 13 Gyr, 10⁴ au from a red dwarf.
        assert_eq!(solar(1.0, 0.3, 1.0e4, 1.3e10), Some(PairLight::Detached));
    }

    /// At version 21 a pair the pre-test passes over keeps its drawn orbit through its stars'
    /// collapses, with no kick and no supernova record (P11.T4.l's finding F3, which the
    /// version-22 batch fixes), so its stars are their own models across a supernova. When T4.l
    /// lands this fails: then a collapse can hand the pair to the engine with its kick, and
    /// P11.T17's collapse rule (the module's item 3) must still hold, or `Detached` must bound
    /// the periastron after the kick (plan 11, P11.T17's 22-batch note).
    #[test]
    fn a_passed_over_pairs_collapse_keeps_its_drawn_orbit_at_version_21() {
        use crate::stellar::binary::evolve;
        let input = super::super::tests::pair(15.0, 3.0, 1.0e5, 0.0, 0.02);
        let until = Years::new(1.0e8);
        assert!(!can_interact(&input, until));
        let timeline = evolve(&input, until);
        assert_eq!(timeline.segments().len(), 1);
        assert!(timeline.supernovae().is_empty());
        assert_eq!(timeline.state_at(until).orbit(), Some(input.orbit()));
        // The bound does not lean on it: the primary may have collapsed, so the pair is not
        // `Detached`, and its verdict is the tables', which sample the kicks.
        let [m1, m2] = input.masses();
        let p = input.orbit().periapsis();
        assert_ne!(
            pair_light_bound(m1, m2, p, &Composition::SOLAR, until..=until),
            Some(PairLight::Detached)
        );
    }

    #[test]
    fn input_outside_the_bound_is_not_bounded() {
        let ages = || Years::new(1.0e9)..=Years::new(1.0e9);
        let one = SolarMasses::new(1.0);
        let wide = Metres::new(1.0e4 * AU_M);
        let solar = &Composition::SOLAR;
        assert_eq!(
            pair_light_bound(SolarMasses::new(0.05), one, wide, solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(SolarMasses::new(f64::NAN), one, wide, solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, Metres::new(0.0), solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, Metres::new(f64::NAN), solar, ages()),
            None
        );
        assert_eq!(
            pair_light_bound(
                one,
                one,
                wide,
                solar,
                Years::new(2.0e10)..=Years::new(2.0e10)
            ),
            None
        );
        assert_eq!(
            pair_light_bound(one, one, wide, solar, Years::new(2.0e9)..=Years::new(1.0e9)),
            None
        );
        let helium = Composition::from_fe_h(
            crate::units::Dex::ZERO,
            crate::units::HeliumExcess::new(0.01),
        );
        assert_eq!(pair_light_bound(one, one, wide, &helium, ages()), None);
        assert_eq!(
            pair_light_bound_over(
                [SolarMasses::new(2.0)..=SolarMasses::new(1.0), one..=one],
                wide,
                solar,
                ages()
            ),
            None
        );
        // A mass that is not finite, or whose ratio to its companion's overflows, is not bounded.
        let young_age = || Years::new(1.0e5)..=Years::new(1.0e5);
        assert_eq!(
            pair_light_bound(
                SolarMasses::new(f64::INFINITY),
                one,
                wide,
                solar,
                young_age()
            ),
            None
        );
        assert_eq!(
            pair_light_bound(
                SolarMasses::new(1.0e308),
                SolarMasses::new(0.08),
                wide,
                solar,
                young_age()
            ),
            None
        );
        // A heavier star than the tracks' top reads the top, as its track does. Young, it is a
        // main-sequence star of some 20 R☉; by 1 Gyr it has collapsed.
        let young = || Years::new(1.0e5)..=Years::new(1.0e5);
        assert_eq!(
            pair_light_bound(SolarMasses::new(180.0), one, wide, solar, young()),
            Some(PairLight::Detached)
        );
        assert_eq!(
            pair_light_bound(SolarMasses::new(180.0), one, wide, solar, ages()),
            None
        );
    }

    // --- The generator's own pairs against the pre-test ------------------------------------------

    use super::super::testing::{Mix, generated_pairs};
    use crate::galaxy::Galaxy;
    use crate::id::Layer;
    use crate::stellar::binary::{BinaryInput, can_interact};

    const SEED: u64 = 0x0b17_0017_a11c_e5ed;

    /// The stellar layers.
    const LAYERS: [Layer; 5] = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];

    fn milky_way() -> Galaxy {
        super::super::testing::milky_way(SEED)
    }

    /// What one layer's pairs came to, at one class of ages.
    #[derive(Debug, Default)]
    struct Tally {
        pairs: u64,
        /// Pairs the pre-test passes over at their window's end.
        passed_over: u64,
        /// Pairs the bound calls `Detached`.
        detached: u64,
        /// Pairs a star of which may have died suddenly by the window's end, which the bound
        /// therefore leaves unbounded (the module's item 3).
        collapsed: u64,
        /// Pairs the bound calls `Detached` that the pre-test passes, described.
        missed: Vec<String>,
    }

    impl Tally {
        fn merge(&mut self, other: Self) {
            self.pairs += other.pairs;
            self.passed_over += other.passed_over;
            self.detached += other.detached;
            self.collapsed += other.collapsed;
            self.missed.extend(other.missed);
        }

        fn check(&mut self, input: &BinaryInput, age: f64) {
            let [m1, m2] = input.masses();
            let periastron = input.orbit().periapsis();
            let at = Years::new(age);
            // T17.a's closed form alone, so that its shares stay comparable: the tables'
            // `Remnants`, asked first, would take some of its pairs.
            let bound = bound_with(
                [m1..=m1, m2..=m2],
                periastron,
                input.composition(),
                at..=at,
                Tables::Skip,
            );
            let interacts = can_interact(input, at);
            let collapsed = [m1, m2].iter().any(|&m| {
                crate::stellar::binary::largest_radius_bound(m..=m, input.composition(), at)
                    .is_some_and(|bound| bound.may_have_collapsed())
            });
            self.collapsed += u64::from(collapsed);
            self.pairs += 1;
            self.passed_over += u64::from(!interacts);
            if bound == Some(PairLight::Detached) {
                self.detached += 1;
                if interacts {
                    self.missed.push(format!(
                        "{:.4} + {:.4} M_sun, periastron {:.4e} R_sun, e {:.3}, [Fe/H] {:.3}, \
                         age {age:.4e} yr",
                        m1.value(),
                        m2.value(),
                        periastron.value() / SOLAR_RADIUS_M,
                        input.orbit().eccentricity().value(),
                        input.composition().fe_h().value(),
                    ));
                }
            }
        }
    }

    /// Each pair of `pairs` checked at its own age and at a young age, log-uniform over
    /// 10⁵–10⁸ years from `salt`'s stream, in eight shares (in turn on wasm32-wasip1): the old and
    /// young tallies.
    fn tally(pairs: &[(BinaryInput, f64)], salt: u64) -> [Tally; 2] {
        const SHARES: usize = 8;
        let mut mix = Mix(SEED ^ salt ^ 0x0000_a9e5);
        let young: Vec<f64> = pairs
            .iter()
            .map(|_| crate::math::exp10(5.0 + 3.0 * mix.unit()))
            .collect();
        let share = |k: usize| {
            let mut part = [Tally::default(), Tally::default()];
            for (i, (input, age)) in pairs.iter().enumerate().skip(k).step_by(SHARES) {
                part[0].check(input, *age);
                part[1].check(input, young[i]);
            }
            part
        };
        #[cfg(not(target_family = "wasm"))]
        let parts: Vec<[Tally; 2]> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..SHARES)
                .map(|k| {
                    let share = &share;
                    scope.spawn(move || share(k))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a share's thread"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let parts: Vec<[Tally; 2]> = (0..SHARES).map(share).collect();
        let mut total = [Tally::default(), Tally::default()];
        for [old, young] in parts {
            total[0].merge(old);
            total[1].merge(young);
        }
        total
    }

    /// Checks `per_layer` generated pairs of each stellar layer, printing each layer's shares, and
    /// asserts that no `Detached` pair passes the pre-test.
    fn no_detached_pair_interacts(per_layer: usize) {
        let galaxy = milky_way();
        let mut missed = 0;
        for (salt, layer) in (0_u64..).zip(LAYERS) {
            let pairs = generated_pairs(&galaxy, layer, per_layer, SEED ^ salt);
            for (class, t) in ["old (the record's age)", "young (1e5-1e8 yr)"]
                .iter()
                .zip(tally(&pairs, salt))
            {
                #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³")]
                let share = |k: u64| k as f64 / t.pairs as f64;
                println!(
                    "layer {layer:?}, {class}: {} pairs; the pre-test passes over {} ({:.4}); \
                     Detached {} ({:.4}), {:.4} of those passed over; a star may have collapsed \
                     in {} ({:.4})",
                    t.pairs,
                    t.passed_over,
                    share(t.passed_over),
                    t.detached,
                    share(t.detached),
                    share(t.detached) / share(t.passed_over.max(1)),
                    t.collapsed,
                    share(t.collapsed),
                );
                for line in t.missed.iter().take(20) {
                    println!("  Detached but passed by the pre-test: {line}");
                }
                missed += t.missed.len();
            }
        }
        assert_eq!(missed, 0, "Detached pairs that the pre-test passes");
    }

    /// P11.T17.a: 2,000 of the generator's pairs, 400 of each stellar layer, each at its record's
    /// age and at a young one: no `Detached` pair passes `can_interact` at its window's end.
    #[test]
    fn detached_generated_pairs_are_passed_over() {
        no_detached_pair_interacts(400);
    }

    /// P11.T17.a's slow test: as [`detached_generated_pairs_are_passed_over`] over 10⁵ pairs of
    /// each stellar layer, recording each layer's share of `Detached` against the pre-test's.
    #[test]
    #[ignore = "slow: 5 × 10⁵ generated pairs, each through the pre-test twice"]
    fn no_detached_pair_is_passed_by_the_pre_test() {
        no_detached_pair_interacts(100_000);
    }

    // --- P11.T17.c's verdicts against the engine ----------------------------------------------

    use super::super::evolve::own_members;
    use super::super::pair_light::{UNSEEN_MAG, is_own_track, member_cuts, part_points};
    use super::super::star::Member;
    use super::super::timeline::Context;
    use crate::coords::GalacticPosition;
    use crate::sky::photometry::absolute_v_of_state;
    use crate::stellar::binary::evolve;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::multiplicity::testing::records_near;
    use crate::stellar::multiplicity::{HierarchyNode, StarIndex, hierarchy_bound};
    use crate::stellar::system::{draw_metallicity, primary_draws};
    use crate::time::ClockWindow;

    /// The seed of P11.T17.c's slow test: its galaxy's and its draws', independent of the fit's
    /// (`pair_light::FIT_SEED`) and of T17.b's slow test.
    const VERDICT_SEED: u64 = 0x7e57_0017_c0c0_a11e;

    /// The pairs of each stellar layer of C, D and E the slow test takes near the Sun, and in the
    /// bulge.
    const SUN_PAIRS: usize = 100_000;
    const BULGE_PAIRS: usize = 25_000;

    /// The pairs of each layer whose lighter star the slow test takes below 0.1 M☉ (the module's
    /// item 4).
    const LIGHT_PAIRS: usize = 10_000;

    /// One pair to check: the generator's input and its age at +H, years.
    type Listed = (BinaryInput, f64);

    /// `n` star–star pairs of the records of `layer` nearest `at`, as P11.T16's
    /// [`hierarchy_bound`] lists them for the census: every pair of two stars of every attempt it
    /// lists, with the generator's masses, composition, drawn orbit, draws at that attempt and
    /// record's age at the epoch, and the record's age at +H.
    fn listed_pairs(galaxy: &Galaxy, layer: Layer, at: &GalacticPosition, n: usize) -> Vec<Listed> {
        let mut wanted = n / 2 + 64;
        loop {
            let records = records_near(galaxy, layer, at, wanted);
            let mut pairs = Vec::with_capacity(n);
            for record in &records {
                let comp = draw_metallicity(galaxy, record);
                let bound = hierarchy_bound(galaxy, record, &comp);
                let horizon = record.age_at(ClockWindow::END).value();
                for listed in bound.attempts() {
                    let attempt = u32::from(listed.attempt().get());
                    let h = listed.hierarchy();
                    let draws = |s: StarIndex| {
                        if s == StarIndex::PRIMARY {
                            primary_draws(galaxy, record)
                        } else {
                            StarDraws::for_attempt(galaxy.seed(), h.star(s).body(), attempt)
                        }
                    };
                    for pair in listed.pairs() {
                        let HierarchyNode::Pair { orbit, .. } = h.node(pair.node()) else {
                            unreachable!("a listed pair is a pair");
                        };
                        let [a, b] = pair.stars();
                        let input = BinaryInput::new(
                            h.star(a).initial_mass(),
                            h.star(b).initial_mass(),
                            comp,
                            *orbit,
                            [draws(a), draws(b)],
                            record.age_at_epoch(),
                        )
                        .expect("a listed pair's stars are of 0.08-150 M_sun");
                        pairs.push((input, horizon));
                        if pairs.len() == n {
                            return pairs;
                        }
                    }
                }
            }
            assert_eq!(
                records.len(),
                wanted,
                "{layer:?}: too few records for {n} pairs"
            );
            wanted *= 2;
        }
    }

    /// `pairs` with the lighter star of each, in turn, put below 0.1 M☉, at 0.08–0.1 M☉ from
    /// `mix`, on the same drawn orbit about the new total mass: `n` of them.
    fn with_light_members(pairs: &[Listed], n: usize, mix: &mut Mix) -> Vec<Listed> {
        pairs
            .iter()
            .cycle()
            .take(n)
            .map(|(input, horizon)| {
                let mut masses = input.masses();
                let k = usize::from(masses[1] < masses[0]);
                masses[k] = SolarMasses::new((0.08 + 0.02 * mix.unit()).min(masses[k].value()));
                let mu = crate::units::GravitationalParameter::from_solar_masses(SolarMasses::new(
                    masses[0].value() + masses[1].value(),
                ));
                let orbit = input.orbit().scaled(1.0, mu).expect("the same orbit");
                let light = BinaryInput::new(
                    masses[0],
                    masses[1],
                    *input.composition(),
                    orbit,
                    input.draws().clone(),
                    input.age_at_epoch(),
                )
                .expect("a pair of 0.08-150 M_sun");
                (light, *horizon)
            })
            .collect()
    }

    /// What a pair holds over a window, as the generator runs it.
    #[derive(Debug, Clone, Copy)]
    struct Held {
        /// Whether a star of the pair lives in the window, pair-evolved or its own model.
        living: bool,
        /// Whether a living star departs from its own single-star model, or a product lives.
        departing: bool,
        /// The brightest V of those, mag: +∞ for none, [`UNSEEN_MAG`] for some without a V.
        departing_mag: f64,
    }

    /// What the pair of `input` holds over the window `[w0, w1]` (years), run as `run_pairs` runs
    /// it to its age at +H, `horizon` (years): through [`evolve`], whose pre-test gate makes a
    /// pair that cannot interact by then one detached segment of its own models.
    ///
    /// Each star's state is read as P11.T17.b's walk reads it (`pair_light::rows_of`), at its
    /// cuts and five points a part inside the window, and compared with its own model's.
    fn held(input: &BinaryInput, horizon: f64, [w0, w1]: [f64; 2]) -> Held {
        let mut held = Held {
            living: false,
            departing: false,
            departing_mag: f64::INFINITY,
        };
        let w0 = w0.max(0.0);
        if w1 < w0 {
            return held;
        }
        let timeline = evolve(input, Years::new(horizon));
        let own = own_members(input, horizon, None);
        let own_ctx = Context::of(input);
        let ctx = timeline.context();
        let Some(first) = timeline.segments().first() else {
            return held;
        };
        let mut cuts = Vec::new();
        let mut points = Vec::new();
        for segment in timeline.segments() {
            let (a, b) = (
                segment.start().value().max(w0),
                segment.end().value().min(w1),
            );
            if b < a {
                continue;
            }
            let members = segment.members().iter().zip(first.members()).zip(&own);
            for (slot, ((member, first_member), own_member)) in members.enumerate() {
                if matches!(member, Member::Gone | Member::Remnant { .. }) {
                    continue;
                }
                member_cuts(member, a, b, &mut cuts);
                points.clear();
                if cuts.len() < 2 {
                    points.push(a);
                } else {
                    points.extend(
                        cuts.windows(2)
                            .flat_map(|part| part_points(part[0], part[1])),
                    );
                }
                let own_track = is_own_track(member, first_member);
                for &t in &points {
                    let state = member.state_at(ctx, slot, t);
                    if !state.phase().is_living() {
                        continue;
                    }
                    held.living = true;
                    if own_track || state == own_member.state_at(&own_ctx, slot, t) {
                        continue;
                    }
                    held.departing = true;
                    let v = absolute_v_of_state(&state).map_or(UNSEEN_MAG, Magnitudes::value);
                    held.departing_mag = held.departing_mag.min(v);
                }
            }
        }
        held
    }

    /// The `Bright` magnitude, mag, that the tables would give the pair of `heavier_msun` and
    /// `lighter_msun` (M☉), metallicity coordinate `fe_h` and periastron `periastron_rsun` (R☉)
    /// over `[w0, w1]` (years) without the thin rule, where that rule makes it `None`.
    fn unthinned(
        heavier_msun: f64,
        lighter_msun: f64,
        fe_h: f64,
        periastron_rsun: f64,
        [w0, w1]: [f64; 2],
    ) -> Option<f64> {
        let (table, cell) = TABLE_LAYERS.iter().find_map(|&layer| {
            let table = PairLightTable::generator(layer)?;
            Some((
                table,
                table.cell_of(heavier_msun, lighter_msun, fe_h, periastron_rsun)?,
            ))
        })?;
        let bins = pair_light::age_bin(w0)?..=pair_light::age_bin(w1)?;
        if tabled(
            heavier_msun,
            lighter_msun,
            fe_h,
            periastron_rsun,
            [w0, w1],
            HeldStars::Possible,
        )
        .is_some()
        {
            return None;
        }
        let fe_cell = table.grid().cell_parts(cell)[2];
        let held = pair_light::held_floor(fe_cell, heavier_msun + lighter_msun)?;
        let (held_from, floor) = (held.first_bin(), held.cmag());
        bins.map(|bin| {
            let changed = table.changed_cmag_for(cell, bin, lighter_msun);
            if bin >= held_from {
                changed.min(floor)
            } else {
                changed
            }
        })
        .filter(|&k| k != DARK_CMAG)
        .map(|k| k.min(FAINTEST_CMAG))
        .min()
        .map(from_cmag)
    }

    /// What one set of a layer's pairs came to.
    #[derive(Debug, Default)]
    struct Checked {
        /// The pairs' verdicts ([`VERDICTS`]) over windows the census asks (a point, up to
        /// 1,000 years, up to 3 × 10⁵ years), and over the wide ones (up to 0.5 dex).
        census: [u64; 5],
        wide: [u64; 5],
        /// The `None`s of a thin bin (the module's), among `census` and `wide`.
        thin: [u64; 2],
        /// Of those, the pairs holding a departing star or product with a V, the least of its V
        /// less the `Bright` magnitude the tables would give without the thin rule, mag, and
        /// those brighter than that magnitude: what the rule guards against.
        thin_held: u64,
        thin_least_room: f64,
        thin_brighter: u64,
        /// The magnitudes of the `Bright` verdicts over the census's windows, mag.
        bright_mags: Vec<f64>,
        /// Over the census's windows, the pairs the pre-test passes over at the window's end
        /// that are not `Detached`, by verdict: what the closed form loses, like for like.
        passed_not_detached: [u64; 5],
        /// `Bright` pairs holding a departing star or product with a V, and the least of its V
        /// less M, mag.
        bright_held: u64,
        least_room: f64,
        /// Pairs holding a departing star or product, among those checked.
        departing: u64,
        violations: Vec<String>,
        /// Pairs on which the engine's debug assertions fired (release builds skip them).
        engine_panics: Vec<String>,
    }

    impl Checked {
        fn new() -> Self {
            Self {
                least_room: f64::INFINITY,
                thin_least_room: f64::INFINITY,
                ..Self::default()
            }
        }

        fn merge(&mut self, other: Self) {
            for k in 0..5 {
                self.census[k] += other.census[k];
                self.wide[k] += other.wide[k];
            }
            for k in 0..2 {
                self.thin[k] += other.thin[k];
            }
            self.thin_held += other.thin_held;
            self.thin_least_room = self.thin_least_room.min(other.thin_least_room);
            self.thin_brighter += other.thin_brighter;
            self.bright_mags.extend(other.bright_mags);
            for k in 0..5 {
                self.passed_not_detached[k] += other.passed_not_detached[k];
            }
            self.bright_held += other.bright_held;
            self.least_room = self.least_room.min(other.least_room);
            self.departing += other.departing;
            self.violations.extend(other.violations);
            self.engine_panics.extend(other.engine_panics);
        }

        /// Checks the pair of `input` over `window`, its age at +H `horizon`, of window kind
        /// `kind`.
        fn check(&mut self, input: &BinaryInput, horizon: f64, window: [f64; 2], kind: usize) {
            let [m1, m2] = input.masses();
            let p = input.orbit().periapsis();
            let comp = input.composition();
            let [w0, w1] = window;
            let verdict = pair_light_bound(m1, m2, p, comp, Years::new(w0)..=Years::new(w1));
            let wide = usize::from(kind % 4 == 3);
            let shares = if wide == 1 {
                &mut self.wide
            } else {
                &mut self.census
            };
            shares[verdict_index(verdict)] += 1;
            if let (Some(PairLight::Bright(m)), 0) = (verdict, wide) {
                self.bright_mags.push(m.value());
            }
            if wide == 0 && verdict != Some(PairLight::Detached) {
                self.passed_not_detached[verdict_index(verdict)] +=
                    u64::from(!can_interact(input, Years::new(w1)));
            }
            let describe = || {
                format!(
                    "{:.4} + {:.4} M_sun, periastron {:.4e} R_sun, e {:.3}, [Fe/H] {:.3}, window \
                     {w0:.6e}-{w1:.6e} yr, +H at {horizon:.6e} yr: {verdict:?}",
                    m1.value(),
                    m2.value(),
                    p.value() / SOLAR_RADIUS_M,
                    input.orbit().eccentricity().value(),
                    comp.fe_h().value()
                )
            };
            let Some(verdict) = verdict else {
                // A thin bin's `None`: what `Bright` would have said without the thin rule.
                let (heavier, lighter) = (m1.value().max(m2.value()), m1.value().min(m2.value()));
                let fe_h = fe_h_of(comp);
                let Some(m) = unthinned(heavier, lighter, fe_h, p.value() / SOLAR_RADIUS_M, window)
                else {
                    return;
                };
                self.thin[wide] += 1;
                let Ok(held) = std::panic::catch_unwind(|| held(input, horizon, window)) else {
                    self.engine_panics.push(describe());
                    return;
                };
                if held.departing_mag < UNSEEN_MAG {
                    self.thin_held += 1;
                    self.thin_least_room = self.thin_least_room.min(held.departing_mag - m);
                    self.thin_brighter += u64::from(held.departing_mag < m);
                }
                return;
            };
            let Ok(held) = std::panic::catch_unwind(|| held(input, horizon, window)) else {
                self.engine_panics.push(describe());
                return;
            };
            self.departing += u64::from(held.departing);
            let violated = match verdict {
                PairLight::Detached => held.departing || can_interact(input, Years::new(w1)),
                PairLight::Unchanged => held.departing,
                PairLight::Remnants => held.living,
                PairLight::Bright(m) => {
                    if held.departing_mag < UNSEEN_MAG {
                        self.bright_held += 1;
                        self.least_room = self.least_room.min(held.departing_mag - m.value());
                    }
                    held.departing_mag < m.value()
                }
            };
            if violated {
                self.violations
                    .push(format!("{}; held {held:?}", describe()));
            }
        }
    }

    /// Checks each of `pairs` over a window in the old class (about its own age at +H) and in the
    /// young (its age at +H log-uniform over 10⁵–10⁸ years), in eight shares (in turn on
    /// wasm32-wasip1): the old and young tallies.
    ///
    /// A window ends up to 2H + L before the age at +H, as the census's light-time windows do,
    /// at the bound's extremes for a quarter of the pairs; its kind is [`window_ending`]'s, in
    /// turn.
    fn checked(pairs: &[Listed], salt: u64) -> [Checked; 2] {
        const SHARES: usize = 8;
        let mut mix = Mix(VERDICT_SEED ^ salt);
        // Per pair: the young age at +H, and each class's window.
        let plans: Vec<(f64, [[f64; 2]; 2])> = (0..pairs.len())
            .map(|i| {
                let young = crate::math::exp10(5.0 + 3.0 * mix.unit());
                let windows = [pairs[i].1, young].map(|horizon| {
                    let before = match i % 8 {
                        0 => 0.0,
                        1 => RUN_HORIZON_PAST_WINDOW_YEARS,
                        _ => RUN_HORIZON_PAST_WINDOW_YEARS * mix.unit(),
                    };
                    window_ending(&mut mix, horizon - before, i / 8)
                });
                (young, windows)
            })
            .collect();
        let share = |k: usize| {
            let mut part = [Checked::new(), Checked::new()];
            for (i, (input, horizon)) in pairs.iter().enumerate().skip(k).step_by(SHARES) {
                let (young, windows) = plans[i];
                part[0].check(input, *horizon, windows[0], i / 8);
                let young_input = BinaryInput::new(
                    input.masses()[0],
                    input.masses()[1],
                    *input.composition(),
                    *input.orbit(),
                    input.draws().clone(),
                    Years::new(young - CLOCK_WINDOW_H.as_julian_years_f64()),
                )
                .expect("the same pair");
                part[1].check(&young_input, young, windows[1], i / 8);
            }
            part
        };
        #[cfg(not(target_family = "wasm"))]
        let parts: Vec<[Checked; 2]> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..SHARES)
                .map(|k| {
                    let share = &share;
                    scope.spawn(move || share(k))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a share's thread"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let parts: Vec<[Checked; 2]> = (0..SHARES).map(share).collect();
        let mut total = [Checked::new(), Checked::new()];
        for [old, young] in parts {
            total[0].merge(old);
            total[1].merge(young);
        }
        total
    }

    /// Prints one set's tallies, and returns its violations.
    fn report(layer: Layer, set: &str, tallies: &[Checked; 2]) -> usize {
        let mut violations = 0;
        for (class, t) in ["old", "young"].iter().zip(tallies) {
            let shares = |counts: &[u64; 5]| {
                let total = counts.iter().sum::<u64>().max(1);
                #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³")]
                let share = |k: u64| k as f64 / total as f64;
                VERDICTS
                    .iter()
                    .zip(counts)
                    .map(|(name, &k)| format!("{name} {k} ({:.4})", share(k)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut mags = t.bright_mags.clone();
            mags.sort_by(f64::total_cmp);
            let quantiles: Vec<String> = [0.1, 0.25, 0.5, 0.75, 0.9]
                .iter()
                .map(|&q| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        clippy::cast_precision_loss,
                        reason = "an index into a list of under 2⁵³, rounded down"
                    )]
                    let k = (q * mags.len().saturating_sub(1) as f64) as usize;
                    mags.get(k)
                        .map_or_else(|| "-".to_owned(), |m| format!("{m:.2}"))
                })
                .collect();
            let lost: u64 = t.passed_not_detached.iter().sum();
            println!(
                "layer {layer:?}, {set}, {class}: the pre-test passes over {} of the census \
                 windows' pairs, {} of them not Detached: {}",
                t.census[0] + lost,
                lost,
                VERDICTS
                    .iter()
                    .zip(&t.passed_not_detached)
                    .map(|(name, k)| format!("{name} {k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "layer {layer:?}, {set}, {class}: census windows {}; wide windows {}; thin none \
                 {:?} (census, wide), of which holding a V {}, least room {:.4} mag without the \
                 rule, {} brighter; Bright's M at 10/25/50/75/90% {}; departing {}; Bright \
                 holding a V {}, least room {:.4} mag; {} violations; {} engine panics",
                shares(&t.census),
                shares(&t.wide),
                t.thin,
                t.thin_held,
                t.thin_least_room,
                t.thin_brighter,
                quantiles.join("/"),
                t.departing,
                t.bright_held,
                t.least_room,
                t.violations.len(),
                t.engine_panics.len()
            );
            for line in t.violations.iter().take(20) {
                println!("  violation: {line}");
            }
            for line in &t.engine_panics {
                println!("  the engine's assertion fired: {line}");
            }
            // The engine's debug assertion fires for about 1 in 3.9 × 10⁴ of E's table samples
            // (plan 11's Risks, P11.T17.b); a rise would shrink what the test covers.
            assert!(
                t.engine_panics.len() <= 5,
                "the engine's assertion fired {} times",
                t.engine_panics.len()
            );
            violations += t.violations.len();
        }
        violations
    }

    /// P11.T17.c's slow test: the verdicts against the engine, over ≥ 10⁵ realised pairs of each
    /// of C, D and E, as P11.T16's `hierarchy_bound` lists them, near the Sun and in the bulge,
    /// and pairs of theirs with a star below 0.1 M☉, each in an old and a young window, on a seed
    /// of its own:
    /// - no `Detached` pair interacts before the window's end, or holds a departing star;
    /// - no `Unchanged` pair holds a departing living star or a living product in the window;
    /// - no `Remnants` pair holds a living star or product in the window;
    /// - no `Bright(M)` pair holds a departing star or product brighter than M, which the census
    ///   takes with each star's own bound.
    #[test]
    #[ignore = "slow: 4.05 × 10⁵ pairs through the binary engine, each in two windows"]
    fn the_pair_verdicts_hold_against_the_engine() {
        let galaxy = super::super::testing::milky_way(VERDICT_SEED);
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let bulge = GalacticPosition::from_light_years([0.0, 2_000.0, 300.0]).expect("in the cube");
        let mut violations = 0;
        for (salt, layer) in (0x0c_u64..).zip([Layer::C, Layer::D, Layer::E]) {
            let near_sun = listed_pairs(&galaxy, layer, &sun, SUN_PAIRS);
            let in_bulge = listed_pairs(&galaxy, layer, &bulge, BULGE_PAIRS);
            let mut mix = Mix(VERDICT_SEED ^ salt ^ 0x0117);
            let light = with_light_members(&near_sun, LIGHT_PAIRS, &mut mix);
            for (set, pairs, k) in [
                ("near the Sun", &near_sun, 0x100),
                ("in the bulge", &in_bulge, 0x200),
                ("a star below 0.1 M_sun", &light, 0x300),
            ] {
                violations += report(layer, set, &checked(pairs, salt ^ k));
            }
        }
        assert_eq!(violations, 0, "verdicts the engine contradicts");
    }

    /// The seed of the samples [`a_capped_timelines_held_stars_are_bounded`] reads, from
    /// P11.T17.c's follow-up probe of D's close, metal-rich pairs of 5–8 M☉.
    const CAPPED_SEED: u64 = 0x7a59_e7ed_d0b1_0517;

    /// A capped timeline's held stars are bounded (the module's held floor; P11.T17.c's follow-up
    /// to R06.T8.g's bulge record `0x61fec2d802000037`). Two of D's table samples reach the
    /// engine's cap on segments as an accretor carried on its main sequence is fed at its end,
    /// and their stretched last segments hold it there to 1.5 × 10¹⁰ years:
    /// - a hydrogen main-sequence gainer of 8.88 M☉ at τ = 1, at M<sub>V</sub> −3.48, beside its
    ///   donor's white dwarf (5.72 + 4.36 M☉); the tables alone read `Bright(−1.38)` at 1 Gyr and
    ///   `Remnants` from 3 Gyr;
    /// - a helium main-sequence star of 1.59 M☉ at M<sub>V</sub> 1.98, fed by a helium giant
    ///   (finding F15; 5.06 + 4.06 M☉); the tables alone read `Remnants` from about 4 Gyr.
    ///
    /// At every window from the cap to the age run to, the verdict holds against the engine, as
    /// the slow test holds it, and it is never `Remnants` or `Unchanged`.
    #[test]
    fn a_capped_timelines_held_stars_are_bounded() {
        let grid = pair_light::PairGrid::of(Layer::D).expect("D's grid");
        for (cell, index) in [(4_817, 10_021), (4_273, 10_253)] {
            let input = pair_light::sample_input(&grid, cell, index, CAPPED_SEED);
            let horizon = pair_light::LAST_AGE_YEARS;
            let timeline = evolve(&input, Years::new(horizon));
            assert!(timeline.hit_segment_cap(), "cell {cell}, sample {index}");
            let cap = timeline
                .segments()
                .last()
                .expect("a segment")
                .start()
                .value();
            let [m1, m2] = input.masses();
            let p = input.orbit().periapsis();
            let comp = input.composition();
            let mut bright = 0;
            for k in 0..=8 {
                let lo = crate::math::log10(cap);
                let hi = crate::math::log10(horizon);
                let t = crate::math::exp10(lo + (hi - lo) * f64::from(k) / 8.0).min(horizon);
                for window in [[t, t], [t * 0.9, t]] {
                    let verdict = pair_light_bound(
                        m1,
                        m2,
                        p,
                        comp,
                        Years::new(window[0])..=Years::new(window[1]),
                    );
                    let truth = held(&input, horizon, window);
                    let what = || {
                        format!("cell {cell}, sample {index}, {window:?}: {verdict:?}, {truth:?}")
                    };
                    assert!(truth.living && truth.departing, "{}", what());
                    match verdict {
                        Some(PairLight::Bright(m)) => {
                            assert!(truth.departing_mag >= m.value(), "{}", what());
                            bright += 1;
                        }
                        None => {}
                        Some(PairLight::Remnants | PairLight::Unchanged | PairLight::Detached) => {
                            panic!("{}", what())
                        }
                    }
                }
            }
            assert!(bright > 0, "cell {cell}, sample {index}: no window Bright");
        }
    }
}
