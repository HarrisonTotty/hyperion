//! Every fixed random draw of one star, each read from a domain tag of its own (or its own words
//! of one) under the star's body ID, so that adding a draw moves none already made (plan 06,
//! P06.T2).
//!
//! A star's draws are made once and never depend on time or on each other: the stellar stage is a
//! pure function of `(m0, Composition, StarDraws, age)`. Each tag opens its own stream,
//! `Stream::open(seed, tag, ObjectKey::from(body))` with the tag named in the field's
//! documentation, and its fields read their words from the start of the attempt's block (only
//! `star.ns.geometry` holds two fields, on separate words). Attempt k of a
//! conditional redraw (plan 09's catalogue classes, plan 11's binaries) reads the same tags from
//! word 64k ([`StarDraws::for_attempt`]), so a redraw never touches another tag, and attempt 0 is
//! [`StarDraws::for_star`].
//!
//! Draws are typed by how they are used, never as bare `f64`:
//!
//! - a [`UnitUniform`] is a rank, mapped through a distribution's quantile (one word);
//! - a [`StandardNormal`] is a Box–Muller variate (two words);
//! - a [`Mark`] decides against an integer [`Threshold`](crate::rng::Threshold) (one word), as every
//!   random decision in the crate does;
//! - a [`UnitVector`] is an isotropic direction along the galactic axes (two words: z, then the
//!   azimuth).
//!
//! Where a consumer redraws a normal until it meets a condition (the kick score, the neutron
//! star's birth period), the field holds the stream's first [`REDRAW_TRIES`] normals and the
//! consumer takes the first that its condition accepts.

use core::f64::consts::TAU;

use crate::coords::UnitVector;
use crate::id::BodyId;
use crate::math;
use crate::rng::{DomainTag, Mark, ObjectKey, Seed, Stream, tags};

/// Words of each stream that one attempt owns: attempt k starts at word `64 × k`.
///
/// Every field reads at most [`2 × REDRAW_TRIES`](REDRAW_TRIES) words of its block. Changing this
/// constant moves every attempt after the first, which is a generator-version change.
pub const ATTEMPT_WORDS: u64 = 64;

/// Normals held for a draw that its consumer redraws until a condition holds.
///
/// Eight tries leave the kick score (ξ = 1 + 0.45 z rejected below zero, 1.3% a try; P06.T19.a,
/// the brainstorm's 45% scatter after Disberg, Mandel and Hirai 2026) failing all eight about once
/// in 10¹⁵ stars, and the neutron star's birth period (300 + 150 z ms rejected at or below 10 ms,
/// 2.7% a try; P06.T21.a, design note 13 after Faucher-Giguère and Kaspi 2006) about once in
/// 4 × 10¹², fewer than one star in a galaxy either way; the consumer documents its fallback.
/// Sixteen of the block's 64 words. Changing the count changes which try a star whose first eight
/// all fail ends on, which is a generator-version change even though no existing golden line moves.
pub const REDRAW_TRIES: usize = 8;

const _: () = assert!(
    2 * REDRAW_TRIES as u64 <= ATTEMPT_WORDS,
    "an attempt's redraws must fit its block"
);

/// A variate uniform on the open interval (0, 1): a rank to map through a quantile function.
///
/// Drawn with [`Stream::uniform_open`], so it is never 0 or 1 and both `ln u` and a normal quantile
/// of it are finite.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UnitUniform(f64);

impl UnitUniform {
    /// The median rank, one half.
    pub const HALF: Self = Self(0.5);

    /// The rank `value`, or `None` unless 0 < `value` < 1.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value > 0.0 && value < 1.0).then_some(Self(value))
    }

    /// The rank, in (0, 1).
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// The next word of `stream` as a rank. One word.
    fn draw(stream: &mut Stream) -> Self {
        Self(stream.uniform_open())
    }
}

/// A standard normal variate, drawn by Box–Muller from two words.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct StandardNormal(f64);

impl StandardNormal {
    /// The median of the standard normal, zero.
    pub const ZERO: Self = Self(0.0);

    /// The variate `value`, or `None` if it is not finite.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    /// The variate.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// The value at this variate's rank Φ(z) of a normal of `mean` and `sigma` truncated to
    /// [`lower`, `upper`]: the distribution of redrawing a normal until it falls inside, from this
    /// one variate, increasing in it, so that a held draw piles nothing at either end.
    ///
    /// With a = (`lower` − `mean`) ÷ `sigma`, b likewise and w = Φ(b) − Φ(a), the rank's distance
    /// below it is p = Φ(a) + Φ(z) w and above it q = Q(b) + Q(z) w, Q being the upper tail
    /// ½ erfc(x ÷ √2), and the standardised value is Φ⁻¹(p) or −Φ⁻¹(q), whichever of p and q is
    /// smaller, so that neither tail loses its precision. The result is clamped to the interval
    /// against rounding. An interval too narrow or too far in a tail for w to be positive, beyond
    /// about 38 standard deviations, gives the point of it nearest the mean.
    ///
    /// # Panics
    ///
    /// In debug builds, if `sigma` is not positive or `lower` exceeds `upper`.
    ///
    /// # Examples
    ///
    /// A planet's mass of median 7.7 M⊕ and 0.54 dex of scatter, held to 1–20 M⊕ by its rank
    /// rather than by clamping: a variate far above the range still lands inside it, below the
    /// ceiling, and the median variate lands below the median, since more of the law lies above
    /// the range than below it.
    ///
    /// ```
    /// use hyperion_sim::stellar::draws::StandardNormal;
    ///
    /// let (median, sigma, lo, hi) = (7.7_f64.log10(), 0.54, 0.0, 20.0_f64.log10());
    /// let high = StandardNormal::new(3.0).expect("finite");
    /// let x = high.truncated(median, sigma, lo, hi);
    /// assert!(x < hi && x > median);
    /// let mid = StandardNormal::ZERO.truncated(median, sigma, lo, hi);
    /// assert!(mid < median);
    /// ```
    #[must_use]
    pub fn truncated(self, mean: f64, sigma: f64, lower: f64, upper: f64) -> f64 {
        debug_assert!(
            sigma > 0.0 && lower <= upper,
            "a truncated normal needs σ > 0 and a non-empty range: σ = {sigma}, [{lower}, {upper}]"
        );
        let (a, b) = ((lower - mean) / sigma, (upper - mean) / sigma);
        let inside = if a >= 0.0 {
            upper_tail(a) - upper_tail(b)
        } else if b <= 0.0 {
            upper_tail(-b) - upper_tail(-a)
        } else {
            1.0 - upper_tail(-a) - upper_tail(b)
        };
        if inside <= 0.0 || inside.is_nan() {
            return mean.clamp(lower, upper);
        }
        let z = self.0;
        let below = upper_tail(-a) + upper_tail(-z) * inside;
        let above = upper_tail(b) + upper_tail(z) * inside;
        let x = if below <= above {
            if below > 0.0 {
                math::normal_quantile(below)
            } else {
                a
            }
        } else if above > 0.0 {
            -math::normal_quantile(above)
        } else {
            b
        };
        (mean + sigma * x).clamp(lower, upper)
    }

    /// The next two words of `stream` as a standard normal. Two words.
    fn draw(stream: &mut Stream) -> Self {
        Self(stream.standard_normal())
    }

    /// The next `N` standard normals of `stream`. `2N` words.
    fn draw_array<const N: usize>(stream: &mut Stream) -> [Self; N] {
        core::array::from_fn(|_| Self::draw(stream))
    }
}

/// The upper tail of the standard normal, Q(x) = 1 − Φ(x) = ½ erfc(x ÷ √2), accurate in both
/// tails.
#[must_use]
fn upper_tail(x: f64) -> f64 {
    0.5 * math::erfc(x * core::f64::consts::FRAC_1_SQRT_2)
}

/// The mark in the middle of the range, so that it lies below a threshold of probability p exactly
/// when p exceeds one half.
const MEDIAN_MARK: Mark = Mark::from_word(1 << 63);

/// An isotropic direction along the galactic axes from the next two words of `stream`: z uniform
/// on (−1, 1), then the azimuth from +x uniform on [0, 2π).
fn isotropic(stream: &mut Stream) -> UnitVector {
    let z = 2.0 * stream.uniform_open() - 1.0;
    let azimuth = TAU * stream.uniform();
    let (sin, cos) = math::sin_cos(azimuth);
    let across = (1.0 - z * z).sqrt();
    UnitVector::from_components([across * cos, across * sin, z])
        .expect("a point of the unit sphere with |z| < 1 is finite and non-zero")
}

/// Every fixed draw of one star, as explicit variates: what [`StarDraws::from_parts`] takes.
///
/// Each field names the tag its [`StarDraws::for_star`] value is read from and how many of that
/// stream's words it takes; the consumer that maps it to a physical quantity is the task named.
#[derive(Debug, Clone, PartialEq)]
pub struct StarDrawsParts {
    /// Reimers η (P06.T10, design note 7). [`tags::STAR_ETA`], two words.
    pub eta: StandardNormal,
    /// Rotation rank (P06.T25). [`tags::STAR_ROTATION`], one word.
    pub rotation: UnitUniform,
    /// Fossil field presence and strength (P06.T25). [`tags::STAR_MAGNETISM`], one word.
    ///
    /// Present when the mark lies below the field's threshold k; the strength's rank is then
    /// (mark + ½) ÷ k, which is uniform on (0, 1) and never 0.
    pub magnetism: Mark,
    /// Spin axis (P06.T25). [`tags::STAR_SPIN_AXIS`], two words.
    pub spin_axis: UnitVector,
    /// Protostellar disc lifetime rank (P06.T15.c). [`tags::STAR_DISC_LIFETIME`], one word.
    pub disc_lifetime: UnitUniform,
    /// The provisional companion-stripped mark (design note 11). [`tags::STAR_STRIPPED`], one
    /// word.
    pub stripped: Mark,
    /// Neutron star or black hole (P06.T18.a). [`tags::STAR_REMNANT_TYPE`], one word.
    pub remnant_type: Mark,
    /// Complete fallback (P06.T18.a). [`tags::STAR_REMNANT_FALLBACK`], one word.
    pub remnant_fallback: Mark,
    /// Remnant mass scatter (P06.T18.a). [`tags::STAR_REMNANT_MASS`], two words.
    pub remnant_mass: StandardNormal,
    /// Kick score factor tries, in draw order (P06.T19.a). [`tags::STAR_KICK_SCORE`], sixteen
    /// words.
    pub kick_score: [StandardNormal; REDRAW_TRIES],
    /// Low kick mode for a companion-stripped progenitor (P06.T19.c). [`tags::STAR_KICK_MODE`],
    /// one word.
    pub kick_mode: Mark,
    /// Low kick mode's velocity components along the galactic axes (P06.T19.c).
    /// [`tags::STAR_KICK_LOW`], six words.
    pub kick_low: [StandardNormal; 3],
    /// Kick direction (P06.T19.a). [`tags::STAR_KICK_DIRECTION`], two words.
    pub kick_direction: UnitVector,
    /// White dwarf atmosphere (P06.T20.b). [`tags::STAR_WD_ATMOSPHERE`], one word.
    pub wd_atmosphere: Mark,
    /// White dwarf carbon (P06.T20.b). [`tags::STAR_WD_CARBON`], one word.
    pub wd_carbon: Mark,
    /// White dwarf metal pollution (P06.T20.b). [`tags::STAR_WD_METALS`], one word.
    pub wd_metals: Mark,
    /// Neutron star birth period tries, in draw order (P06.T21.a). [`tags::STAR_NS_SPIN`],
    /// sixteen words.
    pub ns_spin: [StandardNormal; REDRAW_TRIES],
    /// Neutron star birth field (P06.T21.a). [`tags::STAR_NS_FIELD`], two words.
    pub ns_field: StandardNormal,
    /// Neutron star spin axis (P06.T21.a). [`tags::STAR_NS_GEOMETRY`], words 0–1.
    pub ns_spin_axis: UnitVector,
    /// Neutron star magnetic inclination rank (P06.T21.a). [`tags::STAR_NS_GEOMETRY`], word 2.
    pub ns_inclination: UnitUniform,
    /// Pulsar phase at the epoch (P06.T21.d). [`tags::STAR_NS_PHASE`], one word.
    pub ns_phase: UnitUniform,
    /// Black hole spin (P06.T22). [`tags::STAR_BH_SPIN`], two words.
    pub bh_spin: StandardNormal,
    /// Planetary nebula expansion speed rank (P06.T16.b). [`tags::STAR_NEBULA`], one word.
    pub nebula: UnitUniform,
}

impl StarDrawsParts {
    /// The median star: every normal zero, every rank and mark one half, every direction galactic
    /// north (a direction has no median; north is the convention).
    ///
    /// Quadratures that integrate over some draws take the rest from here, with struct update
    /// syntax.
    ///
    /// # Examples
    ///
    /// A three-point Gauss–Hermite quadrature over Reimers η alone, every other draw at its
    /// median:
    ///
    /// ```
    /// use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
    ///
    /// let nodes = [(-3f64.sqrt(), 1.0 / 6.0), (0.0, 2.0 / 3.0), (3f64.sqrt(), 1.0 / 6.0)];
    /// let mut stars = Vec::new();
    /// for (z, weight) in nodes {
    ///     let eta = StandardNormal::new(z).ok_or("a node is finite")?;
    ///     let parts = StarDrawsParts { eta, ..StarDrawsParts::MEDIAN };
    ///     stars.push((StarDraws::from_parts(parts), weight));
    /// }
    /// assert_eq!(stars[1].0, StarDraws::median());
    /// assert_eq!(stars[0].0.rotation(), StarDraws::median().rotation());
    /// # Ok::<(), &str>(())
    /// ```
    pub const MEDIAN: Self = Self {
        eta: StandardNormal::ZERO,
        rotation: UnitUniform::HALF,
        magnetism: MEDIAN_MARK,
        spin_axis: UnitVector::NORTH,
        disc_lifetime: UnitUniform::HALF,
        stripped: MEDIAN_MARK,
        remnant_type: MEDIAN_MARK,
        remnant_fallback: MEDIAN_MARK,
        remnant_mass: StandardNormal::ZERO,
        kick_score: [StandardNormal::ZERO; REDRAW_TRIES],
        kick_mode: MEDIAN_MARK,
        kick_low: [StandardNormal::ZERO; 3],
        kick_direction: UnitVector::NORTH,
        wd_atmosphere: MEDIAN_MARK,
        wd_carbon: MEDIAN_MARK,
        wd_metals: MEDIAN_MARK,
        ns_spin: [StandardNormal::ZERO; REDRAW_TRIES],
        ns_field: StandardNormal::ZERO,
        ns_spin_axis: UnitVector::NORTH,
        ns_inclination: UnitUniform::HALF,
        ns_phase: UnitUniform::HALF,
        bh_spin: StandardNormal::ZERO,
        nebula: UnitUniform::HALF,
    };
}

/// Every fixed random draw of one star (plan 06, P06.T2).
///
/// # Examples
///
/// A star's draws depend on the seed and its body ID alone, whatever else has been drawn, and a
/// conditional sampler's second attempt redraws every field on the same tags:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{BodyId, Layer, SystemId};
/// use hyperion_sim::stellar::draws::StarDraws;
///
/// let cell = GenCell::new(CellSize::Ly8, [10, -3, 0])?;
/// let primary = BodyId::new(SystemId::from_parts(Layer::A, cell, 4)?, 0);
/// let seed = Seed::new(42);
/// let draws = StarDraws::for_star(seed, primary);
/// assert_eq!(draws, StarDraws::for_attempt(seed, primary, 0));
/// assert_ne!(draws, StarDraws::for_attempt(seed, primary, 1));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StarDraws {
    parts: StarDrawsParts,
}

impl StarDraws {
    /// The draws of `star` in the universe of `seed`: attempt 0.
    #[must_use]
    pub fn for_star(seed: Seed, star: BodyId) -> Self {
        Self::for_attempt(seed, star, 0)
    }

    /// The draws of `star` for attempt `attempt` of a conditional redraw: every tag read from word
    /// `64 × attempt` of its stream ([`ATTEMPT_WORDS`]).
    #[must_use]
    pub fn for_attempt(seed: Seed, star: BodyId, attempt: u32) -> Self {
        let key = ObjectKey::from(star);
        let start = u64::from(attempt) * ATTEMPT_WORDS;
        let open = |tag: DomainTag| {
            let mut stream = Stream::open(seed, tag, key);
            stream.seek(start);
            stream
        };
        let mut geometry = open(tags::STAR_NS_GEOMETRY);
        let ns_spin_axis = isotropic(&mut geometry);
        let ns_inclination = UnitUniform::draw(&mut geometry);
        Self::from_parts(StarDrawsParts {
            eta: StandardNormal::draw(&mut open(tags::STAR_ETA)),
            rotation: UnitUniform::draw(&mut open(tags::STAR_ROTATION)),
            magnetism: open(tags::STAR_MAGNETISM).mark(),
            spin_axis: isotropic(&mut open(tags::STAR_SPIN_AXIS)),
            disc_lifetime: UnitUniform::draw(&mut open(tags::STAR_DISC_LIFETIME)),
            stripped: open(tags::STAR_STRIPPED).mark(),
            remnant_type: open(tags::STAR_REMNANT_TYPE).mark(),
            remnant_fallback: open(tags::STAR_REMNANT_FALLBACK).mark(),
            remnant_mass: StandardNormal::draw(&mut open(tags::STAR_REMNANT_MASS)),
            kick_score: StandardNormal::draw_array(&mut open(tags::STAR_KICK_SCORE)),
            kick_mode: open(tags::STAR_KICK_MODE).mark(),
            kick_low: StandardNormal::draw_array(&mut open(tags::STAR_KICK_LOW)),
            kick_direction: isotropic(&mut open(tags::STAR_KICK_DIRECTION)),
            wd_atmosphere: open(tags::STAR_WD_ATMOSPHERE).mark(),
            wd_carbon: open(tags::STAR_WD_CARBON).mark(),
            wd_metals: open(tags::STAR_WD_METALS).mark(),
            ns_spin: StandardNormal::draw_array(&mut open(tags::STAR_NS_SPIN)),
            ns_field: StandardNormal::draw(&mut open(tags::STAR_NS_FIELD)),
            ns_spin_axis,
            ns_inclination,
            ns_phase: UnitUniform::draw(&mut open(tags::STAR_NS_PHASE)),
            bh_spin: StandardNormal::draw(&mut open(tags::STAR_BH_SPIN)),
            nebula: UnitUniform::draw(&mut open(tags::STAR_NEBULA)),
        })
    }

    /// Draws from explicit variates, for quadratures and tests that need no ID.
    #[must_use]
    pub const fn from_parts(parts: StarDrawsParts) -> Self {
        Self { parts }
    }

    /// The median star, [`StarDrawsParts::MEDIAN`]: what a quadrature with no star passes.
    #[must_use]
    pub const fn median() -> Self {
        Self::from_parts(StarDrawsParts::MEDIAN)
    }

    /// Every draw, as explicit variates.
    #[must_use]
    pub const fn parts(&self) -> &StarDrawsParts {
        &self.parts
    }

    /// Reimers η's standard normal ([`tags::STAR_ETA`]).
    #[must_use]
    pub const fn eta(&self) -> StandardNormal {
        self.parts.eta
    }

    /// The rotation rank ([`tags::STAR_ROTATION`]).
    #[must_use]
    pub const fn rotation(&self) -> UnitUniform {
        self.parts.rotation
    }

    /// The fossil-field mark ([`tags::STAR_MAGNETISM`]).
    #[must_use]
    pub const fn magnetism(&self) -> Mark {
        self.parts.magnetism
    }

    /// The spin axis along the galactic axes ([`tags::STAR_SPIN_AXIS`]).
    #[must_use]
    pub const fn spin_axis(&self) -> UnitVector {
        self.parts.spin_axis
    }

    /// The protostellar disc lifetime rank ([`tags::STAR_DISC_LIFETIME`]).
    #[must_use]
    pub const fn disc_lifetime(&self) -> UnitUniform {
        self.parts.disc_lifetime
    }

    /// The provisional companion-stripped mark ([`tags::STAR_STRIPPED`]).
    #[must_use]
    pub const fn stripped(&self) -> Mark {
        self.parts.stripped
    }

    /// The remnant-type mark ([`tags::STAR_REMNANT_TYPE`]).
    #[must_use]
    pub const fn remnant_type(&self) -> Mark {
        self.parts.remnant_type
    }

    /// The complete-fallback mark ([`tags::STAR_REMNANT_FALLBACK`]).
    #[must_use]
    pub const fn remnant_fallback(&self) -> Mark {
        self.parts.remnant_fallback
    }

    /// The remnant mass's standard normal ([`tags::STAR_REMNANT_MASS`]).
    #[must_use]
    pub const fn remnant_mass(&self) -> StandardNormal {
        self.parts.remnant_mass
    }

    /// The kick score's tries, in draw order ([`tags::STAR_KICK_SCORE`]).
    #[must_use]
    pub const fn kick_score(&self) -> &[StandardNormal; REDRAW_TRIES] {
        &self.parts.kick_score
    }

    /// The kick-mode mark ([`tags::STAR_KICK_MODE`]).
    #[must_use]
    pub const fn kick_mode(&self) -> Mark {
        self.parts.kick_mode
    }

    /// The low kick mode's three standard normals along x, y, z ([`tags::STAR_KICK_LOW`]).
    #[must_use]
    pub const fn kick_low(&self) -> &[StandardNormal; 3] {
        &self.parts.kick_low
    }

    /// The kick direction along the galactic axes ([`tags::STAR_KICK_DIRECTION`]).
    #[must_use]
    pub const fn kick_direction(&self) -> UnitVector {
        self.parts.kick_direction
    }

    /// The white dwarf atmosphere mark ([`tags::STAR_WD_ATMOSPHERE`]).
    #[must_use]
    pub const fn wd_atmosphere(&self) -> Mark {
        self.parts.wd_atmosphere
    }

    /// The white dwarf carbon mark ([`tags::STAR_WD_CARBON`]).
    #[must_use]
    pub const fn wd_carbon(&self) -> Mark {
        self.parts.wd_carbon
    }

    /// The white dwarf metal-pollution mark ([`tags::STAR_WD_METALS`]).
    #[must_use]
    pub const fn wd_metals(&self) -> Mark {
        self.parts.wd_metals
    }

    /// The neutron star birth period's tries, in draw order ([`tags::STAR_NS_SPIN`]).
    #[must_use]
    pub const fn ns_spin(&self) -> &[StandardNormal; REDRAW_TRIES] {
        &self.parts.ns_spin
    }

    /// The neutron star birth field's standard normal ([`tags::STAR_NS_FIELD`]).
    #[must_use]
    pub const fn ns_field(&self) -> StandardNormal {
        self.parts.ns_field
    }

    /// The neutron star's spin axis along the galactic axes ([`tags::STAR_NS_GEOMETRY`]).
    #[must_use]
    pub const fn ns_spin_axis(&self) -> UnitVector {
        self.parts.ns_spin_axis
    }

    /// The neutron star's magnetic inclination rank ([`tags::STAR_NS_GEOMETRY`]).
    #[must_use]
    pub const fn ns_inclination(&self) -> UnitUniform {
        self.parts.ns_inclination
    }

    /// The pulsar's phase rank at the epoch ([`tags::STAR_NS_PHASE`]).
    #[must_use]
    pub const fn ns_phase(&self) -> UnitUniform {
        self.parts.ns_phase
    }

    /// The black hole spin's standard normal ([`tags::STAR_BH_SPIN`]).
    #[must_use]
    pub const fn bh_spin(&self) -> StandardNormal {
        self.parts.bh_spin
    }

    /// The planetary nebula's expansion speed rank ([`tags::STAR_NEBULA`]).
    #[must_use]
    pub const fn nebula(&self) -> UnitUniform {
        self.parts.nebula
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats;

    use super::*;
    use crate::coords::GenCell;
    use crate::id::{Layer, SystemId};

    fn body(layer: Layer, cell: [i32; 3], index: u32, body_index: u16) -> BodyId {
        let cell = GenCell::new(layer.cell_size(), cell).unwrap();
        let system = SystemId::from_parts(layer, cell, index).unwrap();
        BodyId::new(system, body_index)
    }

    fn bodies() -> Vec<BodyId> {
        vec![
            body(Layer::A, [652, -4_584, 2_047], 7, 0),
            body(Layer::C, [-12, 40, 1], 0, 0),
            body(Layer::E, [3, 3, -1], 2, 0),
            body(Layer::E, [3, 3, -1], 2, 1),
            body(Layer::B, [0, 0, 0], 11, 0),
        ]
    }

    /// Opens `tag` for `star` at attempt `attempt`, as a field of `StarDraws` must.
    fn stream(seed: Seed, tag: DomainTag, star: BodyId, attempt: u32) -> Stream {
        let mut s = Stream::open(seed, tag, ObjectKey::from(star));
        s.seek(u64::from(attempt) * ATTEMPT_WORDS);
        s
    }

    /// Each field equals a fresh read of its own tag's stream alone, so no field depends on the
    /// order of the others, and adding a field under a new tag cannot move one that exists.
    #[test]
    fn every_field_reads_its_own_tag_alone() {
        let seed = Seed::new(0x5eed);
        for star in bodies() {
            for attempt in [0, 1, 7] {
                let d = StarDraws::for_attempt(seed, star, attempt);
                let normal = |tag| stream(seed, tag, star, attempt).standard_normal();
                let open = |tag| stream(seed, tag, star, attempt).uniform_open();
                let mark = |tag| stream(seed, tag, star, attempt).mark();
                assert_same_bits(d.eta().value(), normal(tags::STAR_ETA));
                assert_same_bits(d.rotation().value(), open(tags::STAR_ROTATION));
                assert_eq!(d.magnetism(), mark(tags::STAR_MAGNETISM));
                assert_same_bits(d.disc_lifetime().value(), open(tags::STAR_DISC_LIFETIME));
                assert_eq!(d.stripped(), mark(tags::STAR_STRIPPED));
                assert_eq!(d.remnant_type(), mark(tags::STAR_REMNANT_TYPE));
                assert_eq!(d.remnant_fallback(), mark(tags::STAR_REMNANT_FALLBACK));
                assert_same_bits(d.remnant_mass().value(), normal(tags::STAR_REMNANT_MASS));
                assert_eq!(d.kick_mode(), mark(tags::STAR_KICK_MODE));
                assert_eq!(d.wd_atmosphere(), mark(tags::STAR_WD_ATMOSPHERE));
                assert_eq!(d.wd_carbon(), mark(tags::STAR_WD_CARBON));
                assert_eq!(d.wd_metals(), mark(tags::STAR_WD_METALS));
                assert_same_bits(d.ns_field().value(), normal(tags::STAR_NS_FIELD));
                assert_same_bits(d.ns_phase().value(), open(tags::STAR_NS_PHASE));
                assert_same_bits(d.bh_spin().value(), normal(tags::STAR_BH_SPIN));
                assert_same_bits(d.nebula().value(), open(tags::STAR_NEBULA));
                for (tag, tries) in [
                    (tags::STAR_KICK_SCORE, d.kick_score()),
                    (tags::STAR_NS_SPIN, d.ns_spin()),
                ] {
                    let mut s = stream(seed, tag, star, attempt);
                    for t in tries {
                        assert_same_bits(t.value(), s.standard_normal());
                    }
                }
                let mut low = stream(seed, tags::STAR_KICK_LOW, star, attempt);
                for c in d.kick_low() {
                    assert_same_bits(c.value(), low.standard_normal());
                }
                for (tag, direction) in [
                    (tags::STAR_SPIN_AXIS, d.spin_axis()),
                    (tags::STAR_KICK_DIRECTION, d.kick_direction()),
                ] {
                    assert_eq!(direction, isotropic(&mut stream(seed, tag, star, attempt)));
                }
                let mut geometry = stream(seed, tags::STAR_NS_GEOMETRY, star, attempt);
                assert_eq!(d.ns_spin_axis(), isotropic(&mut geometry));
                assert_same_bits(d.ns_inclination().value(), geometry.uniform_open());
            }
        }
    }

    #[test]
    fn attempt_zero_is_the_star_and_later_attempts_differ() {
        let seed = Seed::new(3);
        for star in bodies() {
            let first = StarDraws::for_star(seed, star);
            assert_eq!(first, StarDraws::for_attempt(seed, star, 0));
            let second = StarDraws::for_attempt(seed, star, 1);
            assert_ne!(first.eta(), second.eta());
            assert_ne!(first.kick_direction(), second.kick_direction());
        }
    }

    #[test]
    fn a_companion_and_another_seed_draw_differently() {
        let all = bodies();
        let (primary, companion) = (all[2], all[3]);
        let seed = Seed::new(9);
        assert_ne!(
            StarDraws::for_star(seed, primary),
            StarDraws::for_star(seed, companion)
        );
        assert_ne!(
            StarDraws::for_star(seed, primary),
            StarDraws::for_star(Seed::new(10), primary)
        );
    }

    #[test]
    fn draws_do_not_depend_on_the_order_of_asking() {
        let seed = Seed::new(0xabc);
        let keys: Vec<(BodyId, u32)> = bodies()
            .into_iter()
            .flat_map(|b| [(b, 0), (b, 3)])
            .collect();
        assert_order_independent(&keys, |&(star, attempt)| {
            StarDraws::for_attempt(seed, star, attempt)
        });
    }

    #[test]
    fn the_same_seed_gives_the_same_draws_twice() {
        let seed = Seed::new(77);
        for star in bodies() {
            assert_eq!(
                StarDraws::for_star(seed, star),
                StarDraws::for_star(seed, star)
            );
        }
    }

    #[test]
    fn directions_are_unit_vectors_and_ranks_lie_inside_the_unit_interval() {
        let seed = Seed::new(1);
        for star in bodies() {
            for attempt in 0..50 {
                let d = StarDraws::for_attempt(seed, star, attempt);
                for v in [d.spin_axis(), d.kick_direction(), d.ns_spin_axis()] {
                    let [x, y, z] = v.components();
                    let square = x * x + y * y + z * z;
                    assert!(
                        (square - 1.0).abs() < 1e-15,
                        "{star} attempt {attempt}: |v|² = {square}"
                    );
                }
                for u in [
                    d.rotation(),
                    d.disc_lifetime(),
                    d.ns_inclination(),
                    d.ns_phase(),
                    d.nebula(),
                ] {
                    assert!(
                        u.value() > 0.0 && u.value() < 1.0,
                        "{star} attempt {attempt}: rank {}",
                        u.value()
                    );
                }
            }
        }
    }

    /// Isotropy is z uniform on (−1, 1) and the azimuth uniform on [0, 2π), independently of
    /// each other (Archimedes' hat-box theorem); drawing the polar angle uniformly instead would
    /// fail the first test.
    #[test]
    fn isotropic_directions_are_uniform_in_z_and_azimuth() {
        let seed = Seed::new(2);
        let star = bodies()[0];
        let (mut zs, mut azimuths): (Vec<f64>, Vec<f64>) = (0..20_000)
            .map(|attempt| {
                let [x, y, z] = StarDraws::for_attempt(seed, star, attempt)
                    .kick_direction()
                    .components();
                (z, math::atan2(y, x).rem_euclid(TAU))
            })
            .unzip();
        let z = stats::ks_one_sample(&mut zs, |z| z.midpoint(1.0).clamp(0.0, 1.0));
        stats::assert_p_value("z", z.p_value, stats::ALPHA);
        let azimuth = stats::ks_one_sample(&mut azimuths, |a| (a / TAU).clamp(0.0, 1.0));
        stats::assert_p_value("azimuth", azimuth.p_value, stats::ALPHA);
    }

    #[test]
    fn the_median_star_is_the_middle_of_every_draw() {
        let median = StarDraws::median();
        assert_same_bits(median.eta().value(), 0.0);
        assert_same_bits(median.rotation().value(), 0.5);
        assert_eq!(median.spin_axis(), UnitVector::NORTH);
        // Every mark is 2⁵², below a threshold of probability p exactly when p > ½: not at ½, and
        // already at the next double above it, ½ + 2⁻⁵³.
        let half = crate::rng::Threshold::from_probability(0.5);
        let above_half = crate::rng::Threshold::from_probability(0.5_f64.next_up());
        for mark in [
            median.magnetism(),
            median.stripped(),
            median.remnant_type(),
            median.remnant_fallback(),
            median.kick_mode(),
            median.wd_atmosphere(),
            median.wd_carbon(),
            median.wd_metals(),
        ] {
            assert_eq!(mark.get(), 1 << 52);
            assert!(!mark.is_below(half));
            assert!(mark.is_below(above_half));
        }
        for rank in [
            median.rotation(),
            median.disc_lifetime(),
            median.ns_inclination(),
            median.ns_phase(),
            median.nebula(),
        ] {
            assert_same_bits(rank.value(), 0.5);
        }
        for normal in [
            median.eta(),
            median.remnant_mass(),
            median.ns_field(),
            median.bh_spin(),
        ]
        .iter()
        .chain(median.kick_score())
        .chain(median.kick_low())
        .chain(median.ns_spin())
        {
            assert_same_bits(normal.value(), 0.0);
        }
        for direction in [
            median.spin_axis(),
            median.kick_direction(),
            median.ns_spin_axis(),
        ] {
            assert_eq!(direction, UnitVector::NORTH);
        }
        assert_eq!(median, StarDraws::from_parts(StarDrawsParts::MEDIAN));
        let tweaked = StarDraws::from_parts(StarDrawsParts {
            eta: StandardNormal::new(1.0).unwrap(),
            ..StarDrawsParts::MEDIAN
        });
        assert_same_bits(tweaked.eta().value(), 1.0);
        assert_eq!(tweaked.nebula(), median.nebula());
    }

    #[test]
    fn typed_variates_reject_values_outside_their_range() {
        assert_eq!(UnitUniform::new(0.0), None);
        assert_eq!(UnitUniform::new(1.0), None);
        assert_eq!(UnitUniform::new(f64::NAN), None);
        assert_same_bits(UnitUniform::new(0.25).unwrap().value(), 0.25);
        assert_eq!(StandardNormal::new(f64::INFINITY), None);
        assert_eq!(StandardNormal::new(f64::NAN), None);
        assert_same_bits(StandardNormal::new(-2.0).unwrap().value(), -2.0);
    }
}
