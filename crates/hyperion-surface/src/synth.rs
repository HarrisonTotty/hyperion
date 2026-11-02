//! The local synthesis: a body's height between its coarse cells, band-limited level by level
//! (plan R09, Design notes 13 and 14).
//!
//! [`Synthesiser`] is the height function over a coarse field, on the server's whole field and a
//! client's part of one alike, through [`FieldView`]. A height at a band level is the sum, in a
//! fixed order, of the base elevation, the coarse cells interpolated over the sphere ([`interp`],
//! R09.T4), and the bands below the coarse cell that the level evaluates ([`levels`], R09.T8): the
//! structural octaves ([`relief`], R09.T5), the channels' incision ([`channels`], R09.T6), the
//! small craters ([`craters`], R09.T7.b) and the coarse craters' rims, sharpened from the coarse
//! band ([`craters::rims`], R09.T7.c). [`Synthesiser::bake_patch`] makes a patch's heights,
//! parent-band heights and gradients from the same function ([`bake`]), and [`levels`] bounds how
//! much a level's heights differ from any finer level's ([`level_bound_m`]) and how far they can
//! reach ([`Synthesiser::height_range_m`]).
//!
//! The module also holds the two types a coarse field's header carries: [`BandLevel`], the quadtree
//! level a height is asked at, and [`BandSpectrum`], the law of the structural relief finer than the
//! coarse cells, from which the relief derives each level's amplitude and the closed-form variance
//! below a cell ([`local_variance`]), with the crust classes' shapes and anchors that make a body's
//! law ([`SpectrumShape`], [`BandSpectrum::anchored`]); and the closed forms of the ground finer
//! than a wavelength, per cell and per contribution ([`unresolved_variance`],
//! [`structure_function`], [`unresolved_rms`]), with the level-keyed ones R10 reads, of what a band
//! level leaves out of the finest level's ground ([`Synthesiser::omitted_variance`],
//! [`Synthesiser::omitted_structure_function`], [`Synthesiser::omitted_rms`]).

pub mod bake;
pub mod channels;
pub mod craters;
pub mod interp;
pub mod levels;
pub mod relief;

use std::borrow::Cow;

use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Metres, SquareMetres};

use crate::cube::{MAX_LEVEL, PatchKey};
use crate::field::{BodyRef, FieldHeader, FieldView, SynthesisCell};
use crate::height::HeightSample;
use crate::noise::LatticeCache;
use crate::num;
pub use bake::PatchHeights;
pub use channels::Channels;
pub use craters::SmallCraters;
use craters::rims::CoarseRims;
use craters::{CraterCache, CraterOctaves};
use interp::ReadCellError;
pub use levels::{LevelBounds, LevelSet, level_bound_m};
pub use relief::{Relief, local_variance};

/// The number of band levels, 0 to [`MAX_LEVEL`]: 25.
pub(crate) const BAND_LEVELS: usize = MAX_LEVEL as usize + 1;

/// The height function over a coarse field: what the server's collision and the client's patches
/// read (Design note 13).
///
/// It is a pure function of the field the view holds, the body's detail seed, the direction and
/// the level: the same on the server and in the client's WebAssembly workers, bit for bit, and in
/// any order of queries, any cache the caller passes being the caller's. It borrows the view and
/// holds the body's contributions, prepared once from the field's header and the seed: the
/// structural octaves, the channels, the small craters' octaves, the coarse craters' rims and each
/// level's set and bounds.
/// Building one takes about 1 to 10 ms (the craters' octaves and the bounds' sampled profiles,
/// most on a screened body), so a caller builds one per body and view, not per query; a client's
/// height worker, whose view grows block by block, keeps the part made from the header and the
/// seed for the life of the body's field instead.
#[derive(Debug)]
pub struct Synthesiser<'a, F> {
    field: &'a F,
    parts: Cow<'a, SynthesisParts>,
}

// By hand, since a derive would bound it on `F: Clone`, which no view need be.
impl<F> Clone for Synthesiser<'_, F> {
    fn clone(&self) -> Self {
        Self {
            field: self.field,
            parts: self.parts.clone(),
        }
    }
}

/// What a [`Synthesiser`] holds besides its view: the body's contributions, every one a function
/// of the field's header and the body's detail seed alone, which no block of a field changes.
///
/// A client's height worker keeps a body's for the life of its field, so that each query's
/// synthesiser over the field as it then is ([`Synthesiser::with_parts`]) skips their build: 17.6
/// ms of every bake on `bake/patch_65`'s Earth-like world, 9.7 ms on its Moon-like world (the
/// bounds' crater marks four fifths of it; natively, one thread, pod, 2026-11-01).
#[derive(Debug, Clone)]
pub(crate) struct SynthesisParts {
    /// The body of the header they were made from, which [`Synthesiser::with_parts`] checks.
    body: BodyRef,
    seed: DetailSeed,
    relief: Relief,
    channels: Channels,
    craters: SmallCraters,
    rims: CoarseRims,
    bounds: LevelBounds,
}

impl SynthesisParts {
    /// The parts of a synthesiser over a field of `header` with the body's detail seed `seed`.
    ///
    /// # Panics
    ///
    /// As [`CraterOctaves::new`], which no header a field holds reaches.
    #[must_use]
    pub(crate) fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        let craters = SmallCraters::new(header, seed);
        let bounds = LevelBounds::of(header, craters.octaves());
        Self {
            body: header.body(),
            seed,
            relief: Relief::new(header, seed),
            channels: Channels::new(header, seed),
            craters,
            rims: CoarseRims::new(header),
            bounds,
        }
    }

    /// The body's detail seed they were made with.
    #[must_use]
    pub(crate) const fn seed(&self) -> DetailSeed {
        self.seed
    }
}

/// One height's contributions at a point and level, each with its gradient, in the order a height
/// sums them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Contributions {
    pub(crate) base: HeightSample,
    pub(crate) relief: HeightSample,
    pub(crate) channels: HeightSample,
    pub(crate) craters: HeightSample,
    pub(crate) rims: HeightSample,
}

/// A contribution of nothing.
const NOTHING: HeightSample = HeightSample {
    height_m: 0.0,
    gradient: [0.0; 3],
};

/// The points a batch of queries walks the structural octaves at together
/// ([`Synthesiser::heights_at`]): four. On a patch's double grid of Earth-like and Moon-like
/// points at L + 4, natively, the walk took 316 ns a point at four lanes, against 694 ns for the
/// point walk and 353 ns at both two and eight lanes (pod, 16 vCPU, 2026-11-01).
pub(crate) const LANES: usize = 4;

/// What a point query makes before its structural octaves
/// ([`Synthesiser::contributions`]): the base elevation, and the relief's last octave and style
/// where the level evaluates an octave.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BeforeRelief {
    base: HeightSample,
    relief: Option<(u8, relief::StyleSample)>,
}

/// What a pair's one pass makes before its structural octaves
/// ([`Synthesiser::contributions_pair`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum PairBeforeRelief {
    /// Both levels' octaves from one walk, the level's through `through` and the parent's through
    /// `up_through`, both from the one base and style.
    Shared {
        base: HeightSample,
        style: relief::StyleSample,
        through: u8,
        up_through: u8,
    },
    /// The level's octaves alone, through `through`; the parent's base the point query's there.
    Own {
        base: HeightSample,
        style: relief::StyleSample,
        through: u8,
        up_base: HeightSample,
    },
    /// Neither level's octaves: the one base elevation.
    Neither { base: HeightSample },
}

impl<'a, F: FieldView> Synthesiser<'a, F> {
    /// The synthesis over `field` with the body's detail seed `seed` (plan 14's
    /// `body.surface.detail`, R09.T1.b), from which the bands below the coarse cell draw.
    ///
    /// # Panics
    ///
    /// As [`CraterOctaves::new`], which no header a field holds reaches.
    #[must_use]
    pub fn new(field: &'a F, seed: DetailSeed) -> Self {
        Self {
            field,
            parts: Cow::Owned(SynthesisParts::new(field.header(), seed)),
        }
    }

    /// The synthesis over `field` from `parts`, which [`SynthesisParts::new`] made from `field`'s
    /// header, or from a header the field's own is the same as: [`new`](Self::new)'s with
    /// `parts`' seed, bit for bit, without their build.
    ///
    /// # Panics
    ///
    /// If `parts` were made for another body's field (a bug of the caller's).
    #[must_use]
    pub(crate) fn with_parts(field: &'a F, parts: &'a SynthesisParts) -> Self {
        assert_eq!(
            field.header().body(),
            parts.body,
            "a synthesiser's parts are its own field's body's"
        );
        Self {
            field,
            parts: Cow::Borrowed(parts),
        }
    }

    /// The field it reads.
    #[must_use]
    pub const fn field(&self) -> &'a F {
        self.field
    }

    /// The body's detail seed.
    #[must_use]
    pub fn seed(&self) -> DetailSeed {
        self.parts.seed
    }

    /// The body's structural octaves.
    #[must_use]
    pub fn relief(&self) -> &Relief {
        &self.parts.relief
    }

    /// The body's channels.
    #[must_use]
    pub fn channels(&self) -> &Channels {
        &self.parts.channels
    }

    /// The body's small craters.
    #[must_use]
    pub fn craters(&self) -> &SmallCraters {
        &self.parts.craters
    }

    /// The body's coarse craters' rims, which sharpen the field's listed craters.
    #[must_use]
    pub fn rims(&self) -> &CoarseRims {
        &self.parts.rims
    }

    /// The body's level sets and bounds.
    #[must_use]
    pub fn bounds(&self) -> &LevelBounds {
        &self.parts.bounds
    }

    /// The contributions `level` evaluates ([`LevelBounds::set`]).
    #[must_use]
    pub fn level_set(&self, level: BandLevel) -> &LevelSet {
        self.parts.bounds.set(level)
    }

    /// The level bound at `level`, metres ([`level_bound_m`], from the bounds it holds).
    #[must_use]
    pub fn level_bound_m(&self, level: BandLevel) -> f64 {
        self.parts.bounds.bound_m(level)
    }

    /// The height at the unit direction `dir` (body-fixed) at `level`, above the field's spheroid
    /// along its normal, metres, and its gradient in body-fixed space at the spheroid point, metres
    /// per metre.
    ///
    /// It is the base elevation ([`interp::base_elevation`]) plus the level's contributions
    /// ([`level_set`](Self::level_set)): the structural octaves ([`Relief::at`]), the channels'
    /// incision ([`Channels::incision_at`]), the small craters ([`SmallCraters::at`]) and the
    /// coarse craters' rims ([`CoarseRims::at`], at every level), summed in that order, height and
    /// gradient alike, and the sum asserted finite. `cache` is the caller's, and no result depends
    /// on what it holds.
    ///
    /// The channels take no point of a dry coarse cell below its floor, except on a world dry now
    /// (R09.T6.f, R09.T6.g; decision-r09-channel-term §4(b) and §9.4): with h₀ the height without
    /// their incision c and f the floor, the height is max(h₀ + c, min(h₀, f)), the floor where
    /// they would cut through it, and h₀ where the ground lies below it without them. The floor
    /// is the sea level, flat, and towards a playa's pit above the sea the pit's ground
    /// ([`Channels`]' floor, bilinear between the cell's corners). It is continuous and moves no
    /// height towards anything but h₀, so the level bound holds; it binds only where the sum lies
    /// below it and c is negative, and every other height is the sum to the bit.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold:
    /// the base elevation's reads come first, then the octaves', then the channels'.
    ///
    /// # Panics
    ///
    /// If `dir` is zero or has a component that is not finite, or a height is not finite (a bug).
    pub fn height_at(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<HeightSample, QueryHeightError> {
        let parts = self.contributions(cache, dir, level)?;
        self.finish(cache, dir, &parts)
    }

    /// [`height_at`](Self::height_at)'s height alone, metres, bit for bit, its error included:
    /// each contribution's height made by the same operations, from the same reads in the same
    /// order, and no gradient made (the horizon map's far field reads only heights). Every
    /// contribution's gradient arithmetic is left out: the structural octaves', the noise's, the
    /// interpolant's, the channels', the small craters' and the coarse rims'.
    ///
    /// # Errors
    ///
    /// As [`height_at`](Self::height_at).
    ///
    /// # Panics
    ///
    /// As [`height_at`](Self::height_at).
    pub(crate) fn height_m_at(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<f64, QueryHeightError> {
        let header = self.field.header();
        let (base, relief) = match self.base_m_before_relief(cache, dir, level)? {
            (base, Some((through, style))) => {
                let relief = self.parts.relief.height_m_in_style(
                    header,
                    dir,
                    through,
                    &style,
                    cache.lattice_mut(),
                );
                (base, relief)
            }
            (base, None) => (base, 0.0),
        };
        // The small craters read no cell, so they are made before the channels' reads.
        let craters = match self.parts.bounds.set(level).craters_through() {
            Some(through) => {
                self.parts
                    .craters
                    .height_m_at(dir, level, through, &mut cache.craters)
            }
            None => 0.0,
        };
        self.height_m_after_relief(cache, dir, level, (base, relief, craters))
    }

    /// [`height_m_at`](Self::height_m_at)'s first stage: the base elevation, and the relief's last
    /// octave and style where the level evaluates an octave.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell the base elevation reads that the
    /// view does not hold.
    fn base_m_before_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<(f64, Option<(u8, relief::StyleSample)>), QueryHeightError> {
        let field = self.field;
        Ok(match self.parts.bounds.set(level).relief_through() {
            Some(through) if self.parts.relief.evaluates(through) => {
                let (base, style) = match cache.controls.memo_for(self.parts.channels.id()) {
                    Some(memo) => relief::base_m_and_style(field, dir, memo)?,
                    None => relief::base_m_and_style(field, dir, &mut interp::Fresh)?,
                };
                (base, Some((through, style)))
            }
            Some(_) | None => (interp::base_elevation_m(field, dir)?, None),
        })
    }

    /// [`height_m_at`](Self::height_m_at)'s last stages, from the base elevation's, the relief's
    /// and the small craters' heights at `dir`, metres: the channels' and the coarse rims' heights,
    /// the sum in the point query's order and the channels' floor.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell the channels, the rims or the floor
    /// read that the view does not hold, in that order.
    fn height_m_after_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        (base_m, relief_m, craters_m): (f64, f64, f64),
    ) -> Result<f64, QueryHeightError> {
        let field = self.field;
        let header = field.header();
        let channels = match self.parts.bounds.set(level).channels_through() {
            Some(through) => self
                .parts
                .channels
                .incision_height_m_at(field, dir, through, cache)?,
            None => 0.0,
        };
        let rims = self.parts.rims.height_m_at(field, dir, level)?;
        // `finish`'s sum and the channels' floor, on the heights alone.
        let mut height = base_m;
        for part in [relief_m, channels, craters_m, rims] {
            height += part;
        }
        let sea = header.sea_level().value();
        if height < sea && channels < 0.0 {
            let (_, record) = self.cell_at(dir)?;
            if !record.is_under_water() && !record.epoch_incision.is_epoch() {
                let mut without = base_m;
                for part in [relief_m, craters_m, rims] {
                    without += part;
                }
                height = if without >= sea { sea } else { without };
            }
        }
        Ok(num::assert_finite(height))
    }

    /// [`height_at`](Self::height_at) at `level` and at `parent`, a coarser level, from one pass
    /// where the two share it: each the point query's answer at its level, bit for bit, its error
    /// included.
    ///
    /// The base elevation and the relief's styles are the same at every level, and the structural
    /// octaves and the channels' levels are summed coarsest first, each term reading only coarser
    /// ones, so `parent`'s are a prefix of `level`'s and are kept as the sum passes them; the small
    /// craters and the coarse craters' rims are band-limited at the level asked, and are made for
    /// each. Where a read is refused, or the two levels' sets do not nest, each is asked on its
    /// own. A bake asks it at each even vertex, whose morph target is its parent level's height
    /// (R10.T6.a's lever, measured at `bake/patch_65`), several at a time
    /// ([`height_pairs_at`](Self::height_pairs_at)), for which this point query is the tests' twin.
    ///
    /// # Panics
    ///
    /// As [`height_at`](Self::height_at).
    #[cfg(test)]
    pub(crate) fn height_pair_at(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        parent: BandLevel,
    ) -> (
        Result<HeightSample, QueryHeightError>,
        Result<HeightSample, QueryHeightError>,
    ) {
        match self.shared_pair(cache, dir, level, parent) {
            Some((own, up)) => (Ok(own), Ok(up)),
            None => (
                self.height_at(cache, dir, level),
                self.height_at(cache, dir, parent),
            ),
        }
    }

    /// [`height_pair_at`](Self::height_pair_at)'s one pass, or `None` where a read is refused or
    /// the levels' sets do not nest.
    #[cfg(test)]
    fn shared_pair(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        parent: BandLevel,
    ) -> Option<(HeightSample, HeightSample)> {
        // A refusal is dropped here, safely: each level is then asked on its own, whose answer
        // carries its own error, the first its reads meet, as the point query orders them.
        let (own, up) = self.contributions_pair(cache, dir, level, parent).ok()??;
        Some((
            self.finish(cache, dir, &own).ok()?,
            self.finish(cache, dir, &up).ok()?,
        ))
    }

    /// The height and gradient of `parts`, summed in their order with the channels' floor at
    /// `dir` ([`height_at`](Self::height_at)).
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell the floor reads that the view does
    /// not hold.
    fn finish(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        parts: &Contributions,
    ) -> Result<HeightSample, QueryHeightError> {
        let mut sample = parts.base;
        for part in [parts.relief, parts.channels, parts.craters, parts.rims] {
            sample.height_m += part.height_m;
            for (g, d) in sample.gradient.iter_mut().zip(part.gradient) {
                *g += d;
            }
        }
        // The channels take no dry point below the floor (R09.T6.f; decision-r09-channel-term
        // §4(b)'s per-point floor, and §9.4's at a playa's pit, R09.T6.g): h = max(h₀ + c,
        // min(h₀, f)) with h₀ the height without them and f the floor, the sea level or towards a
        // pit its ground. It binds only where the sum lies below the floor and the channels cut, so
        // that every other point keeps its sum's bits.
        if parts.channels.height_m < 0.0 {
            let (coarse, record) = self.cell_at(dir)?;
            if !record.is_under_water() && !record.epoch_incision.is_epoch() {
                let floor = self
                    .parts
                    .channels
                    .floor_at(self.field, coarse, dir, cache)?;
                let floor_m =
                    floor.map_or_else(|| self.field.header().sea_level().value(), |f| f.height_m);
                if sample.height_m < floor_m {
                    // h₀, the other contributions summed in their order.
                    let mut without = parts.base;
                    for part in [parts.relief, parts.craters, parts.rims] {
                        without.height_m += part.height_m;
                        for (g, d) in without.gradient.iter_mut().zip(part.gradient) {
                            *g += d;
                        }
                    }
                    sample = if without.height_m >= floor_m {
                        // On the floor: flat at the sea, or the pit's floor.
                        floor.unwrap_or(HeightSample {
                            height_m: floor_m,
                            gradient: [0.0; 3],
                        })
                    } else {
                        // Below the floor without the channels: they cut nothing there.
                        without
                    };
                }
            }
        }
        Ok(HeightSample {
            height_m: num::assert_finite(sample.height_m),
            gradient: sample.gradient.map(num::assert_finite),
        })
    }

    /// Each of `level`'s contributions at `dir`, the base elevation first (see
    /// [`height_at`](Self::height_at)).
    pub(crate) fn contributions(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<Contributions, QueryHeightError> {
        let before = self.before_relief(cache, dir, level)?;
        let relief = match &before.relief {
            Some((through, style)) => self.parts.relief.at_in_style(
                self.field.header(),
                dir,
                *through,
                style,
                cache.lattice_mut(),
            ),
            None => NOTHING,
        };
        self.after_relief(cache, dir, level, before.base, relief)
    }

    /// [`contributions`](Self::contributions)' first stage: the base elevation, and the relief's
    /// style and last octave where the level evaluates an octave.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell the base elevation reads that the
    /// view does not hold.
    fn before_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<BeforeRelief, QueryHeightError> {
        let field = self.field;
        let set = self.parts.bounds.set(level);
        // The base elevation and the relief's styles in one fused pass where the relief reads its
        // styles, each value's bits those of its own interpolation, through a bake's memo of their
        // control points from its cover on (R09.T8's follow-up; the measurement is at
        // `interp::ControlMemo`).
        match set.relief_through() {
            Some(through) if self.parts.relief.evaluates(through) => {
                let (base, style) = match cache.controls.memo_for(self.parts.channels.id()) {
                    Some(memo) => relief::base_and_style(field, dir, memo)?,
                    None => relief::base_and_style(field, dir, &mut interp::Fresh)?,
                };
                Ok(BeforeRelief {
                    base,
                    relief: Some((through, style)),
                })
            }
            Some(_) | None => Ok(BeforeRelief {
                base: interp::base_elevation(field, dir)?,
                relief: None,
            }),
        }
    }

    /// [`contributions`](Self::contributions)' last stage: the channels, the small craters and the
    /// coarse rims at `dir`, after the base elevation `base` and the structural octaves' `relief`.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell the channels or the rims read that
    /// the view does not hold.
    fn after_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        base: HeightSample,
        relief: HeightSample,
    ) -> Result<Contributions, QueryHeightError> {
        let craters = self.craters_at(cache, dir, level);
        self.after_craters(cache, dir, level, (base, relief, craters))
    }

    /// The small craters at `dir` at `level`, nothing where the level evaluates none.
    fn craters_at(&self, cache: &mut SynthCache, dir: [f64; 3], level: BandLevel) -> HeightSample {
        match self.parts.bounds.set(level).craters_through() {
            Some(through) => self
                .parts
                .craters
                .at(dir, level, through, &mut cache.craters),
            None => NOTHING,
        }
    }

    /// [`craters_at`](Self::craters_at) at each of `dirs`, each lane's bit for bit, the lanes'
    /// searches shared ([`SmallCraters::at_lanes`]).
    fn craters_lanes<const N: usize>(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]; N],
        level: BandLevel,
    ) -> [HeightSample; N] {
        match self.parts.bounds.set(level).craters_through() {
            Some(through) => self
                .parts
                .craters
                .at_lanes(dirs, level, through, &mut cache.craters),
            None => [NOTHING; N],
        }
    }

    /// [`after_relief`](Self::after_relief) with its small craters made: the channels and the
    /// coarse rims at `dir`, after the base elevation, the structural octaves and the small
    /// craters. The craters read no cell and refuse nothing, so that making them first moves no
    /// error.
    ///
    /// # Errors
    ///
    /// As [`after_relief`](Self::after_relief).
    fn after_craters(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        (base, relief, craters): (HeightSample, HeightSample, HeightSample),
    ) -> Result<Contributions, QueryHeightError> {
        let field = self.field;
        let set = self.parts.bounds.set(level);
        let channels = match set.channels_through() {
            Some(through) => self
                .parts
                .channels
                .incision_at(field, dir, through, cache)?,
            None => NOTHING,
        };
        let rims = self.parts.rims.at(field, dir, level)?;
        Ok(Contributions {
            base,
            relief,
            channels,
            craters,
            rims,
        })
    }

    /// [`contributions`](Self::contributions) at `level` and at `parent` in one pass
    /// ([`height_pair_at`](Self::height_pair_at)), or `None` where the levels' sets do not nest:
    /// `parent`'s octaves or channels evaluated where `level`'s are not, or through a finer octave
    /// or level than `level`'s.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming a cell read that the view does not hold.
    #[cfg(test)]
    fn contributions_pair(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        parent: BandLevel,
    ) -> Result<Option<(Contributions, Contributions)>, QueryHeightError> {
        let Some(before) = self.pair_before_relief(cache, dir, level, parent)? else {
            return Ok(None);
        };
        let header = self.field.header();
        let relief = &self.parts.relief;
        let (own, up) = match before {
            PairBeforeRelief::Shared {
                base,
                style,
                through,
                up_through,
            } => {
                let lattice = cache.lattice_mut();
                let (own, up) =
                    relief.at_in_style_pair(header, dir, through, up_through, &style, lattice);
                ((base, own), (base, up))
            }
            PairBeforeRelief::Own {
                base,
                style,
                through,
                up_base,
            } => {
                let own = relief.at_in_style(header, dir, through, &style, cache.lattice_mut());
                ((base, own), (up_base, NOTHING))
            }
            PairBeforeRelief::Neither { base } => ((base, NOTHING), (base, NOTHING)),
        };
        let craters = (
            self.craters_at(cache, dir, level),
            self.craters_at(cache, dir, parent),
        );
        self.pair_after_relief(cache, dir, (level, parent), own, up, craters)
    }

    /// [`contributions_pair`](Self::contributions_pair)'s first stage: the base elevations and the
    /// relief's style and last octaves at both levels, or `None` where the levels' octaves do not
    /// nest. The parent's base where only the level's octaves are evaluated is the point query's
    /// there, alone.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming a cell the base elevation reads that the view does
    /// not hold.
    fn pair_before_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
        parent: BandLevel,
    ) -> Result<Option<PairBeforeRelief>, QueryHeightError> {
        let field = self.field;
        let (set, up) = (self.parts.bounds.set(level), self.parts.bounds.set(parent));
        let evaluated = |through: Option<u8>| through.filter(|&t| self.parts.relief.evaluates(t));
        match (
            evaluated(set.relief_through()),
            evaluated(up.relief_through()),
        ) {
            (Some(through), up_through) => {
                let (base, style) = match cache.controls.memo_for(self.parts.channels.id()) {
                    Some(memo) => relief::base_and_style(field, dir, memo)?,
                    None => relief::base_and_style(field, dir, &mut interp::Fresh)?,
                };
                match up_through {
                    Some(up_through) if up_through <= through => {
                        Ok(Some(PairBeforeRelief::Shared {
                            base,
                            style,
                            through,
                            up_through,
                        }))
                    }
                    Some(_) => Ok(None),
                    // The parent's base as the point query takes it there, alone.
                    None => Ok(Some(PairBeforeRelief::Own {
                        base,
                        style,
                        through,
                        up_base: interp::base_elevation(field, dir)?,
                    })),
                }
            }
            (None, Some(_)) => Ok(None),
            (None, None) => Ok(Some(PairBeforeRelief::Neither {
                base: interp::base_elevation(field, dir)?,
            })),
        }
    }

    /// [`contributions_pair`](Self::contributions_pair)'s last stage: the channels and the coarse
    /// rims at both levels, after each level's base elevation and structural octaves, `own` and
    /// `up`, and its small craters, `craters`, which refuse nothing, so that making them first
    /// moves no error; `None` where the levels' channels do not nest.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming a cell the channels or the rims read that the view
    /// does not hold.
    fn pair_after_relief(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        (level, parent): (BandLevel, BandLevel),
        (base, relief): (HeightSample, HeightSample),
        (up_base, up_relief): (HeightSample, HeightSample),
        (craters, up_craters): (HeightSample, HeightSample),
    ) -> Result<Option<(Contributions, Contributions)>, QueryHeightError> {
        let field = self.field;
        let (set, up) = (self.parts.bounds.set(level), self.parts.bounds.set(parent));
        let (channels, up_channels) = match (set.channels_through(), up.channels_through()) {
            (Some(through), Some(up_through)) if up_through <= through => self
                .parts
                .channels
                .incision_pair_at(field, dir, through, up_through, cache)?,
            (Some(through), None) => (
                self.parts
                    .channels
                    .incision_at(field, dir, through, cache)?,
                NOTHING,
            ),
            (None, None) => (NOTHING, NOTHING),
            (Some(_) | None, Some(_)) => return Ok(None),
        };
        let rims = self.parts.rims.at(field, dir, level)?;
        let up_rims = self.parts.rims.at(field, dir, parent)?;
        Ok(Some((
            Contributions {
                base,
                relief,
                channels,
                craters,
                rims,
            },
            Contributions {
                base: up_base,
                relief: up_relief,
                channels: up_channels,
                craters: up_craters,
                rims: up_rims,
            },
        )))
    }

    /// [`height_at`](Self::height_at) at each of `dirs` at `level`, in their order, each the point
    /// query's answer bit for bit, its error included: the structural octaves and the small
    /// craters made [`LANES`] points at a time ([`Relief::at_in_style_lanes`],
    /// [`SmallCraters::at_lanes`]), every other stage the point query's own, point by point.
    ///
    /// Every answer is made, an error's followers too: a caller that stops at the first error
    /// reads the point queries' first.
    ///
    /// # Panics
    ///
    /// As [`height_at`](Self::height_at).
    pub(crate) fn heights_at(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]],
        level: BandLevel,
    ) -> Vec<Result<HeightSample, QueryHeightError>> {
        let mut out = Vec::with_capacity(dirs.len());
        let chunks = dirs.as_chunks::<LANES>();
        for lanes in chunks.0 {
            self.heights_lanes(cache, lanes, level, &mut out);
        }
        for dir in chunks.1 {
            self.heights_lanes(cache, &[*dir], level, &mut out);
        }
        out
    }

    /// [`heights_at`](Self::heights_at) at `N` points, appended to `out`.
    fn heights_lanes<const N: usize>(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]; N],
        level: BandLevel,
        out: &mut Vec<Result<HeightSample, QueryHeightError>>,
    ) {
        let before = dirs.map(|dir| self.before_relief(cache, dir, level));
        let asks = before.each_ref().map(|b| {
            b.as_ref()
                .ok()
                .and_then(|b| b.relief.map(|(through, style)| (through, None, style)))
        });
        let reliefs = self.relief_lanes(cache, dirs, &asks);
        let craters = self.craters_lanes(cache, dirs, level);
        for (((dir, before), relief), craters) in dirs.iter().zip(before).zip(reliefs).zip(craters)
        {
            out.push(before.and_then(|before| {
                let relief = relief.map_or(NOTHING, |(own, _)| own);
                let parts =
                    self.after_craters(cache, *dir, level, (before.base, relief, craters))?;
                self.finish(cache, *dir, &parts)
            }));
        }
    }

    /// [`height_m_at`](Self::height_m_at) at each of `dirs`, in their order, each the point
    /// query's height bit for bit, its error included: the structural octaves and the small
    /// craters made [`LANES`] points at a time without gradients, every other stage the point
    /// query's own, point by point, as [`heights_at`](Self::heights_at) makes them with theirs.
    /// Every answer is made, an error's followers too. On a cold high draft's near-field margin
    /// (the Earth-like world's L + 4 patch) its 1,752 points took 9.3k instructions a point fewer
    /// than `heights_at`'s (R10.T6.a's lever, callgrind, pod, 16 vCPU, 2026-11-02).
    ///
    /// # Panics
    ///
    /// As [`height_at`](Self::height_at).
    pub(crate) fn heights_m_at(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]],
        level: BandLevel,
    ) -> Vec<Result<f64, QueryHeightError>> {
        let mut out = Vec::with_capacity(dirs.len());
        let chunks = dirs.as_chunks::<LANES>();
        for lanes in chunks.0 {
            self.heights_m_lanes(cache, lanes, level, &mut out);
        }
        for dir in chunks.1 {
            self.heights_m_lanes(cache, &[*dir], level, &mut out);
        }
        out
    }

    /// [`heights_m_at`](Self::heights_m_at) at `N` points, appended to `out`.
    fn heights_m_lanes<const N: usize>(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]; N],
        level: BandLevel,
        out: &mut Vec<Result<f64, QueryHeightError>>,
    ) {
        let before = dirs.map(|dir| self.base_m_before_relief(cache, dir, level));
        let asks = before
            .each_ref()
            .map(|b| b.as_ref().ok().and_then(|&(_, relief)| relief));
        // One level, so every lane that asks the octaves asks the same; a lane that asks none, or
        // was refused, takes the first asking lane's place in the walk, its heights dropped.
        let reliefs = match asks.iter().find_map(|&ask| ask) {
            Some((through, first_style)) => {
                debug_assert!(
                    asks.iter().flatten().all(|&(t, _)| t == through),
                    "a level's points ask the same octaves"
                );
                let first = asks.iter().position(Option::is_some).expect("a lane asks");
                let lane_dirs: [[f64; 3]; N] =
                    std::array::from_fn(|l| dirs[if asks[l].is_some() { l } else { first }]);
                let styles: [relief::StyleSample; N] =
                    asks.map(|ask| ask.map_or(first_style, |(_, style)| style));
                let walked = self.parts.relief.height_m_in_style_lanes(
                    self.field.header(),
                    &lane_dirs,
                    through,
                    &styles,
                    cache.lattice_mut(),
                );
                std::array::from_fn(|l| if asks[l].is_some() { walked[l] } else { 0.0 })
            }
            None => [0.0; N],
        };
        let craters = match self.parts.bounds.set(level).craters_through() {
            Some(through) => {
                self.parts
                    .craters
                    .height_m_lanes(dirs, level, through, &mut cache.craters)
            }
            None => [0.0; N],
        };
        for (l, (dir, before)) in dirs.iter().zip(before).enumerate() {
            out.push(before.and_then(|(base, _)| {
                self.height_m_after_relief(cache, *dir, level, (base, reliefs[l], craters[l]))
            }));
        }
    }

    /// [`height_pair_at`](Self::height_pair_at) at each of `dirs` at `level` and `parent`, in
    /// their order, each pair the point query's bit for bit, as
    /// [`heights_at`](Self::heights_at) answers the point queries: the structural octaves and the
    /// small craters of both levels made [`LANES`] points at a time, every other stage the pair's
    /// own, point by point.
    ///
    /// # Panics
    ///
    /// As [`height_at`](Self::height_at).
    pub(crate) fn height_pairs_at(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]],
        level: BandLevel,
        parent: BandLevel,
    ) -> Vec<(
        Result<HeightSample, QueryHeightError>,
        Result<HeightSample, QueryHeightError>,
    )> {
        let mut out = Vec::with_capacity(dirs.len());
        let chunks = dirs.as_chunks::<LANES>();
        for lanes in chunks.0 {
            self.height_pairs_lanes(cache, lanes, level, parent, &mut out);
        }
        for dir in chunks.1 {
            self.height_pairs_lanes(cache, &[*dir], level, parent, &mut out);
        }
        out
    }

    /// [`height_pairs_at`](Self::height_pairs_at) at `N` points, appended to `out`: each lane
    /// whose one pass is refused anywhere, or whose levels do not nest, asked at each level on its
    /// own, as [`height_pair_at`](Self::height_pair_at) asks it.
    #[expect(
        clippy::type_complexity,
        reason = "the pairs of answers height_pair_at gives, in a list"
    )]
    fn height_pairs_lanes<const N: usize>(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]; N],
        level: BandLevel,
        parent: BandLevel,
        out: &mut Vec<(
            Result<HeightSample, QueryHeightError>,
            Result<HeightSample, QueryHeightError>,
        )>,
    ) {
        // A refusal is dropped here, safely, as `shared_pair` drops it: the lane is then asked at
        // each level on its own, whose answer carries its own error.
        let before = dirs.map(|dir| {
            self.pair_before_relief(cache, dir, level, parent)
                .ok()
                .flatten()
        });
        let asks = before.each_ref().map(|b| match b {
            Some(PairBeforeRelief::Shared {
                style,
                through,
                up_through,
                ..
            }) => Some((*through, Some(*up_through), *style)),
            Some(PairBeforeRelief::Own { style, through, .. }) => Some((*through, None, *style)),
            Some(PairBeforeRelief::Neither { .. }) | None => None,
        });
        let reliefs = self.relief_lanes(cache, dirs, &asks);
        let craters = self.craters_lanes(cache, dirs, level);
        let up_craters = self.craters_lanes(cache, dirs, parent);
        for ((((dir, before), relief), craters), up_craters) in dirs
            .iter()
            .zip(before)
            .zip(reliefs)
            .zip(craters)
            .zip(up_craters)
        {
            let shared = before.and_then(|before| {
                let (own, up) = match (before, relief) {
                    (PairBeforeRelief::Shared { base, .. }, Some((own, Some(up)))) => {
                        ((base, own), (base, up))
                    }
                    (PairBeforeRelief::Own { base, up_base, .. }, Some((own, None))) => {
                        ((base, own), (up_base, NOTHING))
                    }
                    (PairBeforeRelief::Neither { base }, None) => {
                        ((base, NOTHING), (base, NOTHING))
                    }
                    _ => unreachable!("a lane's octaves are walked as its first stage asked"),
                };
                let (own, up) = self
                    .pair_after_relief(cache, *dir, (level, parent), own, up, (craters, up_craters))
                    .ok()??;
                Some((
                    self.finish(cache, *dir, &own).ok()?,
                    self.finish(cache, *dir, &up).ok()?,
                ))
            });
            out.push(match shared {
                Some((own, up)) => (Ok(own), Ok(up)),
                None => (
                    self.height_at(cache, *dir, level),
                    self.height_at(cache, *dir, parent),
                ),
            });
        }
    }

    /// The structural octaves at each lane of `dirs` that `asks` asks them of, through its last
    /// octave and its parent's where it gives one, in its style: the lanes that ask the same
    /// octaves as the first that asks any walked together ([`Relief::at_in_style_lanes`]), the
    /// others the point walk's own, each lane's samples the point walk's bit for bit. A lane that
    /// asks none (`None`) is answered `None`.
    fn relief_lanes<const N: usize>(
        &self,
        cache: &mut SynthCache,
        dirs: &[[f64; 3]; N],
        asks: &[Option<(u8, Option<u8>, relief::StyleSample)>; N],
    ) -> [Option<(HeightSample, Option<HeightSample>)>; N] {
        let header = self.field.header();
        let relief = &self.parts.relief;
        let Some(first) = asks.iter().position(Option::is_some) else {
            return [None; N];
        };
        let (through, up_through, _) = asks[first].expect("the first lane that asks");
        let together = |l: usize| asks[l].is_some_and(|(t, u, _)| t == through && u == up_through);
        // A lane that walks alone or asks nothing takes the first's place in the walk, and its
        // samples there are dropped.
        let lane_dirs: [[f64; 3]; N] =
            std::array::from_fn(|l| dirs[if together(l) { l } else { first }]);
        let lane_styles: [relief::StyleSample; N] = std::array::from_fn(|l| {
            asks[if together(l) { l } else { first }]
                .expect("a walked lane asks")
                .2
        });
        let walked = relief.at_in_style_lanes(
            header,
            &lane_dirs,
            through,
            up_through,
            &lane_styles,
            cache.lattice_mut(),
        );
        std::array::from_fn(|l| match asks[l] {
            Some(_) if together(l) => Some(walked[l]),
            Some((through, Some(up_through), style)) => {
                let lattice = cache.lattice_mut();
                let (own, up) =
                    relief.at_in_style_pair(header, dirs[l], through, up_through, &style, lattice);
                Some((own, Some(up)))
            }
            Some((through, None, style)) => Some((
                relief.at_in_style(header, dirs[l], through, &style, cache.lattice_mut()),
                None,
            )),
            None => None,
        })
    }

    /// The heights, parent-band heights and gradients of `patch`'s 65 × 65 vertices, in R05's bake
    /// layout ([`PatchHeights`]): each vertex's [`height_at`](Self::height_at) at the patch's
    /// level, and at its parent's at the even vertices, bit for bit, the structural octaves'
    /// lattice covered once for the patch first.
    ///
    /// # Errors
    ///
    /// The first [`QueryHeightError`] of its queries, in row order, own levels first.
    ///
    /// # Panics
    ///
    /// If a height is not finite (a bug).
    pub fn bake_patch(
        &self,
        cache: &mut SynthCache,
        patch: PatchKey,
    ) -> Result<PatchHeights, QueryHeightError> {
        bake::bake(self, cache, patch)
    }

    /// The lowest and highest heights `patch`'s vertices can have at its level and its parent's,
    /// metres, for R05's culling: the base elevation's range over the held cells its points read,
    /// which the approximating spline never leaves, widened by each level's
    /// [`LevelBounds::contribution_range_m`]. Where the view holds none of those cells, any
    /// elevation a cell can encode.
    #[must_use]
    pub fn height_range_m(&self, patch: PatchKey) -> (f64, f64) {
        bake::height_range(self, patch)
    }

    /// The variance of `cell`'s ground in wavelengths shorter than `finer_than`, per contribution
    /// ([`unresolved_variance`], from the small craters' octaves it holds). What a band level
    /// leaves out is more than this at its `λ_n`: that is
    /// [`omitted_variance`](Self::omitted_variance).
    ///
    /// # Panics
    ///
    /// As [`unresolved_variance`].
    #[must_use]
    pub fn unresolved_variance(
        &self,
        cell: PatchKey,
        finer_than: Metres,
    ) -> Option<UnresolvedVariance> {
        unresolved_with(self.field, self.parts.craters.octaves(), cell, finer_than)
    }

    /// The structure function of `cell`'s ground in wavelengths shorter than `finer_than` at a
    /// lag of `lag` ([`structure_function`], from the small craters' octaves it holds). That of
    /// what a band level leaves out is
    /// [`omitted_structure_function`](Self::omitted_structure_function).
    ///
    /// # Panics
    ///
    /// As [`structure_function`].
    #[must_use]
    pub fn structure_function(
        &self,
        cell: PatchKey,
        finer_than: Metres,
        lag: Metres,
    ) -> Option<SquareMetres> {
        structure_with(
            self.field,
            self.parts.craters.octaves(),
            cell,
            finer_than,
            lag,
        )
    }

    /// The RMS of `cell`'s ground unresolved at `resolution` ([`unresolved_rms`], from the small
    /// craters' octaves it holds). That of what a band level leaves out is
    /// [`omitted_rms`](Self::omitted_rms).
    ///
    /// # Panics
    ///
    /// As [`unresolved_rms`].
    #[must_use]
    pub fn unresolved_rms(&self, cell: PatchKey, resolution: Metres) -> Option<Metres> {
        self.unresolved_variance(cell, resolution).map(rms_of)
    }

    /// What band level `level` leaves out of the finest level's ground in `cell`, per
    /// contribution: the level-keyed closed forms (added by R10.T5;
    /// `decision-r10-t5-closed-forms.md`), or `None` where the view does not hold the cell, the
    /// receiver of its trunk, or, where water drains through it, an edge neighbour: one king move
    /// away inside the margin, whose base level and trunks the channels' form reads (R09.T6.f,
    /// R09.T6.g).
    ///
    /// Each contribution's wavelength-keyed form ([`unresolved_variance`]) is taken from the cut
    /// at which it counts what the level leaves out, since a level evaluates each contribution by
    /// its own scale ([`levels`]): the structural octaves from the first degree of the first octave
    /// the level does not evaluate; the channels below the last evaluated level's mean cell width;
    /// and the coarse rims at the level's `λ_n`, the half of which is its fillet. The small
    /// craters count each octave the level keeps by its sharpening from the level's fillet to the
    /// band limit's, and each it leaves out whole at the band limit's fillet (R09.T7.h; the
    /// [`craters`] module's closed forms). It is zero from the band limit's level, and at a level
    /// that evaluates none of them it is the whole variance below the cell.
    ///
    /// # Panics
    ///
    /// If `cell` is not of the field's level.
    #[must_use]
    pub fn omitted_variance(&self, cell: PatchKey, level: BandLevel) -> Option<UnresolvedVariance> {
        let set = self.level_set(level);
        omitted_variance_in(
            self.field,
            set,
            self.parts.craters.octaves().omitted_variance(set),
            cell,
        )
    }

    /// The structure function at a lag of `lag` of what band level `level` leaves out of the
    /// finest level's ground in `cell`, square metres: the contributions' structure functions as
    /// [`omitted_variance`](Self::omitted_variance) takes them, at its cuts and the small craters'
    /// octaves kept and left out, summed in its order (added by R10.T5; R09.T7.h); `None` where
    /// the view does not hold what the channels' form reads (R09.T6.f, R09.T6.g; as
    /// [`omitted_variance`](Self::omitted_variance)). Each tends to twice
    /// its share of that variance at lags long against its wavelengths.
    ///
    /// # Panics
    ///
    /// If `lag` is not finite and non-negative, or `cell` is not of the field's level.
    #[must_use]
    pub fn omitted_structure_function(
        &self,
        cell: PatchKey,
        level: BandLevel,
        lag: Metres,
    ) -> Option<SquareMetres> {
        let field = self.field;
        let set = self.level_set(level);
        let cuts = levels::LevelCuts::of(field.header(), set);
        let relief = relief::relief_structure_from(field, cell, cuts.relief_degree, lag)?;
        let channels = channels::channel_structure_function(field, cell, cuts.channels, lag)?;
        let craters = self
            .parts
            .craters
            .octaves()
            .omitted_structure_function(set, lag);
        let rims = craters::rims::rims_structure_function(field, cell, cuts.rims, lag)?;
        Some(relief + channels + craters + rims)
    }

    /// The RMS of what band level `level` leaves out of the finest level's ground in `cell`: the
    /// square root of [`omitted_variance`](Self::omitted_variance)'s total, which R10's readings
    /// quote as the uncertainty of a height read at that level (its Design note 6; added by
    /// R10.T5); `None` where the view does not hold what the channels' form reads (R09.T6.f,
    /// R09.T6.g; as [`omitted_variance`](Self::omitted_variance)).
    ///
    /// # Panics
    ///
    /// If `cell` is not of the field's level.
    #[must_use]
    pub fn omitted_rms(&self, cell: PatchKey, level: BandLevel) -> Option<Metres> {
        self.omitted_variance(cell, level).map(rms_of)
    }

    /// The cell that contains the unit direction `dir`, and its record, from which the categorical
    /// fields (plate, crust, boundary kind, flow direction, surface class) are read
    /// ([`interp::cell_at`]).
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] if the view does not hold the cell.
    ///
    /// # Panics
    ///
    /// If `dir` is zero or has a component that is not finite.
    pub fn cell_at(
        &self,
        dir: [f64; 3],
    ) -> Result<(PatchKey, &'a SynthesisCell), QueryHeightError> {
        Ok(interp::cell_at(self.field, dir)?)
    }
}

/// What the band level of `set` leaves out of the finest level's ground in `cell` of `field`,
/// per contribution, with the small craters' part given: [`Synthesiser::omitted_variance`]'s
/// closed forms and sum, which it calls with `small_craters` its octaves'
/// [`CraterOctaves::omitted_variance`] at `set`; `None` where `field` does not hold the cell, the
/// receiver of its trunk or, where water drains through it, an edge neighbour (R09.T6.f,
/// R09.T6.g).
///
/// The small craters' part is the body's, the same in every cell and a sum over the octaves and
/// their sampled profiles, so a caller that asks of many cells at a level builds it once (R10.T4.b,
/// the client worker's body σ). Given that part, the answer is
/// [`Synthesiser::omitted_variance`]'s, bit for bit.
///
/// # Panics
///
/// If `cell` is not of the field's level.
#[must_use]
pub(crate) fn omitted_variance_in(
    field: &impl FieldView,
    set: &LevelSet,
    small_craters: SquareMetres,
    cell: PatchKey,
) -> Option<UnresolvedVariance> {
    let cuts = levels::LevelCuts::of(field.header(), set);
    let relief = relief::relief_variance_from(field, cell, cuts.relief_degree)?;
    let channels = channels::unresolved_channel_variance(field, cell, cuts.channels)?;
    Some(UnresolvedVariance {
        relief,
        channels,
        craters: small_craters,
        rims: craters::rims::unresolved_rims(field, cell, cuts.rims)?,
    })
}

/// The caller's cache for a [`Synthesiser`]'s queries, one per worker or per bake.
///
/// It holds R05's lattice cache, generalised to the body's detail seed (R09.T5), for the
/// structural octaves ([`relief`]): their octave table, built once per seed, and the lattice
/// corners a bake covers; the channels' cells (R09.T6.b), keyed by packed cell key
/// ([`PatchKey::to_u64`]): the network's key points and segments, each level's grooves by cell and
/// each level's mean and its correction by parent cell ([`Channels::incision_at`]), kept for one
/// body and seed at a time and started afresh for another's; and the small craters drawn in each
/// octave's cells ([`CraterCache`], R09.T7.b), which starts afresh at 16,384 cells; and, for
/// R10's classifier ([`crate::material::material_at`]), each cell's cut of the shadowed ground
/// its partial ice share lies on, kept as the channels' part is, which starts afresh at 16,384
/// cells; and, for R10's horizon maps ([`crate::bake::horizon`]), the far field's vertex heights by
/// face, level and lattice vertex, kept as the channels' part is, which starts afresh at 131,072
/// heights. No height, and no material, depends on what it holds, only the time a query takes, so
/// long as it is used with views of one field whose held cells only grow, as a worker's are: a
/// result it holds answers a later query that a view lacking one of the cells it read would
/// refuse.
///
/// The policy (R09.T8): the lattice cover is set afresh by each bake, and the craters' part is
/// bounded, but the channels' part grows with every cell a query builds and is never pruned, so a
/// caller that moves across a body drops the cache when it moves on, as a worker does between
/// descents; one cache serves a patch and its neighbours and children, which share their coarser
/// cells.
///
/// From a bake's cover on, it also keeps the base elevation's and the styles' control points that
/// queries read (R09.T8's follow-up, the fused pass): the cells' values about the patch, shared by
/// its vertices, in a bounded memo that each bake's cover empties, for the field the channels' part
/// is kept for alone (its body, level, figure and marks). Its queries after the bake read the memo
/// too, so the base elevation's own refusals rest on the rule above as the channels' do: a cell it
/// holds answers a later query that a view lacking the cell would refuse.
#[derive(Debug, Clone, Default)]
pub struct SynthCache {
    lattice: LatticeCache,
    channels: channels::ChannelCache,
    craters: CraterCache,
    ice_cuts: crate::material::IceCuts,
    controls: BakeControls,
    far_heights: crate::bake::horizon::FarHeights,
}

/// The fused pass's control points a [`SynthCache`] keeps from a bake's cover on, for one field.
#[derive(Debug, Clone, Default)]
struct BakeControls {
    /// The field whose bake last covered the cache, as the channels' part tells one field from
    /// another ([`channels::NetworkId`]); `None` before any, when queries read their control
    /// points afresh.
    field: Option<channels::NetworkId>,
    memo: interp::ControlMemo<{ relief::ELEVATION_AND_STYLE }>,
}

impl BakeControls {
    /// Empties the memo and keeps it for the field of `id`, from a bake's cover on.
    fn start(&mut self, id: channels::NetworkId) {
        self.field = Some(id);
        self.memo.clear();
    }

    /// The memo, if it is kept for the field of `id`.
    #[must_use]
    fn memo_for(
        &mut self,
        id: channels::NetworkId,
    ) -> Option<&mut interp::ControlMemo<{ relief::ELEVATION_AND_STYLE }>> {
        (self.field == Some(id)).then_some(&mut self.memo)
    }
}

impl SynthCache {
    /// An empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lattice: LatticeCache::new(),
            channels: channels::ChannelCache::new(),
            craters: CraterCache::new(),
            ice_cuts: crate::material::IceCuts::new(),
            controls: BakeControls {
                field: None,
                memo: interp::ControlMemo::new(),
            },
            far_heights: crate::bake::horizon::FarHeights::new(),
        }
    }

    /// Starts a bake of a patch of the field `id` names, as the synthesiser's [`Channels`] name
    /// it: its queries share their control points from here on ([`BakeControls`]), none kept from
    /// before.
    pub(crate) fn start_bake(&mut self, id: channels::NetworkId) {
        self.controls.start(id);
    }

    /// The structural octaves' lattice cache.
    #[must_use]
    pub(crate) fn lattice_mut(&mut self) -> &mut LatticeCache {
        &mut self.lattice
    }

    /// The channels' cache.
    #[must_use]
    pub(crate) fn channels_mut(&mut self) -> &mut channels::ChannelCache {
        &mut self.channels
    }

    /// The small craters' cache, which R10.T1.c's maturity at a point reads through
    /// ([`crate::material::maturity_at`]).
    #[must_use]
    pub(crate) fn craters_mut(&mut self) -> &mut CraterCache {
        &mut self.craters
    }

    /// R10's classifier's cuts of each cell's shadowed ground.
    #[must_use]
    pub(crate) fn ice_cuts_mut(&mut self) -> &mut crate::material::IceCuts {
        &mut self.ice_cuts
    }

    /// R10's horizon maps' far-field vertex heights.
    #[must_use]
    pub(crate) fn far_heights_mut(&mut self) -> &mut crate::bake::horizon::FarHeights {
        &mut self.far_heights
    }
}

/// The variance of a cell's ground in wavelengths shorter than a given one, per contribution:
/// what R10's material weights and slope readouts read (Design note 18).
///
/// Each is a closed form, of the cell's own fields or, for the small craters, of the body's header
/// alone: the structural octaves' (R09.T5), the channels' (R09.T6.b), the small craters'
/// (R09.T7.b), and the coarse craters' rims' (R09.T7.c) from the craters the cell's index holds
/// (R09.T8).
///
/// "Wavelengths shorter than λ" means, for each contribution, its own placement: the structural
/// octaves' spherical-harmonic degrees whose wavelength 2πR ÷ l is shorter than λ; a channel level
/// counted from its mean cell's width (none) to twice it (all); a small-crater octave counted
/// whole at the band limit's fillet from the smallest diameter it keeps (none) to its upper one
/// (all), and the rest of it by its sharpening finer than ½ λ (R09.T7.h); and the coarse rims'
/// sharpening finer than ½ λ, held to the coarse band. A band level evaluates each by its own
/// scale, so what it leaves out is more than the variance at its `λ_n`, and the level-keyed form,
/// [`Synthesiser::omitted_variance`], counts it (R10.T5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UnresolvedVariance {
    relief: SquareMetres,
    channels: SquareMetres,
    craters: SquareMetres,
    rims: SquareMetres,
}

impl UnresolvedVariance {
    /// The structural octaves' share, square metres: their variance in the band asked for, each
    /// octave's share of the spectrum's band times (a `g_m`^b)², the cell's amplitude and, on a
    /// mountain belt, the octave's belt factor ([`relief`]; [`local_variance`]'s note).
    #[must_use]
    pub const fn relief(&self) -> SquareMetres {
        self.relief
    }

    /// The channels' share, square metres: the variance of their incision's levels in the band
    /// asked for, from the cell's record and its neighbours' as the synthesis cuts them
    /// ([`channels::drawn_channel_variance`]).
    #[must_use]
    pub const fn channels(&self) -> SquareMetres {
        self.channels
    }

    /// The small craters' share, square metres: the body's, the same in every cell
    /// ([`craters::unresolved_variance`]).
    #[must_use]
    pub const fn craters(&self) -> SquareMetres {
        self.craters
    }

    /// The coarse craters' rims' share, square metres: the part of each listed crater's sharpening
    /// finer than the wavelength, its mean square over the crater's reach summed over the craters
    /// that reach the cell, a crater-wide estimate (R09.T7.c's rims; R09.T8).
    #[must_use]
    pub const fn rims(&self) -> SquareMetres {
        self.rims
    }

    /// The whole, square metres: the contributions are taken as independent, so their variances
    /// add, in the order the fields are listed: the structural share, the channels', the small
    /// craters', the coarse craters' rims'.
    #[must_use]
    pub fn total(&self) -> SquareMetres {
        self.relief + self.channels + self.craters + self.rims
    }
}

/// The variance of `cell`'s ground in wavelengths shorter than `finer_than`, per contribution, or
/// `None` where `field` does not hold the cell, the receiver of its trunk or, where water drains
/// through it, an edge neighbour (Design note 18; R09.T6.f, R09.T6.g).
///
/// Each contribution reads `finer_than` in its own placement ([`UnresolvedVariance`]), so a band
/// level leaves out more than this at its `λ_n`: what it leaves out is
/// [`Synthesiser::omitted_variance`].
///
/// It builds the body's small-crater octaves, about 1 to 10 ms; a [`Synthesiser`] holds them
/// ([`Synthesiser::unresolved_variance`]).
///
/// # Panics
///
/// If `finer_than` is not finite and positive, or `cell` is not of the field's level; and as
/// [`CraterOctaves::new`], which no header a field holds reaches.
#[must_use]
pub fn unresolved_variance(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<UnresolvedVariance> {
    unresolved_with(field, &CraterOctaves::new(field.header()), cell, finer_than)
}

/// [`unresolved_variance`] with the body's small-crater octaves `octaves`.
#[must_use]
fn unresolved_with(
    field: &impl FieldView,
    octaves: &CraterOctaves,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<UnresolvedVariance> {
    let relief = relief::unresolved_relief_variance(field, cell, finer_than)?;
    let channels = channels::unresolved_channel_variance(field, cell, finer_than)?;
    Some(UnresolvedVariance {
        relief,
        channels,
        craters: octaves.unresolved_variance(finer_than),
        rims: craters::rims::unresolved_rims(field, cell, finer_than)?,
    })
}

/// The structure function of `cell`'s ground in wavelengths shorter than `finer_than`, at a lag of
/// `lag`: E[(h(p + Δ e) − h(p))²] along a tangent e, square metres, the sum of its contributions'
/// closed forms, taken as independent; `None` where `field` does not hold the cell or the receiver
/// of its trunk (Design note 18; R10's slope readouts; R09.T6.f).
///
/// The contributions are the structural octaves', octave by octave from their pinned unit
/// structure functions at the cell's style (`decision-r09-t5.md` item 7.3), the channels'
/// ([`channels::channel_structure_function`]), the small craters'
/// ([`CraterOctaves::structure_function`]) and the coarse craters' rims', each crater's
/// 2V (1 − exp(−s Δ² ÷ 2V)) from its sharpening's crater-wide mean square and slopes. Each tends
/// to twice its share of
/// [`unresolved_variance`] at lags long against its wavelengths and to its slopes' variance times
/// the lag squared at short ones.
///
/// Each contribution reads `finer_than` in its own placement ([`UnresolvedVariance`]). The
/// structure function of what a band level leaves out is
/// [`Synthesiser::omitted_structure_function`].
///
/// It builds the body's small-crater octaves, about 1 to 10 ms; a [`Synthesiser`] holds them
/// ([`Synthesiser::structure_function`]).
///
/// # Panics
///
/// If `finer_than` is not finite and positive, `lag` is not finite and non-negative, or `cell` is
/// not of the field's level; and as [`CraterOctaves::new`], which no header a field holds
/// reaches.
#[must_use]
pub fn structure_function(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
    lag: Metres,
) -> Option<SquareMetres> {
    structure_with(
        field,
        &CraterOctaves::new(field.header()),
        cell,
        finer_than,
        lag,
    )
}

/// [`structure_function`] with the body's small-crater octaves `octaves`.
#[must_use]
fn structure_with(
    field: &impl FieldView,
    octaves: &CraterOctaves,
    cell: PatchKey,
    finer_than: Metres,
    lag: Metres,
) -> Option<SquareMetres> {
    let relief = relief::relief_structure_function(field, cell, finer_than, lag)?;
    let channels = channels::channel_structure_function(field, cell, finer_than, lag)?;
    let craters = octaves.structure_function(finer_than, lag);
    let rims = craters::rims::rims_structure_function(field, cell, finer_than, lag)?;
    Some(relief + channels + craters + rims)
}

/// The RMS of `cell`'s ground in wavelengths shorter than `resolution`: the square root of
/// [`unresolved_variance`]'s total, one standard deviation of every contribution in wavelengths
/// shorter than the resolution, each in its own placement ([`UnresolvedVariance`]); `None` where
/// `field` does not hold the cell, the receiver of its trunk or, where water drains through it, an
/// edge neighbour (R09.T6.f, R09.T6.g).
///
/// It falls as the resolution refines and is zero at the 2 m band limit and below, where the
/// synthesis has nothing finer. A height read at a band level leaves out more than this at the
/// level's `λ_n`: R10's readings quote [`Synthesiser::omitted_rms`] as their uncertainty (its
/// Design note 6).
///
/// It builds the body's small-crater octaves, about 1 to 10 ms; a [`Synthesiser`] holds them
/// ([`Synthesiser::unresolved_rms`]).
///
/// # Panics
///
/// If `resolution` is not finite and positive, or `cell` is not of the field's level; and as
/// [`CraterOctaves::new`], which no header a field holds reaches.
#[must_use]
pub fn unresolved_rms(
    field: &impl FieldView,
    cell: PatchKey,
    resolution: Metres,
) -> Option<Metres> {
    unresolved_variance(field, cell, resolution).map(rms_of)
}

/// The square root of `variance`'s total, metres.
#[must_use]
fn rms_of(variance: UnresolvedVariance) -> Metres {
    Metres::new(variance.total().value().sqrt())
}

/// Why a [`Synthesiser`] could not answer a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryHeightError {
    /// The query reads a cell the view does not hold: neither surveyed nor in a survey's margin
    /// ([`crate::field::SYNTHESIS_MARGIN_CELLS`]), so the height there is not known to this side.
    NotSurveyed(PatchKey),
}

impl std::fmt::Display for QueryHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSurveyed(c) => write!(
                f,
                "cell ({}, {}) of face {} at level {} is not surveyed or in a survey's margin",
                c.i(),
                c.j(),
                c.face().index(),
                c.level()
            ),
        }
    }
}

impl std::error::Error for QueryHeightError {}

impl From<ReadCellError> for QueryHeightError {
    fn from(e: ReadCellError) -> Self {
        Self::NotSurveyed(e.cell)
    }
}

/// A quadtree level at which the synthesis is evaluated, 0 to [`MAX_LEVEL`]: a patch's level, whose
/// band is the relief its vertices resolve (Design note 13). R10 makes one from a patch's `u8`
/// level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BandLevel(u8);

/// Why a level is not a [`BandLevel`]: it is deeper than [`MAX_LEVEL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NewBandLevelError(pub u8);

impl std::fmt::Display for NewBandLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "band level {} is above the maximum {MAX_LEVEL}", self.0)
    }
}

impl std::error::Error for NewBandLevelError {}

impl BandLevel {
    /// The band of quadtree level `level`.
    ///
    /// # Errors
    ///
    /// [`NewBandLevelError`] above [`MAX_LEVEL`].
    pub const fn new(level: u8) -> Result<Self, NewBandLevelError> {
        if level > MAX_LEVEL {
            Err(NewBandLevelError(level))
        } else {
            Ok(Self(level))
        }
    }

    /// The quadtree level.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for BandLevel {
    type Error = NewBandLevelError;

    fn try_from(level: u8) -> Result<Self, Self::Error> {
        Self::new(level)
    }
}

/// The spectrum of a body's structural relief finer than its coarse cells: a degree variance that
/// breaks once, V(l) = V₁ l^−β₁ below the break degree `l_b` and V₁ `l_b`^(β₂−β₁) l^−β₂ from it,
/// continuous at the break (Design note 7; `decision-r09-t5.md` items 2–4).
///
/// V(l) is the variance of height contributed by the 2l + 1 spherical harmonics of degree l, so
/// that the height variance about the mean is the sum of V(l) over every degree from 1, and the
/// synthesis's expected variance below a coarse cell is the sum from the cell's Nyquist degree up,
/// in closed form ([`local_variance`]). It is the law of the structural share alone, the relief
/// that is neither cratered nor incised (tectonic and volcanic relief, and the hillslopes between
/// channels): the craters (R09.T7) and the channels (R09.T6) add their own variance on top, and
/// the coarse pass books each share in `σ_h`'s budget rather than in this law (item 4). It is the
/// law of the reference style, amplitude 1 ([`relief::ReliefStyle`]): the styles are absolute
/// ratios to it, and nothing divides it by their mean square (item 1).
///
/// A body's law is [`BandSpectrum::anchored`] of its crust's [`SpectrumShape`]: one break, from
/// β₁ at the coarse cells' degrees to a steeper β₂ below a wavelength of a kilometre or two, which
/// every dataset the ruling measured shows. The single law V₁ l^−1.9 that R09.T2 began with gave
/// slopes of 56° along a profile at 1 m on the Earth-like world (R09.T5's science check). The
/// relief ([`relief`]) derives each level's amplitude from the law
/// ([`BandSpectrum::level_variance`]), so the header carries the law alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandSpectrum {
    exponent: f64,
    unit_degree_variance: SquareMetres,
    break_degree: f64,
    small_scale_exponent: f64,
}

/// The parts of a [`BandSpectrum`], which [`BandSpectrum::new`] checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandSpectrumParts {
    /// The exponent β₁ of V(l) ∝ l^−β₁ below the break degree: finite and above 1.
    pub exponent: f64,
    /// The degree-1 variance V₁, square metres, the law's scale: finite and non-negative.
    pub unit_degree_variance: SquareMetres,
    /// The break degree `l_b`, from which the exponent is β₂: finite and at least 1, and need not
    /// be an integer.
    pub break_degree: f64,
    /// The exponent β₂ of V(l) ∝ l^−β₂ from the break degree up: finite and above 1.
    pub small_scale_exponent: f64,
}

/// Why [`BandSpectrum::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBandSpectrumError {
    /// The exponent β₁ is not finite and above 1, so the variance would not sum.
    Exponent(f64),
    /// The degree-1 variance V₁ is not finite and non-negative.
    Variance(f64),
    /// The break degree is not finite and at least 1.
    BreakDegree(f64),
    /// The small-scale exponent β₂ is not finite and above 1, so the variance would not sum.
    SmallScaleExponent(f64),
    /// The law's whole variance, Σ V(l) from degree 1, is not finite: parts each in range whose
    /// closed forms overflow (a break degree far beyond every field's degrees with a much steeper
    /// β₂, or a V₁ near the largest `f64`), so that every variance the relief reads stays finite.
    Total(f64),
}

impl std::fmt::Display for BuildBandSpectrumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exponent(b) => write!(f, "spectral exponent {b} is not finite and above 1"),
            Self::Variance(v) => {
                write!(f, "degree variance {v} m² is not finite and non-negative")
            }
            Self::BreakDegree(l) => write!(f, "break degree {l} is not finite and at least 1"),
            Self::SmallScaleExponent(b) => write!(
                f,
                "small-scale spectral exponent {b} is not finite and above 1"
            ),
            Self::Total(v) => write!(f, "the spectrum's whole variance {v} m² is not finite"),
        }
    }
}

impl std::error::Error for BuildBandSpectrumError {}

/// Why [`SpectrumShape::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSpectrumShapeError {
    /// The exponent β₁ is not finite and above 1.
    Exponent(f64),
    /// The break wavelength is not finite and positive, metres.
    BreakWavelength(f64),
    /// The small-scale exponent β₂ is not finite and above 1.
    SmallScaleExponent(f64),
}

impl std::fmt::Display for BuildSpectrumShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exponent(b) => write!(f, "spectral exponent {b} is not finite and above 1"),
            Self::BreakWavelength(w) => {
                write!(f, "break wavelength {w} m is not finite and positive")
            }
            Self::SmallScaleExponent(b) => write!(
                f,
                "small-scale spectral exponent {b} is not finite and above 1"
            ),
        }
    }
}

impl std::error::Error for BuildSpectrumShapeError {}

/// The shape of a crust class's structural spectrum: β₁ from the coarse cell to the break
/// wavelength `λ_b`, and β₂ below it (`decision-r09-t5.md` item 2), which
/// [`BandSpectrum::anchored`] scales to a body.
///
/// The break is a wavelength, not a degree, so that equal crusts on bodies of different radius
/// break at the same scale. It has no gravity term: breaks between about 1 and 3 km are found on
/// bodies whose gravity spans a factor of 7 (Earth, the Moon's breakover, Europa), and Landais,
/// Schmidt and Lovejoy (2019, MNRAS 484, 787, doi:10.1093/mnras/sty3253; their 2018 preprint,
/// arXiv:1805.11249) find a common transition near 10 km on Earth, Mars, the Moon and Mercury,
/// which they attribute to the elastic thickness; the ruling infers from it that the break carries
/// no gravity term. Age acts through the craters, not through the law. The two constants are
/// the set the coarse pass chooses from (R09.T12.e), a closed one for the composition audit (the
/// plan's Risks); [`new`](Self::new) makes another crust class's shape from its measurements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectrumShape {
    exponent: f64,
    break_wavelength: Metres,
    small_scale_exponent: f64,
}

impl SpectrumShape {
    /// The shape of exponent β₁ `exponent` above the break wavelength `break_wavelength` and
    /// `small_scale_exponent` β₂ below it.
    ///
    /// # Errors
    ///
    /// [`BuildSpectrumShapeError::Exponent`] unless β₁ is finite and above 1,
    /// [`BuildSpectrumShapeError::BreakWavelength`] unless the break wavelength is finite and
    /// positive, and [`BuildSpectrumShapeError::SmallScaleExponent`] unless β₂ is finite and
    /// above 1, checked in that order.
    pub fn new(
        exponent: f64,
        break_wavelength: Metres,
        small_scale_exponent: f64,
    ) -> Result<Self, BuildSpectrumShapeError> {
        if !(exponent.is_finite() && exponent > 1.0) {
            return Err(BuildSpectrumShapeError::Exponent(exponent));
        }
        let w = break_wavelength.value();
        if !(w.is_finite() && w > 0.0) {
            return Err(BuildSpectrumShapeError::BreakWavelength(w));
        }
        if !(small_scale_exponent.is_finite() && small_scale_exponent > 1.0) {
            return Err(BuildSpectrumShapeError::SmallScaleExponent(
                small_scale_exponent,
            ));
        }
        Ok(Self {
            exponent,
            break_wavelength,
            small_scale_exponent,
        })
    }

    /// The exponent β₁ above the break wavelength.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The break wavelength `λ_b`.
    #[must_use]
    pub const fn break_wavelength(&self) -> Metres {
        self.break_wavelength
    }

    /// The exponent β₂ below the break wavelength.
    #[must_use]
    pub const fn small_scale_exponent(&self) -> f64 {
        self.small_scale_exponent
    }

    /// A silicate crust's shape, a rock surface in any regime but an ice shell: β₁ 2.0 to a break
    /// at 2 km, then β₂ 3.0 (`decision-r09-t5.md` item 2).
    ///
    /// Above the break, Earth's land measures β 2.04 at 26–2.8 km in 60 random tiles of SRTM's 1″
    /// grid (Farr et al. 2007, Rev. Geophys. 45, RG2004), 2.0 at 10–40 km on Earth2014's land (the
    /// grid of Hirt and Rexer 2015, Int. J. Appl. Earth Obs. Geoinf. 39, 103) and 2.04–2.17 in
    /// Gagnon, Lovejoy and Schertzer 2006 (Nonlin. Processes Geophys. 13, 541, Table 5), and the
    /// sea floor 2.0–2.3 above about 2 km (slope ratios on GMRT, the grid of Ryan et al. 2009, G³
    /// 10, Q03014). The land steepens through 2–3 km, mountains first, to 2.79 at 2.8 km–350 m and
    /// 3.14 at 350–87 m (the SRTM sample; Perron, Kirchner and Dietrich 2008, JGR 113, F04003, ¶36,
    /// their 2D exponents less 1), and the sea floor to 2.4–2.7 at 0.25–1 km (GMRT) and about 2.8
    /// (von Kármán ν ≈ 0.9, the ruling's reading of Goff 2020, GRL 47, e2020GL088162; Goff 2010,
    /// JGR 115, B12104, ¶33, takes a fractal dimension of 2.2 as typical, ν ≈ 0.8). β₂ = 3.0
    /// matches the land's slope statistics from 1 km down to the 2 m band limit. The soil-mantled
    /// rollover below about 200 m on wet worlds (β 3.5–4.2, Perron et al. 2008) is not a second
    /// break, since bedrock and mountains, which carry most of the slope variance, stay near β
    /// 3.1–3.4 there; and the cratered bodies' own breaks near 10 km are their craters', which
    /// R09.T7 adds. These measurements are the ruling's (its scratch spectra). Medium confidence.
    pub const SILICATE: Self = Self {
        exponent: 2.0,
        break_wavelength: Metres::new(2_000.0),
        small_scale_exponent: 3.0,
    };

    /// An ice-rich crust's shape, an ice surface or an ice shell: β₁ 1.8 to a break at 1 km, then
    /// β₂ 2.6 (`decision-r09-t5.md` item 2).
    ///
    /// Europa's roughness takes one figure above the break in the ruling's reading, β₁ 1.8 (a
    /// Hurst exponent of 0.4) from 70 km down to it, with "a break in slope at about 1 km" and a
    /// Hurst exponent of 0.7–0.8 below it, β 2.4–2.6 (Nimmo and Schenk 2008, LPSC XXXIX, abstract
    /// 1464; Schenk and Nimmo 2017, Europa Deep Dive I, LPI Contrib. 2048, abstract 7015;
    /// Steinbrügge et al. 2020, Icarus 343, 113669). β₁ is a choice between the sources, not a
    /// measured value: the stereo models give H about 0.3 (β 1.6) at 1–10 km and the limb
    /// profiles about 0.2 (β 1.4) at 12–630 km, while bridging the models' RMS deviation at 1 km
    /// to the profiles' at 70 km gives H 0.5–0.6 (R09.T5's second follow-up's science check).
    /// Medium-low confidence: one body's measurements.
    pub const ICE_RICH: Self = Self {
        exponent: 1.8,
        break_wavelength: Metres::new(1_000.0),
        small_scale_exponent: 2.6,
    };
}

/// The wavelength below which a crust class's structural RMS is anchored, 72 km: the Earth's coarse
/// cell's Nyquist wavelength, 2πR⊕ ÷ 556, held in physical wavelength so that equal crusts on
/// bodies of different radius are equally rough (`decision-r09-t5.md` item 3).
pub const ANCHOR_WAVELENGTH: Metres = Metres::new(72_000.0);

/// A mobile lid's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`] at the Earth's
/// gravity, 200 m (`decision-r09-t5.md` item 3). On another body it is this times g⊕ ÷ g, the
/// strength-limited factor the coarse pass gives belts and trenches (R09.T12.b, 1 on the reference
/// Earth), so that a belt's coarse and fine relief scale together.
///
/// Earth2014's BED layer carries 183 m above degree 556 (Hirt and Rexer 2015's fitted degree
/// variance, Int. J. Appl. Earth Obs. Geoinf. 39, 103, recomputed by the ruling); its bathymetry,
/// about 90% altimetric at 4–5′, lacks the abyssal hills, which the multibeam of GMRT (the grid of
/// Ryan et al. 2009, G³ 10, Q03014, doi:10.1029/2008GC002332) shows carrying 50–230 m RMS at
/// 2–10 km over 71% of the surface (the ruling's reading; no abyssal-hill source is re-read here).
/// Added in quadrature to the BED layer's 183 m over the 71%, √(183² + 0.71 x²), that gives
/// 188–266 m, and Design note 7's "about 200 m" is a choice within it, labelled as one. Medium
/// confidence.
pub const STRUCTURAL_RMS_MOBILE_LID: Metres = Metres::new(200.0);

/// A rocky stagnant lid's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`], 60 m,
/// with no gravity term; a heat-pipe or episodic lid's too (`decision-r09-t5.md` item 3).
///
/// Crater-poor plains in 35 km windows measure 45 m (Vastitas Borealis), 55 m (Amazonis) and 43 m
/// (Daedalia) on MOLA's 32 ppd grid (Smith et al. 2001, JGR 106, 23689), and the lunar maria 89 m
/// with their small craters and wrinkle ridges on LOLA's 16 ppd grid (Smith et al. 2010, GRL 37,
/// L18204), times 1.15, the Earth's ratio of that window RMS to the RMS above 72 km (the ruling's
/// measurements). The maria and Mars's plains differ by under 2× at 10 km though their gravities
/// differ by 2.3×, both being set by emplacement and craters rather than by strength. The windows'
/// bounds, detrending and statistic are not recorded, so 60 m is not derived from them: times
/// 1.15 they give 49–63 m for Mars's plains and 102 m for the maria (R09.T12.e's science check),
/// and 60 m is a choice within the plains' range, labelled as one. Medium-low confidence.
pub const STRUCTURAL_RMS_STAGNANT_LID: Metres = Metres::new(60.0);

/// An ice-rich crust's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`], 50 m, with
/// no gravity term (`decision-r09-t5.md` item 3).
///
/// Europa's ridged plains have an RMS deviation of 35–50 m over a 1 km baseline (Nimmo and Schenk
/// 2008, LPSC XXXIX, abstract 1464, Fig. 5a; its Table 1 gives 5.6–8.5 m at 100 m), and in the
/// ruling's reading a Hurst exponent of about 0.4 from 1 to 70 km, the ice shape's β₁ of 1.8
/// ([`SpectrumShape::ICE_RICH`], whose documentation gives the sources' own exponents). With that
/// shape the anchor gives an RMS deviation of 24.7 m at 1 km, 0.49–0.71 of Fig. 5a's (meeting it
/// would take an anchor of about 71–101 m), and 6.5 m at 100 m, inside Table 1's; the value is
/// kept (main's ruling for R09.T12.e, 2026-10-19). Its adirectional RMS slopes (√2 times a
/// profile's) are 2.0°, 5.3° and 9.5° over 1 km, 100 m and 10 m (R09.T12.e's science check),
/// against Europa's 2.8–4.1°, 4.5–6.9° and 8.5–13° (the last extrapolated). Medium-low
/// confidence.
pub const STRUCTURAL_RMS_ICE_RICH: Metres = Metres::new(50.0);

impl BandSpectrum {
    /// The default exponent β₁, per-degree variance ∝ l^−2 above the break: a silicate crust's
    /// ([`SpectrumShape::SILICATE`]).
    pub const DEFAULT_EXPONENT: f64 = 2.0;

    /// The spectrum of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildBandSpectrumError::Exponent`] unless β₁ is finite and above 1, where the sum over
    /// degrees converges; [`BuildBandSpectrumError::Variance`] unless V₁ is finite and
    /// non-negative; [`BuildBandSpectrumError::BreakDegree`] unless the break degree is finite and
    /// at least 1; [`BuildBandSpectrumError::SmallScaleExponent`] unless β₂ is finite and above
    /// 1; and [`BuildBandSpectrumError::Total`] unless the whole variance from degree 1 is finite;
    /// checked in that order.
    pub fn new(parts: BandSpectrumParts) -> Result<Self, BuildBandSpectrumError> {
        let BandSpectrumParts {
            exponent,
            unit_degree_variance,
            break_degree,
            small_scale_exponent,
        } = parts;
        if !(exponent.is_finite() && exponent > 1.0) {
            return Err(BuildBandSpectrumError::Exponent(exponent));
        }
        let v = unit_degree_variance.value();
        if !(v.is_finite() && v >= 0.0) {
            return Err(BuildBandSpectrumError::Variance(v));
        }
        if !(break_degree.is_finite() && break_degree >= 1.0) {
            return Err(BuildBandSpectrumError::BreakDegree(break_degree));
        }
        if !(small_scale_exponent.is_finite() && small_scale_exponent > 1.0) {
            return Err(BuildBandSpectrumError::SmallScaleExponent(
                small_scale_exponent,
            ));
        }
        let spectrum = Self {
            exponent,
            unit_degree_variance,
            break_degree,
            small_scale_exponent,
        };
        // Every band the relief reads, and the break's factor within it, is bounded by the whole;
        // an overflowing factor makes the whole infinite, or NaN where V₁ is 0.
        let total = spectrum.variance_from_degree(1).value();
        if !total.is_finite() {
            return Err(BuildBandSpectrumError::Total(total));
        }
        Ok(spectrum)
    }

    /// The structural spectrum of a crust of shape `shape` on a body of mean radius `radius`,
    /// whose RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`] is `rms_shorter_than`
    /// (`decision-r09-t5.md` item 3).
    ///
    /// The break degree is 2πR ÷ `λ_b`, or 1 on a body so small that its whole spectrum lies below
    /// the break wavelength. V₁ is the one for which the variance from degree
    /// `l_72` = max(2, round(2πR ÷ 72 km)) up
    /// ([`variance_from_degree`](Self::variance_from_degree)) is `rms_shorter_than`²: anchored at
    /// the coarse cell, never from degree 1, which the Earth's continent–ocean dichotomy dominates
    /// and no power law describes. The coarse pass (R09.T12.e) and the synthetic worlds call it
    /// alike, with the crust class's anchor:
    /// [`STRUCTURAL_RMS_MOBILE_LID`] times g⊕ ÷ g, [`STRUCTURAL_RMS_STAGNANT_LID`] or
    /// [`STRUCTURAL_RMS_ICE_RICH`]. An anchor of zero gives V₁ = 0, a body with no relief finer
    /// than its cells.
    ///
    /// # Panics
    ///
    /// If `radius` is not finite and positive or is over about 10²⁰ m (its anchor degree past
    /// 2⁵³), or `rms_shorter_than` is not finite and non-negative; or if [`new`](Self::new)
    /// refuses the law on this body, from a shape outside every crust's: a break wavelength so
    /// short against the radius that the break degree is not finite, a unit law whose variance
    /// from `l_72` underflows to 0, or a whole variance that is not finite.
    #[must_use]
    pub fn anchored(shape: SpectrumShape, radius: Metres, rms_shorter_than: Metres) -> Self {
        let r = radius.value();
        assert!(
            r.is_finite() && r > 0.0,
            "a body's radius must be finite and positive, got {r} m"
        );
        let sigma = rms_shorter_than.value();
        assert!(
            sigma.is_finite() && sigma >= 0.0,
            "an anchor RMS must be finite and non-negative, got {sigma} m"
        );
        let circumference = 2.0 * core::f64::consts::PI * r;
        let unit = BandSpectrumParts {
            exponent: shape.exponent,
            unit_degree_variance: SquareMetres::new(1.0),
            break_degree: num::max(circumference / shape.break_wavelength.value(), 1.0),
            small_scale_exponent: shape.small_scale_exponent,
        };
        let refused = |e: BuildBandSpectrumError| -> Self {
            panic!("a spectrum shape must give a valid law on this body: {e}")
        };
        let per_unit = Self::new(unit)
            .unwrap_or_else(refused)
            .variance_from_degree(anchor_degree(circumference))
            .value();
        Self::new(BandSpectrumParts {
            unit_degree_variance: SquareMetres::new(sigma * sigma / per_unit),
            ..unit
        })
        .unwrap_or_else(refused)
    }

    /// The exponent β₁ of V(l) ∝ l^−β₁ below the break degree.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The degree-1 variance V₁, square metres: the law's scale.
    #[must_use]
    pub const fn unit_degree_variance(&self) -> SquareMetres {
        self.unit_degree_variance
    }

    /// The break degree `l_b`, at least 1, from which the exponent is β₂.
    #[must_use]
    pub const fn break_degree(&self) -> f64 {
        self.break_degree
    }

    /// The exponent β₂ of V(l) ∝ l^−β₂ from the break degree up.
    #[must_use]
    pub const fn small_scale_exponent(&self) -> f64 {
        self.small_scale_exponent
    }

    /// The degree variance at degree `l`, square metres: V₁ l^−β₁ below the break degree and
    /// V₁ `l_b`^(β₂−β₁) l^−β₂ from it. `l` need not be an integer, so that a structure function
    /// may integrate it (R09.T8, R10).
    ///
    /// # Panics
    ///
    /// If `l` is not finite and at least 1.
    #[must_use]
    pub fn degree_variance(&self, l: f64) -> SquareMetres {
        assert!(
            l.is_finite() && l >= 1.0,
            "a degree must be finite and at least 1, got {l}"
        );
        let v1 = self.unit_degree_variance.value();
        SquareMetres::new(if l < self.break_degree {
            v1 * math::powf(l, -self.exponent)
        } else {
            v1 * self.break_factor() * math::powf(l, -self.small_scale_exponent)
        })
    }

    /// `l_b`^(β₂−β₁), the factor that makes the law continuous at the break.
    #[must_use]
    pub(crate) fn break_factor(&self) -> f64 {
        math::powf(self.break_degree, self.small_scale_exponent - self.exponent)
    }
}

/// The first degree of [`BandSpectrum::anchored`]'s anchor on a body of circumference
/// `circumference` metres: max(2, round(2πR ÷ [`ANCHOR_WAVELENGTH`])).
///
/// # Panics
///
/// If the degree is 2⁵³ or more, a body some 10¹⁰ AU round.
#[must_use]
fn anchor_degree(circumference: f64) -> u64 {
    let degree = (circumference / ANCHOR_WAVELENGTH.value()).round();
    if degree <= 2.0 {
        return 2;
    }
    assert!(
        degree < 9.007_199_254_740_992e15,
        "a body of circumference {circumference} m is beyond every degree"
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive integer below 2^53, checked above"
    )]
    let degree = degree as u64;
    degree
}

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod height_golden;

#[cfg(test)]
mod tests;
