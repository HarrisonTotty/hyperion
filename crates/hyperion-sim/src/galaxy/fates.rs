//! Stellar fates and the mean mass of a system.
//!
//! The galaxy's system count is its stellar mass divided by the mean present-day mass of a
//! system: a once-per-galaxy quadrature over the mass function, multiplicity, the age
//! distribution, stellar lifetimes and remnant masses (brainstorm, "Galaxy parameters"). Lifetimes
//! and remnant masses belong to the stellar stage (plan 06) and multiplicity to plan 11, so the
//! quadrature reads them through [`StellarFates`], and the first milestone uses
//! [`ProvisionalFates`] (plan 02, Design note 4). Replacing the fates changes the system count,
//! which scales every density: it moves every star and bumps the generator version.
//!
//! # Companions
//!
//! [`StellarFates::mean_companions`] counts the *stellar* companions of a primary, as the surveys
//! behind Duchêne and Kraus's (2013) companion frequencies do: their M-dwarf figure counts few
//! brown dwarfs (3 of 23 companions in the volume-limited sample). A companion's mass ratio is
//! uniform between 0.1 and 1, and a companion below the hydrogen-burning limit is neither drawn
//! nor counted, so its mass is uniform on `[max(0.1 m, 0.08 M☉), m]` for a primary of mass `m`.
//! Each companion has the system's age and evolves like a primary of its own mass. This is the
//! reading under which the model gives the brainstorm's figures for all stars below 0.5 M☉, 76.4%
//! under Kroupa's function for primaries, 66.9% under Chabrier's system function as published and
//! 70.9% under the default, Chabrier's with its branch above 1 M☉ scaled by 0.68, with 1.40–1.44
//! stars per system. The other reading, a ratio uniform on 0.1–1 with the companions that fall
//! below 0.08 M☉ dropped, gives 74.5% and 65% under the first two and 1.29 stars per system under
//! Kroupa's, outside the brainstorm's figures and its 1.33–1.45.
//!
//! # Quadrature
//!
//! Every function here is the same nested quadrature in ln m over the stellar range: 16-point
//! Gauss–Legendre panels no wider than [`MAX_PANEL_LN_MASS`] in ln m, with edges at every break
//! of the mass function and the fates, at 0.8 M☉ where the companion range stops being cut by
//! the hydrogen-burning limit, and at each mass whose lifetime equals an edge of the age
//! distribution, so that no panel straddles a kink. For each primary node, the companions' mean
//! is the integral over the companion range of the polynomials through the same nodes' values
//! ([`Gl16Panel`]), so each node's quantity is evaluated once. The normalisation is the mass
//! function's closed-form integral. The panel scheme is part of the generator version.

use std::fmt;

use super::ages::AgeDistribution;
use super::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO, MassFunction};
use super::quad::{Gl16Panel, bisect};
use crate::math;
use crate::tables::gauss_legendre::GL16_WEIGHTS;
use crate::units::{SolarMasses, Years};

/// The widest panel of the outer quadrature, in ln m.
pub const MAX_PANEL_LN_MASS: f64 = 0.5;

/// The smallest companion mass ratio (plan 02, Design note 4).
pub const MIN_MASS_RATIO: f64 = 0.1;

/// The primary mass above which the companion range is no longer cut by the hydrogen-burning
/// limit: 0.08 M☉ ÷ 0.1 = 0.8 M☉.
const COMPANION_RANGE_SWITCH: f64 = MASS_LIMIT_LO / MIN_MASS_RATIO;

/// Iterations of the bisection that finds the mass with a given lifetime.
const LIFETIME_BISECTIONS: u32 = 64;

/// The fates of stars, as the mean-mass quadrature needs them: how long a star lives, what it
/// leaves, and how many stellar companions a primary has.
///
/// Masses are initial masses in M☉ on the stellar range, 0.08–150 M☉. Plans 06 and 11 replace
/// [`ProvisionalFates`] with implementations built on stellar tracks and a multiplicity model.
pub trait StellarFates: fmt::Debug + Send + Sync {
    /// The star's lifetime: the age at which it becomes a remnant. It must not rise with mass.
    fn lifetime(&self, m: f64) -> Years;

    /// The mass of the remnant the star leaves, M☉; 0 for none.
    fn remnant_mass(&self, m: f64) -> f64;

    /// The mean number of stellar companions of a primary of mass `m` (see the module
    /// documentation for how a companion's mass is distributed).
    fn mean_companions(&self, m: f64) -> f64;

    /// Masses, M☉, where any of the three functions has a kink or a jump, ascending, for the
    /// quadrature's panel edges. None by default.
    fn breaks(&self) -> &[f64] {
        &[]
    }
}

/// The first milestone's fates (plan 02, Design note 4), until plans 06 and 11 replace them.
///
/// - **Lifetime** by Raiteri, Villata and Navarro's (1996, A&A 315, 105, eq. 3) fit to the Padova
///   tracks, `log₁₀ t = a₀ + a₁ log₁₀ m + a₂ (log₁₀ m)²` with coefficients quadratic in
///   `log₁₀ Z`, taken at the tracks' solar Z = 0.02
///   ([`LIFETIME_METALLICITY`](Self::LIFETIME_METALLICITY)): 9.5 Gyr at 1 M☉, 1.06 Gyr at
///   2 M☉, 100 Myr at 5 M☉, 9 Myr at 20 M☉. The fit holds on
///   0.6–120 M☉; below 0.6 M☉ the mass is held at 0.6, where the lifetime is 60 Gyr, and above the
///   fit's turning point at 106 M☉ at that point, so that the lifetime never rises with mass. The
///   3 Myr floor of Design note 4 is kept and never binds.
/// - **Remnant mass** 0.109 m + 0.394 M☉ below 8 M☉, a white dwarf by Kalirai et al.'s (2008,
///   Astrophysical Journal 676, 594) initial–final mass relation, `M_final = (0.109 ± 0.007)
///   M_initial + (0.394 ± 0.025)`, re-checked against the paper; 1.4 M☉ for 8–22 M☉, a neutron
///   star; and min(0.4 m, 40 M☉) above, a black hole (Design note 4, no source).
/// - **Mean stellar companions** 0.30, 0.60, 1.0 and 1.3 for primaries of 0.08–0.5, 0.5–1.5,
///   1.5–16 and over 16 M☉: the companion frequencies of Duchêne and Kraus (2013, ARA&A 51, 269,
///   Table 1), 33 ± 5% for 0.1–0.5 M☉, 62 ± 3% for 0.7–1.3 M☉, 100 ± 10% for 1.5–5 M☉,
///   100 ± 20% for 8–16 M☉ and 130 ± 20% above 16 M☉, each bin at its published value.
///
/// Two of these differ from Design note 4, which asked for them to be re-checked. Its 1.4
/// companions above 8 M☉ match neither of Duchêne and Kraus's two massive bins, and are replaced
/// by those bins. Its lifetime, 10 Gyr × m^−2.5, keeps stars of 3–8 M☉ alive about twice as long
/// as stellar tracks do (640 Myr at 3 M☉ against 350), so the old thin disc, whose ages reach down
/// to 100 Myr, kept too many of them: its mean mass came out 6.7% above the halo's, against the
/// brainstorm's "only 3%" (±3% in the research behind it, whose model gives 5.0%), and a 10 Gyr
/// declining history came to 0.505–0.511 M☉ under Kroupa's function against 0.48 ± 0.03. The
/// Raiteri fit is the lifetime law that research used, and with it the spread is 5.1% and the
/// history 0.498–0.503 M☉, with nothing tuned. The rest of the offset from 0.48 is the black-hole
/// law, whose masses of up to 40 M☉ are those of metal-poor stars; plan 06 replaces it.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fates::{ProvisionalFates, StellarFates};
///
/// let fates = ProvisionalFates;
/// assert!((fates.lifetime(1.0).value() / 9.52e9 - 1.0).abs() < 1e-3);
/// assert!((fates.remnant_mass(1.0) - 0.503).abs() < 1e-12);
/// assert_eq!(fates.remnant_mass(10.0), 1.4);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProvisionalFates;

impl ProvisionalFates {
    /// The metallicity Z at which the lifetime fit is taken: the solar value of the Padova tracks
    /// that Raiteri, Villata and Navarro (1996) fitted.
    pub const LIFETIME_METALLICITY: f64 = 0.02;

    /// The lowest mass of the lifetime fit's range, M☉: lighter stars live as long as this one.
    pub const LIFETIME_MIN_MASS: f64 = 0.6;

    /// The shortest lifetime, years (Design note 4).
    pub const LIFETIME_FLOOR: Years = Years::new(3e6);

    /// The initial mass above which a star leaves a neutron star, M☉.
    pub const NEUTRON_STAR_MIN_MASS: f64 = 8.0;

    /// The initial mass above which a star leaves a black hole, M☉.
    pub const BLACK_HOLE_MIN_MASS: f64 = 22.0;

    /// A neutron star's mass, M☉.
    pub const NEUTRON_STAR_MASS: f64 = 1.4;

    /// The heaviest black hole, M☉.
    pub const BLACK_HOLE_MAX_MASS: f64 = 40.0;

    /// The slope of Kalirai et al.'s initial–final mass relation.
    pub const WHITE_DWARF_SLOPE: f64 = 0.109;

    /// The intercept of Kalirai et al.'s initial–final mass relation, M☉.
    pub const WHITE_DWARF_INTERCEPT: f64 = 0.394;

    /// The upper edges of the companion-frequency bins, M☉, and each bin's mean number of stellar
    /// companions; the last bin runs to the top of the stellar range.
    pub const COMPANIONS: [(f64, f64); 4] =
        [(0.5, 0.30), (1.5, 0.60), (16.0, 1.0), (f64::INFINITY, 1.3)];
}

/// `log₁₀ Z` at [`ProvisionalFates::LIFETIME_METALLICITY`], 0.02 = 2 ÷ 100, in closed form so that
/// the coefficients below are constants.
const LIFETIME_LOG10_Z: f64 = core::f64::consts::LOG10_2 - 2.0;

/// Raiteri, Villata and Navarro's (1996, eq. 3) `a₀`, `a₁` and `a₂`, each `c₀ + c₁ log₁₀ Z +
/// c₂ (log₁₀ Z)²`, at [`ProvisionalFates::LIFETIME_METALLICITY`].
const LIFETIME_COEFFICIENTS: [f64; 3] = {
    const C: [[f64; 3]; 3] = [
        [10.13, 0.075_47, -0.008_084],
        [-4.424, -0.793_9, -0.118_7],
        [1.262, 0.338_5, 0.054_17],
    ];
    let z = LIFETIME_LOG10_Z;
    [
        C[0][0] + C[0][1] * z + C[0][2] * z * z,
        C[1][0] + C[1][1] * z + C[1][2] * z * z,
        C[2][0] + C[2][1] * z + C[2][2] * z * z,
    ]
};

/// `log₁₀ m` of the lifetime fit's turning point, `−a₁ ÷ 2a₂`: above it the quadratic would rise
/// with mass, so the mass is held there.
const LIFETIME_TURNING_LOG10_MASS: f64 =
    -LIFETIME_COEFFICIENTS[1] / (2.0 * LIFETIME_COEFFICIENTS[2]);

impl StellarFates for ProvisionalFates {
    fn lifetime(&self, m: f64) -> Years {
        let [a0, a1, a2] = LIFETIME_COEFFICIENTS;
        let x = math::log10(m.max(Self::LIFETIME_MIN_MASS)).min(LIFETIME_TURNING_LOG10_MASS);
        let years = math::exp10(a0 + a1 * x + a2 * x * x);
        Years::new(years.max(Self::LIFETIME_FLOOR.value()))
    }

    fn remnant_mass(&self, m: f64) -> f64 {
        if m < Self::NEUTRON_STAR_MIN_MASS {
            Self::WHITE_DWARF_SLOPE * m + Self::WHITE_DWARF_INTERCEPT
        } else if m < Self::BLACK_HOLE_MIN_MASS {
            Self::NEUTRON_STAR_MASS
        } else {
            (0.4 * m).min(Self::BLACK_HOLE_MAX_MASS)
        }
    }

    fn mean_companions(&self, m: f64) -> f64 {
        Self::COMPANIONS
            .iter()
            .find(|&&(edge, _)| m < edge)
            .map_or(Self::COMPANIONS[3].1, |&(_, count)| count)
    }

    fn breaks(&self) -> &[f64] {
        // The companion bins' edges and the remnant laws'. The lifetime's two limits need none:
        // at 0.6 M☉ it is 60 Gyr, longer than any age, and at the turning point its slope is 0.
        &[0.5, 1.5, 8.0, 16.0, 22.0]
    }
}

/// The range of a primary's companion masses, `[max(0.1 m, 0.08), m]`.
fn companion_range(m: f64) -> (f64, f64) {
    ((MIN_MASS_RATIO * m).max(MASS_LIMIT_LO), m)
}

/// The panel edges for the quadratures of this module: the stellar range's ends, the given
/// breaks inside it, ascending and without repeats.
fn panel_edges<'a>(breaks: impl IntoIterator<Item = &'a f64>) -> Vec<f64> {
    let mut edges: Vec<f64> = breaks
        .into_iter()
        .copied()
        .filter(|&m| m > MASS_LIMIT_LO && m < MASS_LIMIT_HI)
        .collect();
    edges.push(MASS_LIMIT_LO);
    edges.push(MASS_LIMIT_HI);
    edges.push(COMPANION_RANGE_SWITCH);
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    edges
}

/// The mass whose lifetime is `age`, if one inside the stellar range has it.
fn mass_with_lifetime(fates: &(impl StellarFates + ?Sized), age: f64) -> Option<f64> {
    let longest = fates.lifetime(MASS_LIMIT_LO).value();
    let shortest = fates.lifetime(MASS_LIMIT_HI).value();
    if !(shortest < age && age < longest) {
        return None;
    }
    let ln_m = bisect(
        |ln_m| fates.lifetime(math::exp(ln_m)).value() - age,
        math::ln(MASS_LIMIT_LO),
        math::ln(MASS_LIMIT_HI),
        LIFETIME_BISECTIONS,
    );
    Some(math::exp(ln_m))
}

/// The panels of the quadrature in `u = ln m`: the intervals between consecutive edges, each split
/// into equal parts no wider than [`MAX_PANEL_LN_MASS`].
fn ln_mass_panels(edges: &[f64]) -> Vec<(f64, f64)> {
    let mut panels = Vec::with_capacity(4 * edges.len());
    for pair in edges.windows(2) {
        let (start, end) = (math::ln(pair[0]), math::ln(pair[1]));
        let mut pieces = 1_u32;
        while (end - start) / f64::from(pieces) > MAX_PANEL_LN_MASS {
            pieces += 1;
        }
        let step = (end - start) / f64::from(pieces);
        for i in 0..pieces {
            let lo = start + step * f64::from(i);
            let hi = if i + 1 == pieces { end } else { lo + step };
            panels.push((lo, hi));
        }
    }
    panels
}

/// The running integral `G(m) = ∫ g dm` from 0.08 M☉, from `g`'s values at the panels' nodes: the
/// whole panels below `m`, then the partial panel through its interpolating polynomial
/// ([`Gl16Panel`]). Every edge of `g`'s kinks is a panel edge, so `g` is smooth on each panel.
struct Running {
    /// `ln m` at each panel's start.
    starts: Vec<f64>,
    /// Each panel's `g(eᵘ) eᵘ`, the integrand in `u`.
    panels: Vec<Gl16Panel>,
    /// `G` at each panel's start.
    sums: Vec<f64>,
}

impl Running {
    /// `G` at the mass `eᵘ`, for `u` inside the panels.
    fn at(&self, u: f64) -> f64 {
        let j = self.starts.partition_point(|&s| s <= u).max(1) - 1;
        self.sums[j] + self.panels[j].integral_to(u)
    }
}

/// The mean per system of a quantity `g` summed over a system's stars: for each primary, `g` of
/// the primary plus the mean number of companions times the mean of `g` over the companion range,
/// weighted by the mass function and divided by its integral.
///
/// The quadrature is 16-point Gauss–Legendre in ln m on the panels of [`ln_mass_panels`], and `g`
/// is evaluated once at each node. The companions' mean of `g` over `[lo, hi]` is
/// `(G(hi) − G(lo)) ÷ (hi − lo)`, with `G` the running integral of the same values ([`Running`]),
/// so the inner integrals cost no evaluation of `g`. (Taking each inner integral by a fresh
/// 16-point rule, as a first version did, evaluated `g` 33 times per node and took 0.6 ms per
/// population in a release build, most of a galaxy's parameters.)
fn per_system_mean(
    mass_function: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
    mut quantity: impl FnMut(f64) -> f64,
    extra_breaks: &[f64],
) -> f64 {
    let edges = panel_edges(
        mass_function
            .breaks()
            .iter()
            .chain(fates.breaks())
            .chain(extra_breaks),
    );
    let panels = ln_mass_panels(&edges);
    // Each node's ln m, m and g, panel by panel; then the running integral from them.
    let nodes: Vec<([f64; 16], [f64; 16], [f64; 16])> = panels
        .iter()
        .map(|&(lo, hi)| {
            let ln_masses = Gl16Panel::nodes(lo, hi);
            let masses = ln_masses.map(math::exp);
            (ln_masses, masses, masses.map(&mut quantity))
        })
        .collect();
    let mut running = Running {
        starts: Vec::with_capacity(panels.len()),
        panels: Vec::with_capacity(panels.len()),
        sums: Vec::with_capacity(panels.len()),
    };
    let mut integral = 0.0;
    for (&(lo, hi), (_, masses, own)) in panels.iter().zip(&nodes) {
        let mut integrand = [0.0; 16];
        for ((value, &m), &g) in integrand.iter_mut().zip(masses).zip(own) {
            *value = g * m;
        }
        let panel = Gl16Panel::new(lo, hi, &integrand);
        running.starts.push(lo);
        running.sums.push(integral);
        integral += panel.integral();
        running.panels.push(panel);
    }
    let ln_lowest = math::ln(MASS_LIMIT_LO);
    let ln_ratio = math::ln(MIN_MASS_RATIO);
    let mut sum = 0.0;
    for (&(lo, hi), (ln_masses, masses, own)) in panels.iter().zip(&nodes) {
        let mut panel_sum = 0.0;
        for (((&w, &u), &m), &g) in GL16_WEIGHTS.iter().zip(ln_masses).zip(masses).zip(own) {
            let (c_lo, c_hi) = companion_range(m);
            let companions = if c_hi > c_lo {
                // ln c_lo = max(ln m + ln 0.1, ln 0.08), as `companion_range` has it.
                let u_lo = (u + ln_ratio).max(ln_lowest);
                (running.at(u) - running.at(u_lo)) / (c_hi - c_lo)
            } else {
                g
            };
            panel_sum += w * mass_function.pdf(m) * (g + fates.mean_companions(m) * companions) * m;
        }
        sum += panel_sum * 0.5 * (hi - lo);
    }
    sum / mass_function.integral(MASS_LIMIT_LO, MASS_LIMIT_HI)
}

/// The mean present-day mass of a system whose age distribution is `ages`: living stars at their
/// initial mass, dead ones as their remnants, companions included (plan 02, Design note 4).
///
/// For a star of mass m the expected present mass over the ages is `m F + m_rem (1 − F)`, with `F`
/// the fraction of born systems younger than the star's lifetime ([`AgeDistribution::born_cdf`]);
/// the unborn systems of a still-forming population are left out, since its density is
/// normalised to its born systems. Under the default, Chabrier's system function with its
/// provisional scale, a 10 Gyr declining history comes to 0.57–0.58 M☉ per system, inside the
/// brainstorm's 0.55–0.59, and under Kroupa's function to about 0.5, its 0.48 ± 0.03.
///
/// # Panics
///
/// If no system of `ages` is born.
///
/// # Examples
///
/// The old halo holds less mass per system than a disc still forming stars:
///
/// ```
/// use hyperion_sim::galaxy::ages::{AgeDistribution, FeatureShare, HALO_AGES};
/// use hyperion_sim::galaxy::fates::{ProvisionalFates, mean_present_mass};
/// use hyperion_sim::galaxy::imf::Kroupa;
/// use hyperion_sim::units::Years;
///
/// let halo = AgeDistribution::uniform(HALO_AGES[0], HALO_AGES[1])?;
/// let young = AgeDistribution::young_disc(Years::new(7e9), FeatureShare::None)?;
/// let old = mean_present_mass(&Kroupa, &ProvisionalFates, &halo);
/// assert!(old < mean_present_mass(&Kroupa, &ProvisionalFates, &young));
/// assert!((0.45..0.50).contains(&old.value()));
/// # Ok::<(), hyperion_sim::galaxy::ages::BuildAgeDistributionError>(())
/// ```
#[must_use]
pub fn mean_present_mass(
    f: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
    ages: &AgeDistribution,
) -> SolarMasses {
    mean_present_mass_of_mixture(f, fates, &[(1.0, ages)])
}

/// [`mean_present_mass`] for a mixture of age distributions with the given weights, such as the
/// halo's components, in one quadrature.
///
/// The distribution restricted to born systems is `Σ wᵢ (Fᵢ(t) − Fᵢ(0)) ÷ Σ wᵢ (1 − Fᵢ(0))`. The
/// weights need not sum to 1.
///
/// # Panics
///
/// If no system of the mixture is born, or if `parts` is empty.
#[must_use]
pub fn mean_present_mass_of_mixture(
    f: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
    parts: &[(f64, &AgeDistribution)],
) -> SolarMasses {
    assert!(!parts.is_empty(), "a mixture of no age distributions");
    let unborn: Vec<f64> = parts.iter().map(|(_, a)| a.cdf(Years::ZERO)).collect();
    let born = parts
        .iter()
        .zip(&unborn)
        .fold(0.0, |sum, ((w, _), u)| sum + w * (1.0 - u));
    assert!(
        born > 0.0,
        "a mixture of age distributions with no born system"
    );
    let alive_fraction = |lifetime: Years| {
        if lifetime.value() <= 0.0 {
            return 0.0;
        }
        let younger = parts
            .iter()
            .zip(&unborn)
            .fold(0.0, |sum, ((w, a), u)| sum + w * (a.cdf(lifetime) - u));
        (younger / born).clamp(0.0, 1.0)
    };
    let mut kinks: Vec<f64> = parts
        .iter()
        .flat_map(|(_, a)| a.edges())
        .filter(|&age| age > 0.0)
        .filter_map(|age| mass_with_lifetime(fates, age))
        .collect();
    kinks.sort_by(f64::total_cmp);
    let present = |m: f64| {
        let remnant = fates.remnant_mass(m);
        remnant + (m - remnant) * alive_fraction(fates.lifetime(m))
    };
    SolarMasses::new(per_system_mean(f, fates, present, &kinks))
}

/// The initial mass of the primary and its companions, per system: the mass formed per system,
/// with nothing dead.
///
/// It reads only the mass function and [`StellarFates::mean_companions`], never a lifetime, so it
/// is one number per galaxy. Rates quoted per solar mass formed (plan 09's Type Ia delay times,
/// plan 11's class shares) use it. It equals [`mean_present_mass`] for systems too young for any
/// star to have died, and exceeds it otherwise.
#[must_use]
pub fn mean_formed_mass(
    f: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
) -> SolarMasses {
    SolarMasses::new(per_system_mean(f, fates, |m| m, &[]))
}

/// The mean number of stars per system: 1 plus the mean number of stellar companions.
#[must_use]
pub fn mean_stars_per_system(
    f: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
) -> f64 {
    per_system_mean(f, fates, |_| 1.0, &[])
}

/// The fraction of all stars, companions included, whose initial mass lies below `m` M☉.
///
/// This is the arbiter between the mass functions (brainstorm, "Sizing the layers"): the 20 pc
/// census, which counts primaries and companions directly, has 69% of all its stars below 0.5 M☉
/// (Kirkpatrick et al. 2024, ApJS 271, 55, Table 18: 69.2% of those of 0.08 M☉ or more). With the
/// provisional companions, Kroupa's function for primaries gives 76.4% and fails; Chabrier's system
/// function gives 66.9% as published and 70.9% with its branch above 1 M☉ scaled by 0.68, so the
/// two bracket the census.
#[must_use]
pub fn stars_below(
    f: &(impl MassFunction + ?Sized),
    fates: &(impl StellarFates + ?Sized),
    m: f64,
) -> f64 {
    // The primary's indicator jumps at `m`, and the share of the companion range below `m` has a
    // kink where the range's lower end reaches `m`, at a primary of 10 m.
    let below = per_system_mean(
        f,
        fates,
        |x| if x < m { 1.0 } else { 0.0 },
        &[m, m / MIN_MASS_RATIO],
    );
    below / mean_stars_per_system(f, fates)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::ages::{
        BULGE_AGES, FeatureShare, HALO_AGES, LONG_BAR_AGES, THICK_DISC_AGES, THIN_DISC_HISTORY,
        YOUNG_AGE_LIMIT,
    };
    use crate::galaxy::consts::YEARS_PER_GIGAYEAR;
    use crate::galaxy::imf::{Chabrier, Kroupa};

    fn gyr(x: f64) -> Years {
        Years::new(x * YEARS_PER_GIGAYEAR)
    }

    fn history(tau: f64) -> AgeDistribution {
        AgeDistribution::exponential_history(gyr(tau), Years::ZERO, THIN_DISC_HISTORY).unwrap()
    }

    fn old_thin(tau: f64) -> AgeDistribution {
        AgeDistribution::exponential_history(gyr(tau), YOUNG_AGE_LIMIT, THIN_DISC_HISTORY).unwrap()
    }

    fn young(tau: f64) -> AgeDistribution {
        AgeDistribution::young_disc(gyr(tau), FeatureShare::None).unwrap()
    }

    /// The populations whose stars are all old: thick disc, bulge, bar, nuclear disc, halo.
    fn old_populations() -> [(&'static str, AgeDistribution); 5] {
        let uniform = |[lo, hi]: [Years; 2]| AgeDistribution::uniform(lo, hi).unwrap();
        [
            ("thick", uniform(THICK_DISC_AGES)),
            ("bulge", uniform(BULGE_AGES)),
            ("bar", uniform(LONG_BAR_AGES)),
            ("nuclear", AgeDistribution::nuclear_disc()),
            ("halo", uniform(HALO_AGES)),
        ]
    }

    fn present(f: &dyn MassFunction, ages: &AgeDistribution) -> f64 {
        mean_present_mass(f, &ProvisionalFates, ages).value()
    }

    /// Brainstorm, "Galaxy parameters": about 0.55–0.59 M☉ under the default, Chabrier's system
    /// function with its branch above 1 M☉ scaled, and 0.48 under Kroupa's, for a 10 Gyr declining
    /// history, at every timescale the seed can draw.
    #[test]
    fn a_declining_history_matches_the_brainstorm() {
        for tau in [5.0, 7.0, 9.0] {
            let default = present(&Chabrier::provisional(), &history(tau));
            assert!(
                (0.55..=0.59).contains(&default),
                "Chabrier, τ = {tau}: {default}"
            );
            let kroupa = present(&Kroupa, &history(tau));
            assert!((kroupa - 0.48).abs() <= 0.03, "Kroupa, τ = {tau}: {kroupa}");
        }
    }

    /// "Varies by only 3% between the old populations", under the default, which the brainstorm's
    /// worked figures use, and under Kroupa's function. The research behind the figure puts every
    /// population but the young disc within ±3% (its own model spreads by 5.0% from the halo to the
    /// old thin disc), so every old population, the old thin disc at each timescale the seed can
    /// draw included, lies within 3% of their midpoint. The five uniformly old ones agree to 3%
    /// outright.
    #[test]
    fn old_populations_agree_within_three_per_cent() {
        for f in [&Chabrier::provisional() as &dyn MassFunction, &Kroupa] {
            old_populations_agree_under(f);
        }
    }

    fn old_populations_agree_under(f: &dyn MassFunction) {
        let mut named: Vec<(String, f64)> = old_populations()
            .iter()
            .map(|(name, ages)| ((*name).to_owned(), present(f, ages)))
            .collect();
        let uniformly_old: Vec<f64> = named.iter().map(|(_, m)| *m).collect();
        let lightest = uniformly_old.iter().copied().fold(f64::INFINITY, f64::min);
        let heaviest = uniformly_old.iter().copied().fold(0.0, f64::max);
        assert!(heaviest / lightest <= 1.03, "{named:?}");
        for tau in [5.0, 7.0, 9.0] {
            named.push((format!("old thin, τ = {tau}"), present(f, &old_thin(tau))));
        }
        let lightest = named.iter().map(|(_, m)| *m).fold(f64::INFINITY, f64::min);
        let heaviest = named.iter().map(|(_, m)| *m).fold(0.0, f64::max);
        let midpoint = f64::midpoint(lightest, heaviest);
        assert!(
            heaviest / midpoint - 1.0 <= 0.03,
            "{:.2}% about {midpoint}: {named:?}",
            100.0 * (heaviest / midpoint - 1.0)
        );
    }

    /// The young disc has lost almost nothing yet: 35–45% more mass per system than the old thin
    /// disc of the same history, under either function.
    #[test]
    fn the_young_disc_is_heavier_by_a_third_or_more() {
        for f in [&Chabrier::provisional() as &dyn MassFunction, &Kroupa] {
            for tau in [5.0, 7.0, 9.0] {
                let ratio = present(f, &young(tau)) / present(f, &old_thin(tau));
                assert!((1.35..=1.45).contains(&ratio), "{f:?}, τ = {tau}: {ratio}");
            }
        }
    }

    #[test]
    fn stars_per_system_lie_in_the_observed_range() {
        for f in [&Kroupa as &dyn MassFunction, &Chabrier::provisional()] {
            let stars = mean_stars_per_system(f, &ProvisionalFates);
            assert!((1.33..=1.45).contains(&stars), "{f:?}: {stars}");
        }
    }

    /// The arbiter of "Sizing the layers": the 20 pc census has 69% of all its stars below 0.5 M☉
    /// (Kirkpatrick et al. 2024, Table 18: 69.2%), and 66–68% of its primaries (66.5% within 20 pc
    /// and 67.8% within 10 pc, tallied from Kirkpatrick et al. 2024, Table 4). Chabrier's
    /// system function as published and with its branch above 1 M☉ scaled by 0.68 bracket the
    /// first, at the brainstorm's 66.9% and 70.9% to the precision printed; the published
    /// function's share of primaries, 66%, lies among the census's. Kroupa's gives the brainstorm's
    /// 76.4% of all stars and 76% of primaries, above both.
    #[test]
    fn chabrier_s_system_function_brackets_the_census() {
        const CENSUS_ALL_STARS: f64 = 0.69;
        let printed = |share: f64, percent: f64| (100.0 * share - percent).abs() <= 0.05;
        let published = Chabrier::new(1.0).unwrap();
        let scaled = Chabrier::provisional();
        let below = |f: &dyn MassFunction| stars_below(f, &ProvisionalFates, 0.5);
        let (low, high) = (below(&published), below(&scaled));
        assert!(
            low < CENSUS_ALL_STARS && CENSUS_ALL_STARS < high,
            "{low} and {high} bracket 0.69"
        );
        assert!(printed(low, 66.9), "published: {low}");
        assert!(printed(high, 70.9), "scaled: {high}");
        let primaries =
            crate::galaxy::imf::BandShares::of(&published).share(crate::galaxy::imf::MassBand::A);
        assert!((0.66..=0.68).contains(&primaries), "primaries: {primaries}");
        let kroupa = below(&Kroupa);
        assert!(printed(kroupa, 76.4), "Kroupa: {kroupa}");
        assert_same_bits(stars_below(&Kroupa, &ProvisionalFates, 0.08), 0.0);
        let all = stars_below(&scaled, &ProvisionalFates, 150.5);
        assert!((all - 1.0).abs() < 1e-12, "{all}");
    }

    /// With every system far younger than the shortest lifetime, nothing has died.
    #[test]
    fn formed_mass_equals_present_mass_when_nothing_has_died() {
        let newborn = AgeDistribution::uniform(Years::ZERO, Years::new(1.0)).unwrap();
        for f in [&Kroupa as &dyn MassFunction, &Chabrier::provisional()] {
            let formed = mean_formed_mass(f, &ProvisionalFates).value();
            let present = present(f, &newborn);
            assert!(
                ((formed - present) / formed).abs() < 1e-9,
                "{formed} against {present}"
            );
        }
    }

    #[test]
    fn formed_mass_exceeds_present_mass_for_every_population() {
        for f in [&Chabrier::provisional() as &dyn MassFunction, &Kroupa] {
            let formed = mean_formed_mass(f, &ProvisionalFates).value();
            let mut all = vec![("young", young(7.0)), ("old thin", old_thin(7.0))];
            all.extend(old_populations());
            for (name, ages) in all {
                let present = present(f, &ages);
                assert!(formed > present, "{f:?}, {name}: {present} ≥ {formed}");
            }
        }
    }

    /// The quadrature against a brute-force one: 20,000 log-spaced midpoints for the primary and
    /// 400 for each primary's companions.
    #[test]
    fn the_quadrature_matches_brute_force() {
        let ages = old_thin(7.0);
        let fates = ProvisionalFates;
        let p = |m: f64| {
            let r = fates.remnant_mass(m);
            r + (m - r) * ages.born_cdf(fates.lifetime(m))
        };
        let midpoints = |lo: f64, hi: f64, n: u32| {
            let (a, b) = (math::ln(lo), math::ln(hi));
            (0..n).map(move |i| {
                let m = math::exp(a + (b - a) * (f64::from(i) + 0.5) / f64::from(n));
                (m, m * (b - a) / f64::from(n))
            })
        };
        let mut numerator = 0.0;
        for (m, dm) in midpoints(MASS_LIMIT_LO, MASS_LIMIT_HI, 20_000) {
            let (lo, hi) = companion_range(m);
            let companions = if hi > lo {
                midpoints(lo, hi, 400).map(|(x, dx)| p(x) * dx).sum::<f64>() / (hi - lo)
            } else {
                p(m)
            };
            numerator += Kroupa.pdf(m) * (p(m) + fates.mean_companions(m) * companions) * dm;
        }
        let brute = numerator / Kroupa.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
        let quadrature = present(&Kroupa, &ages);
        assert!(
            ((brute - quadrature) / brute).abs() < 1e-4,
            "{brute} against {quadrature}"
        );
    }

    #[test]
    fn a_mixture_of_one_is_the_distribution() {
        let halo = AgeDistribution::uniform(HALO_AGES[0], HALO_AGES[1]).unwrap();
        let single = mean_present_mass(&Kroupa, &ProvisionalFates, &halo);
        let mixture = mean_present_mass_of_mixture(&Kroupa, &ProvisionalFates, &[(3.0, &halo)]);
        assert_eq!(single, mixture);
        let early = AgeDistribution::uniform(gyr(12.0), gyr(13.0)).unwrap();
        let late = AgeDistribution::uniform(gyr(10.0), gyr(11.0)).unwrap();
        let both = mean_present_mass_of_mixture(
            &Kroupa,
            &ProvisionalFates,
            &[(0.5, &early), (0.5, &late)],
        );
        let average = f64::midpoint(present(&Kroupa, &early), present(&Kroupa, &late));
        assert!(((both.value() - average) / average).abs() < 1e-6);
    }

    /// The lifetime is Raiteri, Villata and Navarro's (1996) fit at Z = 0.02, computed here from
    /// their equation directly, and near the Padova tracks it was fitted to.
    #[test]
    fn provisional_lifetimes_follow_raiteri_et_al() {
        let fates = ProvisionalFates;
        let log_z = math::log10(0.02);
        let a = |c0: f64, c1: f64, c2: f64| c0 + c1 * log_z + c2 * log_z * log_z;
        let (a0, a1, a2) = (
            a(10.13, 0.075_47, -0.008_084),
            a(-4.424, -0.793_9, -0.118_7),
            a(1.262, 0.338_5, 0.054_17),
        );
        for m in [0.6, 0.8, 1.0, 2.0, 5.0, 20.0, 100.0] {
            let x = math::log10(m);
            let expected = math::exp10(a0 + a1 * x + a2 * x * x);
            let actual = fates.lifetime(m).value();
            assert!(
                ((actual - expected) / expected).abs() < 1e-13,
                "{m}: {actual}"
            );
        }
        // Main-sequence lifetimes at solar metallicity: about 10 Gyr, 1 Gyr, 100 Myr and 9 Myr.
        for (m, gyr) in [(1.0, 9.5), (2.0, 1.06), (5.0, 0.100), (20.0, 0.0091)] {
            let actual = fates.lifetime(m).value() / 1e9;
            assert!((actual / gyr - 1.0).abs() < 0.01, "{m} M☉: {actual} Gyr");
        }
        // Held below 0.6 M☉ and above the turning point, 106.3 M☉, so it never rises with mass.
        assert_eq!(fates.lifetime(0.1), fates.lifetime(0.6));
        let turning = math::exp10(-a1 / (2.0 * a2));
        assert!((turning - 106.3).abs() < 0.05, "{turning}");
        assert_eq!(fates.lifetime(150.0), fates.lifetime(turning));
        let mut previous = f64::INFINITY;
        for i in 0..=10_000 {
            let m = math::exp(
                math::ln(MASS_LIMIT_LO)
                    + (math::ln(MASS_LIMIT_HI) - math::ln(MASS_LIMIT_LO)) * f64::from(i) / 1e4,
            );
            let t = fates.lifetime(m).value();
            assert!(t <= previous, "the lifetime rises at {m} M☉");
            assert!(t > ProvisionalFates::LIFETIME_FLOOR.value());
            previous = t;
        }
        let to_one_gyr = mass_with_lifetime(&fates, 1e9).unwrap();
        assert!(((fates.lifetime(to_one_gyr).value() - 1e9) / 1e9).abs() < 1e-12);
        assert_eq!(mass_with_lifetime(&fates, 1e6), None);
        assert_eq!(mass_with_lifetime(&fates, 1e12), None);
    }

    #[test]
    fn provisional_remnants_and_companions_follow_their_sources() {
        let fates = ProvisionalFates;
        assert!((fates.remnant_mass(7.9) - (0.109 * 7.9 + 0.394)).abs() < 1e-15);
        assert_same_bits(fates.remnant_mass(21.9), 1.4);
        assert!((fates.remnant_mass(30.0) - 12.0).abs() < 1e-12);
        assert_same_bits(fates.remnant_mass(150.0), 40.0);
        // Duchêne and Kraus (2013), Table 1, each bin at its published companion frequency.
        let counts: Vec<f64> = [0.1, 0.49, 0.5, 1.0, 1.5, 7.9, 8.0, 15.9, 16.0, 150.0]
            .iter()
            .map(|&m| fates.mean_companions(m))
            .collect();
        assert_eq!(
            counts,
            vec![0.30, 0.30, 0.60, 0.60, 1.0, 1.0, 1.0, 1.0, 1.3, 1.3]
        );
    }
}
