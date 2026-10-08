//! The diffuse galactic light: the starlight the band's dust scatters into each of its rays
//! (rendering plan R06, R06.T9.g; Design note 15; decided 2026-10-07, `decision-r06-t9g-dgl.md`).
//!
//! Interstellar dust scatters as well as absorbs: about two thirds of the light it takes out of a
//! beam in the visible is scattered (Draine 2003, ApJ 598, 1017). The starlight it scatters
//! towards the observer is the diffuse galactic light (DGL), some 10–35% of the integrated
//! starlight by direction near the Sun (Toller 1981, as Leinert et al. 1998, A&AS 127, 1, Table
//! 39, give his ratios). The band holds it, so the eye's limits are taken against it.
//!
//! **The illumination** ([`Illumination`]). Every scattering point's field is taken as the
//! observer's own sky of all starlight, the local-field approximation: exact in a uniform medium,
//! and good for an observer within the stellar layer, where the dust within an optical depth of
//! the observer lies within the field's coherence scale. That sky is the band at
//! [`ILLUMINATION_SPEC`], 16² texels a face (the eye cut's pre-pass's directions), complete
//! nowhere and with no census, so it holds the tables' expected light of every star and layer,
//! listed or not, reddened by the solar row's curves as the band is. Each texel k keeps its five
//! sums F<sub>k</sub> (the band's photopic light, linear Rec. 709 red, green and blue, and
//! scotopic light), in cd m⁻², and its ray's A<sub>V</sub> to the root cube's edge,
//! A<sub>∞,k</sub>. It depends on the observer and the time alone, never on the cut, the eye, the
//! cone, the census or the replies: it is built once a request, before the eye's cut, and stated
//! on the request's queries ([`SkyQueryBuilder::illumination`]).
//!
//! **Single scattering.** For a ray along d and each sum X:
//!
//! - J<sub>X</sub>(d) = Σ<sub>k</sub> Φ<sub>X</sub>(d · d<sub>k</sub>) F<sub>X,k</sub>
//!   Ω<sub>k</sub> ÷ Σ<sub>k</sub> Φ<sub>X</sub>(d · d<sub>k</sub>) Ω<sub>k</sub> over the
//!   illumination's texels (centres d<sub>k</sub>, solid angles Ω<sub>k</sub>), with
//!   Φ<sub>X</sub>(μ) = (1 − g²) ÷ [4π (1 + g² − 2 g μ)<sup>3/2</sup>] Henyey and Greenstein's
//!   phase function (Draine 2003, eq. 4) at the sum's g. The scattering
//!   angle θ has cos θ = d · d<sub>k</sub>, so the forward peak is the light of the sources behind
//!   the dust. The kernel is normalised per target, so a uniform sky gives J = F;
//! - D<sub>X</sub> = 1 − t<sub>X</sub>(A<sub>∞</sub>), with t<sub>X</sub> the solar row's
//!   transmission of the sum through the ray's own A<sub>∞</sub> ([`Reddening::through`]). With
//!   the field the same at every depth, the emission integral along the ray is closed-form, and
//!   taking each depth's emission at the extinction rate of the light reddened to there makes it
//!   exactly this: κ<sub>X</sub> A<sub>∞</sub> ÷ 1.0857 for thin dust, κ<sub>X</sub> the sum's own
//!   moment, and 1 for thick dust. So the scattered light is reddened
//!   as the band's light is;
//! - the single-scattered light is ω<sub>X</sub> D<sub>X</sub> J<sub>X</sub>\[F\].
//!
//! **The higher orders, where the dust is thick.** S is the fixed point, on the illumination's own
//! texels, of S<sub>X</sub> = ω<sub>X</sub> D<sub>X</sub> (J<sub>X</sub>\[F\] +
//! J<sub>X</sub>\[S\]): the local field's sum over orders. A ray's light is
//!
//! DGL<sub>X</sub> = ω<sub>X</sub> D<sub>X</sub> (J<sub>X</sub>\[F\] + D<sub>X</sub>
//! J<sub>X</sub>\[S\]):
//!
//! the higher orders weighted by the ray's own depth, so they act where the dust is thick. With D
//! = 1 it is the whole series, ω ÷ (1 − ω) of a uniform medium's light. The weight is a calibration
//! against the exact plane-parallel solution, not a derivation: the plain local field's series is
//! 19–49% high at high latitude, where its first order already holds the exact multiple
//! scattering, and single scattering alone 18–31% low in the plane, where the series is right.
//! With the weight the light is within −8% to +11% of the exact solution in every latitude bin
//! near the Sun, for the fixture's dust and for a realistic layer (the ruling's §1.6).
//!
//! Each sum's iteration runs from S = 0, on its own kernel over the illumination's texels, the
//! photopic first. It stops once its largest change δ<sub>X</sub> in any texel, times
//! L<sub>X</sub> ÷ (1 − L<sub>X</sub>), is under 10⁻⁴ of the largest photopic S, at most 64
//! times. L<sub>X</sub> = ω<sub>X</sub> max<sub>k</sub> |D<sub>X,k</sub>| bounds the map's
//! contraction, since J is a weighted mean, so the error left is under 10⁻⁴ of the largest
//! photopic S. That is the ruling's "to 10⁻⁴": its literal stop, the change alone under 10⁻⁴,
//! leaves about 2 × 10⁻⁴ behind thick dust, where the ratio is ω (main's ruling of 2026-10-07;
//! R06's Risks, "Deviations in T9.g, as built"). Near the Sun the ratio is about 0.3–0.4, and a
//! sum takes about ten steps.
//!
//! **The dust's albedo and phase function.** ω and g are Draine's (2003) for the Milky Way's dust
//! at R<sub>V</sub> 3.1 (Weingartner and Draine 2001's model, the table
//! `kext_albedo_WD_MW_3.1_60_D03.all`), linear in ln λ, each at the wavelength where the sim's
//! extinction law ([`extinction_ratio`], Cardelli, Clayton and Mathis's in the optical) equals the
//! sum's moment for the solar row: ω 0.677 and g 0.536 photopic (0.554 µm), 0.675 and 0.553
//! scotopic (0.498 µm), and 0.667–0.677 and 0.50–0.565 in the channels (the red at 0.643 µm, the
//! blue at 0.443). Draine's is the self-consistent dust model of the extinction curve the sim uses.
//! His g of about 0.54 is below most fitted optical values (0.6–0.8; Gordon 2004, ASP Conf. Ser.
//! 309, 77; Mattila et al. 2018, A&A 617, A42): g 0.7 with ω 0.65 would lower the high-latitude
//! light per magnitude of dust by a fifth to a quarter (R06's Risks). Henyey and Greenstein's form
//! is within 10% r.m.s. of Draine's model over 0.48–0.96 µm, and "a good approximation … between
//! ∼0.4 and 1 µm", which takes in the blue sum's 0.443 µm. Its backward peak is about 20% low in
//! the optical, and it is about 10% low at the forward peak and 10% high near 60° (Baes, Camps and
//! Kapoor 2022, A&A 659, A149, §3.1): minor terms near the Sun, where the light is forward and side
//! scattered.
//!
//! **Left out** (R06's Risks): extended red emission, photoluminescence rather than scattering
//! (Chellew, Brandt and Hensley 2022, ApJ 932, 112: about 0.20 of the scattered light over
//! 5,000–8,000 Å in the south and 0.06 in the north; some 30% of the R band in cirrus, Witt et al.
//! 2008, ApJ 679, 497); reflection nebulae about listed stars, since the field is smooth at 16²;
//! the field's departure from the observer's own off the disc and inside clouds; and the realised
//! stars' light, since the field is the tables' expected light.
//!
//! **In the band.** [`march_rows`] keeps each ray's five diffuse sums beside its slots, from its
//! direction and its A<sub>∞</sub> alone, and [`sum_rows`] adds them to each texel's light and to
//! the eye's, so the light is the same in every reply and under any cut, census or cone. A query
//! with no illumination gives the band as before, bit for bit.
//!
//! Every value here is a fixed-order sum of IEEE operations, `sqrt`, and the pinned `libm`'s
//! `exp10` and `ln`, so it has the same bits natively and as WebAssembly, whatever the split of the
//! illumination's jobs.
//!
//! [`march_rows`]: super::band::march_rows
//! [`sum_rows`]: super::band::sum_rows
//! [`SkyQueryBuilder::illumination`]: super::census::SkyQueryBuilder::illumination

use std::fmt;
use std::ops::Range;

use crate::coords::UnitVector;
use crate::galaxy::Galaxy;
use crate::galaxy::gas::ccm::extinction_ratio;
use crate::math;
use crate::observe::Observer;
use crate::units::{Magnitudes, Micrometres};

use super::band::{BandSpec, CompleteTo, CubeFace, Sums, march_rows};
use super::census::{SkyContext, SkyQuery};
use super::colour::{Reddened, Reddening, solar_colour};
use super::eye::MAX_CUT_V;

/// The illumination's band (R06.T9.g): 16² texels a face on the standard nodes a decade.
///
/// It holds 1,536 rays of 5.2° on average, the eye cut's pre-pass's. At g 0.54 the phase
/// function's half width at half maximum is about 28°; against an 8² illumination the 16² differs
/// by at most 4% in a texel and 2% over the sky (R06's Risks, "Deviations in T9.g, as built").
pub const ILLUMINATION_SPEC: BandSpec = BandSpec::new(16, BandSpec::STANDARD.nodes_per_decade())
    .expect("16² faces on the standard nodes are a band");

/// The scattered field's convergence: the error left in every texel and sum, against the largest
/// photopic S (R06.T9.g).
const CONVERGENCE: f64 = 1e-4;

/// The most steps the scattered field's iteration takes.
const MAX_ITERATIONS: u32 = 64;

/// The cut of the illumination's query, V. It is read by nothing: the illumination is complete
/// nowhere, so every ray holds all of its light and no node takes the light fainter than a cut.
const ILLUMINATION_CUT_V: f64 = MAX_CUT_V;

/// Draine's (2003, ApJ 598, 1017) albedo and ⟨cos θ⟩ of the Milky Way's dust at R<sub>V</sub> 3.1,
/// Weingartner and Draine's (2001) model with its 60 ppm of carbon in very small grains, rows of
/// his table `kext_albedo_WD_MW_3.1_60_D03.all` over 0.4–1 µm: (wavelength, µm; albedo; ⟨cos θ⟩).
/// The rows are the ruling's (`decision-r06-t9g-dgl.md`, §1.4), with the file's own wavelengths;
/// over the band's wavelengths they give the full table's ω and g within 1.5 × 10⁻⁴ (science check
/// of R06.T9.g).
const DRAINE_2003_ROWS: [[f64; 3]; 12] = [
    [0.398_107, 0.6546, 0.5699],
    [0.4405, 0.6662, 0.5655],
    [0.446_684, 0.6676, 0.5645],
    [0.501_187, 0.6754, 0.5522],
    [0.547, 0.6774, 0.5383],
    [0.555, 0.6774, 0.5355],
    [0.602_560, 0.6763, 0.5181],
    [0.6492, 0.6731, 0.4996],
    [0.707_946, 0.6672, 0.4754],
    [0.802, 0.6542, 0.4381],
    [0.893, 0.6390, 0.4036],
    [1.000, 0.6153, 0.3630],
];

/// The wavelengths a sum's moment is sought over, µm: Draine's first row to the optical end of the
/// sim's extinction law, short of its join to the infrared at 0.9 µm, where the law is one
/// polynomial and falls monotonically.
const MOMENT_WAVELENGTHS_UM: (f64, f64) = (0.398_107, 0.9);

/// Draine's (2003) albedo and ⟨cos θ⟩ at `wavelength_um`, µm: linear in ln λ between his rows,
/// and held at the first and last rows beyond them.
#[must_use]
fn draine_2003_at(wavelength_um: f64) -> (f64, f64) {
    let rows = &DRAINE_2003_ROWS;
    let (first, last) = (rows[0], rows[rows.len() - 1]);
    if wavelength_um.is_nan() || wavelength_um <= first[0] {
        return (first[1], first[2]);
    }
    if wavelength_um >= last[0] {
        return (last[1], last[2]);
    }
    let i = rows.partition_point(|row| row[0] <= wavelength_um) - 1;
    let (low, high) = (rows[i], rows[i + 1]);
    let along =
        (math::ln(wavelength_um) - math::ln(low[0])) / (math::ln(high[0]) - math::ln(low[0]));
    (
        low[1] + along * (high[1] - low[1]),
        low[2] + along * (high[2] - low[2]),
    )
}

/// The wavelength, µm, at which the sim's extinction law's A<sub>λ</sub> ÷ A<sub>V</sub>
/// ([`extinction_ratio`]) is `moment`: bisected over [`MOMENT_WAVELENGTHS_UM`] in a fixed 64
/// steps, and held at its ends.
#[must_use]
fn wavelength_of_moment(moment: f64) -> f64 {
    let ratio = |wavelength: f64| {
        extinction_ratio(Micrometres::new(wavelength))
            .expect("an optical wavelength has an extinction ratio")
    };
    let (mut short, mut long) = MOMENT_WAVELENGTHS_UM;
    if moment.is_nan() || moment >= ratio(short) {
        return short;
    }
    if moment <= ratio(long) {
        return long;
    }
    for _ in 0..64 {
        let middle = f64::midpoint(short, long);
        if ratio(middle) > moment {
            short = middle;
        } else {
            long = middle;
        }
    }
    f64::midpoint(short, long)
}

/// Henyey and Greenstein's phase function, per steradian, at asymmetry `asymmetry` (g = ⟨cos θ⟩,
/// in (−1, 1)) and scattering angle θ of cosine `cos_theta` (Draine 2003, eq. 4): (1 − g²) ÷ [4π
/// (1 + g² − 2 g cos θ)<sup>3/2</sup>], whose integral over the sphere is 1.
#[cfg(test)]
#[must_use]
pub(crate) fn henyey_greenstein(asymmetry: f64, cos_theta: f64) -> f64 {
    let g = asymmetry;
    (1.0 - g * g) / (4.0 * core::f64::consts::PI)
        * unnormalised_phase(1.0 + g * g, 2.0 * g, cos_theta)
}

/// Henyey and Greenstein's phase function at cos θ `cos_theta` without its constant (1 − g²) ÷ 4π,
/// from `spread`, 1 + g², and `twice`, 2g: (1 + g² − 2 g cos θ)<sup>−3/2</sup>.
#[must_use]
fn unnormalised_phase(spread: f64, twice: f64, cos_theta: f64) -> f64 {
    let x = spread - twice * cos_theta;
    1.0 / (x * x.sqrt())
}

/// How the dust scatters one of the band's five sums (R06.T9.g): Draine's (2003) albedo and
/// ⟨cos θ⟩ at the wavelength where the sim's extinction law is the sum's moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scatterer {
    /// The sum's moment, A<sub>X</sub> ÷ A<sub>V</sub> as A<sub>V</sub> → 0.
    moment: f64,
    /// Its wavelength, µm.
    wavelength_um: f64,
    /// The albedo ω.
    albedo: f64,
    /// The asymmetry g, ⟨cos θ⟩.
    asymmetry: f64,
    /// 1 + g².
    spread: f64,
    /// 2g.
    twice: f64,
}

impl Scatterer {
    /// The scatterer of a sum of moment `moment`.
    #[must_use]
    fn of_moment(moment: f64) -> Self {
        let wavelength_um = wavelength_of_moment(moment);
        let (albedo, asymmetry) = draine_2003_at(wavelength_um);
        Self {
            moment,
            wavelength_um,
            albedo,
            asymmetry,
            spread: 1.0 + asymmetry * asymmetry,
            twice: 2.0 * asymmetry,
        }
    }

    /// The five sums' scatterers for the dust of the reddening curves `dust`.
    #[must_use]
    fn of_sums(dust: &Reddening) -> [Self; 5] {
        dust.sum_moments().map(Self::of_moment)
    }

    /// The albedo ω.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn albedo(&self) -> f64 {
        self.albedo
    }

    /// The asymmetry g, ⟨cos θ⟩.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn asymmetry(&self) -> f64 {
        self.asymmetry
    }

    /// The sum's moment, A<sub>X</sub> ÷ A<sub>V</sub> as A<sub>V</sub> → 0.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn moment(&self) -> f64 {
        self.moment
    }

    /// The wavelength the albedo and ⟨cos θ⟩ are taken at, µm.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn wavelength_um(&self) -> f64 {
        self.wavelength_um
    }

    /// The phase function without its constant, the kernel's weight before the per-target
    /// normalisation, at cos θ `cos_theta`.
    #[must_use]
    fn weight(&self, cos_theta: f64) -> f64 {
        unnormalised_phase(self.spread, self.twice, cos_theta)
    }
}

/// Each sum's scattering depth D<sub>X</sub> = 1 − t<sub>X</sub> through dust of transmission
/// `through`: the photopic light, red, green and blue, and the scotopic light, in the band's order.
#[must_use]
pub(crate) fn depths(through: &Reddened) -> Sums {
    let [red, green, blue] = through.transmission();
    [
        1.0 - through.photopic_transmission(),
        1.0 - red,
        1.0 - green,
        1.0 - blue,
        1.0 - through.scotopic_transmission(),
    ]
}

/// A texel of the illumination: its centre's direction, unit, on the galactic axes, and its solid
/// angle, sr.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Texel {
    direction: [f64; 3],
    solid_angle_sr: f64,
}

/// The texels of a band of `spec`, in the band's order (each face in [`CubeFace::ALL`]'s, row by
/// row from the top, each row from its left).
#[must_use]
fn texels_of(spec: BandSpec) -> Vec<Texel> {
    let side = spec.face_texels();
    CubeFace::ALL
        .into_iter()
        .flat_map(|face| {
            (0..side).flat_map(move |row| {
                (0..side).map(move |column| Texel {
                    direction: spec.texel_direction(face, row, column).components(),
                    solid_angle_sr: spec.texel_solid_angle_sr(row, column),
                })
            })
        })
        .collect()
}

/// The partial sums each phase-weighted mean keeps: texel k's terms go to lane k mod 4, and the
/// lanes are joined in a fixed order at the end, so the sums are the same whether or not the
/// compiler runs four texels' terms together.
const LANES: usize = 4;

/// Per-sum values on the illumination's texels, sum by sum (the band's order of sums), each a run
/// over the texels in the band's order: the layout the phase-weighted means read four at a time.
type Columns = [Vec<f64>; 5];

/// Per-texel sums `rows` as [`Columns`].
#[must_use]
fn columns_of(rows: &[Sums]) -> Columns {
    std::array::from_fn(|s| rows.iter().map(|row| row[s]).collect())
}

/// The illumination's texels as the phase-weighted means read them: their centres' galactic x, y
/// and z, unit, and their solid angles, sr, each a run over the texels in the band's order.
#[derive(Debug, Clone, PartialEq)]
struct Grid {
    x: Vec<f64>,
    y: Vec<f64>,
    z: Vec<f64>,
    solid_angle_sr: Vec<f64>,
}

impl Grid {
    /// The grid of `texels`.
    #[must_use]
    fn of(texels: &[Texel]) -> Self {
        Self {
            x: texels.iter().map(|t| t.direction[0]).collect(),
            y: texels.iter().map(|t| t.direction[1]).collect(),
            z: texels.iter().map(|t| t.direction[2]).collect(),
            solid_angle_sr: texels.iter().map(|t| t.solid_angle_sr).collect(),
        }
    }

    /// The texels it holds.
    #[must_use]
    fn len(&self) -> usize {
        self.x.len()
    }

    /// Texel `k`'s centre.
    #[must_use]
    fn direction(&self, k: usize) -> [f64; 3] {
        [self.x[k], self.y[k], self.z[k]]
    }

    /// The bytes it holds on the heap.
    #[must_use]
    fn heap_bytes(&self) -> usize {
        (self.x.capacity() + self.y.capacity() + self.z.capacity() + self.solid_angle_sr.capacity())
            * size_of::<f64>()
    }
}

/// The phase-weighted means toward `toward`, a unit direction, of each of `fields` (each sum's
/// value per texel of `grid`): for each sum X, Σ<sub>k</sub> w<sub>X,k</sub> v<sub>X,k</sub> ÷
/// Σ<sub>k</sub> w<sub>X,k</sub>, with w<sub>X,k</sub> = Ω<sub>k</sub> times the scatterer's
/// phase function at d · d<sub>k</sub> without its constant, which the ratio drops. Each sum is
/// taken in [`LANES`] partial sums over the texels in their order, then joined as ((0 + 1) + (2 +
/// 3)): four texels a step, which the compiler may run together.
#[must_use]
fn phase_means<const N: usize>(
    scatterers: &[Scatterer; 5],
    grid: &Grid,
    fields: [&Columns; N],
    toward: [f64; 3],
) -> [Sums; N] {
    let mut weights = [[0.0; LANES]; 5];
    let mut sums = [[[0.0; LANES]; 5]; N];
    let count = grid.len();
    let whole = count - count % LANES;
    let run = |column: &[f64], start: usize| -> [f64; LANES] {
        column[start..start + LANES]
            .try_into()
            .expect("a run of four texels")
    };
    for start in (0..whole).step_by(LANES) {
        let (x, y, z) = (
            run(&grid.x, start),
            run(&grid.y, start),
            run(&grid.z, start),
        );
        let omega = run(&grid.solid_angle_sr, start);
        let cos: [f64; LANES] =
            std::array::from_fn(|l| toward[0] * x[l] + toward[1] * y[l] + toward[2] * z[l]);
        for (s, scatterer) in scatterers.iter().enumerate() {
            let w: [f64; LANES] = std::array::from_fn(|l| omega[l] * scatterer.weight(cos[l]));
            for (total, w) in weights[s].iter_mut().zip(w) {
                *total += w;
            }
            for (sum, field) in sums.iter_mut().zip(&fields) {
                let values = run(&field[s], start);
                for ((sum, w), value) in sum[s].iter_mut().zip(w).zip(values) {
                    *sum += w * value;
                }
            }
        }
    }
    // A grid of an odd face has a run of fewer than four at its end, taken lane by lane.
    for (lane, k) in (whole..count).enumerate() {
        let cos = toward[0] * grid.x[k] + toward[1] * grid.y[k] + toward[2] * grid.z[k];
        for (s, scatterer) in scatterers.iter().enumerate() {
            let w = grid.solid_angle_sr[k] * scatterer.weight(cos);
            weights[s][lane] += w;
            for (sum, field) in sums.iter_mut().zip(&fields) {
                sum[s][lane] += w * field[s][k];
            }
        }
    }
    let weights = weights.map(joined);
    sums.map(|sum| std::array::from_fn(|s| joined(sum[s]) / weights[s]))
}

/// Partial sums in [`LANES`] lanes, joined in the means' fixed order: ((0 + 1) + (2 + 3)).
#[must_use]
fn joined(lanes: [f64; LANES]) -> f64 {
    (lanes[0] + lanes[1]) + (lanes[2] + lanes[3])
}

/// Σ<sub>k</sub> `weights`<sub>k</sub> `values`<sub>k</sub> in [`LANES`] partial sums over k, joined
/// as the phase-weighted means join theirs.
#[must_use]
fn lane_dot(weights: &[f64], values: &[f64]) -> f64 {
    let mut lanes = [0.0; LANES];
    for (k, (w, v)) in weights.iter().zip(values).enumerate() {
        lanes[k % LANES] += w * v;
    }
    joined(lanes)
}

/// One sum's kernel over the illumination's own texels, for its scattered field: row j holds
/// w<sub>jk</sub> = Ω<sub>k</sub> times the phase function at d<sub>j</sub> · d<sub>k</sub>
/// without its constant, with each row's total. Each cosine is taken once a pair, since d<sub>j</sub>
/// · d<sub>k</sub> and d<sub>k</sub> · d<sub>j</sub> have the same bits. It holds n² weights, 19 MB
/// at 16², while its sum is iterated.
struct Kernel {
    count: usize,
    weights: Vec<f64>,
    totals: Vec<f64>,
}

impl Kernel {
    /// The kernel of `scatterer` on `grid`.
    ///
    /// # Panics
    ///
    /// If the grid's pairs number more than the address space holds.
    #[must_use]
    fn of(scatterer: &Scatterer, grid: &Grid) -> Self {
        let count = grid.len();
        let pairs = count
            .checked_mul(count)
            .expect("an illumination's pairs number fewer than the address space holds");
        let mut weights = vec![0.0; pairs];
        for j in 0..count {
            let d = grid.direction(j);
            for k in j..count {
                let cos = d[0] * grid.x[k] + d[1] * grid.y[k] + d[2] * grid.z[k];
                let phase = scatterer.weight(cos);
                weights[j * count + k] = grid.solid_angle_sr[k] * phase;
                weights[k * count + j] = grid.solid_angle_sr[j] * phase;
            }
        }
        let totals = weights
            .chunks_exact(count)
            .map(|row| {
                let mut lanes = [0.0; LANES];
                for (k, w) in row.iter().enumerate() {
                    lanes[k % LANES] += w;
                }
                joined(lanes)
            })
            .collect();
        Self {
            count,
            weights,
            totals,
        }
    }

    /// The phase-weighted mean of `values` (one per texel) toward texel `j`.
    #[must_use]
    fn mean(&self, j: usize, values: &[f64]) -> f64 {
        let row = &self.weights[j * self.count..(j + 1) * self.count];
        lane_dot(row, values) / self.totals[j]
    }
}

/// The scattered field's fixed point on `grid` ([module](self) documentation): S<sub>X</sub> =
/// ω<sub>X</sub> D<sub>X</sub> (J<sub>X</sub>\[F\] + J<sub>X</sub>\[S\]) for the field `field`
/// and each texel's depths `depth`, iterated from zero, and the most steps a sum took.
///
/// Each sum is iterated on its own kernel ([`Kernel`]), the photopic first. It stops once its
/// largest change δ in any texel, times L ÷ (1 − L), is under [`CONVERGENCE`] of the largest
/// photopic S (the photopic's own iterate, then its fixed point), at most [`MAX_ITERATIONS`] steps:
/// L = ω max<sub>k</sub> |D<sub>k</sub>| bounds the map's contraction, since J is a weighted mean,
/// so the error left is under that.
#[must_use]
fn scattered_field(
    scatterers: &[Scatterer; 5],
    grid: &Grid,
    field: &Columns,
    depth: &[Sums],
) -> (Columns, u32) {
    let count = grid.len();
    let mut scattered: Columns = std::array::from_fn(|_| vec![0.0; count]);
    let (mut most, mut largest_photopic) = (0, 0.0);
    for (s, scatterer) in scatterers.iter().enumerate() {
        let kernel = Kernel::of(scatterer, grid);
        let direct: Vec<f64> = (0..count).map(|j| kernel.mean(j, &field[s])).collect();
        // Every value is finite, so each fold's largest is exact whatever the order.
        let contraction = scatterer.albedo * depth.iter().map(|d| d[s].abs()).fold(0.0, f64::max);
        debug_assert!(
            contraction < 1.0,
            "the scattered field's map contracts: {contraction} in sum {s}"
        );
        let bound = contraction / (1.0 - contraction);
        let (mut current, mut next) = (vec![0.0; count], Vec::with_capacity(count));
        let mut steps = 0;
        while steps < MAX_ITERATIONS {
            steps += 1;
            next.clear();
            next.extend(
                (0..count).map(|j| {
                    scatterer.albedo * depth[j][s] * (direct[j] + kernel.mean(j, &current))
                }),
            );
            let change = next
                .iter()
                .zip(&current)
                .map(|(new, old)| (new - old).abs())
                .fold(0.0, f64::max);
            std::mem::swap(&mut current, &mut next);
            if s == 0 {
                largest_photopic = current.iter().copied().fold(0.0, f64::max);
            }
            if change * bound <= CONVERGENCE * largest_photopic {
                break;
            }
        }
        most = most.max(steps);
        scattered[s] = current;
    }
    (scattered, most)
}

/// The observer's own sky of all starlight, which the band's dust scatters into each of its rays,
/// and the field that scattering leaves (R06.T9.g; see the [module](self) documentation).
///
/// It holds, for each texel of [`ILLUMINATION_SPEC`], the band's five sums of all of the
/// starlight in its direction, cd m⁻², its ray's A<sub>V</sub> to the root cube's edge, and the
/// scattered field's fixed point there: about 0.25 MB. It is a function of the observer, its time,
/// the luminosity tables, the gas and its modifiers alone, so a camera's request and an eye-only
/// request at the same observer build the same bits. A server builds it once a request, its rays
/// as jobs ([`march_rows`](Self::march_rows), [`assemble`](Self::assemble)), and states it on every
/// query of the request ([`SkyQueryBuilder::illumination`]).
///
/// [`SkyQueryBuilder::illumination`]: super::census::SkyQueryBuilder::illumination
#[derive(Clone, PartialEq)]
pub struct Illumination {
    observer: Observer,
    /// The reddening curves the depths are taken through: the solar row's, as the band's.
    dust: Reddening,
    scatterers: [Scatterer; 5],
    grid: Grid,
    /// Each texel's five sums of all of the starlight, cd m⁻²: the field F, sum by sum.
    field: Columns,
    /// Each texel's ray's A<sub>V</sub> to the root cube's edge.
    edge_a_v: Vec<Magnitudes>,
    /// Each texel's five depths D.
    depth: Vec<Sums>,
    /// The scattered field's fixed point S, cd m⁻², sum by sum.
    scattered: Columns,
    /// The steps the fixed point took.
    iterations: u32,
}

/// The illumination of some rows of one face, marched as one of a request's jobs
/// ([`Illumination::march_rows`]), for [`Illumination::assemble`].
#[derive(Debug, Clone, PartialEq)]
pub struct IlluminationRows {
    observer: Observer,
    spec: BandSpec,
    face: CubeFace,
    rows: Range<u16>,
    /// Each ray's five sums of all of the starlight, cd m⁻², in the band's order.
    field: Vec<Sums>,
    /// Each ray's A<sub>V</sub> to the root cube's edge.
    edge_a_v: Vec<Magnitudes>,
}

impl IlluminationRows {
    /// The face.
    #[must_use]
    pub const fn face(&self) -> CubeFace {
        self.face
    }

    /// The rows marched, from the top.
    #[must_use]
    pub fn rows(&self) -> Range<u16> {
        self.rows.clone()
    }
}

impl fmt::Debug for Illumination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Illumination")
            .field("observer", &self.observer)
            .field("texels", &self.grid.len())
            .field("iterations", &self.iterations)
            .finish_non_exhaustive()
    }
}

impl Illumination {
    /// The illumination for `observer`, marched face by face as one job.
    ///
    /// It is the observer's sky of all starlight at [`ILLUMINATION_SPEC`], then its scattered
    /// field. A server runs the faces' rows as jobs instead ([`march_rows`](Self::march_rows)) and
    /// assembles them ([`assemble`](Self::assemble)), which gives the same bits.
    ///
    /// It marches 1,536 rays of the luminosity tables, the gas and the modifiers of `ctx`, then
    /// iterates the scattered field, each sum on its kernel of 1,536² pairs (19 MB while it runs),
    /// about ten steps near the Sun: about 0.95 CPU-s together in a release build (R06's Risks,
    /// "Deviations in T9.g, as built").
    ///
    /// # Examples
    ///
    /// A request's illumination, stated on its query, so that its band holds the diffuse galactic
    /// light (`no_run`: the tables take a minute or more to build):
    ///
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
    /// use hyperion_sim::galaxy::gas::noise::NoiseCache;
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::observe::Observer;
    /// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
    /// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery};
    /// use hyperion_sim::sky::dgl::Illumination;
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
    /// // Once a request, before the eye's cut, which takes it too.
    /// let light = Arc::new(Illumination::march(&galaxy, &mut ctx, &observer));
    /// let eye = EyeObserver::default();
    /// let cut = eye_cut(&galaxy, &mut ctx, &observer, &eye, Some(&light));
    /// let query = SkyQuery::builder(observer, cut).eye(eye).illumination(light).build()?;
    /// let mut face = Vec::new();
    /// band_rows(&galaxy, &mut ctx, &query, &SkyCensus::empty(), &CompleteTo::everywhere(),
    ///     &BandSpec::STANDARD, CubeFace::PosZ, 0..64, &mut face);
    /// // Every texel holds the light its dust scatters, beside its starlight.
    /// assert!(face.iter().all(|texel| texel.diffuse_luminance().value() > 0.0));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn march(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer) -> Self {
        Self::march_at(galaxy, ctx, observer, ILLUMINATION_SPEC)
    }

    /// [`march`](Self::march) at a band of `spec`: the tests' coarser illumination.
    #[must_use]
    fn march_at(
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        observer: &Observer,
        spec: BandSpec,
    ) -> Self {
        let side = spec.face_texels();
        let parts: Vec<IlluminationRows> = CubeFace::ALL
            .into_iter()
            .map(|face| Self::march_rows_at(galaxy, ctx, observer, spec, face, 0..side))
            .collect();
        Self::assemble_at(observer, spec, parts)
    }

    /// Marches rows `rows` (from the top) of `face` of the illumination for `observer`, as one of a
    /// request's jobs.
    ///
    /// [`assemble`](Self::assemble) joins the parts in any order. Each ray is the band's at
    /// [`ILLUMINATION_SPEC`], complete nowhere with no census, reddened as the band is
    /// ([`march_rows`]), so it holds the tables' expected light of every star and layer. `ctx`
    /// supplies the luminosity tables, the gas modifiers and the noise cache; the noise cache
    /// changes the cost, never a value.
    ///
    /// # Panics
    ///
    /// If `rows` reaches past the face's last row.
    #[must_use]
    pub fn march_rows(
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        observer: &Observer,
        face: CubeFace,
        rows: Range<u16>,
    ) -> IlluminationRows {
        Self::march_rows_at(galaxy, ctx, observer, ILLUMINATION_SPEC, face, rows)
    }

    /// [`march_rows`](Self::march_rows) at a band of `spec`.
    #[must_use]
    fn march_rows_at(
        galaxy: &Galaxy,
        ctx: &mut SkyContext<'_>,
        observer: &Observer,
        spec: BandSpec,
        face: CubeFace,
        rows: Range<u16>,
    ) -> IlluminationRows {
        let query = SkyQuery::builder(*observer, Magnitudes::new(ILLUMINATION_CUT_V))
            .build()
            .expect("an observer's query at V 11 with nothing else asked is a query");
        let nowhere = CompleteTo::nowhere();
        let march = march_rows(galaxy, ctx, &query, [nowhere], &spec, face, rows.clone());
        IlluminationRows {
            observer: *observer,
            spec,
            face,
            rows,
            field: march.ray_light(&nowhere),
            edge_a_v: march.edge_a_v().to_vec(),
        }
    }

    /// The illumination for `observer` from its marched rows `parts`, in any order and any split.
    ///
    /// It then iterates its scattered field ([module](self) documentation), which is one job.
    ///
    /// # Panics
    ///
    /// Unless `parts` hold every texel of [`ILLUMINATION_SPEC`] once, each marched for `observer`
    /// at that band.
    #[must_use]
    pub fn assemble(
        observer: &Observer,
        parts: impl IntoIterator<Item = IlluminationRows>,
    ) -> Self {
        Self::assemble_at(observer, ILLUMINATION_SPEC, parts)
    }

    /// [`assemble`](Self::assemble) at a band of `spec`.
    #[must_use]
    fn assemble_at(
        observer: &Observer,
        spec: BandSpec,
        parts: impl IntoIterator<Item = IlluminationRows>,
    ) -> Self {
        let side = usize::from(spec.face_texels());
        let count = CubeFace::ALL.len() * side * side;
        let mut field: Vec<Option<(Sums, Magnitudes)>> = vec![None; count];
        for part in parts {
            assert!(
                part.observer == *observer && part.spec == spec,
                "an illumination's rows of {:?} at {:?} join one of {observer:?} at {spec:?}",
                part.observer,
                part.spec
            );
            let first =
                (usize::from(part.face.layer()) * side + usize::from(part.rows.start)) * side;
            for (i, ray) in part.field.iter().zip(&part.edge_a_v).enumerate() {
                let slot = &mut field[first + i];
                assert!(
                    slot.is_none(),
                    "texel {} of the illumination is marched twice",
                    first + i
                );
                *slot = Some((*ray.0, *ray.1));
            }
        }
        let (field, edge_a_v): (Vec<Sums>, Vec<Magnitudes>) = field
            .into_iter()
            .enumerate()
            .map(|(i, ray)| {
                ray.unwrap_or_else(|| panic!("texel {i} of the illumination is not marched"))
            })
            .unzip();
        Self::of_field(
            *observer,
            spec,
            &solar_colour().reddening(),
            &field,
            edge_a_v,
        )
    }

    /// The illumination of the sky `field` (each texel's five sums, cd m⁻²) and each texel's
    /// A<sub>V</sub> to the edge `edge_a_v`, at a band of `spec`, for `observer`, its depths taken
    /// through the reddening curves `dust`: the solar row's for the band, a grey dust's or a
    /// constructed sky's in the tests.
    #[must_use]
    fn of_field(
        observer: Observer,
        spec: BandSpec,
        dust: &Reddening,
        field: &[Sums],
        edge_a_v: Vec<Magnitudes>,
    ) -> Self {
        let grid = Grid::of(&texels_of(spec));
        assert!(
            field.len() == grid.len() && edge_a_v.len() == grid.len(),
            "an illumination of {} texels takes {} fields and {} depths",
            grid.len(),
            field.len(),
            edge_a_v.len()
        );
        let field = columns_of(field);
        let scatterers = Scatterer::of_sums(dust);
        let depth: Vec<Sums> = edge_a_v.iter().map(|&a| depths(&dust.through(a))).collect();
        let (scattered, iterations) = scattered_field(&scatterers, &grid, &field, &depth);
        Self {
            observer,
            dust: *dust,
            scatterers,
            grid,
            field,
            edge_a_v,
            depth,
            scattered,
            iterations,
        }
    }

    /// The observer, at its time, whose sky this is.
    #[must_use]
    pub const fn observer(&self) -> &Observer {
        &self.observer
    }

    /// The steps the scattered field's fixed point took: about ten near the Sun, at most 64.
    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }

    /// The bytes it holds on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let columns =
            |columns: &Columns| columns.iter().map(Vec::capacity).sum::<usize>() * size_of::<f64>();
        self.grid.heap_bytes()
            + columns(&self.field)
            + columns(&self.scattered)
            + self.depth.capacity() * size_of::<Sums>()
            + self.edge_a_v.capacity() * size_of::<Magnitudes>()
    }

    /// Each texel's ray's A<sub>V</sub> to the root cube's edge, in the band's order.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn edge_a_v(&self) -> &[Magnitudes] {
        &self.edge_a_v
    }

    /// Each texel's centre, unit, on the galactic axes, in the band's order.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn directions(&self) -> Vec<UnitVector> {
        (0..self.grid.len())
            .map(|k| UnitVector::from_components(self.grid.direction(k)).expect("a texel's centre"))
            .collect()
    }

    /// Every value it holds as bits, in a fixed order: the field, the dust to the edge, the depths,
    /// the scattered field and the steps it took, for the tests' bit-for-bit comparisons.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn bits(&self) -> Vec<u64> {
        use hyperion_testkit::float::bits;

        let mut out: Vec<u64> = self.field.iter().flatten().map(|&v| bits(v)).collect();
        out.extend(self.edge_a_v.iter().map(|a| bits(a.value())));
        out.extend(self.depth.iter().flatten().map(|&v| bits(v)));
        out.extend(self.scattered.iter().flatten().map(|&v| bits(v)));
        out.push(u64::from(self.iterations));
        out
    }

    /// The single-scattered light toward `toward` of a ray of dust `edge_a_v` to the edge,
    /// ω<sub>X</sub> D<sub>X</sub> J<sub>X</sub>\[F\], for the tests.
    #[cfg(test)]
    #[must_use]
    fn single_toward(&self, toward: &UnitVector, edge_a_v: Magnitudes) -> Sums {
        let depth = depths(&self.dust.through(edge_a_v));
        let [direct] = phase_means(
            &self.scatterers,
            &self.grid,
            [&self.field],
            toward.components(),
        );
        std::array::from_fn(|s| self.scatterers[s].albedo * depth[s] * direct[s])
    }

    /// The diffuse galactic light toward `toward`, on the galactic axes, of a ray whose
    /// A<sub>V</sub> to the root cube's edge is `edge_a_v`: its five sums, cd m⁻²,
    /// DGL<sub>X</sub> = ω<sub>X</sub> D<sub>X</sub> (J<sub>X</sub>\[F\] + D<sub>X</sub>
    /// J<sub>X</sub>\[S\]) ([module](self) documentation). A ray with no dust holds none.
    #[must_use]
    pub(crate) fn diffuse_toward(&self, toward: &UnitVector, edge_a_v: Magnitudes) -> Sums {
        if edge_a_v.value() <= 0.0 {
            return [0.0; 5];
        }
        let depth = depths(&self.dust.through(edge_a_v));
        let [direct, scattered] = phase_means(
            &self.scatterers,
            &self.grid,
            [&self.field, &self.scattered],
            toward.components(),
        );
        std::array::from_fn(|s| {
            self.scatterers[s].albedo * depth[s] * (direct[s] + depth[s] * scattered[s])
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::sky::census::BuildSkyQueryError;
    use crate::sky::eye::surface_brightness;
    use crate::sky::testing::{sun_illumination, sun_observer};
    use crate::time::UniverseTime;
    use crate::units::CandelasPerSquareMetre;
    use crate::units::consts::RADIANS_PER_DEGREE;

    /// The band's dust: the solar row's reddening curves.
    fn solar() -> Reddening {
        solar_colour().reddening()
    }

    /// Grey dust: every band dimmed as V is.
    fn grey() -> Reddening {
        solar().with_grey_dust()
    }

    /// The magnitudes per optical depth, 2.5 ÷ ln 10: 1.0857.
    fn magnitudes_per_depth() -> f64 {
        2.5 / core::f64::consts::LN_10
    }

    /// An illumination at [`ILLUMINATION_SPEC`] of a constructed sky, for the tests that need no
    /// galaxy: each texel's five sums `field` and its dust to the edge `edge`, depths through `dust`.
    fn sky_of(
        dust: &Reddening,
        field: impl Fn(&Texel) -> Sums,
        edge: impl Fn(&Texel) -> Magnitudes,
    ) -> Illumination {
        let texels = texels_of(ILLUMINATION_SPEC);
        Illumination::of_field(
            sun_observer(),
            ILLUMINATION_SPEC,
            dust,
            &texels.iter().map(field).collect::<Vec<Sums>>(),
            texels.iter().map(edge).collect(),
        )
    }

    /// Henyey and Greenstein's phase function in closed form, written again: (1 − g²) ÷ (4π (1 + g²
    /// − 2gμ)^1.5), through `powf`.
    fn closed_form(g: f64, mu: f64) -> f64 {
        (1.0 - g * g) / (4.0 * core::f64::consts::PI) * math::powf(1.0 + g * g - 2.0 * g * mu, -1.5)
    }

    /// The unit direction of `components`, a unit vector already.
    fn unit(components: [f64; 3]) -> UnitVector {
        UnitVector::from_components(components).expect("a direction")
    }

    /// A band's texel directions and solid angles, at `face_texels`² a face.
    fn grid(face_texels: u16) -> Vec<Texel> {
        texels_of(BandSpec::new(face_texels, 12).expect("a band"))
    }

    /// The galactic latitude |b| of a unit direction, degrees.
    fn latitude(direction: [f64; 3]) -> f64 {
        math::asin(direction[2].abs()) / RADIANS_PER_DEGREE
    }

    /// The kernel is Henyey and Greenstein's to 10⁻⁷ at 10⁴ points of cos θ for each sum's g; each
    /// target's renormalised weights sum to 1 within 10⁻¹² over the 16² and 64² grids, and the
    /// unrenormalised midpoint sum, Σ Φ Ω, is within 2 × 10⁻³ of 1; and Draine's V row gives ω
    /// 0.6774 and g 0.5383 (R06.T9.g, test 1). The sums' wavelengths, albedos and asymmetries are
    /// the ruling's (its §1.4).
    #[test]
    fn the_phase_kernel_is_henyey_greensteins_and_normalised() {
        let scatterers = Scatterer::of_sums(&solar());
        let names = ["photopic", "red", "green", "blue", "scotopic"];
        for (name, scatterer) in names.iter().zip(&scatterers) {
            let g = scatterer.asymmetry();
            let constant = (1.0 - g * g) / (4.0 * core::f64::consts::PI);
            let mut worst = 0.0_f64;
            for i in 0..=10_000_u32 {
                let mu = -1.0 + 2.0 * f64::from(i) / 10_000.0;
                let expected = closed_form(g, mu);
                for got in [constant * scatterer.weight(mu), henyey_greenstein(g, mu)] {
                    worst = worst.max((got / expected - 1.0).abs());
                }
            }
            eprintln!(
                "{name}: moment {:.4}, λ {:.4} µm, ω {:.4}, g {:.4}; the kernel within {worst:.2e} \
                 of its closed form",
                scatterer.moment(),
                scatterer.wavelength_um(),
                scatterer.albedo(),
                g
            );
            assert!(worst < 1e-7, "{name}: {worst}");
        }
        // The ruling's figures (§1.4), each "about": photopic ω 0.677 and g 0.536, scotopic 0.676
        // and 0.551 (its λ about 0.50 µm; at 0.498 µm Draine's table gives 0.675), the channels
        // 0.667–0.677 and 0.51–0.565. The red channel's moment, 0.838, falls at 0.643 µm, not the
        // ruling's about 0.62, so its g is 0.502.
        let [photopic, red, green, blue, scotopic] = scatterers;
        for (scatterer, (albedo, asymmetry)) in
            [(photopic, (0.677, 0.536)), (scotopic, (0.676, 0.551))]
        {
            assert!((scatterer.albedo() - albedo).abs() < 0.002, "{scatterer:?}");
            assert!(
                (scatterer.asymmetry() - asymmetry).abs() < 0.003,
                "{scatterer:?}"
            );
        }
        for channel in [red, green, blue] {
            assert!((0.666..=0.678).contains(&channel.albedo()), "{channel:?}");
            assert!(
                (0.495..=0.570).contains(&channel.asymmetry()),
                "{channel:?}"
            );
        }
        assert!(blue.asymmetry() > green.asymmetry() && green.asymmetry() > red.asymmetry());
        // Draine's V row, bit for bit.
        let (albedo, asymmetry) = draine_2003_at(0.547);
        assert_eq!(
            (bits(albedo), bits(asymmetry)),
            (bits(0.6774), bits(0.5383))
        );
        // The weights over the illumination's texels, for targets at 16² and 64² centres.
        let texels = texels_of(ILLUMINATION_SPEC);
        let (mut renormalised, mut midpoint) = (0.0_f64, 0.0_f64);
        for face_texels in [16, 64] {
            for target in grid(face_texels) {
                for scatterer in &scatterers {
                    let weights: Vec<f64> = texels
                        .iter()
                        .map(|texel| {
                            let cos = target.direction[0] * texel.direction[0]
                                + target.direction[1] * texel.direction[1]
                                + target.direction[2] * texel.direction[2];
                            texel.solid_angle_sr * scatterer.weight(cos)
                        })
                        .collect();
                    let total: f64 = weights.iter().sum();
                    let sum: f64 = weights.iter().map(|w| w / total).sum();
                    renormalised = renormalised.max((sum - 1.0).abs());
                    let g = scatterer.asymmetry();
                    let constant = (1.0 - g * g) / (4.0 * core::f64::consts::PI);
                    midpoint = midpoint.max((constant * total - 1.0).abs());
                }
            }
        }
        eprintln!(
            "the renormalised weights sum to 1 within {renormalised:.2e}; the midpoint sum within \
             {midpoint:.2e} of 1"
        );
        assert!(renormalised < 1e-12, "{renormalised}");
        assert!(midpoint < 2e-3, "{midpoint}");
    }

    /// A uniform sky's phase-weighted mean is its own light exactly, its single scattering is ω D
    /// F to 10⁻¹², and grey dust's depth is 1 − 10^(−0.4 A<sub>∞</sub>) to 10⁻¹² (R06.T9.g, test
    /// 2).
    #[test]
    fn a_uniform_sky_scatters_its_albedo_times_its_depth() {
        // Powers of two, so a sum of weights times each is the weights' sum times it, exactly.
        let field = [1.0, 0.5, 0.25, 2.0, 4.0];
        let a = Magnitudes::new(0.3);
        let light = sky_of(&solar(), |_| field, |_| a);
        let depth = depths(&solar().through(a));
        // The illumination's own texels, and the 8² and 64² bands' targets between them.
        let mut targets = light.directions();
        for face_texels in [8, 64] {
            targets.extend(grid(face_texels).iter().map(|texel| unit(texel.direction)));
        }
        let mut worst = 0.0_f64;
        for toward in &targets {
            let [mean] = phase_means(
                &light.scatterers,
                &light.grid,
                [&light.field],
                toward.components(),
            );
            assert_eq!(mean.map(bits), field.map(bits), "toward {toward:?}");
            let single = light.single_toward(toward, a);
            for s in 0..5 {
                let expected = light.scatterers[s].albedo * depth[s] * field[s];
                worst = worst.max((single[s] / expected - 1.0).abs());
            }
        }
        assert!(worst < 1e-12, "single scattering within {worst} of ω D F");
        let grey = grey();
        for a in [0.01, 0.3, 2.0, 7.5, 30.0, 45.0] {
            let expected = 1.0 - math::exp10(-0.4 * a);
            for (s, d) in depths(&grey.through(Magnitudes::new(a))).iter().enumerate() {
                assert!(
                    (d - expected).abs() < 1e-12,
                    "sum {s} at A {a}: D {d} against {expected}"
                );
            }
        }
    }

    /// Behind dust of A<sub>∞</sub> 100 everywhere, every order sums to ω ÷ (1 − ω) of a uniform
    /// sky's light, in the scattered field and in a ray's light, within 10⁻⁴ (R06.T9.g, test 3).
    #[test]
    fn behind_thick_dust_every_order_sums_to_omega_over_one_minus_omega() {
        let field = [1.0; 5];
        let light = sky_of(&solar(), |_| field, |_| Magnitudes::new(100.0));
        let mut worst = 0.0_f64;
        for (k, toward) in light.directions().into_iter().enumerate() {
            let diffuse = light.diffuse_toward(&toward, Magnitudes::new(100.0));
            for s in 0..5 {
                let albedo = light.scatterers[s].albedo;
                let expected = albedo / (1.0 - albedo) * field[s];
                for got in [light.scattered[s][k], diffuse[s]] {
                    worst = worst.max((got / expected - 1.0).abs());
                }
            }
        }
        eprintln!(
            "behind A 100: every order within {worst:.2e} of ω ÷ (1 − ω), in {} steps",
            light.iterations()
        );
        assert!(worst < 1e-4, "{worst}");
        assert!(
            light.iterations() < MAX_ITERATIONS,
            "{}",
            light.iterations()
        );
    }

    /// Thin dust scatters in proportion to its column: at A<sub>∞</sub> 10⁻³ and 2 × 10⁻³ the
    /// light doubles within 10⁻³, and at 10⁻³ it is ω κ<sub>X</sub> A<sub>∞</sub> ÷ 1.0857 of the
    /// light within 10⁻³ (R06.T9.g, test 4). At 2 × 10⁻³ the blue's second-order term is 1.2 ×
    /// 10⁻³ of it, which the doubling's ratio holds.
    #[test]
    fn thin_dust_scatters_in_proportion_to_its_column() {
        let field = [1.0, 0.8, 1.0, 1.3, 2.3];
        let thin = sky_of(&solar(), |_| field, |_| Magnitudes::new(1e-3));
        let thinner_twice = sky_of(&solar(), |_| field, |_| Magnitudes::new(2e-3));
        let (mut doubling, mut linear) = (0.0_f64, 0.0_f64);
        for toward in thin.directions() {
            let one = thin.diffuse_toward(&toward, Magnitudes::new(1e-3));
            let two = thinner_twice.diffuse_toward(&toward, Magnitudes::new(2e-3));
            for s in 0..5 {
                doubling = doubling.max((two[s] / one[s] / 2.0 - 1.0).abs());
                let scatterer = &thin.scatterers[s];
                let expected =
                    scatterer.albedo * scatterer.moment * 1e-3 / magnitudes_per_depth() * field[s];
                linear = linear.max((one[s] / expected - 1.0).abs());
            }
        }
        eprintln!("thin dust: doubles within {doubling:.2e}, linear within {linear:.2e}");
        assert!(doubling < 1e-3, "{doubling}");
        assert!(linear < 1e-3, "{linear}");
    }

    /// A lone lit texel: the single-scattered light in its own direction over that at 90° is the
    /// kernel's ratio, with the per-target normalisation, to 10⁻⁹, and the light forward of the
    /// source over the light backward is above 1 (R06.T9.g, test 5).
    #[test]
    fn a_lone_source_scatters_forward() {
        let texels = texels_of(ILLUMINATION_SPEC);
        // Texel (5, 9) of +Z, away from the face's symmetry.
        let lit = (4 * 16 + 5) * 16 + 9;
        let source = texels[lit].direction;
        let light = sky_of(
            &solar(),
            |texel| {
                if texel.direction.map(bits) == source.map(bits) {
                    [1.0; 5]
                } else {
                    [0.0; 5]
                }
            },
            |_| Magnitudes::new(0.1),
        );
        let across = UnitVector::from_components([source[1], -source[0], 0.0])
            .expect("a direction across the source");
        let toward_source = unit(source);
        let dust = Magnitudes::new(0.1);
        let ratio = |toward: &UnitVector, g: f64| {
            let toward = toward.components();
            let total: f64 = texels
                .iter()
                .map(|t| {
                    let cos = toward[0] * t.direction[0]
                        + toward[1] * t.direction[1]
                        + toward[2] * t.direction[2];
                    closed_form(g, cos) * t.solid_angle_sr
                })
                .sum();
            let cos = toward[0] * source[0] + toward[1] * source[1] + toward[2] * source[2];
            closed_form(g, cos) * texels[lit].solid_angle_sr / total
        };
        let (forward, side) = (
            light.single_toward(&toward_source, dust),
            light.single_toward(&across, dust),
        );
        for s in 0..5 {
            let g = light.scatterers[s].asymmetry;
            let expected = ratio(&toward_source, g) / ratio(&across, g);
            let got = forward[s] / side[s];
            assert!(
                (got / expected - 1.0).abs() < 1e-9,
                "sum {s}: {got} against the kernel's {expected}"
            );
        }
        let backward = unit(source.map(|c| -c));
        let (ahead, behind) = (
            light.diffuse_toward(&toward_source, dust),
            light.diffuse_toward(&backward, dust),
        );
        eprintln!(
            "a lone source: forward over 90° {:.3}, forward over backward {:.3}",
            forward[0] / side[0],
            ahead[0] / behind[0]
        );
        for s in 0..5 {
            assert!(
                ahead[s] > behind[s],
                "sum {s}: {} against {}",
                ahead[s],
                behind[s]
            );
        }
    }

    /// Each sum's depth is 1 − t<sub>X</sub>(A<sub>∞</sub>) to 10⁻¹⁵ and grows with
    /// A<sub>∞</sub>; at A<sub>∞</sub> 40 it is at least 1 − 10⁻⁸ for the photopic, scotopic, red
    /// and green light; and D<sub>X</sub> ÷ A<sub>∞</sub> is κ<sub>X</sub> ÷ 1.0857 within 10⁻³ at
    /// A<sub>∞</sub> 10⁻³ (R06.T9.g, test 6). A signed channel whose two parts the dust dims apart
    /// can pass zero and come back towards it behind thick dust, as the blue does near A 7–8 and
    /// the green, by some 10⁻¹⁰, past A 20: its depth then rises a hair past 1 and falls back, as
    /// the band's own channel does. The photopic, red, green and scotopic depths grow to 10⁻⁹, and
    /// the blue's lies within 10⁻³ of 1 from there.
    #[test]
    fn the_depth_is_one_less_the_reddened_transmission() {
        let dust = solar();
        let moments = dust.sum_moments();
        let mut previous = [f64::NEG_INFINITY; 5];
        let (mut blue_depth, mut green_depth) =
            ((0.0_f64, f64::INFINITY), (0.0_f64, f64::INFINITY));
        for step in 0..=240_u32 {
            let a = 0.25 * f64::from(step);
            let through = dust.through(Magnitudes::new(a));
            let depth = depths(&through);
            let [red, green, b] = through.transmission();
            let expected = [
                1.0 - through.photopic_transmission(),
                1.0 - red,
                1.0 - green,
                1.0 - b,
                1.0 - through.scotopic_transmission(),
            ];
            for s in 0..5 {
                assert!((depth[s] - expected[s]).abs() <= 1e-15, "sum {s} at A {a}");
            }
            for s in [0, 1, 2, 4] {
                assert!(
                    depth[s] >= previous[s] - 1e-9,
                    "sum {s} at A {a}: {} after {}",
                    depth[s],
                    previous[s]
                );
            }
            if step > 0 {
                blue_depth = (
                    blue_depth.0.max(depth[3]),
                    blue_depth.1.min(depth[3] - previous[3]),
                );
                green_depth = (
                    green_depth.0.max(depth[2]),
                    green_depth.1.min(depth[2] - previous[2]),
                );
            }
            previous = depth;
            if step == 160 {
                for s in [0, 1, 2, 4] {
                    assert!(depth[s] >= 1.0 - 1e-8, "sum {s} at A 40: {}", depth[s]);
                }
            }
        }
        eprintln!(
            "the blue's depth reaches {:+.3e} past 1, its least step {:+.3e}; the green's {:+.3e} \
             and {:+.3e}",
            blue_depth.0 - 1.0,
            blue_depth.1,
            green_depth.0 - 1.0,
            green_depth.1
        );
        assert!(blue_depth.0 < 1.0 + 1e-3, "{blue_depth:?}");
        let depth = depths(&dust.through(Magnitudes::new(1e-3)));
        for s in 0..5 {
            let rate = depth[s] / 1e-3;
            let expected = moments[s] / magnitudes_per_depth();
            assert!(
                (rate / expected - 1.0).abs() < 1e-3,
                "sum {s}: {rate} per magnitude against {expected}"
            );
        }
    }

    /// The local light near the Sun that the ruling's plane-parallel sky takes (Flynn et al. 2006,
    /// MNRAS 372, 1149, §2.3: 0.056 L☉ pc⁻³), as 41% in a layer of scale height 100 pc and 59% at
    /// 300 pc, so that its column is Flynn et al.'s Σ<sub>L</sub> of 24.4 L☉ pc⁻²; the heights are
    /// the ruling's (`decision-r06-t9g-dgl.md`, its method): (density, L☉,V pc⁻³; height, pc).
    const FLYNN_LIGHT: [(f64, f64); 2] = [(0.056 * 0.41, 100.0), (0.056 * 0.59, 300.0)];

    /// The plane-parallel sky's grey dust: 0.7 mag kpc⁻¹ in the plane (Marshall et al. 2006, A&A
    /// 453, 635, §3.3, the Besançon model's "usually 0.7"), in an exponential layer of scale height
    /// 125 pc. The exponential at 125 pc is the ruling's assumption: Marshall et al.'s 125 (+17,
    /// −7) pc is their sech² fit, and their exponential fit is 134 (+44, −11) pc (§5.5.1), which
    /// moves the bins by +4% to +7.5% (science check of R06.T9.g; R06's Risks).
    const DUST_IN_THE_PLANE_MAG_PER_PC: f64 = 0.7e-3;
    const DUST_HEIGHT_PC: f64 = 125.0;

    /// The plane-parallel sky's observer, pc above the plane: the ruling's 20.8, the fixture's 68 ly
    /// (20.85 pc) rounded.
    const OBSERVER_PC: f64 = 20.8;

    /// Toller's (1981) diffuse light over the starlight along the line of sight by |b|, at λ ≈ 440
    /// nm, as Leinert et al. (1998, A&AS 127, 1, Table 39) give it: (from, to, degrees; ratio; its
    /// 1σ).
    const TOLLER: [(f64, f64, f64, f64); 8] = [
        (0.0, 5.0, 0.21, 0.05),
        (5.0, 10.0, 0.34, 0.07),
        (10.0, 15.0, 0.31, 0.03),
        (15.0, 20.0, 0.19, 0.04),
        (20.0, 30.0, 0.25, 0.04),
        (30.0, 40.0, 0.17, 0.04),
        (40.0, 60.0, 0.17, 0.02),
        (60.0, 90.1, 0.12, 0.02),
    ];

    /// The ruling's model of the same sky in V, by Toller's bins (`decision-r06-t9g-dgl.md`,
    /// §2.3).
    const RULING_BINS: [f64; 8] = [0.286, 0.213, 0.192, 0.180, 0.165, 0.147, 0.129, 0.113];

    /// Toller's wavelength, µm (Pioneer 10's blue, Leinert et al. 1998, §11 and Table 39).
    const TOLLER_UM: f64 = 0.44;

    /// The dust's ∫ e^(−|z| ÷ h) dz from 0 to `z`, pc.
    fn dust_profile_integral(z: f64) -> f64 {
        z.signum() * DUST_HEIGHT_PC * (1.0 - math::exp(-z.abs() / DUST_HEIGHT_PC))
    }

    /// The plane-parallel sky's A<sub>V</sub> from the observer `s` pc along a direction of sin b
    /// `sin_b`.
    fn plane_column(sin_b: f64, s: f64) -> f64 {
        if sin_b.abs() < 1e-12 {
            return DUST_IN_THE_PLANE_MAG_PER_PC * math::exp(-OBSERVER_PC / DUST_HEIGHT_PC) * s;
        }
        let z = OBSERVER_PC + s * sin_b;
        DUST_IN_THE_PLANE_MAG_PER_PC
            * (dust_profile_integral(z) - dust_profile_integral(OBSERVER_PC))
            / sin_b
    }

    /// The plane-parallel sky's A<sub>V</sub> to infinity along a direction of sin b `sin_b`.
    fn plane_edge(sin_b: f64) -> Magnitudes {
        let near = math::exp(-OBSERVER_PC / DUST_HEIGHT_PC);
        let k = DUST_IN_THE_PLANE_MAG_PER_PC * DUST_HEIGHT_PC;
        Magnitudes::new(if sin_b > 0.0 {
            k * near / sin_b
        } else {
            k * (2.0 - near) / -sin_b
        })
    }

    /// The plane-parallel sky's starlight along a direction of sin b `sin_b`, L☉ pc⁻² per unit
    /// solid angle (to a constant), in a band whose extinction is `ratio` × A<sub>V</sub>: the
    /// stars' light dimmed by the grey dust in front of it, by the trapezoid rule on 100 nodes a
    /// decade from 0.01 pc to 10⁶ pc. The stars' layers are the V band's in every band.
    fn plane_light(sin_b: f64, ratio: f64) -> f64 {
        let emitted = |s: f64| {
            let z = (OBSERVER_PC + s * sin_b).abs();
            let stars: f64 = FLYNN_LIGHT
                .iter()
                .map(|&(density, height)| density * math::exp(-z / height))
                .sum();
            stars * math::exp10(-0.4 * ratio * plane_column(sin_b, s))
        };
        let nodes: Vec<f64> = (0..=800_u32)
            .map(|i| 0.01 * math::exp10(f64::from(i) / 100.0))
            .collect();
        let values: Vec<f64> = nodes.iter().map(|&s| emitted(s)).collect();
        let near = emitted(0.0) * nodes[0];
        near + nodes
            .windows(2)
            .zip(values.windows(2))
            .map(|(d, v)| f64::midpoint(v[0], v[1]) * (d[1] - d[0]))
            .sum::<f64>()
    }

    /// The plane-parallel sky in a band of extinction `ratio` × A<sub>V</sub>, its dust grey at
    /// that ratio, so its scatterer sits at the wavelength where the sim's law is `ratio`: its 16²
    /// illumination, and the diffuse light over the starlight at the 64² band's texels in each of
    /// Toller's bins.
    fn plane_parallel_bins(ratio: f64) -> (Illumination, [f64; 8]) {
        let light = sky_of(
            &solar().with_grey_dust_at(ratio),
            |texel| [plane_light(texel.direction[2], ratio); 5],
            |texel| plane_edge(texel.direction[2]),
        );
        let (mut dgl, mut isl) = ([0.0; 8], [0.0; 8]);
        for target in grid(64) {
            let sin_b = target.direction[2];
            let b = latitude(target.direction);
            let bin = TOLLER
                .iter()
                .position(|&(from, to, ..)| (from..to).contains(&b))
                .expect("every latitude is in a bin");
            let diffuse = light.diffuse_toward(&unit(target.direction), plane_edge(sin_b));
            dgl[bin] += diffuse[0] * target.solid_angle_sr;
            isl[bin] += plane_light(sin_b, ratio) * target.solid_angle_sr;
        }
        (light, std::array::from_fn(|k| dgl[k] / isl[k]))
    }

    /// On the ruling's plane-parallel sky (no galaxy: the sky and each texel's dust by quadrature
    /// here), the diffuse light over the starlight in each of Toller's bins (R06.T9.g, test 9):
    ///
    /// - at Toller's 440 nm (the dust × 1.3245 by the sim's law, Draine's ω and g there) lies
    ///   within ×/÷ 1.5 of his 1σ range (main's rulings of 2026-10-07: the ruling's "within a
    ///   factor 1.5" of his central values is a slip, and his ratios are at 440 nm, not V);
    /// - in V lies within 5% of the ruling's own model, a regression on the implementation.
    ///
    /// The year-mean zenith at 40° N in V is recorded (the ruling: 0.20; Masana et al. 2021's
    /// modelled 0.13). The illumination is the 16² one, of each texel centre's starlight and
    /// dust; the targets are the 64² band's texels, each of its own.
    #[test]
    fn on_a_plane_parallel_sky_the_diffuse_light_follows_tollers_ratios() {
        let blue = extinction_ratio(Micrometres::new(TOLLER_UM)).expect("an optical ratio");
        let (_, at_440) = plane_parallel_bins(blue);
        let (light, in_v) = plane_parallel_bins(1.0);
        for (k, &(from, to, toller, sigma)) in TOLLER.iter().enumerate() {
            let (ratio, v) = (at_440[k], in_v[k]);
            eprintln!(
                "|b| {from:.0}–{to:.0}°: at 440 nm {ratio:.3}, Toller {toller:.2} ± {sigma:.2} \
                 ({:.2} of his); in V {v:.3}, the ruling {:.3} ({:+.1}%)",
                ratio / toller,
                RULING_BINS[k],
                100.0 * (v / RULING_BINS[k] - 1.0)
            );
            assert!(
                ratio >= (toller - sigma) / 1.5 && ratio <= (toller + sigma) * 1.5,
                "|b| {from}–{to}°: {ratio} at 440 nm beyond ×/÷ 1.5 of {toller} ± {sigma}"
            );
            assert!(
                (v / RULING_BINS[k] - 1.0).abs() < 0.05,
                "|b| {from}–{to}°: {v} in V against the ruling's {}",
                RULING_BINS[k]
            );
        }
        // The year-mean zenith at 40° N: the zenith circle's galactic latitudes, at any longitude
        // on a plane-parallel sky (Hipparcos's north galactic pole, α 192.85948°, δ 27.12825°).
        let (ngp_ra, ngp_dec) = (
            192.859_48 * RADIANS_PER_DEGREE,
            27.128_25 * RADIANS_PER_DEGREE,
        );
        let (sin_dec, cos_dec) = math::sin_cos(40.0 * RADIANS_PER_DEGREE);
        let (mut zenith_dgl, mut zenith_isl) = (0.0, 0.0);
        for step in 0..3_600_u32 {
            let ra = f64::from(step) * 0.1 * RADIANS_PER_DEGREE;
            let sin_b = sin_dec * math::sin(ngp_dec)
                + cos_dec * math::cos(ngp_dec) * math::cos(ra - ngp_ra);
            let cos_b = (1.0 - sin_b * sin_b).sqrt();
            let toward = unit([cos_b, 0.0, sin_b]);
            zenith_dgl += light.diffuse_toward(&toward, plane_edge(sin_b))[0];
            zenith_isl += plane_light(sin_b, 1.0);
        }
        eprintln!(
            "the extinction at 440 nm is {blue:.4} A_V; the year-mean zenith at 40° N in V: {:.3} \
             (the ruling 0.20; Masana et al. 0.13, modelled); the scattered field in {} steps",
            zenith_dgl / zenith_isl,
            light.iterations()
        );
    }

    /// Near the Sun at 16², over the texels at |b| over 40°, the median of the diffuse light's
    /// μ<sub>V</sub> plus 2.5 log₁₀ A<sub>∞</sub> lies within 23.82 ± 0.44; and the slope's median
    /// at |b| 30–40° over that above 70° lies in 1.1–1.8 (R06.T9.g, test 10).
    ///
    /// 23.82 is 250 nW m⁻² sr⁻¹ at V per magnitude of A<sub>V</sub>, ×/÷ 1.5, the geometric mean
    /// of the measured DGL–100 µm slopes in V (Kawara et al. 2017, PASJ; Ienaka et al. 2013, ApJ
    /// 767, 80; Matsuoka et al. 2011, ApJ 736, 119; Brandt and Draine 2012, ApJ 744, 129; Postman et
    /// al. 2024, ApJ 972, 95), through Schlafly and Finkbeiner's (2011, ApJ 737, 103) 0.0505 mag of
    /// A<sub>V</sub> per `MJy` sr⁻¹ of I<sub>100</sub> (Schlegel, Finkbeiner and Davis 1998's p
    /// 0.0184 × 2.742). The fall is Zemcov et al.'s (2017, Nat. Commun. 8, 15003) model term, 1.44
    /// from 35° to 75°. The test reads the illumination's own 16² texels, the pre-pass's
    /// directions and dust. The ruling's plane-parallel model of the fixture gives about 24.0 and
    /// 1.3.
    #[test]
    fn the_diffuse_light_per_magnitude_of_dust_near_the_sun() {
        let light = sun_illumination();
        let median = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            let n = values.len();
            assert!(n > 0, "texels in the range");
            if n % 2 == 1 {
                values[n / 2]
            } else {
                f64::midpoint(values[n / 2 - 1], values[n / 2])
            }
        };
        let (mut high, mut middle, mut polar) = (Vec::new(), Vec::new(), Vec::new());
        for (toward, &edge) in light.directions().into_iter().zip(light.edge_a_v()) {
            let (b, a) = (latitude(toward.components()), edge.value());
            let diffuse = light.diffuse_toward(&toward, edge)[0];
            let mu = surface_brightness(CandelasPerSquareMetre::new(diffuse))
                .expect("a lit texel")
                .value();
            if b > 40.0 {
                high.push(mu + 2.5 * math::log10(a));
            }
            if (30.0..40.0).contains(&b) {
                middle.push(diffuse / a);
            }
            if b > 70.0 {
                polar.push(diffuse / a);
            }
        }
        let (slope, fall) = (median(high), median(middle) / median(polar));
        eprintln!(
            "near the Sun: μ_V of the diffuse light at A_V 1, median over |b| > 40°: {slope:.3} \
             (23.82 ± 0.44); the slope at 30–40° over |b| > 70°: {fall:.3} (1.1–1.8); {} steps",
            light.iterations()
        );
        assert!((slope - 23.82).abs() <= 0.44, "{slope}");
        assert!((1.1..=1.8).contains(&fall), "{fall}");
    }

    /// One 64² texel's lights near the Sun, cd m⁻²: all of the starlight, the starlight fainter
    /// than V 8.15 (the eye's background) and the diffuse light, and its ray's dust to the edge.
    #[derive(Debug, Clone, Copy)]
    struct Lights {
        all: f64,
        fainter: f64,
        diffuse: f64,
        edge_a_v: Magnitudes,
    }

    /// The fixture's 64² band near the Sun, lit by its illumination, as [`Lights`] per texel in
    /// the band's order: one march keeping two replies, complete nowhere and everywhere.
    fn lights_near_the_sun_at_64() -> Vec<Lights> {
        use crate::galaxy::features::centre::testing::milky_way_galaxy;
        use crate::sky::band::sum_rows;
        use crate::sky::census::SkyCensus;
        use crate::sky::testing::milky_way_context;

        let spec = BandSpec::STANDARD;
        let query = SkyQuery::builder(sun_observer(), Magnitudes::new(8.15))
            .illumination(std::sync::Arc::clone(sun_illumination()))
            .build()
            .expect("a valid query");
        let replies = [CompleteTo::nowhere(), CompleteTo::everywhere()];
        let mut lights = Vec::new();
        for face in CubeFace::ALL {
            let march = march_rows(
                milky_way_galaxy(),
                &mut milky_way_context(),
                &query,
                replies,
                &spec,
                face,
                0..spec.face_texels(),
            );
            let (mut all, mut fainter) = (Vec::new(), Vec::new());
            sum_rows(&march, &SkyCensus::empty(), &replies[0], &mut all);
            sum_rows(&march, &SkyCensus::empty(), &replies[1], &mut fainter);
            for ((all, fainter), &edge_a_v) in all.iter().zip(&fainter).zip(march.edge_a_v()) {
                let diffuse = all.diffuse_luminance().value();
                lights.push(Lights {
                    all: all.luminance().value() - diffuse,
                    fainter: fainter.luminance().value() - diffuse,
                    diffuse,
                    edge_a_v,
                });
            }
        }
        lights
    }

    /// The year-mean zenith at 40° N of the 64² `lights`: its diffuse light over its starlight,
    /// each direction of the zenith circle in its texel, l = 90° along +X and the galactic centre
    /// along −Y (Hipparcos's north galactic pole, α 192.85948°, δ 27.12825°, and the celestial
    /// pole's l, 122.93192°).
    fn zenith_mean_at_40_degrees_north(lights: &[Lights]) -> f64 {
        let spec = BandSpec::STANDARD;
        let side = usize::from(spec.face_texels());
        let (ngp_ra, ngp_dec) = (
            192.859_48 * RADIANS_PER_DEGREE,
            27.128_25 * RADIANS_PER_DEGREE,
        );
        let ncp_l = 122.931_92 * RADIANS_PER_DEGREE;
        let (sin_dec, cos_dec) = math::sin_cos(40.0 * RADIANS_PER_DEGREE);
        let (mut diffuse, mut all) = (0.0, 0.0);
        for step in 0..3_600_u32 {
            let ra = f64::from(step) * 0.1 * RADIANS_PER_DEGREE;
            let (sin_ra, cos_ra) = math::sin_cos(ra - ngp_ra);
            let sin_b = sin_dec * math::sin(ngp_dec) + cos_dec * math::cos(ngp_dec) * cos_ra;
            let l = ncp_l
                - math::atan2(
                    cos_dec * sin_ra,
                    sin_dec * math::cos(ngp_dec) - cos_dec * math::sin(ngp_dec) * cos_ra,
                );
            let cos_b = (1.0 - sin_b * sin_b).sqrt();
            let (sin_l, cos_l) = math::sin_cos(l);
            let (face, row, column) = spec
                .texel_of([cos_b * sin_l, -cos_b * cos_l, sin_b])
                .expect("a direction");
            let at =
                (usize::from(face.layer()) * side + usize::from(row)) * side + usize::from(column);
            diffuse += lights[at].diffuse;
            all += lights[at].all;
        }
        diffuse / all
    }

    /// The fixture's diffuse light near the Sun, recorded (R06.T9.g; the ruling's §2.3 and §4.2,
    /// item 12): over its starlight in Toller's bins, over the whole sky and in the year-mean zenith
    /// at 40° N; over the eye's background (the starlight fainter than V 8.15) at the poles and in
    /// the band, and its largest share of a 64² texel's; the scattered field's steps; the dust to the
    /// edge at high latitude; and the 16² illumination against an 8² one. The ruling's plane-parallel
    /// model of the fixture gives 0.59 to 0.30 by bin and 0.47 over the sky.
    #[test]
    #[ignore = "a record of the fixture's diffuse light near the Sun: a 64² band, minutes"]
    fn record_the_fixtures_diffuse_light_near_the_sun() {
        let light = sun_illumination();
        let texels = texels_of(BandSpec::STANDARD);
        let lights = lights_near_the_sun_at_64();
        let (mut dgl, mut isl) = ([0.0; 8], [0.0; 8]);
        let (mut pole, mut plane) = ((0.0, 0.0), (0.0, 0.0));
        let mut largest = (0.0_f64, 0.0_f64);
        for (texel, here) in texels.iter().zip(&lights) {
            let (b, w) = (latitude(texel.direction), texel.solid_angle_sr);
            let bin = TOLLER
                .iter()
                .position(|&(from, to, ..)| (from..to).contains(&b))
                .expect("every latitude is in a bin");
            dgl[bin] += here.diffuse * w;
            isl[bin] += here.all * w;
            if b > 80.0 {
                pole = (pole.0 + here.diffuse * w, pole.1 + here.fainter * w);
            }
            if b < 5.0 {
                plane = (plane.0 + here.diffuse * w, plane.1 + here.fainter * w);
            }
            if here.diffuse / here.fainter > largest.0 {
                largest = (here.diffuse / here.fainter, b);
            }
        }
        for (k, &(from, to, toller, sigma)) in TOLLER.iter().enumerate() {
            eprintln!(
                "record: |b| {from:.0}–{to:.0}°: the fixture's diffuse light over its starlight \
                 {:.3} (Toller {toller:.2} ± {sigma:.2}; the ruling's plane-parallel sky {:.3})",
                dgl[k] / isl[k],
                RULING_BINS[k]
            );
        }
        eprintln!(
            "record: over the whole sky {:.3} (Leinert et al.: typically 0.20–0.30; the ruling's \
             model of the fixture 0.47); the year-mean zenith at 40° N {:.3} (Masana et al. 0.13, \
             modelled; the ruling's model of the fixture 0.48)",
            dgl.iter().sum::<f64>() / isl.iter().sum::<f64>(),
            zenith_mean_at_40_degrees_north(&lights)
        );
        // The all-sky mean starlight, as the ruling's §2.2 compares it: μ_V, and λI_λ at V by its
        // conversion, 842 × 10^(−0.4 μ) × 10⁹ nW m⁻² sr⁻¹ (V 0 at 3.63 × 10⁻⁹ erg cm⁻² s⁻¹ Å⁻¹).
        let mean = isl.iter().sum::<f64>() / (4.0 * core::f64::consts::PI);
        let mu = surface_brightness(CandelasPerSquareMetre::new(mean))
            .expect("a lit sky")
            .value();
        eprintln!(
            "record: the all-sky mean starlight μ_V {mu:.2}, {:.0} nW m⁻² sr⁻¹ (the ruling's \
             plane-parallel fixture 433, its realistic sky 762)",
            842.0 * math::exp10(-0.4 * mu) * 1e9
        );
        eprintln!(
            "record: over the eye's background (the starlight fainter than V 8.15): at the poles \
             (|b| > 80°) {:.3}, in the band (|b| < 5°) {:.3}; at 64² at most {:.3} (|b| {:.1}°); \
             the scattered field in {} steps",
            pole.0 / pole.1,
            plane.0 / plane.1,
            largest.0,
            largest.1,
            light.iterations()
        );
        let mut polar: Vec<f64> = light
            .directions()
            .into_iter()
            .zip(light.edge_a_v())
            .filter(|(d, _)| latitude(d.components()) > 60.0)
            .map(|(_, a)| a.value())
            .collect();
        polar.sort_by(f64::total_cmp);
        eprintln!(
            "record: A_∞ over the 16² rays at |b| > 60°: least {:.3}, median {:.3}, most {:.3}",
            polar[0],
            polar[polar.len() / 2],
            polar[polar.len() - 1]
        );
        record_an_8_squared_illumination(&texels, &lights);
    }

    /// Records the 8² illumination's diffuse light against the 16²'s at the 64² band's texels
    /// `texels`, of lights `lights` (the ruling's expectation: within 3%).
    fn record_an_8_squared_illumination(texels: &[Texel], lights: &[Lights]) {
        use crate::galaxy::features::centre::testing::milky_way_galaxy;
        use crate::sky::testing::milky_way_context;

        let coarse = Illumination::march_at(
            milky_way_galaxy(),
            &mut milky_way_context(),
            &sun_observer(),
            BandSpec::new(8, BandSpec::STANDARD.nodes_per_decade()).expect("a band"),
        );
        let (mut worst, mut differs, mut total) = (0.0_f64, 0.0, 0.0);
        let (mut band_fine, mut band_coarse) = (0.0, 0.0);
        for (texel, here) in texels.iter().zip(lights) {
            let rough = coarse.diffuse_toward(&unit(texel.direction), here.edge_a_v)[0];
            worst = worst.max((rough / here.diffuse - 1.0).abs());
            differs += (rough - here.diffuse).abs() * texel.solid_angle_sr;
            total += here.diffuse * texel.solid_angle_sr;
            if latitude(texel.direction) < 5.0 {
                band_fine += here.diffuse * texel.solid_angle_sr;
                band_coarse += rough * texel.solid_angle_sr;
            }
        }
        eprintln!(
            "record: the 8² illumination against the 16² at 64²: at most {worst:.3} in a texel, \
             {:.4} of the light over the sky, {:+.4} over the band (|b| < 5°); 8² in {} steps",
            differs / total,
            band_coarse / band_fine - 1.0,
            coarse.iterations()
        );
    }

    /// The scattered field's kernel, built once a sum over the illumination's own texels, gives the
    /// phase-weighted means of a direct sum toward each texel bit for bit: the same weights (each
    /// pair's cosine once, both ways, since d<sub>j</sub> · d<sub>k</sub> and d<sub>k</sub> ·
    /// d<sub>j</sub> have the same bits) in the same lanes and the same order, so the field S the
    /// rays read was iterated with their own J (determinism audit of R06.T9.g). On a 4² grid, of
    /// values that are not powers of two.
    #[test]
    fn the_fields_kernel_is_the_direct_phase_weighted_mean_bit_for_bit() {
        let grid = Grid::of(&grid(4));
        let mut draws = crate::sky::testing::uniforms(0x0d91);
        let values: Columns = std::array::from_fn(|_| {
            (0..grid.len())
                .map(|_| 0.1 + draws.next().expect("a draw"))
                .collect()
        });
        for (s, scatterer) in Scatterer::of_sums(&solar()).iter().enumerate() {
            let kernel = Kernel::of(scatterer, &grid);
            for j in 0..grid.len() {
                let toward = grid.direction(j);
                let [direct] = phase_means(&Scatterer::of_sums(&solar()), &grid, [&values], toward);
                assert_eq!(
                    bits(kernel.mean(j, &values[s])),
                    bits(direct[s]),
                    "sum {s}, texel {j}"
                );
                for k in 0..grid.len() {
                    let cos = toward[0] * grid.x[k] + toward[1] * grid.y[k] + toward[2] * grid.z[k];
                    assert_eq!(
                        bits(kernel.weights[j * grid.len() + k]),
                        bits(grid.solid_angle_sr[k] * scatterer.weight(cos)),
                        "sum {s}, pair ({j}, {k})"
                    );
                }
            }
        }
    }

    /// One face's rows of a constructed 2² illumination, each texel's field `value`.
    fn rows_of(face: CubeFace, rows: Range<u16>, value: f64) -> IlluminationRows {
        let spec = BandSpec::new(2, 12).expect("a band");
        let count = rows.len() * 2;
        IlluminationRows {
            observer: sun_observer(),
            spec,
            face,
            rows,
            field: vec![[value; 5]; count],
            edge_a_v: vec![Magnitudes::new(0.1); count],
        }
    }

    #[test]
    #[should_panic(expected = "texel 12 of the illumination is marched twice")]
    fn an_illumination_marched_twice_somewhere_is_refused() {
        let spec = BandSpec::new(2, 12).expect("a band");
        let mut parts: Vec<IlluminationRows> = CubeFace::ALL
            .iter()
            .map(|&face| rows_of(face, 0..2, 1.0))
            .collect();
        parts.push(rows_of(CubeFace::NegY, 0..1, 2.0));
        let _ = Illumination::assemble_at(&sun_observer(), spec, parts);
    }

    #[test]
    #[should_panic(expected = "texel 22 of the illumination is not marched")]
    fn an_illumination_missing_a_row_is_refused() {
        let spec = BandSpec::new(2, 12).expect("a band");
        let mut parts: Vec<IlluminationRows> = CubeFace::ALL[..5]
            .iter()
            .map(|&face| rows_of(face, 0..2, 1.0))
            .collect();
        parts.push(rows_of(CubeFace::NegZ, 0..1, 1.0));
        let _ = Illumination::assemble_at(&sun_observer(), spec, parts);
    }

    /// An illumination has the same bits whatever was marched before it on the same context, whose
    /// noise cache stays warm, in any order, and the Sun's are those the tests share, of a cold
    /// context's (determinism audit of R06.T9.g).
    #[test]
    fn the_illumination_is_the_same_whatever_was_marched_before_it() {
        use std::cell::RefCell;

        use hyperion_testkit::order::assert_order_independent;

        use crate::galaxy::features::centre::testing::milky_way_galaxy;
        use crate::sky::testing::milky_way_context;

        let above = Observer::new(
            crate::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 1_000.0])
                .expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let ctx = RefCell::new(milky_way_context());
        let marched = |at: &Observer| {
            Illumination::march(milky_way_galaxy(), &mut ctx.borrow_mut(), at).bits()
        };
        assert_order_independent(&[sun_observer(), above], marched);
        assert_eq!(marched(&sun_observer()), sun_illumination().bits());
    }

    /// The builder refuses an illumination of another observer's place or time
    /// ([`BuildSkyQueryError::Illumination`]), and takes the observer's own.
    #[test]
    fn an_illumination_of_another_observer_is_refused() {
        let light = std::sync::Arc::new(sky_of(&solar(), |_| [1.0; 5], |_| Magnitudes::new(0.1)));
        let here = sun_observer();
        let later = Observer::new(
            *here.position(),
            UniverseTime::EPOCH
                .checked_add(crate::time::Span::from_seconds_f64(3.0e7).expect("a span"))
                .expect("on the clock"),
        )
        .expect("an observer");
        let elsewhere = Observer::new(
            crate::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 1_000.0])
                .expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let query = |observer: Observer| {
            SkyQuery::builder(observer, Magnitudes::new(8.0))
                .illumination(light.clone())
                .build()
        };
        assert!(query(here).is_ok());
        assert_eq!(query(later).err(), Some(BuildSkyQueryError::Illumination));
        assert_eq!(
            query(elsewhere).err(),
            Some(BuildSkyQueryError::Illumination)
        );
    }
}
