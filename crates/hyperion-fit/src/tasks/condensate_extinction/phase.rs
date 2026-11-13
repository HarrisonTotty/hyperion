//! A non-spherical file's particles on the fine grid from its own phase table (galaxy plan 14,
//! P14.T49.e; science-r08-nonspherical.md §6.4: "`κ_ext` for `nonSphericalMineral` and `crystal`
//! modes is read from R08.T5.c's tables' `C_ext`, not from sphere Mie, so that the sim's mass
//! extinction and the client's optics agree"), read as the client reads them
//! (`apps/hyperion/src/renderer/src/view/atmosphere/nonSpherical.ts`, `tableModeOptics` and
//! `sumsOverNode`, rendering plan R08.T5.c).
//!
//! # The size measure
//!
//! A mode's radius is the volume-to-area radius r = 3V ÷ 4A, A the orientation-averaged projected
//! area (P14.T24.c), so its `r_eff` is (3 ÷ 4)⟨V⟩ ÷ ⟨A⟩ and `Q̄_ext` is ⟨`C_ext`⟩ ÷ ⟨A⟩. A table
//! indexes its sizes by x = 2πD ÷ λ, D the maximum dimension, as `TAMUdust2020`'s kernels do (M.
//! Saito, P. Yang, J. Ding and X. Liu, J. Atmos. Sci. 78, 2089–2111, 2021), and gives each size
//! node's Ṽ, Ã, `C̃_ext` and `C̃_sca` in powers of λ ÷ 2π (the files' `encoding`). Its r̃ = 3Ṽ ÷ 4Ã is
//! then the node's volume-to-area size parameter 2πr ÷ λ, the client's `rTilde`: a fine node of
//! radius r reads the table at r̃ = 2πr ÷ λ, λ = 550 nm, and the table's own x is not used. The
//! hexahedra's Ã ÷ πr̃² is 2.79 at every size, water ice's aggregate's 4.85, a sphere's 1.
//!
//! # Each fine node's particle
//!
//! A fine node holds the particle's A, `C_ext` and `C_sca` over πr² (so a sphere's are 1, `Q_ext`
//! and `Q_sca`), which the cells sum with the area weight r² n(r):
//!
//! - **within the table's sizes**, Ã, `C̃_ext` and `C̃_sca` log-linear in ln r̃ between its size nodes
//!   (the client's `logLerp`);
//! - **below its smallest**, as the client draws them: for `TAMUdust2020` the sphere of the
//!   particle's own volume, (3Ṽ₀ ÷ 4π)^(1/3) ÷ r̃₀ times r̃ in radius (right in the Rayleigh limit
//!   that tail lies in, 0.46 nm at 550 nm); for Yang et al. 2013's ice, Ã₀ ÷ πr̃₀² spheres of
//!   radius r̃, which keep the crystal's V and A (T. C. Grenfell and S. G. Warren, J. Geophys. Res.
//!   104, 31697–31709, 1999), below 0.21–0.32 µm;
//! - **above its largest** (r̃ 184.5 µm for the shortwave kernel's tables at 550 nm, 23.1 µm for
//!   the longwave kernel's, 1.07–1.58 mm for water ice's), the largest size node's Ã ÷ πr̃²
//!   spheres of radius r̃, Grenfell and Warren's again, at the index the node's table was made at
//!   (`n_table`, `k_table`) and scaled as its table is, so that they continue it: the client
//!   refuses a distribution that reaches past a table, and at its top the hexahedra's `Q_ext` and
//!   `Q_sca` lie within 0.5% of the equal-V/A sphere's at the same index (mars dust's at 184.5 µm:
//!   2.0031 and 1.1002 against 2.0121 and 1.1004);
//! - **at a floor**, a node whose table was made at a kernel's k floor above the material's own k
//!   (10⁻⁴ shortwave, 10⁻³ longwave), and at every node of Yang's table, the table's particles'
//!   absorption, and the spheres' above it, scaled by k ÷ `k_table`, k the particles' own at
//!   550 nm (the client's `scaledAbsorption`); the spheres below the table keep their own Mie at
//!   that k, as the client's do;
//! - **between wavelength nodes**, the two about 550 nm blended linearly in wavelength, the nearer
//!   end's alone beyond the table's (the client's `blend`; only `mars-dust.json` has a node at
//!   550 nm), each node's table read at 550 nm's size parameter.
//!
//! # Where the hexahedra are not read
//!
//! A `TAMUdust2020` table is not read, and the file falls back to spheres ([`within_the_kernels`]),
//! where its material's index at 550 nm lies
//!
//! - past the kernels by the client's `kernelEdgeRule` (decision-r08-t5d ruling 2: k above
//!   [`KERNEL_K_MAX`], or n past [`KERNEL_EDGE_N`] by more than [`KERNEL_EDGE_MODEL_EXCESS`] with a
//!   normal-incidence reflectance moved by more than [`KERNEL_EDGE_REFLECTANCE_TOLERANCE`]), where
//!   no committed file's index lies; or
//! - outside both kernels, n above [`KERNEL_TRANSPARENT_N_MAX`] with k below [`LONGWAVE_K_FLOOR`]
//!   (the shortwave kernel stops at n 1.70, the longwave starts at k 10⁻³; science-r08-nonspherical
//!   §2.5): aluminium oxide, sulphur, titanium dioxide and zinc sulphide. Their tables are the
//!   longwave kernel's floor's, whose absorption, scaled to k ≈ 0, leaves a large grain's
//!   extinction at its floor's scattering alone: `Q̄_ext` 1.2 at 101 µm and 1.5 at 11 µm, where a
//!   lossless grain's is 2.0 and 2.1 (the extinction paradox holds for any shape). The client
//!   itself draws only their modes of `r_eff` about 0.02–0.2 µm from those tables, where the mode's
//!   own ω on the floor's table is at least 0.99, and Mie spheres for the rest (its floor rule's
//!   fallback above n 1.70).
//!
//! # What is not the client's
//!
//! The client's floor rule judges each mode, and its switch from a table to Mie spheres is a step
//! in `r_eff` and `v_eff` that the table's interpolation cannot follow, so the sim judges each
//! file instead, at 550 nm:
//!
//! - the four files outside both kernels are spheres here at every size, where the client draws
//!   their modes of about 0.02–0.2 µm from the floor's tables (up to 4.8 times the spheres'
//!   `Q̄_ext`, titanium dioxide's at 40 nm);
//! - manganese sulphide, whose k at 550 nm (1.03 × 10⁻³) lies within the longwave kernel, keeps its
//!   table, one of whose nodes is the floor's, scaled by 1.03, where the client falls back to Mie
//!   once its own ω on the floor's table is below 0.99, as an absorbing grain's is at most sizes;
//! - the shortwave kernel's floor files (potassium chloride, sodium chloride, silicon dioxide and
//!   enstatite glass, n below 1.70) keep their tables, as the client's analogue does, and so share
//!   the floor's large-grain deficit with it: `Q̄_ext` 0.78–0.82 of the spheres' at 101 µm and
//!   0.90–0.92 at 28 µm (plan 14's record of P14.T49.e).

use std::f64::consts::PI;

use hyperion_sim::math;
use hyperion_sim::substance::aerosol::EXTINCTION_WAVELENGTH_NM;

use std::num::NonZeroUsize;

use super::mie::{
    ComplexIndex, ComputeMieError, MAX_SIZE_PARAMETER, MIN_SIZE_PARAMETER, efficiencies,
};
use super::{
    Spectrum, WAVELENGTH_M, fine_grid, fine_radius_m, mie_failure, size_parameter, spectrum,
};
use crate::materials::{PhaseModel, PhaseTableFile, PhaseTableNode};
use crate::task::RunTaskError;

/// The longwave kernel's range in n, the widest of `TAMUdust2020`'s (Saito et al. 2021, its `LW`
/// tables), the client's `KERNEL_EDGE_N`.
pub const KERNEL_EDGE_N: (f64, f64) = (0.4, 3.2);

/// The longwave kernel's largest k, the client's `KERNEL_K_MAX`.
pub const KERNEL_K_MAX: f64 = 4.0;

/// The largest real index either kernel reaches with k below [`LONGWAVE_K_FLOOR`]: the shortwave
/// kernel's 1.70, the client's `KERNEL_TRANSPARENT_N_MAX` (science-r08-nonspherical.md §2.5).
pub const KERNEL_TRANSPARENT_N_MAX: f64 = 1.7;

/// The longwave kernel's k floor, 10⁻³ (Saito et al. 2021; the client's floor rule).
pub const LONGWAVE_K_FLOOR: f64 = 1e-3;

/// The largest |n − `n_edge`| that is still the kernel's model, half its n step of 0.1, the
/// client's `KERNEL_EDGE_MODEL_EXCESS` (decision-r08-t5d ruling 2).
pub const KERNEL_EDGE_MODEL_EXCESS: f64 = 0.05;

/// The largest relative change of the normal-incidence reflectance for which the edge's table
/// stands in, the client's `KERNEL_EDGE_REFLECTANCE_TOLERANCE` (decision-r08-t5d ruling 2).
pub const KERNEL_EDGE_REFLECTANCE_TOLERANCE: f64 = 0.2;

/// The normal-incidence reflectance ((n − 1)² + k²) ÷ ((n + 1)² + k²).
#[must_use]
fn normal_reflectance(n: f64, k: f64) -> f64 {
    ((n - 1.0) * (n - 1.0) + k * k) / ((n + 1.0) * (n + 1.0) + k * k)
}

/// Whether the hexahedra's tables hold a material of index `index`: inside one of the kernels,
/// not n above [`KERNEL_TRANSPARENT_N_MAX`] with k below [`LONGWAVE_K_FLOOR`], and by the client's
/// `kernelEdgeRule` its `model` or its `analogue`, not its `fallback` (the module's remarks).
#[must_use]
pub fn within_the_kernels(index: ComplexIndex) -> bool {
    let (n, k) = (index.n(), index.k());
    if k > KERNEL_K_MAX || (n > KERNEL_TRANSPARENT_N_MAX && k < LONGWAVE_K_FLOOR) {
        return false;
    }
    let (low, high) = KERNEL_EDGE_N;
    if (low..=high).contains(&n) {
        return true;
    }
    let edge = if n > high { high } else { low };
    if (n - edge).abs() <= KERNEL_EDGE_MODEL_EXCESS {
        return true;
    }
    let reflectance = normal_reflectance(n, k);
    (reflectance - normal_reflectance(edge, k)).abs() / reflectance
        <= KERNEL_EDGE_REFLECTANCE_TOLERANCE
}

/// The spheres a table's particles below its smallest size are drawn as (the module's remarks).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tail {
    /// One sphere of the particle's own volume, this many times its r̃ in radius.
    Volume {
        /// (3Ṽ₀ ÷ 4π)^(1/3) ÷ r̃₀.
        sphere_over_radius: f64,
    },
    /// This many spheres of the particle's r̃ (Ã₀ ÷ πr̃₀²).
    VolumeToArea {
        /// Ã₀ ÷ πr̃₀².
        spheres: f64,
    },
}

/// One wavelength node of a table, as the fine grid reads it.
#[derive(Debug, Clone, PartialEq)]
struct NodeReading {
    /// ln r̃ of each size node.
    ln_radius: Vec<f64>,
    /// ln Ã, ln `C̃_ext` and ln `C̃_sca` of each size node.
    ln_area: Vec<f64>,
    ln_extinction: Vec<f64>,
    ln_scattering: Vec<f64>,
    tail: Tail,
    /// Ã ÷ πr̃² at the largest size node, the spheres above it per particle.
    top_spheres: f64,
    /// The spheres above the largest size node: the sphere spectrum at the node's own index.
    above: Spectrum,
    /// k ÷ `k_table` at a floor or on Yang's table, which scales the table's absorption.
    absorption_scale: Option<f64>,
}

impl NodeReading {
    /// The reading of `node` of a table of `model`, for particles of index `index`, with `above`
    /// the sphere spectrum at the node's own index (`n_table`, `k_table`).
    #[must_use]
    fn of(node: &PhaseTableNode, model: PhaseModel, index: ComplexIndex, above: Spectrum) -> Self {
        let radius: Vec<f64> = node
            .volumes()
            .iter()
            .zip(node.areas())
            .map(|(v, a)| 0.75 * v / a)
            .collect();
        let ln = |values: &[f64]| values.iter().map(|&v| math::ln(v)).collect::<Vec<_>>();
        let first = radius[0];
        let last = radius.len() - 1;
        let tail = match model {
            PhaseModel::TamuDust2020 => Tail::Volume {
                sphere_over_radius: math::cbrt(3.0 * node.volumes()[0] / (4.0 * PI)) / first,
            },
            PhaseModel::Yang2013 => Tail::VolumeToArea {
                spheres: node.areas()[0] / (PI * first * first),
            },
        };
        let scales =
            (node.k_table() > node.k() || model == PhaseModel::Yang2013) && node.k_table() > 0.0;
        Self {
            ln_radius: ln(&radius),
            ln_area: ln(node.areas()),
            ln_extinction: ln(node.extinction()),
            ln_scattering: ln(node.scattering()),
            tail,
            top_spheres: node.areas()[last] / (PI * radius[last] * radius[last]),
            above,
            absorption_scale: scales.then(|| index.k() / node.k_table()),
        }
    }

    /// `extinction` with its absorption, `extinction` − `scattering`, scaled as the node's is.
    #[must_use]
    fn scaled(&self, extinction: f64, scattering: f64) -> f64 {
        self.absorption_scale.map_or(extinction, |scale| {
            scattering + (extinction - scattering) * scale
        })
    }

    /// r̃ of the smallest and largest size nodes.
    #[must_use]
    fn ends(&self) -> (f64, f64) {
        let last = self.ln_radius.len() - 1;
        (
            math::exp(self.ln_radius[0]),
            math::exp(self.ln_radius[last]),
        )
    }

    /// The particle at fine node `j` of size parameter `x` (its r̃): its A, `C_ext` and `C_sca`
    /// over πr̃², from the table or the spheres outside it (`spheres`, the file's sphere spectrum at
    /// its own index, below; the node's own above).
    fn particle(
        &self,
        j: usize,
        x: f64,
        index: ComplexIndex,
        spheres: &Spectrum,
    ) -> Result<[f64; 3], ComputeMieError> {
        let ln_x = math::ln(x);
        let last = self.ln_radius.len() - 1;
        if ln_x < self.ln_radius[0] {
            return Ok(match self.tail {
                Tail::Volume { sphere_over_radius } => {
                    let sphere = efficiencies(
                        (sphere_over_radius * x).clamp(MIN_SIZE_PARAMETER, MAX_SIZE_PARAMETER),
                        index,
                    )?;
                    let area = sphere_over_radius * sphere_over_radius;
                    [area, sphere.q_ext() * area, sphere.q_sca() * area]
                }
                Tail::VolumeToArea { spheres: count } => {
                    [count, spheres.q_ext[j] * count, spheres.q_sca[j] * count]
                }
            });
        }
        if ln_x > self.ln_radius[last] {
            let count = self.top_spheres;
            let (extinction, scattering) = (self.above.q_ext[j], self.above.q_sca[j]);
            return Ok([
                count,
                self.scaled(extinction, scattering) * count,
                scattering * count,
            ]);
        }
        let s = self
            .ln_radius
            .partition_point(|&r| r <= ln_x)
            .clamp(1, last)
            - 1;
        let t = ((ln_x - self.ln_radius[s]) / (self.ln_radius[s + 1] - self.ln_radius[s]))
            .clamp(0.0, 1.0);
        let lerp = |values: &[f64]| math::exp(values[s] + t * (values[s + 1] - values[s]));
        let geometric = PI * x * x;
        let area = lerp(&self.ln_area) / geometric;
        let scattering = lerp(&self.ln_scattering) / geometric;
        let extinction = self.scaled(lerp(&self.ln_extinction) / geometric, scattering);
        Ok([area, extinction, scattering])
    }
}

/// A file's particles from its phase table at 550 nm: their spectrum on the fine grid (the
/// module's remarks) and the table's sizes, the volume-to-area radii, m, from its smallest particle
/// to its largest where both wavelength nodes read hold particles.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableParticles {
    pub(crate) spectrum: Spectrum,
    pub(crate) radii_m: (f64, f64),
}

/// The wavelength nodes of `table` about 550 nm and the weight of the upper, as the client's
/// `tableModeOptics` picks them: the last node at or below 550 nm and the next (the first two, or
/// the last two, beyond the table's), the weight linear in wavelength and held to [0, 1].
#[must_use]
fn bracket(table: &PhaseTableFile) -> (&PhaseTableNode, &PhaseTableNode, f64) {
    let nodes = table.nodes();
    let last = nodes.len() - 1;
    let mut i = 0;
    while i + 1 < last && nodes[i + 1].wavelength_nm() <= EXTINCTION_WAVELENGTH_NM {
        i += 1;
    }
    let lower = &nodes[i];
    let upper = &nodes[(i + 1).min(last)];
    let t = if i + 1 > last {
        0.0
    } else {
        ((EXTINCTION_WAVELENGTH_NM - lower.wavelength_nm())
            / (upper.wavelength_nm() - lower.wavelength_nm()))
        .clamp(0.0, 1.0)
    };
    (lower, upper, t)
}

/// The sphere spectrum of file `file` at the index node `node`'s table was made at, on `threads`
/// threads.
///
/// # Errors
///
/// As [`spectrum`], and [`super::FitExtinctionError::Mie`] for an index the Mie refuses.
fn node_spheres(
    file: &'static str,
    node: &PhaseTableNode,
    threads: NonZeroUsize,
) -> Result<Spectrum, RunTaskError> {
    let index =
        ComplexIndex::new(node.n_table(), node.k_table()).map_err(|e| mie_failure(file, e))?;
    spectrum(file, index, threads)
}

/// The particles of file `file`'s table `table` for a material of index `index` at 550 nm, with
/// `spheres` its sphere spectrum at that index (the module's remarks), on `threads` threads.
///
/// # Errors
///
/// [`RunTaskError::Input`] carrying a [`super::FitFileError`] of `file` if a sphere's optics
/// cannot be computed, and as [`spectrum`].
pub(crate) fn table_particles(
    file: &'static str,
    table: &PhaseTableFile,
    index: ComplexIndex,
    spheres: &Spectrum,
    threads: NonZeroUsize,
) -> Result<TableParticles, RunTaskError> {
    let (lower, upper, t) = bracket(table);
    let low = NodeReading::of(
        lower,
        table.model(),
        index,
        node_spheres(file, lower, threads)?,
    );
    let high = if t > 0.0 {
        Some(NodeReading::of(
            upper,
            table.model(),
            index,
            node_spheres(file, upper, threads)?,
        ))
    } else {
        None
    };
    let mie = |e| mie_failure(file, e);
    let grid = fine_grid();
    let mut spectrum = Spectrum {
        area: Vec::with_capacity(grid.nodes),
        q_ext: Vec::with_capacity(grid.nodes),
        q_sca: Vec::with_capacity(grid.nodes),
        lossless: index.k() <= 0.0,
        ..Spectrum::default()
    };
    for j in 0..grid.nodes {
        let x = size_parameter(fine_radius_m(grid, j));
        let mut particle = low.particle(j, x, index, spheres).map_err(mie)?;
        if let Some(high) = &high {
            let other = high.particle(j, x, index, spheres).map_err(mie)?;
            for (value, upper_value) in particle.iter_mut().zip(other) {
                *value = (1.0 - t) * *value + t * upper_value;
            }
        }
        let [area, extinction, scattering] = particle;
        spectrum.area.push(area);
        spectrum.q_ext.push(extinction);
        spectrum.q_sca.push(scattering);
    }
    spectrum.largest_extinction = spectrum.q_ext.iter().copied().fold(0.0, f64::max);
    spectrum.largest_area = spectrum.area.iter().copied().fold(0.0, f64::max);
    let (mut smallest, mut largest) = low.ends();
    if let Some(high) = &high {
        let (first, top) = high.ends();
        smallest = smallest.max(first);
        largest = largest.min(top);
    }
    let length_m = WAVELENGTH_M / (2.0 * PI);
    Ok(TableParticles {
        spectrum,
        radii_m: (smallest * length_m, largest * length_m),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kernels hold an index as the client's rules do: inside the shortwave kernel's n with any
    /// k up to 4, and above it with k from the longwave kernel's floor; a little past the longwave
    /// kernel's n range a model or an analogue; far past it, above k 4, or transparent above n
    /// 1.70, not.
    #[test]
    fn condensate_extinction_the_kernel_edge_is_the_client_s() {
        let index = |n: f64, k: f64| ComplexIndex::new(n, k).unwrap();
        for (n, k) in [
            (1.5, 0.003),
            (1.5, 0.0),
            (1.7, 0.0),
            (3.18, 3.33),
            (2.87, 1.03e-3),
            (3.24, 0.01),
            (3.3, 3.9),
        ] {
            assert!(within_the_kernels(index(n, k)), "{n} + {k}i");
        }
        for (n, k) in [
            (1.5, 4.5),
            (4.5, 0.01),
            (0.2, 0.01),
            (1.77, 0.0),
            (2.44, 5.4e-8),
            (2.39, 9.9e-4),
        ] {
            assert!(!within_the_kernels(index(n, k)), "{n} + {k}i");
        }
    }
}
