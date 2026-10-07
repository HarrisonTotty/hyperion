//! The naked eye's limit in every direction of the band, and each listed star's own: the limit
//! map and the eye offsets (rendering plan R06, R06.T9.c, R06.T9.h and R06.T9.i; Design notes 4,
//! 5 and 17).
//!
//! Each texel's limit is Crumey's threshold ([`naked_eye_limit`]) against the background the eye
//! sees in the direction of the texel's centre, the direction its ray takes: the band's light there,
//! the stars the census did not list ([`BandTexel`]), plus the veiling glare of every listed star
//! within 90° of it, CIE 146:2002's general disability glare ([`veiling_luminance`]; Vos 2003,
//! Lighting Res. Technol. 35, 163), which Design note 4 takes for the "standard way" (Adrian 1989,
//! Lighting Res. Technol. 21, 181) to which Crumey (2014, MNRAS 442, 2600, §1.6.3) leaves glare.
//!
//! **The eye's own sky** (R06.T9.j; decided 2026-10-06, `decision-r06-t9c-glare.md`). The eye's
//! map is the eye-only request's at the same census radii, whatever the request's cut. Where a
//! camera's deeper cut sets the band's, the background is the expected light fainter than the
//! eye's cut ([`SkyQuery::eye_cut`](super::census::SkyQuery::eye_cut)), which the band's march
//! keeps beside its own light ([`march_rows`](super::band::march_rows)), and only the listed stars
//! at or brighter than the eye's cut glare ([`Glare::of_listed`]). A listed star fainter than it
//! adds neither glare nor background, since its light is in that expected light already, and its
//! eye offset is its colour offset alone. At the same census radius, opening a camera view so
//! changes none of the eye's limits or offsets, bit for bit. A camera's deeper caps reach further:
//! there the census lists the rare stars brighter than the eye's cut, under one expected a layer
//! by the caps' rule, which glare and may be seen, where an eye-only request holds them as
//! expected light. The contract is so the eye-only request's map at the camera's census radii
//! (the same replies and `n_max`), and those real stars are accepted as the truer sky: they take
//! the place of their expected light in the eye's background, in expectation, not star by star
//! (decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2).
//!
//! The glare's illuminance is taken in the plane of the eye, perpendicular to the line of sight,
//! as CIE 146:2002 defines it (and IJspeert et al. 1990, Vision Res. 30, 699; Stiles and Crawford
//! 1937, Proc. R. Soc. B 122, 255; see [`veiling_luminance`]): a star of illuminance E at θ from a
//! texel's centre gives E cos θ, and a star at or beyond 90°, behind the eye's plane, gives none.
//! That nothing beyond 90° is taken, where CIE's range runs to 100°, is the ruling's inference from
//! that definition (decided 2026-10-06, `decision-r06-t9c-glare.md`); it also makes the veil
//! continuous at its reach.
//!
//! The glare is taken as the stars' own light scattered in the eye, in each star's colour (Design
//! note 4). Intraocular straylight is not spectrally neutral (near λ⁻⁴ in young, well-pigmented
//! eyes, with a red component added in light ones; Coppens, Franssen and van den Berg 2006, Exp.
//! Eye Res. 82, 688), so the veil's rod weight is uncertain by some tens of per cent: up to about
//! 0.1 mag where the veil dominates a texel. With E★ a listed star's photopic illuminance at the
//! eye after its own reddening, ρ★ its reddened S/P ratio
//! (`star.colour().reddened(star.a_v()).sp_ratio()`, R06.T9.e) and K(θ) the veil per lux in the
//! eye's plane at θ from the texel's centre (θ clamped at 0.1°, cos θ the plane's factor, nothing
//! at or beyond 90°), a texel of band luminance B and ratio ρ is seen against
//!
//! (B and ρ the light fainter than the eye's cut, the band's own at the eye's cut)
//!
//! - a photopic luminance B′ = B + Σ E★ K(θ★) cos θ★, and
//! - an S/P ratio ρ′ = (ρ B + Σ ρ★ E★ K(θ★) cos θ★) ÷ B′, the band's and the veils' light together.
//!
//! In a scotopic background the threshold then reads (ρ′ ÷ 1.408) B′ of Blackwell's light: each
//! star's veil weighed by ρ★ ÷ 1.408 as the band's light is by ρ ÷ 1.408, Design note 4's rod
//! weighting. In a mesopic one each light is weighed by its MES2 mesopic luminance at the photopic
//! weight of the whole background, band and veils together, since the eye adapts to all it sees
//! ([`blackwell_equivalent_factor`]'s construction). F stays the observer's: the glare is
//! modelled, not folded into F (R06's Risks, "Glare double count").
//!
//! **The far field** (R06.T9.i; decided 2026-10-06, `decision-r06-t9c-glare.md`, item 2). The
//! sum over every listed star costs texels × stars: for a 64² band at the census's 300,000 stars,
//! about 140 CPU-s at T9.c's 19 ns a pair, and 161 CPU-s measured on one thread in T9.i (21.9 ns
//! a pair, R06's Risks, "Deviations in T9.i, as built"). So the veil is summed over a pyramid of
//! the band's own texels
//! ([`Glare`]): star by star within 4° of a texel's centre, and beyond by node monopoles under an
//! opening angle of 0.25. Every texel's limit is then within 0.001 mag of the exact sum, the
//! quantum of the wire's texel limit (millimagnitudes, Design note 17). A texel always opens its
//! own leaf, so each star's own term below is still the term the map added for it.
//!
//! **A star's own veil is not its own background** (R06.T9.h). Crumey's eq. 34 fits Blackwell's
//! thresholds for small targets seen by real eyes, so the target's own light scattered in the eye
//! is already in its threshold, and CIE 146's veil is one source's light over another target. The
//! map's texels are each a function of every listed star, as the other stars see them; a listed
//! star's own limit is taken against its texel's background less the veil the map added for it,
//! at its angle from its texel's centre, and [`eye_offsets`] gives each star that limit less its
//! texel's: its self-exclusion and its colour offset ([`star_colour_offset`]) together. The wire
//! carries it on the star's eye offset (Design note 17), and a view keeps a star whose V is
//! brighter than its texel's limit plus it, its own limit.
//!
//! The band's own luminance, chroma and ρ are left as they are: the glare is the eye's, not the
//! sky's light, and the band layer draws only the sky's. The glare of the observer's own stars and
//! of sunlit bodies is not in the map (Design note 4).
//!
//! **The eye's cut** ([`eye_cut`], R06.T9.d; Design note 5), the faintest V a sky with the eye
//! lists, is set before the census, since the map is computed from the census: a coarse pre-pass
//! of the band with no census and no glare gives each direction's limit, and the cut is the
//! darkest one's plus the largest colour offset any star has plus a pad, so that every star an eye
//! view can see is listed.
//!
//! [`blackwell_equivalent_factor`]: super::eye::blackwell_equivalent_factor

use std::ops::Range;

use crate::coords::UnitVector;
use crate::galaxy::Galaxy;
use crate::math;
use crate::observe::Observer;
use crate::units::consts::RADIANS_PER_DEGREE;
use crate::units::{CandelasPerSquareMetre, Degrees, Lux, Magnitudes};

use super::band::{BandSpec, BandTexel, CompleteTo, CubeFace, band_rows, unextinguished_lux};
use super::census::{SkyCensus, SkyContext, SkyQuery, SkyStar, sky_order};
use super::colour::largest_sp_ratio;
use super::eye::{
    EyeObserver, MAX_CUT_V, SkyBackground, SpRatio, naked_eye_limit, star_colour_offset,
    veiling_luminance,
};

/// The glare's near field, degrees (R06.T9.i; decided 2026-10-06, `decision-r06-t9c-glare.md`,
/// item 2). A node of the glare's pyramid is taken whole only where its stars may all lie this far
/// or more from a texel's centre. Nearer, CIE's 10 ÷ θ³ term varies too fast across a node for one
/// direction to stand for it. In the ruling's model, on 30,000 stars, an opening angle alone took
/// monopoles a few tenths of a degree from bright stars, with errors of up to 0.13 mag at an
/// opening angle of 0.5 and 0.013 mag at 0.25.
const NEAR_FIELD_DEG: f64 = 4.0;

/// The glare pyramid's opening angle (the same ruling): a node is taken whole only where its
/// radius is at most this fraction of its angle from a texel's centre.
const OPENING_ANGLE: f64 = 0.25;

/// The veil per lux in the plane of the eye, cd m⁻² lx⁻¹, of light whose direction has the cosine
/// `cos` with a texel's centre. It is [`veiling_luminance`]'s per lux at that angle θ, times cos θ
/// for the illuminance in the plane of the eye. It is `None` at or beyond 90°, behind the eye's
/// plane, which gives no illuminance there.
///
/// Every term of the map's veil, a star's or a node's, and each star's own term
/// ([`eye_offsets`]) is this. So the term a star is excluded by is, bit for bit, the term the map
/// added for it.
#[must_use]
fn veil_per_lux_at(eye: &EyeObserver, cos: f64) -> Option<f64> {
    if cos <= 0.0 {
        return None;
    }
    let angle = math::acos(cos.min(1.0)) / RADIANS_PER_DEGREE;
    let per_lux = veiling_luminance(eye, Lux::new(1.0), Degrees::new(angle))
        .expect("a unit illuminance and an angle of 0–90° are valid")
        .value();
    Some(per_lux * cos)
}

/// The photopic and scotopic veil, cd m⁻², of light `light` (photopic and scotopic illuminance at
/// the eye, lux) from direction `from` over a texel whose centre lies in direction `toward`: none
/// where [`veil_per_lux_at`] is `None`.
#[must_use]
fn veil_of(
    eye: &EyeObserver,
    toward: &UnitVector,
    from: &UnitVector,
    light: [f64; 2],
) -> Option<[f64; 2]> {
    veil_per_lux_at(eye, toward.dot(from)).map(|per_lux| [light[0] * per_lux, light[1] * per_lux])
}

/// The angle between `a` and `b`, degrees.
#[must_use]
fn angle_deg(a: &UnitVector, b: &UnitVector) -> f64 {
    math::acos(a.dot(b).clamp(-1.0, 1.0)) / RADIANS_PER_DEGREE
}

/// A `u32` index into one of the pyramid's arrays, as a `usize`.
#[must_use]
fn at(index: u32) -> usize {
    usize::try_from(index).expect("a u32 index fits the usize of every target the sim builds for")
}

/// One listed star as the glare reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlareSource {
    /// Its direction from the observer, on the galactic axes: `None` for a star at the observer's
    /// own position, which has none and glares nothing.
    direction: Option<UnitVector>,
    /// Its photopic illuminance at the eye after its own reddening, normal to its direction, lux,
    /// that glares: zero for a listed star fainter than the eye's cut, which glares nothing
    /// (R06.T9.j).
    photopic: f64,
    /// Its scotopic light at the eye, its photopic illuminance times its reddened S/P ratio, lux.
    scotopic: f64,
    /// Its reddened S/P ratio, which sets its colour offset.
    sp_ratio: f64,
}

impl GlareSource {
    /// Its photopic and scotopic light, lux.
    #[must_use]
    const fn light(&self) -> [f64; 2] {
        [self.photopic, self.scotopic]
    }

    /// The photopic and scotopic veil this source adds over a texel in direction `toward`, cd m⁻²
    /// ([`veil_of`]): none for a source with no direction, and none at or beyond 90°.
    #[must_use]
    fn veil(&self, eye: &EyeObserver, toward: &UnitVector) -> Option<[f64; 2]> {
        veil_of(eye, toward, &self.direction?, self.light())
    }
}

/// A count of what the glare's traversal evaluates, so that its cost can be stated independently
/// of the machine (R06.T9.i). The map's own runs count nothing (`()`).
trait Tally {
    /// A node tested.
    fn visit(&mut self);
    /// A node taken whole, by its monopole.
    fn whole(&mut self);
    /// A star–texel pair summed, in an opened leaf.
    fn pair(&mut self);
}

impl Tally for () {
    fn visit(&mut self) {}
    fn whole(&mut self) {}
    fn pair(&mut self) {}
}

/// How the glare's pyramid is traversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    /// By the ruling's rule: the near field star by star, the far field by node monopoles.
    Pyramid,
    /// Every node opened, every star summed one by one: the exact sum, which the tests take as the
    /// reference.
    #[cfg(test)]
    Every,
}

/// A square of a face's texels, the region a node of the glare's pyramid covers: its face, its
/// level (0 for a texel), and its row and column at that level, a texel's row and column halved
/// `level` times. A face of a side that is not a power of two has partial squares at its bottom
/// and right edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Square {
    face: CubeFace,
    level: u8,
    row: u16,
    column: u16,
}

impl Square {
    /// The square of the next level up that holds this one.
    #[must_use]
    const fn parent(self) -> Self {
        Self {
            face: self.face,
            level: self.level + 1,
            row: self.row >> 1,
            column: self.column >> 1,
        }
    }

    /// Whether it holds the texel at `row` and `column` of `face`.
    #[must_use]
    fn holds(self, face: CubeFace, row: u16, column: u16) -> bool {
        self.face == face && row >> self.level == self.row && column >> self.level == self.column
    }
}

/// What a node of the glare's pyramid holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeHolds {
    /// Its children, `len` node indices from `start` in [`Pyramid::links`], in their squares'
    /// order.
    Children { start: u32, len: u32 },
    /// A leaf's stars, `len` from `start` in [`Pyramid::stars`], in the census's order.
    Stars { start: u32, len: u32 },
}

/// A glaring star as the pyramid holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlareLight {
    /// Its direction from the observer.
    direction: UnitVector,
    /// Its photopic and scotopic illuminance at the eye, lux.
    light: [f64; 2],
}

/// One node of the glare's pyramid: the glaring stars of a square of a face's texels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlareNode {
    /// Its stars' photopic-weighted mean direction, at which it is taken whole: a lone star's own
    /// direction, bit for bit.
    centre: UnitVector,
    /// Its stars' photopic and scotopic illuminance at the eye, lux, summed.
    light: [f64; 2],
    /// The cosine between a texel's centre and [`centre`](Self::centre) at or below which the node
    /// lies wholly behind the eye's plane, d − r ≥ 90°: −sin r, r its radius (below every cosine
    /// for r ≥ 90°).
    behind_at: f64,
    /// The cosine at or below which it is taken whole, r ≤ 0.25 d and d − r ≥ 4°: cos max(4 r, r +
    /// 4°) (below every cosine where that reaches 180°).
    whole_at: f64,
    /// The texels it covers.
    square: Square,
    holds: NodeHolds,
}

impl GlareNode {
    /// The node of radius `radius_deg` about `centre` (the angle from it within which its stars
    /// lie, degrees), with its rule's two cosines.
    #[must_use]
    fn new(
        centre: UnitVector,
        light: [f64; 2],
        radius_deg: f64,
        square: Square,
        holds: NodeHolds,
    ) -> Self {
        let behind_at = if radius_deg < 90.0 {
            -math::sin(radius_deg * RADIANS_PER_DEGREE)
        } else {
            f64::NEG_INFINITY
        };
        let whole_from_deg = (radius_deg / OPENING_ANGLE).max(radius_deg + NEAR_FIELD_DEG);
        let whole_at = if whole_from_deg < 180.0 {
            math::cos(whole_from_deg * RADIANS_PER_DEGREE)
        } else {
            f64::NEG_INFINITY
        };
        Self {
            centre,
            light,
            behind_at,
            whole_at,
            square,
            holds,
        }
    }
}

/// A node while the pyramid is built: its square, its index, its stars' photopic-weighted sum of
/// directions (lux) and its radius, degrees.
#[derive(Debug, Clone, Copy)]
struct Built {
    square: Square,
    index: u32,
    weighted: [f64; 3],
    radius_deg: f64,
}

/// The glare's pyramid over a band's texels (R06.T9.i; decided 2026-10-06,
/// `decision-r06-t9c-glare.md`, item 2): per face, from its leaves, the band's texels, up to one
/// node a face, each node of a square of texels holding its glaring stars' light, their mean
/// direction and their radius about it.
#[derive(Debug, Clone, PartialEq)]
struct Pyramid {
    /// The side of the band's faces, texels: the leaves'.
    face_texels: u16,
    nodes: Vec<GlareNode>,
    /// Each face's root, in [`CubeFace::ALL`]'s order, for the faces that hold a glaring star.
    roots: Vec<u32>,
    /// The internal nodes' children.
    links: Vec<u32>,
    /// The glaring stars, leaf by leaf.
    stars: Vec<GlareLight>,
}

impl Pyramid {
    /// The pyramid of `sources` over the texels of a band of `spec`. A star glares where it has a
    /// direction and light; its leaf is the texel its direction falls in ([`BandSpec::texel_of`]),
    /// as [`eye_offsets`] finds it.
    ///
    /// Each leaf keeps its stars in the census's order. Each node's sums run over its stars or its
    /// children in a fixed order, so the pyramid is a function of `sources` and the band alone.
    #[must_use]
    fn build(spec: BandSpec, sources: &[GlareSource]) -> Self {
        let mut placed: Vec<(Square, GlareLight)> = sources
            .iter()
            .filter_map(|source| {
                let direction = source.direction?;
                if source.photopic <= 0.0 {
                    return None;
                }
                let (face, row, column) = spec
                    .texel_of(direction.components())
                    .expect("a unit direction falls in a texel");
                let square = Square {
                    face,
                    level: 0,
                    row,
                    column,
                };
                let light = source.light();
                debug_assert!(
                    light.iter().all(|l| l.is_finite()),
                    "a star's light of {light:?} lx"
                );
                Some((square, GlareLight { direction, light }))
            })
            .collect();
        // A stable sort: each leaf keeps the census's order.
        placed.sort_by_key(|(square, _)| *square);
        let mut pyramid = Self {
            face_texels: spec.face_texels(),
            nodes: Vec::new(),
            roots: Vec::new(),
            links: Vec::new(),
            stars: placed.iter().map(|(_, star)| *star).collect(),
        };
        let mut level = Vec::new();
        let mut start = 0;
        for leaf in placed.chunk_by(|a, b| a.0 == b.0) {
            let end = start + leaf.len();
            level.push(pyramid.leaf(leaf[0].0, start, end));
            start = end;
        }
        let mut side = spec.face_texels();
        while side > 1 {
            let mut children = std::mem::take(&mut level);
            children.sort_by_key(|child| (child.square.parent(), child.square));
            for family in children.chunk_by(|a, b| a.square.parent() == b.square.parent()) {
                level.push(pyramid.parent(family));
            }
            side = side.div_ceil(2);
        }
        pyramid.roots = level.iter().map(|root| root.index).collect();
        pyramid
    }

    /// Adds the leaf of `square` holding the stars `start..end`.
    fn leaf(&mut self, square: Square, start: usize, end: usize) -> Built {
        let members = &self.stars[start..end];
        let (mut light, mut weighted) = ([0.0, 0.0], [0.0; 3]);
        for star in members {
            light = [light[0] + star.light[0], light[1] + star.light[1]];
            let u = star.direction.components();
            weighted = [0, 1, 2].map(|k| weighted[k] + star.light[0] * u[k]);
        }
        let (centre, radius_deg) = if let [star] = members {
            (star.direction, 0.0)
        } else {
            let centre = UnitVector::from_components(weighted)
                .expect("the stars of one texel, each of positive light, have a mean direction");
            let radius = members
                .iter()
                .map(|star| angle_deg(&centre, &star.direction))
                .fold(0.0, f64::max);
            (centre, radius)
        };
        let holds = NodeHolds::Stars {
            start: u32::try_from(start).expect("under 2³² glaring stars"),
            len: u32::try_from(members.len()).expect("under 2³² glaring stars"),
        };
        self.push(
            GlareNode::new(centre, light, radius_deg, square, holds),
            weighted,
            radius_deg,
        )
    }

    /// Adds the parent of `family`, the nodes of one square's quarters, in their squares' order.
    fn parent(&mut self, family: &[Built]) -> Built {
        let start = self.links.len();
        self.links.extend(family.iter().map(|child| child.index));
        let (mut light, mut weighted) = ([0.0, 0.0], [0.0; 3]);
        for child in family {
            let node = &self.nodes[at(child.index)];
            light = [light[0] + node.light[0], light[1] + node.light[1]];
            weighted = [0, 1, 2].map(|k| weighted[k] + child.weighted[k]);
        }
        let (centre, radius_deg) = if let [only] = family {
            (self.nodes[at(only.index)].centre, only.radius_deg)
        } else {
            let centre = UnitVector::from_components(weighted).expect(
                "the stars of one face, each of positive light and within 55° of its axis, have a \
                 mean direction",
            );
            let radius = family
                .iter()
                .map(|child| {
                    angle_deg(&centre, &self.nodes[at(child.index)].centre) + child.radius_deg
                })
                .fold(0.0, f64::max);
            (centre, radius)
        };
        let holds = NodeHolds::Children {
            start: u32::try_from(start).expect("under 2³² nodes"),
            len: u32::try_from(family.len()).expect("at most four quarters"),
        };
        let square = family[0].square.parent();
        self.push(
            GlareNode::new(centre, light, radius_deg, square, holds),
            weighted,
            radius_deg,
        )
    }

    /// Adds `node`, of the photopic-weighted sum of directions `weighted` and radius `radius_deg`.
    fn push(&mut self, node: GlareNode, weighted: [f64; 3], radius_deg: f64) -> Built {
        let index = u32::try_from(self.nodes.len()).expect("under 2³² nodes");
        let square = node.square;
        self.nodes.push(node);
        Built {
            square,
            index,
            weighted,
            radius_deg,
        }
    }

    /// The photopic and scotopic veil over the texel `texel` (its face, row and column), whose
    /// centre lies in direction `toward`, cd m⁻² (the [`Glare`] documentation): the roots in face
    /// order, each node's children in their squares' order, each opened leaf's stars in the
    /// census's order. `stack` is the traversal's, its contents discarded.
    #[must_use]
    fn veil(
        &self,
        eye: &EyeObserver,
        toward: &UnitVector,
        texel: (CubeFace, u16, u16),
        opening: Opening,
        stack: &mut Vec<u32>,
        tally: &mut impl Tally,
    ) -> [f64; 2] {
        let (face, row, column) = texel;
        let (mut photopic, mut scotopic) = (0.0, 0.0);
        stack.clear();
        stack.extend(self.roots.iter().rev());
        while let Some(index) = stack.pop() {
            let node = &self.nodes[at(index)];
            tally.visit();
            // A texel always opens its own leaf, and so every node above it, whatever the rule's
            // radius and angle: the star's own term is then the term `eye_offsets` subtracts.
            if opening == Opening::Pyramid && !node.square.holds(face, row, column) {
                let cos = toward.dot(&node.centre);
                if cos <= node.behind_at {
                    continue;
                }
                if cos <= node.whole_at {
                    tally.whole();
                    if let Some(per_lux) = veil_per_lux_at(eye, cos) {
                        photopic += node.light[0] * per_lux;
                        scotopic += node.light[1] * per_lux;
                    }
                    continue;
                }
            }
            match node.holds {
                NodeHolds::Children { start, len } => {
                    stack.extend(self.links[at(start)..at(start + len)].iter().rev());
                }
                NodeHolds::Stars { start, len } => {
                    for star in &self.stars[at(start)..at(start + len)] {
                        tally.pair();
                        if let Some([p, s]) = veil_of(eye, toward, &star.direction, star.light) {
                            photopic += p;
                            scotopic += s;
                        }
                    }
                }
            }
        }
        [photopic, scotopic]
    }
}

/// The glare of a census's listed stars as the limit map reads it (Design note 4): each star's
/// direction from the observer, and its photopic and scotopic illuminance at the eye after its own
/// reddening, summed over a pyramid of the band's texels (R06.T9.i). Only the stars at or brighter
/// than the eye's cut glare (R06.T9.j).
///
/// A request builds it once from its census, for its band, and every job of the band's rows reads
/// it: each star's reddening is resolved here, once. It holds one entry a listed star, in the
/// census's order, which [`eye_offsets`] keeps. The default holds no star, as a band's limits
/// against its own light alone (the eye cut's pre-pass, R06.T9.d) are, and serves a band of any
/// size.
///
/// **The pyramid** (decided 2026-10-06, `decision-r06-t9c-glare.md`, item 2). Per face, a pyramid
/// of the band's own texels runs from its leaves, the texels, up to one node a face, halving each
/// square of texels at each level. Each node keeps its glaring stars' photopic and scotopic
/// illuminance, their photopic-weighted mean direction (a lone star's own) and an angular radius r
/// within which they lie about it. Over each texel, a node at angle d from the texel's centre,
/// measured to its mean direction:
///
/// - is skipped when d − r ≥ 90°, since every star it holds is then behind the eye's plane;
/// - is taken whole when r ≤ 0.25 d and d − r ≥ 4°: its illuminance times the veil per lux in the
///   plane of the eye at its mean direction;
/// - is otherwise opened, and an opened leaf is summed star by star.
///
/// A node's scotopic light is taken whole at its photopic mean direction too. About that
/// direction the photopic sum's first-order error vanishes, but the scotopic sum keeps one, in the
/// spread of the stars' ρ within the node. The tests bound both together.
///
/// A texel always opens its own leaf, so the term [`eye_offsets`] subtracts for a star is exactly
/// the term the map added. The traversal and the sums run in a fixed order, so each texel's veil
/// is a function of its own direction and the census alone, and any split of rows gives the same
/// bits. Against the exact sum, every star summed one by one, each texel's limit is within 0.001
/// mag, at 64² near the Sun and on a synthetic sky of 300,000 stars to V 10.06, which the pyramid
/// sums in about a three-hundredth of the exact sum's evaluations (R06's Risks, "Deviations in
/// T9.i, as built").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glare {
    sources: Vec<GlareSource>,
    /// The pyramid the veil is summed over, for the band it was built for: `None` for the default,
    /// which holds no star.
    pyramid: Option<Pyramid>,
    /// The eye's cut its stars glare to, where it was built of a census's listed stars (R06.T9.j):
    /// a band whose texels hold the light fainter than an eye's cut must be of this one.
    eye_cut: Option<Magnitudes>,
}

impl Glare {
    /// The glare of the stars `listed` (a [`SkyCensus`](super::census::SkyCensus)'s listed stars,
    /// not its overflow, whose light the band holds) as `observer` sees them, over the texels of a
    /// band of `spec`, the band whose limits it sets, for an eye of cut `eye_cut`
    /// ([`SkyQuery::eye_cut`](super::census::SkyQuery::eye_cut)).
    ///
    /// Each star's photopic illuminance is its unextinguished light (its V less its own V band's
    /// extinction, R06.T8.k) through its own
    /// [`StarColour::reddened`](super::colour::StarColour::reddened) at its extinction, as the
    /// band's overflow points take it, and its scotopic light that times the reddened S/P ratio. A
    /// star at the observer's own position, which has no direction, adds no glare.
    ///
    /// Only the stars the eye's own request lists glare (R06.T9.j; decided 2026-10-06,
    /// `decision-r06-t9c-glare.md`): those at or brighter than `eye_cut`, the census's boundary
    /// (R06.T8.k). A star fainter than it, which a camera's deeper cut lists, keeps its entry with
    /// no light: it glares nothing, since its light is in the eye's background already as expected
    /// light, and its eye offset is its colour offset alone. So the glare of a census at a camera's
    /// cut is, bit for bit, that of the eye-only census of the same radius, whose stars are its
    /// first ones. At the eye's own cut every listed star glares.
    ///
    /// Each leaf of the pyramid sums its stars in the census's order, so `listed` is the census's
    /// own, by [`sky_order`](super::census::sky_order), as
    /// [`SkyCensus::listed`](super::census::SkyCensus::listed) gives it.
    ///
    /// # Panics
    ///
    /// In debug builds, if `listed` is not in that order.
    #[must_use]
    pub fn of_listed(
        observer: &Observer,
        listed: &[SkyStar],
        spec: &BandSpec,
        eye_cut: Magnitudes,
    ) -> Self {
        debug_assert!(
            listed
                .windows(2)
                .all(|pair| sky_order(&pair[0], &pair[1]).is_lt()),
            "the listed stars in the census's order"
        );
        let origin = observer.position();
        let sources = listed
            .iter()
            .map(|star| {
                let direction =
                    UnitVector::from_components(origin.displacement_to(star.apparent()).metres());
                let reddened = star.colour().reddened(star.a_v());
                let photopic = if star.v() <= eye_cut {
                    unextinguished_lux(star.colour(), star.v(), &reddened)
                        * reddened.photopic_transmission()
                } else {
                    0.0
                };
                GlareSource {
                    direction,
                    photopic,
                    scotopic: photopic * reddened.sp_ratio(),
                    sp_ratio: reddened.sp_ratio(),
                }
            })
            .collect();
        Self {
            eye_cut: Some(eye_cut),
            ..Self::of_sources(sources, *spec)
        }
    }

    /// The glare of point sources `points`, each its direction from the observer, its photopic
    /// illuminance at the eye and its S/P ratio, over the texels of a band of `spec`: a synthetic
    /// sky's, as the limit map's bench takes 300,000 stars (`sky/limit_map`). The sources are
    /// summed in their order within each leaf, as [`of_listed`](Self::of_listed)'s stars are.
    ///
    /// # Panics
    ///
    /// If an illuminance is negative or not finite.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::coords::UnitVector;
    /// use hyperion_sim::sky::band::BandSpec;
    /// use hyperion_sim::sky::eye::{SpRatio, illuminance_of_magnitude};
    /// use hyperion_sim::sky::limits::Glare;
    /// use hyperion_sim::units::Magnitudes;
    ///
    /// // A V 0 star towards the north galactic pole and a V 1 star along +x, both of the
    /// // reference colour.
    /// let rho = SpRatio::REFERENCE;
    /// let points = [
    ///     (UnitVector::NORTH, illuminance_of_magnitude(Magnitudes::new(0.0)), rho),
    ///     (UnitVector::X, illuminance_of_magnitude(Magnitudes::new(1.0)), rho),
    /// ];
    /// let glare = Glare::of_points(points, &BandSpec::STANDARD);
    /// assert_eq!(glare.len(), 2);
    /// ```
    #[must_use]
    pub fn of_points(
        points: impl IntoIterator<Item = (UnitVector, Lux, SpRatio)>,
        spec: &BandSpec,
    ) -> Self {
        let sources = points
            .into_iter()
            .map(|(direction, illuminance, sp_ratio)| {
                let photopic = illuminance.value();
                assert!(
                    photopic.is_finite() && photopic >= 0.0,
                    "a glaring source of {photopic} lx"
                );
                GlareSource {
                    direction: Some(direction),
                    photopic,
                    scotopic: photopic * sp_ratio.value(),
                    sp_ratio: sp_ratio.value(),
                }
            })
            .collect();
        Self::of_sources(sources, *spec)
    }

    /// The glare of `sources` over the texels of a band of `spec`.
    #[must_use]
    fn of_sources(sources: Vec<GlareSource>, spec: BandSpec) -> Self {
        let pyramid = Pyramid::build(spec, &sources);
        Self {
            sources,
            pyramid: Some(pyramid),
            eye_cut: None,
        }
    }

    /// The number of listed stars it holds, one a star, whether or not each glares.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether it holds no star.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Panics unless it was built for a band of `spec`'s faces, or holds no pyramid.
    fn assert_for(&self, spec: BandSpec) {
        if let Some(pyramid) = &self.pyramid {
            assert_eq!(
                pyramid.face_texels,
                spec.face_texels(),
                "a glare built for faces of {0}² texels read on a band of {1}² texels",
                pyramid.face_texels,
                spec.face_texels()
            );
        }
    }

    /// The photopic and scotopic veiling luminance over the texel `texel` (its face, row and
    /// column) in direction `toward`, cd m⁻² ([`Pyramid::veil`]): none for the default.
    #[must_use]
    fn veil(
        &self,
        eye: &EyeObserver,
        toward: &UnitVector,
        texel: (CubeFace, u16, u16),
        opening: Opening,
        stack: &mut Vec<u32>,
        tally: &mut impl Tally,
    ) -> [f64; 2] {
        self.pyramid.as_ref().map_or([0.0, 0.0], |pyramid| {
            pyramid.veil(eye, toward, texel, opening, stack, tally)
        })
    }
}

/// The background the eye sees over `texel` with the veil `[photopic, scotopic]` over it, cd m⁻²
/// (the [module](self) documentation): the light fainter than the eye's cut
/// ([`BandTexel::eye_background`], the texel's own unless a camera's deeper cut set the band's,
/// R06.T9.j) and the veil. With no veil it is that light's luminance and ρ, bit for bit.
#[must_use]
fn background(texel: &BandTexel, veil: [f64; 2]) -> SkyBackground {
    let [photopic_veil, scotopic_veil] = veil;
    let (light, sp_ratio) = texel.eye_background();
    let light = light.value();
    debug_assert!(
        light.is_finite() && sp_ratio.is_finite(),
        "a band texel's eye light {light} and ratio {sp_ratio}"
    );
    debug_assert!(
        photopic_veil.is_finite() && scotopic_veil.is_finite(),
        "a veil of {veil:?}"
    );
    let (luminance, sp_ratio) = if photopic_veil > 0.0 {
        let luminance = light + photopic_veil;
        (luminance, (light * sp_ratio + scotopic_veil) / luminance)
    } else {
        (light, sp_ratio)
    };
    SkyBackground::new(
        CandelasPerSquareMetre::new(luminance.min(SkyBackground::MAX_LUMINANCE.value())),
        held_ratio(sp_ratio),
    )
    .expect("a non-negative luminance held at the brightest accepted")
}

/// The ratio `sp_ratio` held within [`SpRatio`]'s 0.01–100, which no starlight leaves.
#[must_use]
fn held_ratio(sp_ratio: f64) -> SpRatio {
    SpRatio::new(sp_ratio.clamp(SpRatio::MIN.value(), SpRatio::MAX.value()))
        .expect("a finite ratio clamped into the accepted range")
}

/// The eye's limit against `texel`'s light and the veil `[photopic, scotopic]` over it, cd m⁻²
/// (the [module](self) documentation).
#[must_use]
fn texel_limit(eye: &EyeObserver, texel: &BandTexel, veil: [f64; 2]) -> Magnitudes {
    naked_eye_limit(eye, &background(texel, veil))
}

/// Sets the eye's limit of each texel of rows `rows` (from the top) of `face`, `texels` in
/// [`band_rows`](super::band::band_rows)' order (row by row, each from its left): Crumey's
/// threshold for `eye` against the texel's light fainter than the eye's cut (its own, or the eye's
/// that the march kept beside it under a camera's deeper cut, R06.T9.j) and the veiling glare
/// `glare` over it (Design note 4; the [module](self) documentation), summed over the glare's
/// pyramid ([`Glare`], R06.T9.i). Each texel keeps the veil, photopic and scotopic, beside its
/// limit, for [`eye_offsets`].
///
/// Under a camera's deeper cut the map is so the eye-only request's at the camera's census radii
/// (the same replies and `n_max`), bit for bit. Beyond the eye-only request's own caps the camera's census lists the real stars
/// brighter than the eye's cut, under one expected a layer, which glare here where the eye-only
/// request holds their expected light (decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum
/// 2).
///
/// A server runs it on each job's rows of the band; each texel's limit is a function of its own
/// light, its direction and the glare alone, the pyramid traversed in a fixed order, so any split
/// of a face's rows gives the same bits. The texels' light, chroma and ρ are not changed.
///
/// An S/P ratio of the band and the veils together outside 0.01–100, which no starlight reaches, is
/// read at the nearer bound, and a background brighter than 10¹² cd m⁻² at that.
///
/// # Panics
///
/// If `rows` reaches past the face's last row, `texels` is not those rows' texels in number, or
/// `glare` was built for a band of faces of another size; and in debug builds, if a texel's light
/// or the veil over it is not finite, which no band or census gives, or if a texel holds the light
/// fainter than an eye's cut other than the one `glare`'s stars glare to (R06.T9.j), which would
/// count the stars between the two twice or not at all.
pub fn limit_rows(
    eye: &EyeObserver,
    spec: &BandSpec,
    glare: &Glare,
    face: CubeFace,
    rows: Range<u16>,
    texels: &mut [BandTexel],
) {
    set_limits(
        eye,
        *spec,
        glare,
        face,
        rows,
        texels,
        Opening::Pyramid,
        &mut (),
    );
}

/// [`limit_rows`], with the pyramid opened by `opening`, its evaluations counted in `tally`.
#[expect(
    clippy::too_many_arguments,
    reason = "limit_rows' six, the opening and the tally"
)]
fn set_limits(
    eye: &EyeObserver,
    spec: BandSpec,
    glare: &Glare,
    face: CubeFace,
    rows: Range<u16>,
    texels: &mut [BandTexel],
    opening: Opening,
    tally: &mut impl Tally,
) {
    let side = spec.face_texels();
    assert!(
        rows.end <= side,
        "rows {rows:?} reach past a face of {side} rows"
    );
    assert_eq!(
        texels.len(),
        rows.len() * usize::from(side),
        "texels for rows {rows:?} of a face of {side}² texels"
    );
    glare.assert_for(spec);
    let mut stack = Vec::new();
    let places = rows.flat_map(|row| (0..side).map(move |column| (row, column)));
    for (texel, (row, column)) in texels.iter_mut().zip(places) {
        debug_assert!(
            texel
                .eye_light_cut()
                .is_none_or(|cut| glare.eye_cut.is_none_or(|glares_to| glares_to == cut)),
            "a texel of the light fainter than V {:?} under a glare to V {:?}",
            texel.eye_light_cut(),
            glare.eye_cut
        );
        let toward = spec.texel_direction(face, row, column);
        let veil = glare.veil(
            eye,
            &toward,
            (face, row, column),
            opening,
            &mut stack,
            tally,
        );
        let limit = texel_limit(eye, texel, veil);
        texel.set_eye_limit(limit, veil);
    }
}

/// Sets the eye's limit of every texel of a band, `band` its six faces in [`CubeFace::ALL`]'s order,
/// each face's texels in [`band_rows`](super::band::band_rows)' order: [`limit_rows`] over each
/// face.
///
/// # Panics
///
/// If `band` is not six faces of `spec`'s texels in number, or as [`limit_rows`] panics.
///
/// # Examples
///
/// The eye's limits near the Sun against the band of the stars fainter than V 8.15, about the
/// eye's cut there, with no listed star's glare (`no_run`: the tables take a minute or more to
/// build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::EyeObserver;
/// use hyperion_sim::sky::limits::{Glare, limit_map};
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let query = SkyQuery::builder(observer, Magnitudes::new(8.15)).build()?;
/// let spec = BandSpec::new(16, 12).ok_or("a band")?;
/// let mut band = Vec::new();
/// for face in CubeFace::ALL {
///     band_rows(&galaxy, &mut ctx, &query, &SkyCensus::empty(), &CompleteTo::everywhere(),
///         &spec, face, 0..16, &mut band);
/// }
/// limit_map(&EyeObserver::default(), &spec, &Glare::default(), &mut band);
/// let darkest = band.iter().filter_map(|t| t.eye_limit()).fold(f64::MIN, |a, m| a.max(m.value()));
/// assert!(darkest > 7.0, "the darkest sky near the Sun shows stars fainter than V 7");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn limit_map(eye: &EyeObserver, spec: &BandSpec, glare: &Glare, band: &mut [BandTexel]) {
    map_limits(eye, *spec, glare, band, Opening::Pyramid, &mut ());
}

/// [`limit_map`], with the pyramid opened by `opening`, its evaluations counted in `tally`.
fn map_limits(
    eye: &EyeObserver,
    spec: BandSpec,
    glare: &Glare,
    band: &mut [BandTexel],
    opening: Opening,
    tally: &mut impl Tally,
) {
    let side = spec.face_texels();
    let face_len = usize::from(side) * usize::from(side);
    assert_eq!(
        band.len(),
        CubeFace::ALL.len() * face_len,
        "a band of six faces of {side}² texels"
    );
    for (face, texels) in CubeFace::ALL
        .into_iter()
        .zip(band.chunks_exact_mut(face_len))
    {
        set_limits(eye, spec, glare, face, 0..side, texels, opening, tally);
    }
}

/// One listed star's eye offset in its two parts, mag (R06.T9.h).
#[derive(Debug, Clone, Copy, PartialEq)]
struct EyeOffsetParts {
    /// Its own limit before its colour offset, `naked_eye_limit` at its texel's background less its
    /// own veil, less its texel's limit.
    self_exclusion: Magnitudes,
    /// Its colour offset against its texel's background less its own veil.
    colour: Magnitudes,
}

impl EyeOffsetParts {
    /// The eye offset, the two parts' sum.
    #[must_use]
    fn total(self) -> Magnitudes {
        self.self_exclusion + self.colour
    }
}

/// A background of no light, against which a colour offset is the scotopic 2.5 log₁₀(ρ★ ÷ 2.297).
#[must_use]
fn scotopic_sky() -> SkyBackground {
    SkyBackground::new(CandelasPerSquareMetre::ZERO, SpRatio::REFERENCE)
        .expect("no light is a valid background")
}

/// The eye offset of `source`, in its two parts, against the limits [`limit_map`] set in `band`
/// (the [`eye_offsets`] documentation).
///
/// # Panics
///
/// If the source's texel has no eye limit; in debug builds, if its own veil exceeds the veil its
/// texel's limit was taken against.
#[must_use]
fn eye_offset_parts(
    eye: &EyeObserver,
    spec: BandSpec,
    source: &GlareSource,
    band: &[BandTexel],
) -> EyeOffsetParts {
    let rho = held_ratio(source.sp_ratio);
    let Some(direction) = source.direction else {
        return EyeOffsetParts {
            self_exclusion: Magnitudes::ZERO,
            colour: star_colour_offset(rho, &scotopic_sky()),
        };
    };
    let (face, row, column) = spec
        .texel_of(direction.components())
        .expect("a unit direction falls in a texel");
    let side = usize::from(spec.face_texels());
    let texel =
        &band[(usize::from(face.layer()) * side + usize::from(row)) * side + usize::from(column)];
    let (Some(limit), Some(veil)) = (texel.eye_limit(), texel.eye_veil()) else {
        panic!("the limit map sets every texel's eye limit before the eye offsets are read");
    };
    let own = source
        .veil(eye, &spec.texel_direction(face, row, column))
        .unwrap_or([0.0, 0.0]);
    // The texel's veil is a sum of non-negative terms that holds this one, bit for bit, since a
    // texel always opens its own leaf of the pyramid. So in floating point it is at least this
    // term, and each difference is at least zero.
    debug_assert!(
        own[0] <= veil[0] && own[1] <= veil[1],
        "a star's own veil {own:?} within its texel's {veil:?}: the band's limits are this glare's"
    );
    let background = background(texel, [veil[0] - own[0], veil[1] - own[1]]);
    EyeOffsetParts {
        self_exclusion: naked_eye_limit(eye, &background) - limit,
        colour: star_colour_offset(rho, &background),
    }
}

/// Each listed star's eye offset, its own eye limit less its texel's, mag: one a star of `glare`,
/// in the census's order (R06.T9.h; Design notes 4 and 17). `band` is the band whose limits
/// [`limit_map`] set for `eye`, `spec` and `glare`, its six faces in [`CubeFace::ALL`]'s order.
///
/// A star's own veil is not its own background (the [module](self) documentation). A star's texel
/// is the one its direction falls in ([`BandSpec::texel_of`]), and its own limit is
/// [`naked_eye_limit`] against that texel's background less the veil the map added for it, at its
/// angle from the texel's centre (the same term, bit for bit, since a texel always opens its own
/// leaf of the glare's pyramid, R06.T9.i), plus its [`star_colour_offset`] at its reddened ρ
/// against that background. Its offset is that less the texel's limit, the sum of:
///
/// - its self-exclusion, its own limit before the colour offset less the texel's limit, which is
///   never negative in a scotopic texel;
/// - its colour offset, Design note 3's 2.5 log₁₀(ρ★ ÷ 2.297) in a scotopic texel, fading by MES2's
///   photopic weight in a mesopic one.
///
/// A star that adds no veil takes its colour offset alone, as a star fainter than the eye's cut
/// does, which glares nothing (R06.T9.j); a star at the observer's own position, which has no
/// direction, takes it against a scotopic sky. The texels' limits do not change.
///
/// The field factor cancels: it moves both limits by −2.5 log₁₀ F, so a view's own factor, applied
/// as that offset to every texel, leaves each star's own limit exact. The wire quantises the offset
/// once, to `i8` centimagnitudes, saturating at −1.28 and +1.27 (Design note 17). A star's V less
/// its texel's limit rises with its V, so the +1.27 culls a star the offset would keep only when a
/// star at its own limit lies more than 1.27 fainter than its texel's limit. At the default eye (F
/// 1.4, 25 years) none does: at Crumey's darkest-sky clamp, where the self-exclusion is largest,
/// the colour table's hottest star close to its texel's centre has an offset of up to 1.45, but a
/// star that bright lies only its colour offset fainter than its texel's limit, and a fainter one
/// at its own limit 1.10 fainter. From F ≈ 2.2 (2.0 at 80 years) the saturation takes up to 0.05
/// mag of such a star's window at Crumey's F 2.4, and 0.12 at 80 years with light eyes, within the
/// 0.16 ruled (R06's Risks, "Deviations in T9.h, as built").
///
/// # Panics
///
/// If `band` is not six faces of `spec`'s texels in number, `glare` was built for a band of faces
/// of another size, or a star's texel has no eye limit (the limit map was not run on `band`); and
/// in debug builds, if a star's own veil exceeds its texel's, as a band whose limits were set
/// against another glare can give.
///
/// # Examples
///
/// The eye offsets of the stars within 30 ly of the Sun brighter than V 8.15, about the eye's cut
/// there, against the band of the rest: a view sees a star whose V is brighter than its
/// texel's limit plus its offset, its own limit (`no_run`: the tables take a minute or more to
/// build):
///
/// ```no_run
/// use core::num::NonZeroU32;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, UnitVector};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
/// use hyperion_sim::sky::census::{
///     CellOffsets, CensusTallies, MAX_N_MAX, NoSkyCellCache, SkyContext, SkyQuery, census_cell,
///     census_plan, merge_census,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::EyeObserver;
/// use hyperion_sim::sky::limits::{Glare, eye_offsets, limit_map};
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{LightYears, Magnitudes};
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let query = SkyQuery::builder(observer, Magnitudes::new(8.15))
///     .eye(EyeObserver::default())
///     .build()?
///     .with_caps_forced(LightYears::new(30.0))?;
/// let plan = census_plan(&galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise);
/// let mut stars = Vec::new();
/// for key in plan.cells() {
///     census_cell(&galaxy, &mut ctx, key, &query, &mut stars);
/// }
/// let all = NonZeroU32::new(MAX_N_MAX).ok_or("not zero")?;
/// let census = merge_census([(stars, CensusTallies::default())], all);
/// let listed = census.listed();
/// let spec = BandSpec::new(16, 12).ok_or("a band")?;
/// let complete_to = CompleteTo::of_caps(plan.caps());
/// let mut band = Vec::new();
/// for face in CubeFace::ALL {
///     band_rows(&galaxy, &mut ctx, &query, &census, &complete_to, &spec, face, 0..16, &mut band);
/// }
/// let eye = *query.eye().ok_or("the eye")?;
/// let glare = Glare::of_listed(&observer, listed, &spec, query.eye_cut().ok_or("the eye's cut")?);
/// limit_map(&eye, &spec, &glare, &mut band);
/// let n = usize::from(spec.face_texels());
/// for (star, offset) in listed.iter().zip(eye_offsets(&eye, &spec, &glare, &band)) {
///     // The texel `eye_offsets` reads: the one the star's unit direction falls in.
///     let toward = observer.position().displacement_to(star.apparent()).metres();
///     let toward = UnitVector::from_components(toward).ok_or("a star away from the observer")?;
///     let (face, row, column) = spec.texel_of(toward.components()).ok_or("a direction")?;
///     let index = (usize::from(face.layer()) * n + usize::from(row)) * n + usize::from(column);
///     let own = band[index].eye_limit().ok_or("the map's limit")? + offset;
///     println!("V {:.2}, seen to {:.2}: {}", star.v().value(), own.value(), star.v() < own);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn eye_offsets(
    eye: &EyeObserver,
    spec: &BandSpec,
    glare: &Glare,
    band: &[BandTexel],
) -> Vec<Magnitudes> {
    let side = spec.face_texels();
    assert_eq!(
        band.len(),
        CubeFace::ALL.len() * usize::from(side) * usize::from(side),
        "a band of six faces of {side}² texels"
    );
    glare.assert_for(*spec);
    glare
        .sources
        .iter()
        .map(|source| eye_offset_parts(eye, *spec, source, band).total())
        .collect()
}

/// The eye cut's pre-pass band (Design note 5): 16² texels a face, 7.1° wide at the faces' centres
/// and 5.2° on average, on the server's nodes a decade.
const PRE_PASS_SPEC: BandSpec = BandSpec::new(16, BandSpec::STANDARD.nodes_per_decade())
    .expect("16² faces on the standard nodes are a band");

/// The pre-pass's provisional cut, V 7.85 (Design note 5): its band holds the light of the stars
/// fainter than it, until the cut it gives is known.
const PROVISIONAL_CUT_V: f64 = 7.85;

/// The eye cut's pad, mag (Design note 5). It covers a texel of the server's 64² band darker than
/// the pre-pass's darkest 16² texel, and the final band, at the cut, a little darker than the
/// repeat's, at the cut before it. The colour offset is the table's own largest, so the pad keeps
/// all of its 0.1 for these.
const CUT_PAD_MAG: f64 = 0.1;

/// The largest eye colour offset any star has, mag: [`star_colour_offset`] of the colour table's
/// largest S/P ratio in a scotopic sky, where every offset is largest, since in a mesopic one each
/// fades towards zero (Design note 3), and dust only lowers a star's ρ.
///
/// It is +0.4526, from the 500,000 K blackbody rows' ρ 3.4850. This is the table's own maximum,
/// which the orchestrator ruled on 2026-10-06 (T9.h's open question 1). Design note 5 first
/// gave it as 0.43, and the cut as +0.45.
#[must_use]
fn largest_colour_offset() -> Magnitudes {
    star_colour_offset(held_ratio(largest_sp_ratio()), &scotopic_sky())
}

/// One run of the eye cut's pre-pass (Design note 5).
#[derive(Debug, Clone, Copy, PartialEq)]
struct PrePass {
    /// Its darkest texel's limit, V.
    darkest: Magnitudes,
    /// The cut it gives, V: `darkest` plus [`largest_colour_offset`] plus [`CUT_PAD_MAG`], held at
    /// [`MAX_CUT_V`].
    cut: Magnitudes,
}

/// The eye cut's pre-pass at [`PROVISIONAL_CUT_V`], and its one repeat at the cut that gave, when
/// that is deeper.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EyeCutPasses {
    first: PrePass,
    repeat: Option<PrePass>,
}

impl EyeCutPasses {
    /// The eye's cut: the repeat's, or the first pass's where there was none.
    #[must_use]
    fn cut(&self) -> Magnitudes {
        self.repeat.unwrap_or(self.first).cut
    }
}

/// The pre-pass's band for `observer` and `eye` at `band_cut`, its six faces in
/// [`CubeFace::ALL`]'s order, with their limits against no glare: [`band_rows`] at
/// [`PRE_PASS_SPEC`] with no census and complete everywhere, then [`limit_map`].
#[must_use]
fn pre_pass_band(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    observer: &Observer,
    eye: &EyeObserver,
    band_cut: Magnitudes,
) -> Vec<BandTexel> {
    let query = SkyQuery::builder(*observer, band_cut)
        .build()
        .expect("a finite cut held at V 11 is a query");
    let side = PRE_PASS_SPEC.face_texels();
    let mut band = Vec::with_capacity(CubeFace::ALL.len() * usize::from(side) * usize::from(side));
    for face in CubeFace::ALL {
        band_rows(
            galaxy,
            ctx,
            &query,
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &PRE_PASS_SPEC,
            face,
            0..side,
            &mut band,
        );
    }
    limit_map(eye, &PRE_PASS_SPEC, &Glare::default(), &mut band);
    band
}

/// One run of the pre-pass whose band holds the light fainter than `band_cut`
/// ([`pre_pass_band`]), its cut taking `offset`, the largest colour offset.
#[must_use]
fn pre_pass(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    observer: &Observer,
    eye: &EyeObserver,
    band_cut: Magnitudes,
    offset: Magnitudes,
) -> PrePass {
    let darkest = pre_pass_band(galaxy, ctx, observer, eye, band_cut)
        .iter()
        .map(|texel| {
            texel
                .eye_limit()
                .expect("the limit map sets every texel's limit")
                .value()
        })
        // Every limit is finite and positive (eq. 34's threshold is), so the largest is exact
        // whatever the order: no NaN and no signed zero reach the fold.
        .fold(f64::NEG_INFINITY, f64::max);
    PrePass {
        darkest: Magnitudes::new(darkest),
        cut: Magnitudes::new((darkest + offset.value() + CUT_PAD_MAG).min(MAX_CUT_V)),
    }
}

/// The pre-pass and its repeat ([`eye_cut`]).
#[must_use]
fn eye_cut_passes(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    observer: &Observer,
    eye: &EyeObserver,
) -> EyeCutPasses {
    let offset = largest_colour_offset();
    let provisional = Magnitudes::new(PROVISIONAL_CUT_V);
    let first = pre_pass(galaxy, ctx, observer, eye, provisional, offset);
    let repeat = (first.cut > provisional).then(|| {
        let repeat = pre_pass(galaxy, ctx, observer, eye, first.cut, offset);
        // A deeper cut only takes light from the band, which only deepens its limits (to a
        // rounding), so what one repeat leaves lies on the side the pad covers.
        debug_assert!(
            repeat.darkest.value() >= first.darkest.value() - 1e-9,
            "a deeper cut leaves the band darker: {first:?}, then {repeat:?}"
        );
        repeat
    });
    EyeCutPasses { first, repeat }
}

/// The eye's cut for `observer` and `eye`, V: the faintest V a sky with the eye lists (Design note
/// 5; R06.T9.d).
///
/// The server sets it before the census, since the limit map is computed from the census. A coarse
/// pre-pass of the band, [`band_rows`] from the luminosity tables alone at 16² texels a face, with
/// no census and complete everywhere, holds the light of the stars fainter than a provisional cut
/// of V 7.85. [`limit_map`] with no glare then gives each texel its limit, Crumey's threshold at
/// the texel's own light and ρ ([`naked_eye_limit`], which holds it at his darkest background:
/// 7.99 at F 1.4). The cut is:
///
/// - the darkest texel's limit;
/// - plus the largest eye colour offset any star has: +0.453, the colour table's 500,000 K
///   blackbody rows in a scotopic sky;
/// - plus a pad of 0.1 mag.
///
/// If that is deeper than V 7.85, the pre-pass runs once more at it, and the repeat gives the cut.
/// A deeper cut takes little light from the band, so one repeat suffices. Near the Sun the repeat
/// moves the cut by about 0.04 mag. The darkest limit moves about 0.11 mag per magnitude of cut, so
/// what is left, about 0.005, makes the final band darker, the side the pad covers.
///
/// So every star an eye view can see is listed, in the eye-only request's map, which is also the
/// eye's map under a camera's deeper cut (R06.T9.j). A star's own limit is its texel's limit in the
/// final map plus its eye offset ([`eye_offsets`], R06.T9.h). That is at most its texel's limit
/// with no glare plus its colour offset, and no texel of that map is deeper than the cut less the
/// largest colour offset. Glare is left out of the pre-pass, which is conservative, since glare
/// only makes limits shallower. The pad covers a texel of the server's 64² band darker than the
/// pre-pass's darkest, and the final band at the cut a little darker than the repeat's. R06's
/// Risks, "Deviations in T9.d, as built", records the margin near the Sun.
///
/// The cut is held at [`MAX_CUT_V`]. Only an eye of field factor under about 0.15, far keener than
/// any observer (Crumey's 1.4–2.4), would pass it.
///
/// It reads the tables, the noise and the modifiers of `ctx`, and marches two bands of 1,536 rays
/// near the Sun (one where the first cut is no deeper than V 7.85).
///
/// # Panics
///
/// In debug builds, if the repeat's darkest limit is shallower than the first pass's, or as
/// [`limit_rows`] panics on a texel's light that is not finite; no band gives either.
///
/// # Examples
///
/// The cut near the Sun for the default eye, at which the census then lists the sky (`no_run`: the
/// tables take a minute or more to build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext, SkyQuery};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::EyeObserver;
/// use hyperion_sim::sky::limits::eye_cut;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let eye = EyeObserver::default();
/// let cut = eye_cut(&galaxy, &mut ctx, &observer, &eye);
/// assert!(cut.value() > 7.85, "the darkest sky near the Sun deepens the provisional cut");
/// let query = SkyQuery::builder(observer, cut).eye(eye).build()?;
/// # let _ = query;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn eye_cut(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    observer: &Observer,
    eye: &EyeObserver,
) -> Magnitudes {
    eye_cut_passes(galaxy, ctx, observer, eye).cut()
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use core::num::NonZeroU32;

    use hyperion_testkit::float::{bits, bits_f32};
    use hyperion_testkit::golden::f64_digest;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::sky::band::{BandMarch, CompleteTo, band_rows, march_rows, sum_rows};
    use crate::sky::caps::{CAPPED_LAYERS, LayerCap};
    use crate::sky::census::{
        CensusTallies, MAX_N_MAX, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery, census_cell,
        census_plan, merge_census,
    };
    use crate::sky::colour::{AtmosphereGrid, solar_colour, star_colour};
    use crate::sky::eye::{
        BLACKWELL_SP_RATIO, DARKEST_BACKGROUND, PhotopicWeight, REFERENCE_SP_RATIO,
        illuminance_of_magnitude, luminance, mesopic_weight, surface_brightness,
    };
    use crate::sky::testing::{milky_way_envelope, milky_way_offsets, milky_way_tables, uniforms};
    use crate::time::UniverseTime;
    use crate::units::{Kelvin, LightYears, MagnitudesPerArcsec2};

    /// The Sun's place in the fixture, ly, as the sim's other sky tests stand.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// The eye's cut near the Sun, V: T9.d's reference for the real sky (decided 2026-10-06,
    /// `decision-r06-t9b-band.md`), at which T9.c's medians are ruled.
    const EYE_CUT: f64 = 8.15;

    /// How far out the glare's census looks near the Sun, ly: its brightest stars are the nearest
    /// ones, and a census this deep and wide costs seconds, not minutes.
    const GLARE_RADIUS_LY: f64 = 100.0;

    /// The ruled medians near the Sun, V, and their tolerances (`decision-r06-t9b-band.md`, items 1
    /// and 2): Crumey's limit at Gaia DR3's light fainter than the cut, μ 22.18 in the band and
    /// 24.62 at the poles, and T9.b's 0.5 mag of μ through Crumey's slope there.
    const BAND_MEDIAN: (f64, f64) = (6.5, 0.20);
    const POLES_MEDIAN: (f64, f64) = (7.55, 0.22);

    /// The eye offsets the wire holds, mag: `i8` centimagnitudes, saturating (Design note 17).
    const WIRE_MIN_EYE_OFFSET_MAG: f64 = -1.28;
    const WIRE_MAX_EYE_OFFSET_MAG: f64 = 1.27;

    fn context() -> SkyContext<'static> {
        SkyContext {
            tables: milky_way_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn observer() -> Observer {
        let at = GalacticPosition::from_light_years(SUN).expect("in the cube");
        Observer::new(at, UniverseTime::EPOCH).expect("an observer")
    }

    fn query(cut: f64) -> SkyQuery {
        SkyQuery::builder(observer(), Magnitudes::new(cut))
            .build()
            .expect("a valid query")
    }

    fn spec(face_texels: u16) -> BandSpec {
        BandSpec::new(face_texels, BandSpec::STANDARD.nodes_per_decade()).expect("a spec")
    }

    /// Rows `rows` of `face` of the band near the Sun at the eye's cut with no census, complete
    /// everywhere: the light of the stars fainter than the cut, as the final reply's band holds it
    /// to the caps.
    fn band_near_the_sun(spec: BandSpec, face: CubeFace, rows: Range<u16>) -> Vec<BandTexel> {
        let mut out = Vec::new();
        band_rows(
            milky_way_galaxy(),
            &mut context(),
            &query(EYE_CUT),
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &spec,
            face,
            rows,
            &mut out,
        );
        out
    }

    /// The whole band near the Sun at 16² faces ([`band_near_the_sun`]), as T9.b's tests and the
    /// ruling's probe take it: built once for the tests that read it.
    fn near_the_sun() -> &'static [BandTexel] {
        static BAND: OnceLock<Vec<BandTexel>> = OnceLock::new();
        BAND.get_or_init(|| {
            let spec = spec(16);
            CubeFace::ALL
                .into_iter()
                .flat_map(|face| band_near_the_sun(spec, face, 0..16))
                .collect()
        })
    }

    /// The stars a census near the Sun lists brighter than `cut` within `radius_ly`, every cap
    /// forced to it, with no eye.
    fn listed_near_the_sun(cut: f64, radius_ly: f64) -> Vec<SkyStar> {
        let galaxy = milky_way_galaxy();
        let query = query(cut)
            .with_caps_forced(LightYears::new(radius_ly))
            .expect("a forced cap");
        let mut ctx = context();
        let plan = census_plan(galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise);
        let mut stars = Vec::new();
        for key in plan.cells() {
            census_cell(galaxy, &mut ctx, key, &query, &mut stars);
        }
        let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        merge_census([(stars, CensusTallies::default())], all)
            .listed()
            .to_vec()
    }

    /// The stars a census near the Sun lists brighter than the eye's cut within
    /// [`GLARE_RADIUS_LY`] ([`listed_near_the_sun`]): built once.
    fn nearby_stars() -> &'static [SkyStar] {
        static STARS: OnceLock<Vec<SkyStar>> = OnceLock::new();
        STARS.get_or_init(|| listed_near_the_sun(EYE_CUT, GLARE_RADIUS_LY))
    }

    /// The direction `degrees` from `direction`, turned towards the galactic plane's +X or, for a
    /// direction along X, towards +Z.
    fn off(direction: UnitVector, degrees: f64) -> UnitVector {
        let d = direction.components();
        let other = if d[0].abs() > 0.9 {
            UnitVector::NORTH
        } else {
            UnitVector::X
        };
        let across =
            UnitVector::from_components(direction.cross(&other)).expect("not along the other");
        let (sin, cos) = math::sin_cos(degrees * RADIANS_PER_DEGREE);
        let a = across.components();
        UnitVector::from_components([
            cos * d[0] + sin * a[0],
            cos * d[1] + sin * a[1],
            cos * d[2] + sin * a[2],
        ])
        .expect("a direction")
    }

    /// The angle between `a` and `b`, degrees, from the sine and the cosine, independently of the
    /// map's arccosine.
    fn angle_deg(from: &UnitVector, to: &UnitVector) -> f64 {
        let sine = from.cross(to).iter().map(|c| c * c).sum::<f64>().sqrt();
        math::atan2(sine, from.dot(to)) / RADIANS_PER_DEGREE
    }

    /// A star of V `v` and colour (`teff` K, `log_g`) unreddened, as a source: its direction, its
    /// photopic illuminance, lux, and its S/P ratio.
    fn star_at(direction: UnitVector, v: f64, teff: f64, log_g: f64) -> (UnitVector, f64, f64) {
        let colour = star_colour(Kelvin::new(teff), log_g, AtmosphereGrid::MainSequence);
        (
            direction,
            illuminance_of_magnitude(Magnitudes::new(v)).value() * colour.lux_per_v0(),
            colour.sp_ratio(),
        )
    }

    /// The listed stars as sources, read through the public colour and eye functions: each one's
    /// direction from the observer, its photopic illuminance after its own reddening, lux, and its
    /// reddened S/P ratio.
    fn sources_of(stars: &[SkyStar]) -> Vec<(UnitVector, f64, f64)> {
        let origin = *observer().position();
        stars
            .iter()
            .map(|star| {
                let reddened = star.colour().reddened(star.a_v());
                // The census's V holds the star's own V extinction (R06.T8.k).
                let unextinguished = illuminance_of_magnitude(star.v() - reddened.v_extinction())
                    .value()
                    * star.colour().lux_per_v0();
                (
                    UnitVector::from_components(origin.displacement_to(star.apparent()).metres())
                        .expect("a star away from the observer"),
                    unextinguished * reddened.photopic_transmission(),
                    reddened.sp_ratio(),
                )
            })
            .collect()
    }

    /// The glare of `sources`, each its direction, its photopic illuminance, lux, and its S/P
    /// ratio, over a band of `spec`.
    fn glare_of(spec: BandSpec, sources: &[(UnitVector, f64, f64)]) -> Glare {
        let sources = sources
            .iter()
            .map(|&(u, photopic, sp_ratio)| GlareSource {
                direction: Some(u),
                photopic,
                scotopic: photopic * sp_ratio,
                sp_ratio,
            })
            .collect();
        Glare::of_sources(sources, spec)
    }

    /// The plan's definition, written out again: the background over a texel in direction
    /// `toward`, its luminance plus each source's veil, `veiling_luminance` of its illuminance in
    /// the plane of the eye, E cos θ, at its angle θ (from `atan2`) for every source within 90°,
    /// and none beyond; at the S/P ratio of the band's and the veils' light together. Source
    /// `skip`, a star's own, is left out.
    fn reference_background(
        eye: &EyeObserver,
        texel: &BandTexel,
        toward: &UnitVector,
        sources: &[(UnitVector, f64, f64)],
        skip: Option<usize>,
    ) -> SkyBackground {
        let light = texel.luminance().value();
        let (mut photopic, mut scotopic) = (light, light * texel.sp_ratio());
        for (i, (u, e, rho)) in sources.iter().enumerate() {
            let theta = angle_deg(toward, u);
            if skip == Some(i) || theta >= 90.0 {
                continue;
            }
            let (angle, in_plane) = (
                Degrees::new(theta),
                e * math::cos(theta * RADIANS_PER_DEGREE),
            );
            photopic += veiling_luminance(eye, Lux::new(in_plane), angle)
                .expect("a valid glare")
                .value();
            scotopic += veiling_luminance(eye, Lux::new(in_plane * rho), angle)
                .expect("a valid glare")
                .value();
        }
        let rho = if photopic > light {
            scotopic / photopic
        } else {
            texel.sp_ratio()
        };
        SkyBackground::new(
            CandelasPerSquareMetre::new(photopic),
            SpRatio::new(rho).expect("a starlit ratio"),
        )
        .expect("a background")
    }

    /// Each texel's direction and galactic latitude |b|, degrees, in a band's order.
    fn texel_latitudes(spec: BandSpec) -> Vec<(UnitVector, f64)> {
        let n = spec.face_texels();
        let mut out = Vec::new();
        for face in CubeFace::ALL {
            for row in 0..n {
                for column in 0..n {
                    let u = spec.texel_direction(face, row, column);
                    out.push((u, math::asin(u.components()[2]).abs() / RADIANS_PER_DEGREE));
                }
            }
        }
        out
    }

    /// The median of the eye limits of the texels whose |b| lies in `latitudes`, and their number.
    fn median_limit(band: &[BandTexel], spec: BandSpec, latitudes: Range<f64>) -> (f64, usize) {
        let mut limits: Vec<f64> = band
            .iter()
            .zip(texel_latitudes(spec))
            .filter(|(_, (_, b))| latitudes.contains(b))
            .map(|(t, _)| t.eye_limit().expect("a limit").value())
            .collect();
        limits.sort_by(f64::total_cmp);
        let n = limits.len();
        assert!(n > 0, "texels at |b| in {latitudes:?}");
        let median = if n % 2 == 1 {
            limits[n / 2]
        } else {
            f64::midpoint(limits[n / 2 - 1], limits[n / 2])
        };
        (median, n)
    }

    /// The near-Sun band with its limits set against `glare`.
    fn limited(glare: &Glare, eye: &EyeObserver) -> Vec<BandTexel> {
        let mut band = near_the_sun().to_vec();
        limit_map(eye, &spec(16), glare, &mut band);
        band
    }

    /// The near-Sun band with its limits set against `glare` by the exact sum, every node of the
    /// pyramid opened, as T9.c's and T9.h's identities take it (R06.T9.i).
    fn limited_exactly(glare: &Glare, eye: &EyeObserver) -> Vec<BandTexel> {
        let mut band = near_the_sun().to_vec();
        map_limits(eye, spec(16), glare, &mut band, Opening::Every, &mut ());
        band
    }

    /// What a traversal of the glare's pyramid evaluated (R06.T9.i).
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct Evaluations {
        /// Nodes tested.
        visits: u64,
        /// Nodes taken whole.
        wholes: u64,
        /// Star–texel pairs summed in opened leaves.
        pairs: u64,
    }

    impl Evaluations {
        /// Its evaluations of the kernel: the pairs and the nodes taken whole.
        fn evaluated(self) -> u64 {
            self.pairs + self.wholes
        }
    }

    impl Tally for Evaluations {
        fn visit(&mut self) {
            self.visits += 1;
        }
        fn whole(&mut self) {
            self.wholes += 1;
        }
        fn pair(&mut self) {
            self.pairs += 1;
        }
    }

    /// The texel of a band of `spec` that `direction` falls in: its index in the band's order and
    /// the direction of its centre.
    fn texel_of(spec: BandSpec, direction: &UnitVector) -> (usize, UnitVector) {
        let (face, row, column) = spec
            .texel_of(direction.components())
            .expect("a unit direction falls in a texel");
        let n = usize::from(spec.face_texels());
        (
            (usize::from(face.layer()) * n + usize::from(row)) * n + usize::from(column),
            spec.texel_direction(face, row, column),
        )
    }

    /// The fixture's sources: the stars the census lists within [`GLARE_RADIUS_LY`], then a V −1.5
    /// white star 0.05° from a texel's centre (read at 0.1°), and a V −9 one 0.3° from another's,
    /// which makes that texel's background mesopic.
    fn fixture_sources(spec: BandSpec) -> Vec<(UnitVector, f64, f64)> {
        let mut sources = sources_of(nearby_stars());
        sources.extend(placed_sources(spec));
        sources
    }

    /// T9.c's two placed sources at a band of `spec`: a V −1.5 white star 0.05° from the centre of
    /// texel (8, 8) of +Z, and a V −9 solar one 0.3° from that of texel (7, 7) of +X.
    fn placed_sources(spec: BandSpec) -> [(UnitVector, f64, f64); 2] {
        [
            star_at(
                off(spec.texel_direction(CubeFace::PosZ, 8, 8), 0.05),
                -1.5,
                9_940.0,
                4.3,
            ),
            star_at(
                off(spec.texel_direction(CubeFace::PosX, 7, 7), 0.3),
                -9.0,
                5_772.0,
                4.438,
            ),
        ]
    }

    /// The identity: every texel's limit is `naked_eye_limit` at the background written again from
    /// the definition, and every listed star's own limit, its texel's plus its eye offset, is
    /// `naked_eye_limit` plus `star_colour_offset` at that background without its own veil, to
    /// 10⁻⁹ mag, on the near-Sun fixture and its two placed sources. The map takes the exact sum,
    /// every node of the glare's pyramid opened (R06.T9.i).
    #[test]
    fn each_texels_limit_and_each_stars_own_are_crumeys_at_their_light_and_glare() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let sources = fixture_sources(spec);
        let glare = glare_of(spec, &sources);
        let band = limited_exactly(&glare, &eye);
        let (mut worst, mut mesopic) = (0.0_f64, 0);
        for (texel, (toward, _)) in band.iter().zip(texel_latitudes(spec)) {
            let background = reference_background(&eye, texel, &toward, &sources, None);
            if mesopic_weight(&background).value() > 0.0 {
                mesopic += 1;
            }
            let expected = naked_eye_limit(&eye, &background).value();
            let got = texel.eye_limit().expect("a limit").value();
            worst = worst.max((got - expected).abs());
        }
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(offsets.len(), sources.len());
        let mut worst_own = 0.0_f64;
        for (i, ((u, _, rho), offset)) in sources.iter().zip(&offsets).enumerate() {
            let (index, toward) = texel_of(spec, u);
            let texel = &band[index];
            let background = reference_background(&eye, texel, &toward, &sources, Some(i));
            let expected = naked_eye_limit(&eye, &background)
                + star_colour_offset(SpRatio::new(*rho).expect("a ratio"), &background);
            let got = texel.eye_limit().expect("a limit") + *offset;
            worst_own = worst_own.max((got - expected).value().abs());
        }
        eprintln!(
            "{} sources ({} listed within {GLARE_RADIUS_LY} ly): the worst texel differs from \
             the definition by {worst:.3e} mag, the worst star's own limit by {worst_own:.3e}; \
             {mesopic} mesopic texels",
            sources.len(),
            nearby_stars().len()
        );
        assert!(worst < 1e-9, "a texel differs by {worst} mag");
        assert!(
            worst_own < 1e-9,
            "a star's own limit differs by {worst_own} mag"
        );
        assert!(mesopic > 0, "the V −9 star's texel is mesopic");
    }

    /// Near the Sun, with the band at the eye's cut there (8.15), the median texel limit is 6.5 ±
    /// 0.20 in the band (|b| under 5°) and 7.55 ± 0.22 at the poles (|b| over 80°)
    /// (`decision-r06-t9b-band.md`): against the band alone, as the ruling's reference is, and with
    /// the glare of the census's nearest stars.
    #[test]
    fn near_the_sun_the_median_limits_are_crumeys_at_gaias_light_fainter_than_the_cut() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let (none, nearby) = (
            Glare::default(),
            Glare::of_listed(&observer(), nearby_stars(), &spec, Magnitudes::new(EYE_CUT)),
        );
        for (what, glare) in [("the band alone", &none), ("with the glare", &nearby)] {
            let band = limited(glare, &eye);
            let (plane, in_plane) = median_limit(&band, spec, 0.0..5.0);
            let (poles, at_poles) = median_limit(&band, spec, 80.0..90.1);
            eprintln!(
                "near the Sun at cut {EYE_CUT}, {what} ({} stars glare): the median limit is \
                 {plane:.3} over {in_plane} texels in the band and {poles:.3} over {at_poles} at \
                 the poles",
                glare.len()
            );
            assert!(
                (plane - BAND_MEDIAN.0).abs() <= BAND_MEDIAN.1,
                "{what}: the band's median limit {plane}"
            );
            assert!(
                (poles - POLES_MEDIAN.0).abs() <= POLES_MEDIAN.1,
                "{what}: the poles' median limit {poles}"
            );
        }
    }

    /// A texel whose centre lies 0.5° from a V −1.5 star (Sirius's colour) is at least 0.3 mag
    /// shallower than its eight neighbours' mean, towards the north galactic pole and in the plane,
    /// at the server's 64² faces.
    #[test]
    fn a_texel_within_1_degree_of_a_v_minus_1_5_star_is_shallower_than_its_neighbours() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let n = usize::from(spec.face_texels());
        for (face, place) in [
            (CubeFace::PosZ, "towards the north galactic pole"),
            (CubeFace::PosX, "in the plane"),
        ] {
            let rows = 31..34;
            let mut texels = band_near_the_sun(spec, face, rows.clone());
            let centre = spec.texel_direction(face, 32, 32);
            let (u, e, rho) = star_at(off(centre, 0.5), -1.5, 9_940.0, 4.3);
            assert!(angle_deg(&centre, &u) < 1.0);
            let glare = glare_of(spec, &[(u, e, rho)]);
            limit_rows(&eye, &spec, &glare, face, rows, &mut texels);
            let limit = |row: usize, column: usize| {
                texels[(row - 31) * n + column]
                    .eye_limit()
                    .expect("a limit")
                    .value()
            };
            let neighbours: Vec<f64> = (31..=33)
                .flat_map(|row| (31..=33).map(move |column| (row, column)))
                .filter(|&place| place != (32, 32))
                .map(|(row, column)| limit(row, column))
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "eight neighbours")]
            let mean = neighbours.iter().sum::<f64>() / neighbours.len() as f64;
            let own = limit(32, 32);
            eprintln!(
                "{place}: the texel 0.5° from a V −1.5 star sees to {own:.3}, its neighbours to \
                 {mean:.3} on average"
            );
            assert!(own <= mean - 0.3, "{place}: {own} against {mean}");
        }
    }

    /// The glare's colour and the band's light are the map's only inputs: a field factor F′ moves
    /// every texel's limit by −2.5 log₁₀(F′ ÷ F), as the client's `fieldFactorOffsetMag` assumes,
    /// since neither the band nor the veil depends on F; and F cancels in every eye offset, which
    /// is a difference of two limits at one F (R06.T9.h).
    #[test]
    fn a_field_factor_moves_every_limit_by_its_own_offset_and_no_eye_offset() {
        let spec = spec(16);
        let glare = glare_of(spec, &fixture_sources(spec));
        let (keen_eye, typical_eye) = (
            EyeObserver::default(),
            EyeObserver::new(2.0, 25.0, 0.5).expect("an eye"),
        );
        let keen = limited(&glare, &keen_eye);
        let typical = limited(&glare, &typical_eye);
        let offset = -2.5 * math::log10(2.0 / EyeObserver::DEFAULT_FIELD_FACTOR);
        for (a, b) in keen.iter().zip(&typical) {
            let moved = b.eye_limit().expect("a limit") - a.eye_limit().expect("a limit");
            assert!(
                (moved.value() - offset).abs() < 1e-9,
                "{} against {offset}",
                moved.value()
            );
        }
        let keen_offsets = eye_offsets(&keen_eye, &spec, &glare, &keen);
        let typical_offsets = eye_offsets(&typical_eye, &spec, &glare, &typical);
        let mut worst = 0.0_f64;
        for (a, b) in keen_offsets.iter().zip(&typical_offsets) {
            worst = worst.max((*b - *a).value().abs());
        }
        eprintln!("F = 2 against 1.4: the eye offsets differ by {worst:.3e} mag at most");
        assert!(worst < 1e-9, "an eye offset moves by {worst} mag");
    }

    /// The map is a function of the band's light and the listed stars alone: any split of a face's
    /// rows gives the same bits; the texels' chroma does not enter; with no glare each limit is
    /// `naked_eye_limit` at the texel's own light, bit for bit; and a texel with no light and no
    /// glare is seen to Crumey's clamp.
    #[test]
    fn the_map_reads_only_the_bands_light_and_the_listed_stars() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let glare = Glare::of_listed(&observer(), nearby_stars(), &spec, Magnitudes::new(EYE_CUT));
        let face = &near_the_sun()[..256];
        // Each texel's limit and the veil it keeps, which the eye offsets read.
        let limits = |texels: &[BandTexel]| -> Vec<(u64, [u64; 2])> {
            texels
                .iter()
                .map(|t| {
                    (
                        bits(t.eye_limit().expect("a limit").value()),
                        t.eye_veil().expect("a veil").map(bits),
                    )
                })
                .collect()
        };
        let mut whole = face.to_vec();
        limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..16, &mut whole);
        for split in [1_u16, 5, 15] {
            let mut parts = face.to_vec();
            let (top, bottom) = parts.split_at_mut(usize::from(split) * 16);
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, split..16, bottom);
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..split, top);
            assert_eq!(limits(&parts), limits(&whole), "split at row {split}");
        }
        // The eye offsets of a band set whole and of one set in split rows, the last rows first.
        let offsets = |band: &[BandTexel]| -> Vec<u64> {
            eye_offsets(&eye, &spec, &glare, band)
                .iter()
                .map(|o| bits(o.value()))
                .collect()
        };
        let side = usize::from(spec.face_texels());
        let mut split = near_the_sun().to_vec();
        for (face, texels) in CubeFace::ALL
            .into_iter()
            .zip(split.chunks_exact_mut(side * side))
        {
            let (top, bottom) = texels.split_at_mut(7 * side);
            limit_rows(&eye, &spec, &glare, face, 7..16, bottom);
            limit_rows(&eye, &spec, &glare, face, 0..7, top);
        }
        assert_eq!(
            offsets(&split),
            offsets(&limited(&glare, &eye)),
            "the eye offsets"
        );
        let mut white: Vec<BandTexel> = face
            .iter()
            .map(|t| BandTexel::of_light(t.luminance().value(), t.sp_ratio()))
            .collect();
        limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..16, &mut white);
        assert_eq!(limits(&white), limits(&whole), "the chroma");
        // No glare: the texel's own background.
        let alone = limited(&Glare::default(), &eye);
        for texel in &alone {
            let own = SkyBackground::new(
                texel.luminance(),
                SpRatio::new(texel.sp_ratio()).expect("a ratio"),
            )
            .expect("a background");
            assert_eq!(
                bits(texel.eye_limit().expect("a limit").value()),
                bits(naked_eye_limit(&eye, &own).value())
            );
        }
        assert_ne!(limits(&alone[..256]), limits(&whole), "the glare moves it");
        // No light, no glare: Crumey's zero background.
        let mut dark = vec![BandTexel::of_light(0.0, solar_colour().sp_ratio()); 64];
        limit_rows(
            &eye,
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::NegZ,
            0..8,
            &mut dark,
        );
        let clamp = naked_eye_limit(
            &eye,
            &SkyBackground::new(
                luminance(MagnitudesPerArcsec2::new(40.0)),
                SpRatio::REFERENCE,
            )
            .expect("a background"),
        );
        for texel in &dark {
            assert_eq!(
                bits(texel.eye_limit().expect("a limit").value()),
                bits(clamp.value())
            );
        }
    }

    /// The glare reads each listed star's light after its own reddening: its photopic illuminance
    /// is its unextinguished light times its photopic transmission, and its scotopic light that
    /// times its reddened ρ, `colour().reddened(a_v()).sp_ratio()`.
    #[test]
    fn the_glare_reads_each_stars_reddened_light() {
        let stars = nearby_stars();
        let glare = Glare::of_listed(&observer(), stars, &spec(16), Magnitudes::new(EYE_CUT));
        assert_eq!(glare.len(), stars.len());
        assert!(!glare.is_empty());
        let mut reddened_stars = 0;
        for (source, (star, (u, e, rho))) in glare
            .sources
            .iter()
            .zip(stars.iter().zip(sources_of(stars)))
        {
            assert_eq!(bits(source.photopic), bits(e));
            assert_eq!(bits(source.scotopic), bits(e * rho));
            assert_eq!(bits(source.sp_ratio), bits(rho));
            assert!(angle_deg(&source.direction.expect("a direction"), &u) < 1e-9);
            if star.a_v().value() > 0.0 {
                assert!(rho < star.colour().sp_ratio(), "the dust lowers ρ");
                reddened_stars += 1;
            }
        }
        eprintln!(
            "{} listed stars within {GLARE_RADIUS_LY} ly, {reddened_stars} behind some dust",
            stars.len()
        );
        assert!(reddened_stars > 0, "some stars lie behind dust");
    }

    /// The glare reaches 90°, the plane of the eye, and no further (R06.T9.h): a source 89.995°
    /// from a texel's centre veils it by exactly its `veiling_luminance` per lux times cos θ times
    /// its illuminance, and one at 90.005°, behind the eye's plane, adds exactly nothing, as those
    /// at 99.995° and 120° do not. The texel keeps that veil beside its limit. The sources are
    /// summed exactly, every node of the glare's pyramid opened; the pyramid gives a lone source's
    /// bits too, since a node of one star is taken at the star's own direction (R06.T9.i).
    #[test]
    fn the_glare_reaches_90_degrees_in_the_plane_of_the_eye_and_no_further() {
        let spec = BandSpec::new(8, 12).expect("a spec");
        let eye = EyeObserver::default();
        let toward = spec.texel_direction(CubeFace::PosX, 4, 4);
        let (e, rho) = (illuminance_of_magnitude(Magnitudes::new(-1.5)).value(), 2.6);
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(24.5)).value(), 2.26);
        let lit_by = |angles: &[f64], opening: Opening| {
            let sources: Vec<_> = angles
                .iter()
                .map(|&angle| (off(toward, angle), e, rho))
                .collect();
            let glare = glare_of(spec, &sources);
            let mut texels = vec![texel; 64];
            set_limits(
                &eye,
                spec,
                &glare,
                CubeFace::PosX,
                0..8,
                &mut texels,
                opening,
                &mut (),
            );
            texels[4 * 8 + 4]
        };
        let lit = |angles: &[f64]| lit_by(angles, Opening::Every);
        let limit_bits = |texel: BandTexel| bits(texel.eye_limit().expect("a limit").value());
        let near = off(toward, 89.995);
        let cos = toward.dot(&near);
        let angle = Degrees::new(math::acos(cos) / RADIANS_PER_DEGREE);
        assert!((angle.value() - 89.995).abs() < 1e-6, "{angle:?}");
        let per_lux = veiling_luminance(&eye, Lux::new(1.0), angle)
            .expect("a valid glare")
            .value()
            * cos;
        let veil = [e * per_lux, e * rho * per_lux];
        let expected = bits(texel_limit(&eye, &texel, veil).value());
        let within = lit(&[89.995]);
        assert_eq!(limit_bits(within), expected);
        assert_eq!(within.eye_veil().map(|v| v.map(bits)), Some(veil.map(bits)));
        assert_eq!(lit_by(&[89.995], Opening::Pyramid), within, "the pyramid");
        assert_eq!(
            limit_bits(lit_by(&[90.005], Opening::Pyramid)),
            bits(texel_limit(&eye, &texel, [0.0, 0.0]).value()),
            "the pyramid"
        );
        assert_eq!(limit_bits(lit(&[89.995, 90.005, 99.995, 120.0])), expected);
        let own = bits(texel_limit(&eye, &texel, [0.0, 0.0]).value());
        let behind = lit(&[90.005, 99.995, 120.0]);
        assert_eq!(limit_bits(behind), own);
        assert_eq!(behind.eye_veil(), Some([0.0, 0.0]));
        assert_ne!(expected, own, "the source within the reach veils the texel");
    }

    /// A star of the reference colour 0.05° from a 64² texel's centre towards the north galactic
    /// pole, 0.05 mag brighter than its own limit (its texel's background less its own veil), is
    /// culled at its texel's limit, which its own veil makes shallower, and kept at its own, as a
    /// view's cull (`starIsSeen`: V < limit + eye offset) keeps it (R06.T9.h).
    #[test]
    fn a_star_near_its_texels_centre_is_culled_at_its_texels_limit_and_kept_at_its_own() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let n = usize::from(spec.face_texels());
        let centre = spec.texel_direction(CubeFace::PosZ, 32, 32);
        let pole = band_near_the_sun(spec, CubeFace::PosZ, 32..33)[32];
        let own_limit = texel_limit(&eye, &pole, [0.0, 0.0]);
        let v = own_limit - Magnitudes::new(0.05);
        let u = off(centre, 0.05);
        let glare = glare_of(
            spec,
            &[(u, illuminance_of_magnitude(v).value(), REFERENCE_SP_RATIO)],
        );
        let mut band = vec![pole; CubeFace::ALL.len() * n * n];
        limit_map(&eye, &spec, &glare, &mut band);
        let (index, _) = texel_of(spec, &u);
        assert_eq!(
            index,
            (4 * n + 32) * n + 32,
            "the star lies in the pole's texel"
        );
        let limit = band[index].eye_limit().expect("a limit");
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(offsets.len(), 1, "one offset for one star");
        let offset = offsets[0];
        eprintln!(
            "towards the pole, a reference star of V {:.3} 0.05° from a 64² texel's centre: the \
             texel sees to {:.3}, the star's own limit is {:.3} (offset {:+.3})",
            v.value(),
            limit.value(),
            (limit + offset).value(),
            offset.value()
        );
        // One source: the texel's veil is its term alone, so the term subtracted leaves exactly
        // none, and the star's own limit is its texel's with no glare, bit for bit.
        assert_eq!(bits(offset.value()), bits((own_limit - limit).value()));
        assert!(
            v >= limit,
            "culled at its texel's limit: V {v:?} against {limit:?}"
        );
        assert!(
            v < limit + offset,
            "kept at its own: V {v:?} against {:?}",
            limit + offset
        );
    }

    /// In the fixture's scotopic texels no listed star's self-exclusion is negative: its own light
    /// only brightens its texel's background there; and the wire's saturation of the eye offset
    /// changes no star's verdict. Also prints T9.h's records for the fixture:
    /// the largest self-exclusion that decides a star (its V between its limit before the
    /// self-exclusion and its own), the largest eye offset such a star and any star has, and the
    /// largest self-exclusion of a star within 0.5 mag of its own limit and 0.6° or more from its
    /// texel's centre.
    #[test]
    fn every_self_exclusion_is_non_negative_in_a_scotopic_texel() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let stars = nearby_stars();
        let glare = Glare::of_listed(&observer(), stars, &spec, Magnitudes::new(EYE_CUT));
        let band = limited(&glare, &eye);
        let (mut scotopic, mut deciding, mut saturated) = (0, 0, 0);
        let (mut decides, mut decided_offset, mut largest_offset, mut far) =
            (0.0_f64, f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0_f64);
        for (star, source) in stars.iter().zip(&glare.sources) {
            let direction = source.direction.expect("a star away from the observer");
            let (index, toward) = texel_of(spec, &direction);
            let texel = &band[index];
            let parts = eye_offset_parts(&eye, spec, source, &band);
            let (self_exclusion, total) = (parts.self_exclusion.value(), parts.total().value());
            let glared = background(texel, texel.eye_veil().expect("a veil"));
            if mesopic_weight(&glared) == PhotopicWeight::SCOTOPIC {
                scotopic += 1;
                assert!(
                    self_exclusion >= 0.0,
                    "a self-exclusion of {self_exclusion}"
                );
            }
            let limit = texel.eye_limit().expect("a limit").value();
            let own = limit + total;
            let v = star.v().value();
            largest_offset = largest_offset.max(total);
            let carried = total.clamp(WIRE_MIN_EYE_OFFSET_MAG, WIRE_MAX_EYE_OFFSET_MAG);
            if (v < limit + carried) != (v < own) {
                saturated += 1;
            }
            if own - self_exclusion <= v && v < own {
                deciding += 1;
                decides = decides.max(self_exclusion);
                decided_offset = decided_offset.max(total);
            }
            if (v - own).abs() <= 0.5 && angle_deg(&toward, &direction) >= 0.6 {
                far = far.max(self_exclusion);
            }
        }
        eprintln!(
            "{} listed stars at 16², {scotopic} in scotopic texels: {deciding} decided by their \
             self-exclusion, the largest {decides:.4} mag (their largest eye offset \
             {decided_offset:+.4}); the largest eye offset of any star {largest_offset:+.4}; the \
             largest self-exclusion within 0.5 mag of a star's own limit and 0.6° or more from its \
             texel's centre {far:.2e}",
            stars.len()
        );
        assert!(scotopic > 0, "stars in scotopic texels");
        assert_eq!(
            saturated, 0,
            "the wire's saturation changes a star's verdict"
        );
    }

    /// The wire's saturation at Crumey's clamp, for one eye ([`saturation_at_the_clamp`]).
    #[derive(Debug)]
    struct Saturation {
        /// The star's own limit.
        own: f64,
        /// The V of the brightest star its self-exclusion decides, and that star's texel's limit
        /// and eye offset.
        first: (f64, f64, EyeOffsetParts),
        /// How much fainter than its texel's limit a star at its own limit lies.
        at_own: f64,
        /// The window the saturation takes: V from its texel's limit + 1.27 to its own limit.
        lost: f64,
    }

    /// A star of S/P ratio `rho`, 0.05° from a 64² texel's centre (read at 0.1°), alone over a
    /// texel whose colour-corrected background lies exactly at Crumey's clamp, 10⁻⁵ cd m⁻², where
    /// the self-exclusion is largest, seen by `eye`: it is seen while V < its own limit, and the
    /// wire's +1.27 culls it where V ≥ its texel's limit + 1.27.
    fn saturation_at_the_clamp(eye: &EyeObserver, rho: f64) -> Saturation {
        let spec = BandSpec::STANDARD;
        let n = usize::from(spec.face_texels());
        let u = off(spec.texel_direction(CubeFace::PosZ, 32, 32), 0.05);
        let (index, _) = texel_of(spec, &u);
        let dark = BandTexel::of_light(DARKEST_BACKGROUND.value(), BLACKWELL_SP_RATIO);
        // The star of V `v`: its texel's limit, and its eye offset in its two parts.
        let at = |v: f64| {
            let illuminance = illuminance_of_magnitude(Magnitudes::new(v)).value();
            let glare = glare_of(spec, &[(u, illuminance, rho)]);
            let mut band = vec![dark; CubeFace::ALL.len() * n * n];
            limit_map(eye, &spec, &glare, &mut band);
            let parts = eye_offset_parts(eye, spec, &glare.sources[0], &band);
            (band[index].eye_limit().expect("a limit").value(), parts)
        };
        // The faintest V at which `fainter` holds, by bisection over [lo, hi], where it holds at
        // hi and not at lo.
        let bisect = |fainter: &dyn Fn(f64) -> bool, (mut lo, mut hi): (f64, f64)| {
            for _ in 0..80 {
                let mid = f64::midpoint(lo, hi);
                if fainter(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hi
        };
        let (_, faint) = at(20.0);
        let own = texel_limit(eye, &dark, [0.0, 0.0]).value() + faint.colour.value();
        // The brightest star its self-exclusion decides: its V is its limit before the
        // self-exclusion, its texel's limit plus its colour offset. V less its texel's limit
        // rises with V, so the faintest star the wire could cull lies at its own limit.
        let first = bisect(
            &|v| {
                let (limit, parts) = at(v);
                v >= limit + parts.colour.value()
            },
            (own - 3.0, own),
        );
        let (limit, parts) = at(first);
        let at_own = own - at(own).0;
        let lost = if at_own <= WIRE_MAX_EYE_OFFSET_MAG {
            0.0
        } else {
            own - bisect(&|v| v - at(v).0 >= WIRE_MAX_EYE_OFFSET_MAG, (first, own))
        };
        Saturation {
            own,
            first: (first, limit, parts),
            at_own,
            lost,
        }
    }

    /// At Crumey's darkest-sky clamp the wire's saturation of the eye offset at +1.27 (Design note
    /// 17) costs a star near its texel's centre at most 0.16 mag of its window (the ruling's
    /// bound, `decision-r06-t9c-glare.md`, item 1), for the colour table's hottest star, over
    /// Crumey's field factors to 2.4 and CIE's ages to 80 years; and nothing at the default eye,
    /// whose largest offset passes +1.27 only for stars no fainter than their texel's limit plus
    /// their colour offset.
    #[test]
    fn at_crumeys_clamp_the_wires_saturation_costs_a_star_at_most_0_16_mag_of_its_window() {
        use crate::tables::star_colour::{NORMAL, WHITE_DWARF};
        let hottest = NORMAL
            .iter()
            .chain(&WHITE_DWARF)
            .map(|row| row[3])
            .fold(f64::NEG_INFINITY, f64::max);
        for (f, age, p) in [(1.4, 25.0, 0.5), (2.4, 25.0, 0.5), (2.4, 80.0, 1.2)] {
            let eye = EyeObserver::new(f, age, p).expect("an eye");
            let Saturation {
                own,
                first: (first, limit, parts),
                at_own,
                lost,
            } = saturation_at_the_clamp(&eye, hottest);
            let largest = parts.total().value();
            eprintln!(
                "at Crumey's clamp, F {f}, age {age}, p {p}, the hottest row (ρ {hottest:.4}) \
                 0.05° from a 64² texel's centre: own limit {own:.4}; the brightest star its \
                 self-exclusion decides, V {first:.4} (texel's limit {limit:.4}), has the largest \
                 eye offset, {largest:+.4} ({:.4} self-exclusion, {:+.4} colour), {:+.4} past the \
                 wire's +1.27; a star at its own limit lies {at_own:.4} fainter than its texel's \
                 limit; the saturation takes {lost:.4} mag of the window",
                parts.self_exclusion.value(),
                parts.colour.value(),
                largest - WIRE_MAX_EYE_OFFSET_MAG,
            );
            assert!(
                (limit + largest - own).abs() < 1e-9,
                "the star's own limit is fixed"
            );
            assert!(
                lost <= 0.16,
                "F {f}, age {age}: the saturation takes {lost} mag of the window"
            );
            if eye == EyeObserver::default() {
                assert!(largest > WIRE_MAX_EYE_OFFSET_MAG, "the offset saturates");
                assert!(
                    at_own < WIRE_MAX_EYE_OFFSET_MAG,
                    "but the saturation culls no star: {at_own}"
                );
            }
        }
    }

    /// A star that adds no veil takes its colour offset alone: one with no illuminance takes it
    /// against its texel's background, and one at the observer's own position, which has no
    /// direction, against a scotopic sky.
    #[test]
    fn a_star_that_adds_no_veil_takes_its_colour_offset_alone() {
        let spec = BandSpec::new(8, 12).expect("a spec");
        let eye = EyeObserver::default();
        let glare = Glare::of_sources(
            vec![
                GlareSource {
                    direction: Some(UnitVector::NORTH),
                    photopic: 0.0,
                    scotopic: 0.0,
                    sp_ratio: 3.0,
                },
                GlareSource {
                    direction: None,
                    photopic: 1e-6,
                    scotopic: 3e-6,
                    sp_ratio: 3.0,
                },
            ],
            spec,
        );
        assert!(
            glare.pyramid.as_ref().is_some_and(|p| p.stars.is_empty()),
            "neither glares"
        );
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(17.0)).value(), 2.26);
        let mut band = vec![texel; CubeFace::ALL.len() * 64];
        limit_map(&eye, &spec, &glare, &mut band);
        let rho = SpRatio::new(3.0).expect("a ratio");
        let mesopic = background(&texel, [0.0, 0.0]);
        assert!(mesopic_weight(&mesopic).value() > 0.0, "a mesopic texel");
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(
            offsets.iter().map(|o| bits(o.value())).collect::<Vec<_>>(),
            [
                bits(star_colour_offset(rho, &mesopic).value()),
                bits(star_colour_offset(rho, &scotopic_sky()).value()),
            ]
        );
        let scotopic = 2.5 * math::log10(3.0 / REFERENCE_SP_RATIO);
        assert!((offsets[1].value() - scotopic).abs() < 1e-12, "{offsets:?}");
        assert!(
            (offsets[0].value() - scotopic).abs() > 1e-3,
            "the mesopic offset fades"
        );
    }

    /// The tolerance on a texel's limit and a star's eye offset against the exact sum, mag
    /// (R06.T9.i; `decision-r06-t9c-glare.md`, item 2): the wire's quantum for a texel's limit
    /// (millimagnitudes), and a tenth of an eye offset's (centimagnitudes).
    const PYRAMID_TOLERANCE_MAG: f64 = 0.001;

    /// The tolerance on the texels' mean difference from the exact sum, mag (the same ruling).
    const PYRAMID_MEAN_TOLERANCE_MAG: f64 = 0.0003;

    /// How far T9.i's near-Sun census looks, ly: its fixture's stars are those a census lists
    /// within it brighter than the eye's cut, 8.15.
    const FAR_FIELD_RADIUS_LY: f64 = 200.0;

    /// The stars a census near the Sun lists brighter than the eye's cut within
    /// [`FAR_FIELD_RADIUS_LY`]: built once.
    fn stars_within_200_ly() -> &'static [SkyStar] {
        static STARS: OnceLock<Vec<SkyStar>> = OnceLock::new();
        STARS.get_or_init(|| listed_near_the_sun(EYE_CUT, FAR_FIELD_RADIUS_LY))
    }

    /// The band near the Sun at the eye's cut at the server's 64² faces, with no census, complete
    /// everywhere: built once.
    fn near_the_sun_at_64() -> &'static [BandTexel] {
        static BAND: OnceLock<Vec<BandTexel>> = OnceLock::new();
        BAND.get_or_init(|| band_at(EYE_CUT, BandSpec::STANDARD))
    }

    /// The glare pyramid's limits against the exact sum's, over one band.
    #[derive(Debug)]
    struct AgainstExact {
        /// The largest and the mean difference of a texel's limit, mag.
        largest: f64,
        mean: f64,
        /// The largest difference of an eye offset, mag.
        largest_offset: f64,
        /// What the pyramid evaluated.
        evaluations: Evaluations,
        /// The band with the pyramid's limits.
        band: Vec<BandTexel>,
    }

    /// The limits of `band` (a band of `spec` without them) against `glare` summed over the
    /// pyramid, against those of the exact sum, every node opened; and the eye offsets of each.
    fn against_exact(
        eye: &EyeObserver,
        spec: BandSpec,
        glare: &Glare,
        band: &[BandTexel],
    ) -> AgainstExact {
        let mut evaluations = Evaluations::default();
        let mut pyramid = band.to_vec();
        map_limits(
            eye,
            spec,
            glare,
            &mut pyramid,
            Opening::Pyramid,
            &mut evaluations,
        );
        let mut exact = band.to_vec();
        map_limits(eye, spec, glare, &mut exact, Opening::Every, &mut ());
        let differences: Vec<f64> = pyramid
            .iter()
            .zip(&exact)
            .map(|(a, b)| {
                (a.eye_limit().expect("a limit") - b.eye_limit().expect("a limit"))
                    .value()
                    .abs()
            })
            .collect();
        #[expect(clippy::cast_precision_loss, reason = "a band's texels, under 2⁵³")]
        let mean = differences.iter().sum::<f64>() / differences.len() as f64;
        let largest_offset = eye_offsets(eye, &spec, glare, &pyramid)
            .iter()
            .zip(eye_offsets(eye, &spec, glare, &exact))
            .map(|(a, b)| (*a - b).value().abs())
            .fold(0.0, f64::max);
        AgainstExact {
            largest: differences.iter().copied().fold(0.0, f64::max),
            mean,
            largest_offset,
            evaluations,
            band: pyramid,
        }
    }

    /// Prints `found` for `what`, `sources` glaring over `texels`, and holds it to the ruled
    /// tolerances.
    fn assert_within_tolerance(what: &str, found: &AgainstExact, sources: usize, texels: usize) {
        let Evaluations {
            visits,
            wholes,
            pairs,
        } = found.evaluations;
        let exact_pairs = u64::try_from(sources * texels).expect("under 2⁶⁴ pairs");
        #[expect(clippy::cast_precision_loss, reason = "counts under 2⁵³")]
        let fewer = exact_pairs as f64 / found.evaluations.evaluated() as f64;
        eprintln!(
            "{what}: {sources} sources over {texels} texels; the pyramid against the exact sum: \
             texel limits within {:.2e} mag at most, {:.2e} on average, eye offsets within \
             {:.2e}; {pairs} pairs and {wholes} nodes taken whole ({fewer:.1}× fewer \
             evaluations than the exact sum's {exact_pairs} pairs), {visits} nodes tested",
            found.largest, found.mean, found.largest_offset
        );
        assert!(
            found.largest <= PYRAMID_TOLERANCE_MAG,
            "{what}: a texel's limit differs by {}",
            found.largest
        );
        assert!(
            found.mean <= PYRAMID_MEAN_TOLERANCE_MAG,
            "{what}: the texels differ by {} on average",
            found.mean
        );
        assert!(
            found.largest_offset <= PYRAMID_TOLERANCE_MAG,
            "{what}: an eye offset differs by {}",
            found.largest_offset
        );
    }

    /// The limits' bits of the rows of `face` of `band` (a band of `spec` without limits) set in
    /// the jobs `splits` (each a range of rows), the last first, against those of the face set
    /// whole, `whole` (the face's texels with their limits).
    fn assert_rows_split_alike(
        spec: BandSpec,
        glare: &Glare,
        band: &[BandTexel],
        face: CubeFace,
        splits: &[Range<u16>],
        whole: &[BandTexel],
    ) {
        let eye = EyeObserver::default();
        let n = usize::from(spec.face_texels());
        let first = usize::from(face.layer()) * n * n;
        let limits = |texels: &[BandTexel]| -> Vec<(u64, [u64; 2])> {
            texels
                .iter()
                .map(|t| {
                    (
                        bits(t.eye_limit().expect("a limit").value()),
                        t.eye_veil().expect("a veil").map(bits),
                    )
                })
                .collect()
        };
        let mut parts = Vec::new();
        for rows in splits.iter().rev() {
            let at = first + usize::from(rows.start) * n..first + usize::from(rows.end) * n;
            let mut texels = band[at].to_vec();
            limit_rows(&eye, &spec, glare, face, rows.clone(), &mut texels);
            parts.push((rows.start, texels));
        }
        parts.sort_by_key(|(start, _)| *start);
        let split: Vec<BandTexel> = parts.into_iter().flat_map(|(_, t)| t).collect();
        assert_eq!(
            limits(&split),
            limits(whole),
            "{face:?} split at {splits:?}"
        );
    }

    /// Near the Sun at the server's 64² faces, against the glare of the stars a census lists within
    /// 200 ly brighter than the eye's cut, alone and with T9.c's two placed sources, every texel's
    /// limit summed over the glare's pyramid is within 0.001 mag of the exact sum's, their mean
    /// within 0.0003 mag, and every eye offset within 0.001 mag (R06.T9.i). A face set in a
    /// server's jobs, split in rows, has the bits of the face set whole.
    #[test]
    fn near_the_sun_the_pyramid_is_within_0_001_mag_of_the_exact_sum() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let band = near_the_sun_at_64();
        let census = sources_of(stars_within_200_ly());
        let mut placed = census.clone();
        placed.extend(placed_sources(spec));
        for (what, sources) in [
            ("the census", &census),
            ("the census and the placed sources", &placed),
        ] {
            let glare = glare_of(spec, sources);
            let found = against_exact(&eye, spec, &glare, band);
            let (plane, _) = median_limit(&found.band, spec, 0.0..5.0);
            let (poles, _) = median_limit(&found.band, spec, 80.0..90.1);
            eprintln!(
                "near the Sun at 64², cut {EYE_CUT}, with the glare of {what}: the median limit \
                 is {plane:.4} in the band and {poles:.4} at the poles"
            );
            assert_within_tolerance(what, &found, sources.len(), band.len());
        }
        let glare = glare_of(spec, &placed);
        let mut whole = band.to_vec();
        limit_map(&eye, &spec, &glare, &mut whole);
        let n = 64 * 64;
        for (face, splits) in [
            (CubeFace::PosX, [0..7, 7..40, 40..64]),
            (CubeFace::PosZ, [0..1, 1..33, 33..64]),
        ] {
            let whole = &whole[usize::from(face.layer()) * n..][..n];
            assert_rows_split_alike(spec, &glare, band, face, &splits, whole);
        }
    }

    /// The ruling's synthetic sky (`decision-r06-t9c-glare.md`, item 2, its model's `sphere`): `n`
    /// stars over the whole sky, concentrated towards the plane as 1 + 3 exp(−|b| ÷ 10°), of V from
    /// −1.5 to `faintest` with N(< V) ∝ 10^(0.45 V), and of S/P ratio uniform in 1.5–3. Each is
    /// its direction, its photopic illuminance, lux, and its ratio.
    fn synthetic_sky(n: usize, faintest: f64) -> Vec<(UnitVector, f64, f64)> {
        const SLOPE: f64 = 0.45;
        let (lo, hi) = (math::exp10(SLOPE * -1.5), math::exp10(SLOPE * faintest));
        let mut deviates = uniforms(0x0009_1a00);
        let mut next = || deviates.next().expect("an endless stream");
        let mut sky = Vec::with_capacity(n);
        while sky.len() < n {
            let z = 2.0 * next() - 1.0;
            let (sin, cos) = math::sin_cos(std::f64::consts::TAU * next());
            let b = math::asin(z).abs() / RADIANS_PER_DEGREE;
            if 4.0 * next() > 1.0 + 3.0 * math::exp(-b / 10.0) {
                continue;
            }
            let across = (1.0 - z * z).sqrt();
            let direction =
                UnitVector::from_components([across * cos, across * sin, z]).expect("a direction");
            let v = math::log10(lo + next() * (hi - lo)) / SLOPE;
            let photopic = illuminance_of_magnitude(Magnitudes::new(v)).value();
            sky.push((direction, photopic, 1.5 + 1.5 * next()));
        }
        sky
    }

    /// A band of `spec` for the synthetic sky, as the ruling's model's `sphere` takes it: its light
    /// runs from μ 24.6 at the poles to 22.2 in the plane, the poles' luminance plus the plane's
    /// excess over it times exp(−|b| ÷ 10°), at ρ 2.26.
    fn synthetic_band(spec: BandSpec) -> Vec<BandTexel> {
        let poles = luminance(MagnitudesPerArcsec2::new(24.6)).value();
        let plane = luminance(MagnitudesPerArcsec2::new(22.2)).value();
        texel_latitudes(spec)
            .into_iter()
            .map(|(_, b)| BandTexel::of_light(poles + (plane - poles) * math::exp(-b / 10.0), 2.26))
            .collect()
    }

    /// The synthetic sky's fingerprint at 300,000 stars to V 10.06: [`f64_digest`] of each star's
    /// direction, illuminance and ratio, in order. The bench's own copy of [`synthetic_sky`]
    /// asserts the same (`benches/sky.rs`, `SYNTHETIC_SKY_DIGEST`), so the two cannot part.
    const SYNTHETIC_SKY_DIGEST: u64 = 0x8b8b_938a_6811_c91f;

    /// The synthetic sky at the census's largest listing, 300,000 stars to V 10.06 (a camera's cut
    /// with the eye open), as its glare over the server's 64² band: built once. Its stars are held
    /// to [`SYNTHETIC_SKY_DIGEST`].
    fn synthetic_glare() -> &'static (usize, Glare) {
        static GLARE: OnceLock<(usize, Glare)> = OnceLock::new();
        GLARE.get_or_init(|| {
            let n = usize::try_from(MAX_N_MAX).expect("300,000 fits a usize");
            let sky = synthetic_sky(n, 10.06);
            let values: Vec<f64> = sky
                .iter()
                .flat_map(|(u, e, rho)| {
                    let [x, y, z] = u.components();
                    [x, y, z, *e, *rho]
                })
                .collect();
            let digest = f64_digest(&values);
            assert_eq!(
                digest, SYNTHETIC_SKY_DIGEST,
                "the synthetic sky's digest {digest:#018x}"
            );
            (sky.len(), glare_of(BandSpec::STANDARD, &sky))
        })
    }

    /// On the synthetic sky of 300,000 stars to V 10.06 at 64², the pyramid evaluates at most a
    /// hundredth of the exact sum's star–texel pairs: its pairs summed star by star and its nodes
    /// taken whole, counted, so the test does not depend on the machine (R06.T9.i; the ruling's
    /// model, about a three-hundredth). One row of each face is held to the exact sum, as the slow
    /// test holds every texel, and a face split in rows has the bits of the face set whole.
    #[test]
    fn on_300_000_synthetic_stars_the_pyramid_evaluates_at_most_a_hundredth_of_the_pairs() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let (n, glare) = synthetic_glare();
        let band = synthetic_band(spec);
        let mut evaluations = Evaluations::default();
        let mut limited = band.clone();
        map_limits(
            &eye,
            spec,
            glare,
            &mut limited,
            Opening::Pyramid,
            &mut evaluations,
        );
        // In u64: the product (7.4 × 10⁹) overflows a 32-bit usize on wasm32.
        let exact_pairs = u64::try_from(*n).expect("a count fits u64")
            * u64::try_from(band.len()).expect("a count fits u64");
        #[expect(clippy::cast_precision_loss, reason = "counts under 2⁵³")]
        let (fewer, per_texel) = (
            exact_pairs as f64 / evaluations.evaluated() as f64,
            evaluations.evaluated() as f64 / band.len() as f64,
        );
        eprintln!(
            "{n} synthetic stars over {} texels: the pyramid sums {} pairs and takes {} nodes \
             whole, {per_texel:.0} evaluations a texel, {fewer:.1}× fewer than the exact sum's \
             {exact_pairs} pairs; {} nodes tested",
            band.len(),
            evaluations.pairs,
            evaluations.wholes,
            evaluations.visits
        );
        assert!(
            evaluations.evaluated() * 100 <= exact_pairs,
            "{evaluations:?} against {exact_pairs} pairs"
        );
        let side = spec.face_texels();
        let mut worst = 0.0_f64;
        for (k, face) in CubeFace::ALL.into_iter().enumerate() {
            let row = u16::try_from(5 + 11 * k).expect("a row");
            let first = (usize::from(face.layer()) * 64 + usize::from(row)) * 64;
            let mut exact = band[first..first + 64].to_vec();
            set_limits(
                &eye,
                spec,
                glare,
                face,
                row..row + 1,
                &mut exact,
                Opening::Every,
                &mut (),
            );
            for (a, b) in limited[first..first + 64].iter().zip(&exact) {
                let d = (a.eye_limit().expect("a limit") - b.eye_limit().expect("a limit")).value();
                worst = worst.max(d.abs());
            }
        }
        eprintln!("one row of each face against the exact sum: within {worst:.2e} mag");
        assert!(worst <= PYRAMID_TOLERANCE_MAG, "{worst}");
        let face = CubeFace::NegY;
        let n_face = usize::from(side) * usize::from(side);
        let whole = &limited[usize::from(face.layer()) * n_face..][..n_face];
        assert_rows_split_alike(spec, glare, &band, face, &[0..30, 30..31, 31..64], whole);
    }

    /// On the synthetic sky of 300,000 stars to V 10.06 at 64², every texel's limit summed over
    /// the glare's pyramid is within 0.001 mag of the exact sum's, their mean within 0.0003 mag,
    /// and every eye offset within 0.001 mag (R06.T9.i).
    #[test]
    #[ignore = "slow: the exact sum over 7.4 × 10⁹ star–texel pairs, some minutes"]
    fn on_300_000_synthetic_stars_the_pyramid_is_within_0_001_mag_of_the_exact_sum() {
        let spec = BandSpec::STANDARD;
        let (n, glare) = synthetic_glare();
        let band = synthetic_band(spec);
        let found = against_exact(&EyeObserver::default(), spec, glare, &band);
        assert_within_tolerance("300,000 synthetic stars", &found, *n, band.len());
    }

    /// A texel always opens its own leaf (R06.T9.i). Two stars near a corner of a 16² texel, over
    /// 4° from its centre and 0.3° apart, lie in a leaf that the rule alone would take whole from
    /// that centre. The texel sums them star by star, so its veil and their eye offsets are the
    /// exact sum's, bit for bit. The texel across the corner takes the leaf whole.
    #[test]
    fn a_texel_always_opens_its_own_leaf() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let face = CubeFace::PosZ;
        // Texel (8, 8) of +Z covers face coordinates 0–0.125 in both, the direction (s, −t, 1) for
        // s along its columns and t down its rows; its corner at (0, 0) is texel (7, 7)'s too.
        let on_face =
            |s: f64, t: f64| UnitVector::from_components([s, -t, 1.0]).expect("a direction");
        let (e, rho) = (illuminance_of_magnitude(Magnitudes::new(0.0)).value(), 2.3);
        let sources = [
            (on_face(0.004, 0.004), e, rho),
            (on_face(0.0092, 0.004), e, rho),
        ];
        for (u, _, _) in &sources {
            assert_eq!(spec.texel_of(u.components()), Some((face, 8, 8)));
        }
        assert!((angle_deg(&sources[0].0, &sources[1].0) - 0.3).abs() < 0.01);
        let glare = glare_of(spec, &sources);
        let pyramid = glare.pyramid.as_ref().expect("a pyramid");
        let leaf = pyramid
            .nodes
            .iter()
            .find(|node| matches!(node.holds, NodeHolds::Stars { len: 2, .. }))
            .expect("the two stars' leaf");
        let NodeHolds::Stars { start, .. } = leaf.holds else {
            unreachable!("a leaf holds stars")
        };
        let held: Vec<UnitVector> = pyramid.stars[at(start)..at(start) + 2]
            .iter()
            .map(|star| star.direction)
            .collect();
        assert_eq!(held, [sources[0].0, sources[1].0], "the census's order");
        let own = spec.texel_direction(face, 8, 8);
        let cos = own.dot(&leaf.centre);
        assert!(
            cos > leaf.behind_at && cos <= leaf.whole_at,
            "the rule alone takes the leaf whole from its own texel: {cos} against {leaf:?}"
        );
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(24.0)).value(), 2.26);
        let veils = |opening: Opening| {
            let mut band = vec![texel; CubeFace::ALL.len() * 256];
            map_limits(&eye, spec, &glare, &mut band, opening, &mut ());
            let offsets: Vec<u64> = eye_offsets(&eye, &spec, &glare, &band)
                .iter()
                .map(|o| bits(o.value()))
                .collect();
            (band, offsets)
        };
        let ((pyramid_band, pyramid_offsets), (exact_band, exact_offsets)) =
            (veils(Opening::Pyramid), veils(Opening::Every));
        let index = (usize::from(face.layer()) * 16 + 8) * 16 + 8;
        let veil_bits = |band: &[BandTexel]| band[index].eye_veil().map(|v| v.map(bits));
        assert_eq!(veil_bits(&pyramid_band), veil_bits(&exact_band));
        assert_eq!(pyramid_offsets, exact_offsets);
        let mut across = Evaluations::default();
        let corner = spec.texel_direction(face, 7, 7);
        let _ = glare.veil(
            &eye,
            &corner,
            (face, 7, 7),
            Opening::Pyramid,
            &mut Vec::new(),
            &mut across,
        );
        assert_eq!(across.pairs, 0, "{across:?}");
        assert_eq!(across.wholes, 1, "{across:?}");
    }

    #[test]
    #[should_panic(expected = "a glare built for faces of 16² texels read on a band of 8² texels")]
    fn a_glare_built_for_another_band_is_refused() {
        let glare = glare_of(spec(16), &[(UnitVector::X, 1e-6, 2.26)]);
        let mut texels = vec![BandTexel::of_light(1e-5, 2.26); 64];
        limit_rows(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &glare,
            CubeFace::PosX,
            0..8,
            &mut texels,
        );
    }

    /// A camera's cut beside the eye's near the Sun, V (R06.T9.j): 10.06, the glare ruling's
    /// figure for a camera with the eye open, beside the eye's ruled 8.15.
    const CAMERA_CUT: f64 = 10.06;

    /// A request near the Sun to `cut` with the eye asked at `eye_cut`.
    fn request(cut: f64, eye_cut: f64) -> SkyQuery {
        SkyQuery::builder(observer(), Magnitudes::new(cut))
            .eye(EyeObserver::default())
            .eye_cut(Magnitudes::new(eye_cut))
            .build()
            .expect("a valid query")
    }

    /// The stars a census near the Sun lists to `cut` within [`GLARE_RADIUS_LY`], every cap forced
    /// to it, as a census complete to that radius lists them (those within it).
    fn within_the_radius(stars: &[SkyStar]) -> Vec<SkyStar> {
        stars
            .iter()
            .filter(|s| s.distance().value() <= GLARE_RADIUS_LY)
            .copied()
            .collect()
    }

    /// The stars a census near the Sun lists to the camera's cut within [`GLARE_RADIUS_LY`]
    /// ([`within_the_radius`]): built once.
    fn camera_stars() -> &'static [SkyStar] {
        static STARS: OnceLock<Vec<SkyStar>> = OnceLock::new();
        STARS.get_or_init(|| within_the_radius(&listed_near_the_sun(CAMERA_CUT, GLARE_RADIUS_LY)))
    }

    /// The replies of T9.j's requests near the Sun: complete to [`GLARE_RADIUS_LY`], as their
    /// censuses are, and everywhere, where the band holds only the light fainter than the cut.
    fn camera_replies() -> [CompleteTo; 2] {
        let caps: Vec<LayerCap> = CAPPED_LAYERS
            .iter()
            .map(|&layer| LayerCap::forced(layer, LightYears::new(GLARE_RADIUS_LY)))
            .collect();
        [CompleteTo::of_caps(&caps), CompleteTo::everywhere()]
    }

    /// The six faces of the band near the Sun at 16² marched for `query`, keeping
    /// [`camera_replies`].
    fn marched(query: &SkyQuery) -> Vec<BandMarch> {
        let mut ctx = context();
        CubeFace::ALL
            .iter()
            .map(|&face| {
                let replies = camera_replies();
                march_rows(
                    milky_way_galaxy(),
                    &mut ctx,
                    query,
                    replies,
                    &spec(16),
                    face,
                    0..16,
                )
            })
            .collect()
    }

    /// [`marched`] for the camera's request, the eye at its cut beside it: built once.
    fn camera_marches() -> &'static [BandMarch] {
        static MARCHES: OnceLock<Vec<BandMarch>> = OnceLock::new();
        MARCHES.get_or_init(|| marched(&request(CAMERA_CUT, EYE_CUT)))
    }

    /// [`marched`] for the eye-only request: built once.
    fn eye_only_marches() -> &'static [BandMarch] {
        static MARCHES: OnceLock<Vec<BandMarch>> = OnceLock::new();
        MARCHES.get_or_init(|| marched(&request(EYE_CUT, EYE_CUT)))
    }

    /// The band of `marches`, a band's six faces, summed for `reply` with `census`.
    fn summed(marches: &[BandMarch], census: &SkyCensus, reply: &CompleteTo) -> Vec<BandTexel> {
        let mut band = Vec::new();
        for march in marches {
            sum_rows(march, census, reply, &mut band);
        }
        band
    }

    /// Every texel's eye limit and the veil beside it, as bits.
    fn map_bits(band: &[BandTexel]) -> Vec<(u64, [u64; 2])> {
        band.iter()
            .map(|t| {
                (
                    bits(t.eye_limit().expect("a limit").value()),
                    t.eye_veil().expect("a veil").map(bits),
                )
            })
            .collect()
    }

    /// The eye's map of a band of `marches` summed for `reply` with `census`, against the glare of
    /// its listed stars at the eye's cut `eye_cut`: the band with its limits, and every listed
    /// star's eye offset as bits.
    fn eye_map(
        marches: &[BandMarch],
        census: &SkyCensus,
        reply: &CompleteTo,
        eye_cut: f64,
    ) -> (Vec<BandTexel>, Vec<u64>) {
        let (spec, eye) = (spec(16), EyeObserver::default());
        let mut band = summed(marches, census, reply);
        let glare = Glare::of_listed(
            &observer(),
            census.listed(),
            &spec,
            Magnitudes::new(eye_cut),
        );
        limit_map(&eye, &spec, &glare, &mut band);
        let offsets = eye_offsets(&eye, &spec, &glare, &band)
            .iter()
            .map(|o| bits(o.value()))
            .collect();
        (band, offsets)
    }

    /// Near the Sun, a request at a camera's cut of V 10.06 with the eye's cut at 8.15 gives the
    /// eye limits and eye offsets of the request at 8.15, bit for bit, for the stars both list
    /// (R06.T9.j; `decision-r06-t9c-glare.md`). Both censuses look within 100 ly, every cap forced
    /// to it, and both bands are complete to that radius, or everywhere, at 16². It holds with
    /// every star listed, with the camera's census overflowing by stars between the cuts, and with
    /// both overflowing. The eye-only census is the camera's stars at or brighter than 8.15, in
    /// its order.
    #[test]
    fn a_camera_cut_leaves_the_eye_the_limits_and_offsets_of_the_eye_only_request() {
        let (camera_stars, eye_stars) = (camera_stars(), within_the_radius(nearby_stars()));
        let key = |s: &SkyStar| (s.system(), s.star(), bits(s.v().value()));
        let brighter: Vec<_> = camera_stars
            .iter()
            .filter(|s| s.v().value() <= EYE_CUT)
            .map(key)
            .collect();
        assert_eq!(
            eye_stars.iter().map(key).collect::<Vec<_>>(),
            brighter,
            "the eye-only census is the camera's to the eye's cut"
        );
        let between = camera_stars.len() - eye_stars.len();
        assert!(between > 0, "stars between the cuts");
        let count = |n: usize| NonZeroU32::new(u32::try_from(n).expect("a count")).expect("some");
        let n_maxes = [
            count(camera_stars.len()),
            count(eye_stars.len() + between / 2),
            count(eye_stars.len() / 2),
        ];
        for (k, reply) in camera_replies().iter().enumerate() {
            for n_max in n_maxes {
                let camera =
                    merge_census([(camera_stars.to_vec(), CensusTallies::default())], n_max);
                let eye_only = merge_census([(eye_stars.clone(), CensusTallies::default())], n_max);
                let what = format!(
                    "reply {k}, n_max {n_max}: {} and {} listed, {} and {} overflowing",
                    camera.listed().len(),
                    eye_only.listed().len(),
                    camera.overflow().len(),
                    eye_only.overflow().len()
                );
                let (camera_band, camera_offsets) =
                    eye_map(camera_marches(), &camera, reply, EYE_CUT);
                let (eye_band, eye_only_offsets) =
                    eye_map(eye_only_marches(), &eye_only, reply, EYE_CUT);
                assert_eq!(
                    map_bits(&camera_band),
                    map_bits(&eye_band),
                    "{what}: the limits"
                );
                let both = eye_only.listed().len();
                assert_eq!(camera.listed()[..both], *eye_only.listed(), "{what}");
                assert_eq!(
                    camera_offsets[..both],
                    eye_only_offsets,
                    "{what}: the eye offsets"
                );
            }
        }
    }

    /// The galactic poles' light between V 8.15 and 10.06 as a step in surface brightness: the
    /// light fainter than 10.06 is 2.5 log₁₀(1 ÷ (1 − 0.380)) = 0.52 mag fainter than that
    /// fainter than 8.15 within 10° of the poles, from Gaia DR3's flux sums (`gaia_source_lite`,
    /// V from G by Riello et al. 2021, A&A 649, A3, Table C.2; the band ruling's own pull,
    /// `decision-r06-t9b-band.md`): 38.5% at the north pole and 37.5% at the south. The
    /// tolerance holds the step of about 0.1 mag between the two poles and Gaia's limit at G
    /// 20.7, and the fixture's own scatter.
    const POLES_STEP_8_15_TO_10_06: (f64, f64) = (0.52, 0.15);

    /// What R06.T9.j changes near the Sun: with both bands complete everywhere at 16², the camera's
    /// band at V 10.06 is darker in every texel than the eye's background, the light fainter than
    /// 8.15, at the poles (|b| over 80°) by Gaia DR3's 0.52 ± 0.15 mag, and the map T9.i made of
    /// it, every star the camera's census lists within 100 ly glaring, is not the eye-only
    /// request's, which the eye now keeps under the camera.
    #[test]
    fn under_a_camera_cut_t9is_map_was_not_the_eyes() {
        let spec = spec(16);
        let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let everywhere = &camera_replies()[1];
        let camera = merge_census([(camera_stars().to_vec(), CensusTallies::default())], all);
        let eye_only = merge_census(
            [(within_the_radius(nearby_stars()), CensusTallies::default())],
            all,
        );
        let (eye_band, _) = eye_map(eye_only_marches(), &eye_only, everywhere, EYE_CUT);
        let (under_the_camera, _) = eye_map(camera_marches(), &camera, everywhere, EYE_CUT);
        assert_eq!(map_bits(&under_the_camera), map_bits(&eye_band));
        let camera_band = summed(camera_marches(), &camera, everywhere);
        let darker = camera_band
            .iter()
            .map(|t| t.luminance().value() / t.eye_background().0.value())
            .fold(0.0, f64::max);
        // The poles' mean light, each texel by its solid angle: the camera's band and the eye's.
        let (mut band_light, mut eye_light) = (0.0, 0.0);
        let side = spec.face_texels();
        let places = CubeFace::ALL.iter().flat_map(|_| {
            (0..side).flat_map(move |row| (0..side).map(move |column| (row, column)))
        });
        for ((texel, (_, b)), (row, column)) in
            camera_band.iter().zip(texel_latitudes(spec)).zip(places)
        {
            if b > 80.0 {
                let omega = spec.texel_solid_angle_sr(row, column);
                band_light += texel.luminance().value() * omega;
                eye_light += texel.eye_background().0.value() * omega;
            }
        }
        let step = 2.5 * math::log10(eye_light / band_light);
        let mut as_built: Vec<BandTexel> = camera_band
            .into_iter()
            .map(BandTexel::without_eye_light)
            .collect();
        let glare = Glare::of_listed(
            &observer(),
            camera.listed(),
            &spec,
            Magnitudes::new(CAMERA_CUT),
        );
        limit_map(&EyeObserver::default(), &spec, &glare, &mut as_built);
        let medians = |band: &[BandTexel]| {
            [0.0..5.0, 80.0..90.1].map(|latitudes| median_limit(band, spec, latitudes).0)
        };
        let ([eye_plane, eye_poles], [built_plane, built_poles]) =
            (medians(&eye_band), medians(&as_built));
        eprintln!(
            "near the Sun at 16², the stars within {GLARE_RADIUS_LY} ly, complete everywhere: the \
             camera's band at V {CAMERA_CUT} holds at most {darker:.3} of the eye's background at \
             {EYE_CUT}; the eye's median limits are {eye_plane:.4} in the band and {eye_poles:.4} \
             at the poles, with or without the camera, where T9.i's map under the camera gave \
             {built_plane:.4} and {built_poles:.4} ({} stars glaring, not {}); at the poles the \
             camera's band is {step:.3} mag fainter than the eye's background",
            camera.listed().len(),
            eye_only.listed().len()
        );
        assert!(
            darker < 1.0,
            "the band between the cuts is the eye's background"
        );
        let (expected, tolerance) = POLES_STEP_8_15_TO_10_06;
        assert!(
            (step - expected).abs() <= tolerance,
            "the poles' step {step} against Gaia's {expected} ± {tolerance}"
        );
        assert_ne!(
            map_bits(&as_built),
            map_bits(&eye_band),
            "T9.i's map moves with the camera"
        );
        assert!(
            built_poles > eye_poles,
            "a darker background deepens T9.i's poles"
        );
    }

    /// How far the wider of the camera's two censuses looks near the Sun, ly, beside
    /// [`GLARE_RADIUS_LY`]: the eye's light at two census radii.
    const WIDER_RADIUS_LY: f64 = 200.0;

    /// The stars the camera's request near the Sun lists within [`WIDER_RADIUS_LY`], every cap
    /// forced to it, as a census complete to that radius lists them (those within it). Its cells
    /// run on up to four threads (one on WebAssembly, which has none), each with its own context,
    /// merged at [`MAX_N_MAX`], whose order is total, so the threads' timing changes no bit.
    fn camera_stars_within_200_ly() -> Vec<SkyStar> {
        let galaxy = milky_way_galaxy();
        let query = request(CAMERA_CUT, EYE_CUT)
            .with_caps_forced(LightYears::new(WIDER_RADIUS_LY))
            .expect("a forced cap");
        let mut ctx = context();
        let keys: Vec<crate::galaxy::placement::CellKey> =
            census_plan(galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise)
                .cells()
                .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let job = || {
            let mut ctx = context();
            let mut stars = Vec::new();
            loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&key) = keys.get(k) else {
                    return stars;
                };
                census_cell(galaxy, &mut ctx, key, &query, &mut stars);
            }
        };
        let stars: Vec<SkyStar> = if cfg!(target_family = "wasm") {
            job()
        } else {
            std::thread::scope(|scope| {
                let threads: Vec<_> = (0..4).map(|_| scope.spawn(job)).collect();
                threads
                    .into_iter()
                    .flat_map(|thread| thread.join().expect("a census thread completes"))
                    .collect()
            })
        };
        let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let census = merge_census([(stars, CensusTallies::default())], all);
        assert!(census.overflow().is_empty());
        census
            .listed()
            .iter()
            .filter(|s| s.distance().value() <= WIDER_RADIUS_LY)
            .copied()
            .collect()
    }

    /// The eye's light under a camera's cut does not depend on how far the camera's census reaches
    /// (R06.T9.j's follow-up; decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2). Near
    /// the Sun at 16², the camera's request at V 10.06 with the eye's cut at 8.15 has its census
    /// forced to 100 ly and to 200 ly, every star listed, and each band complete to its own radius.
    /// The eye's light, its listed stars at or brighter than the eye's cut and its background over
    /// the sphere (each texel's eye background times its solid angle), agrees within 1% at the two,
    /// the tolerance of T9.b's `the_light_does_not_depend_on_the_complete_to_radius` in
    /// `sky::band`. The stars brighter than the eye's cut between the radii, which the wider census
    /// lists, take the place of their expected light in the narrower one's eye background: nothing
    /// is counted twice or lost where a camera's caps reach farther than the eye-only request's.
    /// A double count would read some +4% and a loss some −5%. What is left is one realisation of
    /// the shell's stars against the tables' expectation (printed): a miss with that ratio well
    /// below 1 is a finding on the tables for R06.T5.f, not on the eye's accounting.
    #[test]
    fn the_eyes_light_is_independent_of_the_census_radius() {
        let (galaxy, spec) = (milky_way_galaxy(), spec(16));
        let query = request(CAMERA_CUT, EYE_CUT);
        let stars = camera_stars_within_200_ly();
        let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let side = spec.face_texels();
        let mut ctx = context();
        // At each radius: the stars the eye's light lists, their light and the eye's background, lx.
        let [(near, near_listed, near_band), (far, far_listed, far_band)] =
            [GLARE_RADIUS_LY, WIDER_RADIUS_LY].map(|radius| {
                let within: Vec<SkyStar> = stars
                    .iter()
                    .filter(|s| s.distance().value() <= radius)
                    .copied()
                    .collect();
                let census = merge_census([(within, CensusTallies::default())], all);
                let caps: Vec<LayerCap> = CAPPED_LAYERS
                    .iter()
                    .map(|&layer| LayerCap::forced(layer, LightYears::new(radius)))
                    .collect();
                let mut band = Vec::new();
                for face in CubeFace::ALL {
                    band_rows(
                        galaxy,
                        &mut ctx,
                        &query,
                        &census,
                        &CompleteTo::of_caps(&caps),
                        &spec,
                        face,
                        0..side,
                        &mut band,
                    );
                }
                assert!(
                    band.iter()
                        .all(|t| t.eye_light_cut() == Some(Magnitudes::new(EYE_CUT))),
                    "every texel holds the eye's light beside its own"
                );
                let seen: Vec<SkyStar> = census
                    .listed()
                    .iter()
                    .filter(|s| s.v().value() <= EYE_CUT)
                    .copied()
                    .collect();
                let listed: f64 = sources_of(&seen).iter().map(|&(_, lux, _)| lux).sum();
                let places = CubeFace::ALL.iter().flat_map(|_| {
                    (0..side).flat_map(move |row| (0..side).map(move |column| (row, column)))
                });
                let background: f64 = band
                    .iter()
                    .zip(places)
                    .map(|(texel, (row, column))| {
                        texel.eye_background().0.value() * spec.texel_solid_angle_sr(row, column)
                    })
                    .sum();
                (seen.len(), listed, background)
            });
        assert!(
            far > near,
            "the wider census lists stars the eye's light holds"
        );
        let (near_light, far_light) = (near_listed + near_band, far_listed + far_band);
        let moved = far_light / near_light - 1.0;
        let (realised, expected) = (far_listed - near_listed, near_band - far_band);
        eprintln!(
            "near the Sun at 16², the camera's request at V {CAMERA_CUT} with the eye's cut at \
             {EYE_CUT}: within {GLARE_RADIUS_LY} ly the eye's light lists {near} stars, \
             {near_listed:.5e} lx, beside a background of {near_band:.5e} lx, {near_light:.5e} lx \
             in all; within {WIDER_RADIUS_LY} ly {far} stars, {far_listed:.5e} lx, beside \
             {far_band:.5e} lx, {far_light:.5e} lx in all ({:+.3}%); the {} stars between the radii \
             give {realised:.5e} lx, in place of {expected:.5e} lx of expected light ({:.3} of \
             it)",
            100.0 * moved,
            far - near,
            realised / expected
        );
        assert!(moved.abs() < 0.01, "{near_light} lx against {far_light} lx");
    }

    /// A listed star between the cuts changes no texel's eye limit (R06.T9.j). Near the Sun, the
    /// camera's census within 100 ly lists stars between V 8.15 and 10.06: the map with them,
    /// against the glare at the eye's cut, is the map without them, bit for bit. Each one glares
    /// nothing, and its eye offset is its colour offset alone, against its texel's background,
    /// its self-exclusion exactly zero. Against the glare at the camera's cut, as T9.i took them,
    /// they move the limits, the brightest of them its own texel's.
    #[test]
    fn a_listed_star_between_the_cuts_changes_no_texels_eye_limit() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
        let census = merge_census([(camera_stars().to_vec(), CensusTallies::default())], all);
        let band = summed(camera_marches(), &census, &camera_replies()[0]);
        let brighter: Vec<SkyStar> = census
            .listed()
            .iter()
            .filter(|s| s.v().value() <= EYE_CUT)
            .copied()
            .collect();
        // The camera's cut as T9.i took it: the band's own light, every listed star glaring.
        let map = |stars: &[SkyStar], cut: f64| {
            let glare = Glare::of_listed(&observer(), stars, &spec, Magnitudes::new(cut));
            let mut mapped = if cut < CAMERA_CUT {
                band.clone()
            } else {
                band.iter()
                    .copied()
                    .map(BandTexel::without_eye_light)
                    .collect()
            };
            limit_map(&eye, &spec, &glare, &mut mapped);
            (mapped, glare)
        };
        let (with, glare) = map(census.listed(), EYE_CUT);
        let (without, _) = map(&brighter, EYE_CUT);
        assert_eq!(map_bits(&with), map_bits(&without), "the eye's limits");
        let mut between = 0;
        for (source, star) in glare.sources.iter().zip(census.listed()) {
            if star.v().value() <= EYE_CUT {
                continue;
            }
            between += 1;
            assert_eq!(bits(source.photopic), bits(0.0), "{star:?} glares nothing");
            let parts = eye_offset_parts(&eye, spec, source, &with);
            assert_eq!(bits(parts.self_exclusion.value()), bits(0.0), "{star:?}");
            let (index, _) = texel_of(spec, &source.direction.expect("a direction"));
            let texel = &with[index];
            let against = background(texel, texel.eye_veil().expect("a veil"));
            let colour = star_colour_offset(held_ratio(source.sp_ratio), &against);
            assert_eq!(bits(parts.colour.value()), bits(colour.value()), "{star:?}");
        }
        assert_eq!(between, census.listed().len() - brighter.len());
        // At the camera's cut every listed star glares, as T9.i took them, over the band's own light.
        let (camera_with, _) = map(census.listed(), CAMERA_CUT);
        let (camera_without, _) = map(&brighter, CAMERA_CUT);
        assert_ne!(map_bits(&camera_with), map_bits(&camera_without));
        let brightest = census
            .listed()
            .iter()
            .find(|s| s.v().value() > EYE_CUT)
            .expect("a star between the cuts");
        let toward = observer()
            .position()
            .displacement_to(brightest.apparent())
            .metres();
        let (index, _) = texel_of(
            spec,
            &UnitVector::from_components(toward).expect("a direction"),
        );
        let limit = |band: &[BandTexel]| band[index].eye_limit().expect("a limit").value();
        let moved = limit(&camera_without) - limit(&camera_with);
        eprintln!(
            "{between} listed stars between V {EYE_CUT} and {CAMERA_CUT} within {GLARE_RADIUS_LY} \
             ly: no texel's eye limit moves; glaring, as T9.i took them, the brightest (V {:.3}) \
             would take {moved:.2e} mag from its own texel's limit",
            brightest.v().value()
        );
        assert!(moved > 0.0, "{moved}");
    }

    /// A request whose cut is the eye's gives T9.i's bits (R06.T9.j). Near the Sun at 8.15, the eye
    /// asked at its own cut, stated or by default, and no eye at all march the same texels, with
    /// no eye light of their own, as `band_rows` gives them; the glare of the stars the request
    /// lists is T9.i's, every one glaring with its light as T9.i took it; so the map and the eye
    /// offsets are those T9.i made of them.
    #[test]
    fn a_request_whose_cut_is_the_eyes_gives_t9is_bits() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let reply = camera_replies()[0];
        let census = merge_census(
            [(within_the_radius(nearby_stars()), CensusTallies::default())],
            NonZeroU32::new(MAX_N_MAX).expect("not zero"),
        );
        let by_default = SkyQuery::builder(observer(), Magnitudes::new(EYE_CUT))
            .eye(eye)
            .build()
            .expect("a valid query");
        // Stated, by default, and no eye; each fresh, since a clone's vectors hold less room.
        let marches = [
            marched(&request(EYE_CUT, EYE_CUT)),
            marched(&by_default),
            marched(&query(EYE_CUT)),
        ];
        let mut t9i = Vec::new();
        let mut ctx = context();
        for face in CubeFace::ALL {
            band_rows(
                milky_way_galaxy(),
                &mut ctx,
                &query(EYE_CUT),
                &census,
                &reply,
                &spec,
                face,
                0..16,
                &mut t9i,
            );
        }
        // Every float of a texel as bits.
        let texel_bits = |band: &[BandTexel]| -> Vec<(u64, [u32; 2], u64)> {
            band.iter()
                .map(|t| {
                    (
                        bits(t.luminance().value()),
                        t.chroma().map(bits_f32),
                        bits(t.sp_ratio()),
                    )
                })
                .collect()
        };
        let heap = |marches: &[BandMarch]| marches.iter().map(BandMarch::heap_bytes).sum::<usize>();
        for faces in &marches {
            assert!(
                faces.iter().all(|m| m.eye_cut().is_none()),
                "no eye light kept"
            );
            assert_eq!(heap(faces), heap(&marches[2]), "one set of slots");
            let band = summed(faces, &census, &reply);
            assert_eq!(texel_bits(&band), texel_bits(&t9i), "the texels");
            assert_eq!(
                band, t9i,
                "no eye light of their own, as band_rows gives none"
            );
        }
        let glare = Glare::of_listed(
            &observer(),
            census.listed(),
            &spec,
            Magnitudes::new(EYE_CUT),
        );
        let t9i_glare = glare_of(spec, &sources_of(census.listed()));
        assert_eq!(
            (&glare.sources, &glare.pyramid),
            (&t9i_glare.sources, &t9i_glare.pyramid),
            "every listed star glares, with T9.i's light"
        );
        assert_eq!(glare.eye_cut, Some(Magnitudes::new(EYE_CUT)));
        let mut band = summed(eye_only_marches(), &census, &reply);
        limit_map(&eye, &spec, &glare, &mut band);
        limit_map(&eye, &spec, &t9i_glare, &mut t9i);
        assert_eq!(map_bits(&band), map_bits(&t9i), "the map");
        let offsets = |band: &[BandTexel], glare: &Glare| -> Vec<u64> {
            eye_offsets(&eye, &spec, glare, band)
                .iter()
                .map(|o| bits(o.value()))
                .collect()
        };
        assert_eq!(
            offsets(&band, &glare),
            offsets(&t9i, &t9i_glare),
            "the eye offsets"
        );
    }

    /// A band whose texels hold the light fainter than the eye's cut is refused by a glare built to
    /// another cut (R06.T9.j): a glare to the camera's cut would have the stars between the cuts
    /// glare over a background that holds their light already. A debug assertion, as the band's
    /// cut is the march's and the glare's the caller's.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "under a glare to V")]
    fn a_glare_to_another_eye_cut_is_refused_by_the_eyes_light() {
        let spec = spec(16);
        let census = merge_census(
            [(camera_stars().to_vec(), CensusTallies::default())],
            NonZeroU32::new(MAX_N_MAX).expect("not zero"),
        );
        let mut band = summed(camera_marches(), &census, &camera_replies()[0]);
        let glare = Glare::of_listed(
            &observer(),
            census.listed(),
            &spec,
            Magnitudes::new(CAMERA_CUT),
        );
        limit_map(&EyeObserver::default(), &spec, &glare, &mut band);
    }

    /// The eye cut's passes for the default eye near the Sun: built once.
    fn eye_cut_near_the_sun() -> &'static EyeCutPasses {
        static PASSES: OnceLock<EyeCutPasses> = OnceLock::new();
        PASSES.get_or_init(|| {
            eye_cut_passes(
                milky_way_galaxy(),
                &mut context(),
                &observer(),
                &EyeObserver::default(),
            )
        })
    }

    /// The colour table's largest eye colour offset at μ 30, written again from the table: each
    /// row's [`star_colour_offset`] in a fully scotopic sky of reference light, as the census's
    /// own bound reads it.
    fn table_largest_colour_offset() -> f64 {
        use crate::tables::star_colour::{NORMAL, WHITE_DWARF};
        let dark = SkyBackground::new(
            luminance(MagnitudesPerArcsec2::new(30.0)),
            SpRatio::REFERENCE,
        )
        .expect("a background");
        NORMAL
            .iter()
            .chain(&WHITE_DWARF)
            .map(|row| {
                let rho = SpRatio::new(row[3]).expect("a row's ratio");
                star_colour_offset(rho, &dark).value()
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// The band near the Sun at `cut` with no census, complete everywhere, at `spec`'s faces: six
    /// faces in [`CubeFace::ALL`]'s order, without limits.
    fn band_at(cut: f64, spec: BandSpec) -> Vec<BandTexel> {
        let query = query(cut);
        let side = spec.face_texels();
        let mut band = Vec::new();
        for face in CubeFace::ALL {
            band_rows(
                milky_way_galaxy(),
                &mut context(),
                &query,
                &SkyCensus::empty(),
                &CompleteTo::everywhere(),
                &spec,
                face,
                0..side,
                &mut band,
            );
        }
        band
    }

    /// The darkest texel of `band` by Crumey's limit at its own light and ρ, written again from
    /// `naked_eye_limit` rather than read from the limit map: its index, limit and surface
    /// brightness.
    fn darkest_texel(band: &[BandTexel], eye: &EyeObserver) -> (usize, f64, f64) {
        band.iter()
            .enumerate()
            .map(|(i, texel)| {
                let own = SkyBackground::new(
                    texel.luminance(),
                    SpRatio::new(texel.sp_ratio()).expect("a ratio"),
                )
                .expect("a background");
                let mu = surface_brightness(texel.luminance())
                    .expect("a lit texel")
                    .value();
                (i, naked_eye_limit(eye, &own).value(), mu)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a band of texels")
    }

    /// The cut is the darkest pre-pass texel's `naked_eye_limit` plus the colour table's largest
    /// eye colour offset (0.453) plus 0.1, after the repeat, to 10⁻⁹ mag (R06.T9.d), written again
    /// from the definition: the 16² band near the Sun at the repeat's cut, each texel's limit
    /// Crumey's at its own light and ρ, and the offset the table's largest row's at μ 30. The
    /// first pass's cut is the same at the provisional 7.85. The darkest limits, and each cut from
    /// them, are those bits exactly, and the public function gives the repeat's cut, bit for bit.
    #[test]
    fn the_eye_cut_is_the_darkest_pre_pass_limit_plus_the_largest_colour_offset_and_the_pad() {
        let eye = EyeObserver::default();
        let passes = eye_cut_near_the_sun();
        let offset = table_largest_colour_offset();
        let spec = spec(16);
        let latitudes = texel_latitudes(spec);
        let repeat = passes
            .repeat
            .expect("near the Sun the first cut is deeper than 7.85, so the pre-pass repeats");
        for (band_cut, pass) in [
            (PROVISIONAL_CUT_V, passes.first),
            (passes.first.cut.value(), repeat),
        ] {
            let (index, darkest, mu) = darkest_texel(&band_at(band_cut, spec), &eye);
            let expected = darkest + offset + 0.1;
            eprintln!(
                "near the Sun, the pre-pass at V {band_cut:.4}: the darkest 16² texel (|b| \
                 {:.1}°, μ {mu:.3}) sees to {darkest:.4}, so the cut is {:.4} (+{offset:.4} \
                 +0.1)",
                latitudes[index].1,
                pass.cut.value()
            );
            assert!(
                (pass.darkest.value() - darkest).abs() < 1e-9,
                "the darkest limit {:?} against {darkest}",
                pass.darkest
            );
            assert!(
                (pass.cut.value() - expected).abs() < 1e-9,
                "the cut {:?} against {expected}",
                pass.cut
            );
            // With no glare the map's background is the texel's own light and ρ, bit for bit, and
            // the band is the same whatever the noise cache holds.
            assert_eq!(bits(pass.darkest.value()), bits(darkest));
            assert_eq!(
                bits(pass.cut.value()),
                bits(darkest + largest_colour_offset().value() + CUT_PAD_MAG)
            );
        }
        let cut = eye_cut(milky_way_galaxy(), &mut context(), &observer(), &eye);
        assert_eq!(bits(cut.value()), bits(repeat.cut.value()));
    }

    /// The cut depends on nothing a server's job has asked before: on one context whose noise
    /// cache stays warm, near the Sun and in the nuclear disc in any order, each observer's cut
    /// has the same bits, and the Sun's are those of a cold context's.
    #[test]
    fn the_eye_cut_is_the_same_whatever_was_asked_before_it() {
        use std::cell::RefCell;

        use hyperion_testkit::order::assert_order_independent;

        let eye = EyeObserver::default();
        let nuclear = Observer::new(
            GalacticPosition::from_light_years([0.0, 150.0, 0.0]).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let ctx = RefCell::new(context());
        let cut = |at: &Observer| {
            bits(eye_cut(milky_way_galaxy(), &mut ctx.borrow_mut(), at, &eye).value())
        };
        assert_order_independent(&[observer(), nuclear], cut);
        assert_eq!(cut(&observer()), bits(eye_cut_near_the_sun().cut().value()));
    }

    /// The colour table's largest eye colour offset at μ 30 is at most 0.46 (it is 0.453, the
    /// 500,000 K blackbody rows; the orchestrator's ruling of 2026-10-06 on T9.h's open question
    /// 1), and [`largest_colour_offset`], which the cut adds, is it.
    #[test]
    fn the_colour_tables_largest_eye_colour_offset_is_at_most_0_46() {
        let largest = table_largest_colour_offset();
        eprintln!("the colour table's largest eye colour offset at μ 30 is {largest:+.4}");
        assert!(largest <= 0.46, "{largest}");
        assert!(
            largest > 0.44,
            "the hottest rows' offset lies past the 120,000 K row's +0.442: {largest}"
        );
        assert!(
            (largest_colour_offset().value() - largest).abs() < 1e-12,
            "{:?} against {largest}",
            largest_colour_offset()
        );
    }

    /// Near the Sun the cut is 8.15 ± 0.22 (`decision-r06-t9b-band.md`, item 1): Crumey's limit at
    /// the darkest 16² texel of Gaia DR3's light fainter than the cut (μ 24.73–24.77), plus
    /// 0.553, with T9.b's 0.5 mag of μ through Crumey's slope there. The fixture, whose poles are about
    /// 0.3 mag faint (R06's Risks, "The galaxy's local light is low"), gives about 8.27.
    #[test]
    fn near_the_sun_the_eye_cut_is_8_15_within_0_22() {
        let cut = eye_cut_near_the_sun().cut().value();
        eprintln!("near the Sun the eye's cut is V {cut:.4}");
        assert!((cut - 8.15).abs() <= 0.22, "{cut}");
    }

    /// The pre-pass's one repeat changes the cut by under 0.05 mag near the Sun, and only deepens
    /// it: a deeper cut takes light from the band.
    #[test]
    fn the_repeat_moves_the_cut_by_under_0_05_mag() {
        let passes = eye_cut_near_the_sun();
        let repeat = passes.repeat.expect("the pre-pass repeats near the Sun");
        let moved = (repeat.cut - passes.first.cut).value();
        eprintln!(
            "near the Sun the repeat moves the cut from {:.4} to {:.4}, by {moved:+.4}",
            passes.first.cut.value(),
            repeat.cut.value()
        );
        assert!((0.0..0.05).contains(&moved), "{moved}");
    }

    /// No texel of the full limit map with no glare is deeper than the cut less the largest colour
    /// offset, and a miss is a finding for the pad, not a looser test (R06.T9.d, restated by
    /// `decision-r06-t9c-glare.md`). The no-glare map bounds every listed star's own limit after
    /// its self-exclusion (R06.T9.h).
    ///
    /// This runs near the Sun, at the server's 64² faces and at the eye's cut, against the band
    /// with no census, complete everywhere. That is the darkest band any reply at that cut has: a
    /// census's caps only add the light beyond them, and its overflow adds its stars' own light.
    /// So its limits are the deepest of any reply's map without glare.
    ///
    /// With the glare of the stars a census lists within [`GLARE_RADIUS_LY`] at the cut, every
    /// listed star's own limit, its texel's plus its eye offset, is no deeper than the cut.
    #[test]
    fn no_texel_of_the_map_without_glare_is_deeper_than_the_cut_less_the_largest_colour_offset() {
        let eye = EyeObserver::default();
        let cut = eye_cut_near_the_sun().cut();
        let spec = BandSpec::STANDARD;
        let mut band = band_at(cut.value(), spec);
        limit_map(&eye, &spec, &Glare::default(), &mut band);
        let latitudes = texel_latitudes(spec);
        let (index, deepest) = band
            .iter()
            .map(|t| t.eye_limit().expect("a limit").value())
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("texels");
        let bound = (cut - largest_colour_offset()).value();
        eprintln!(
            "near the Sun at cut {:.4}, the deepest 64² texel without glare (|b| {:.1}°) sees to \
             {deepest:.4}, {:.4} within the cut less the largest colour offset, {bound:.4}",
            cut.value(),
            latitudes[index].1,
            bound - deepest
        );
        assert!(
            deepest <= bound,
            "a texel sees to {deepest}, past {bound}: a finding for the pad"
        );
        let stars = listed_near_the_sun(cut.value(), GLARE_RADIUS_LY);
        let glare = Glare::of_listed(&observer(), &stars, &spec, cut);
        limit_map(&eye, &spec, &glare, &mut band);
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        let (mut faintest_own, mut largest_offset) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (source, offset) in glare.sources.iter().zip(&offsets) {
            let direction = source.direction.expect("a star away from the observer");
            let (at, _) = texel_of(spec, &direction);
            let own = band[at].eye_limit().expect("a limit") + *offset;
            faintest_own = faintest_own.max(own.value());
            largest_offset = largest_offset.max(offset.value());
        }
        eprintln!(
            "{} stars listed within {GLARE_RADIUS_LY} ly at the cut: the deepest own limit is \
             {faintest_own:.4}, the largest eye offset {largest_offset:+.4}",
            stars.len()
        );
        assert!(
            faintest_own <= cut.value(),
            "a listed star sees to {faintest_own}, past the cut {cut:?}"
        );
    }

    /// Where the first cut is no deeper than the provisional 7.85, as in the nuclear disc's bright
    /// sky, the pre-pass does not repeat and its first cut is the eye's.
    #[test]
    fn in_a_bright_sky_the_pre_pass_does_not_repeat() {
        let eye = EyeObserver::default();
        let at = GalacticPosition::from_light_years([0.0, 150.0, 0.0]).expect("in the cube");
        let nuclear = Observer::new(at, UniverseTime::EPOCH).expect("an observer");
        let passes = eye_cut_passes(milky_way_galaxy(), &mut context(), &nuclear, &eye);
        eprintln!(
            "in the nuclear disc the darkest 16² texel sees to {:.4}, so the cut is {:.4}",
            passes.first.darkest.value(),
            passes.first.cut.value()
        );
        assert!(passes.repeat.is_none(), "{passes:?}");
        assert!(passes.first.cut.value() <= PROVISIONAL_CUT_V, "{passes:?}");
        let expected = passes.first.darkest + largest_colour_offset() + Magnitudes::new(0.1);
        assert!(
            (passes.cut() - expected).value().abs() < 1e-12,
            "{:?} against {expected:?}",
            passes.cut()
        );
        let near_the_sun = eye_cut_near_the_sun().cut();
        assert!(
            passes.cut() < near_the_sun,
            "{:?} against the Sun's {near_the_sun:?}",
            passes.cut()
        );
    }

    /// An eye keen enough that its cut would pass V 11, the deepest a sky is asked to, is held
    /// there, in the first pass and the repeat: a field factor of 0.1, beyond any observer's,
    /// deepens every limit by 2.87 mag.
    #[test]
    fn a_cut_past_v_11_is_held_there() {
        let keen = EyeObserver::new(0.1, 25.0, 0.5).expect("an eye");
        let passes = eye_cut_passes(milky_way_galaxy(), &mut context(), &observer(), &keen);
        assert!(passes.repeat.is_some(), "a repeat at V 11");
        assert!(
            passes.first.darkest.value() + largest_colour_offset().value() + 0.1 > MAX_CUT_V,
            "{passes:?}"
        );
        assert_eq!(bits(passes.first.cut.value()), bits(MAX_CUT_V));
        assert_eq!(bits(passes.cut().value()), bits(MAX_CUT_V));
    }

    #[test]
    #[should_panic(expected = "a band of six faces of 8² texels")]
    fn a_band_of_another_size_is_refused_by_the_eye_offsets() {
        let band = vec![BandTexel::of_light(1e-5, 2.26); 6 * 64 - 1];
        let _ = eye_offsets(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            &band,
        );
    }

    #[test]
    #[should_panic(expected = "the limit map sets every texel's eye limit")]
    fn a_band_without_its_limits_is_refused_by_the_eye_offsets() {
        let band = vec![BandTexel::of_light(1e-5, 2.26); 6 * 64];
        let spec = BandSpec::new(8, 12).expect("a spec");
        let glare = glare_of(spec, &[(UnitVector::X, 1e-6, 2.26)]);
        let _ = eye_offsets(&EyeObserver::default(), &spec, &glare, &band);
    }

    #[test]
    #[should_panic(expected = "texels for rows 0..8 of a face of 8² texels")]
    fn texels_of_another_number_are_refused() {
        let mut texels = vec![BandTexel::of_light(1e-5, 2.26); 63];
        limit_rows(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::PosX,
            0..8,
            &mut texels,
        );
    }

    #[test]
    #[should_panic(expected = "reach past a face of 8 rows")]
    fn rows_past_the_face_are_refused() {
        let mut texels = vec![BandTexel::of_light(1e-5, 2.26); 72];
        limit_rows(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::PosX,
            0..9,
            &mut texels,
        );
    }
}
