//! Checking the primary parameters against their ranges (plan 02, P02.T5.a).
//!
//! Every value must lie inside the range its law draws from ([`Law::bounds`]), so that a built
//! galaxy is one the seed could have drawn. A scatter's range is ±9 standard deviations, beyond
//! any value a Box–Muller draw reaches. The checks run in the order of P02.T5.a's table, except
//! that the last major merger comes before the halo, whose ages depend on it, and report the first
//! failure.

use super::BuildGalaxyParamsError;
use super::accretion::Orbit;
use super::draws::{self as d, Law};
use super::inputs::{HaloComponentInput, Inputs, Size};

/// A coupled size's name, its law for the scatter, and its fixed range.
pub(super) struct SizeRange {
    pub parameter: &'static str,
    pub scatter_parameter: &'static str,
    pub scatter: Law,
    pub min: f64,
    pub max: f64,
}

impl SizeRange {
    fn check(&self, size: Size) -> Result<(), BuildGalaxyParamsError> {
        match size {
            Size::Fixed(size) => check_range(self.parameter, size, self.min, self.max),
            Size::Coupled { scatter } => check_law(self.scatter_parameter, scatter, self.scatter),
        }
    }
}

fn check_range(
    parameter: &'static str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), BuildGalaxyParamsError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(BuildGalaxyParamsError::OutOfRange {
            parameter,
            value,
            min,
            max,
        })
    }
}

fn check_law(parameter: &'static str, value: f64, law: Law) -> Result<(), BuildGalaxyParamsError> {
    let (min, max) = law.bounds();
    check_range(parameter, value, min, max)
}

fn check_all(checks: &[(&'static str, f64, Law)]) -> Result<(), BuildGalaxyParamsError> {
    checks
        .iter()
        .try_for_each(|&(parameter, value, law)| check_law(parameter, value, law))
}

fn check_orbit(orbit: &Orbit) -> Result<(), BuildGalaxyParamsError> {
    let apocentre = orbit.apocentre().value();
    check_law("accretion.progenitor.orbit", apocentre, d::ORBIT_APOCENTRE)?;
    check_range(
        "accretion.progenitor.orbit",
        orbit.pericentre().value(),
        0.0,
        apocentre,
    )?;
    check_orientation(orbit)
}

/// The dominant merger's orbit: [`check_orbit`], and a pericentre that gives an eccentricity in
/// [`DOMINANT_ECCENTRICITY`](d::DOMINANT_ECCENTRICITY).
///
/// The pericentre is compared with the apocentre times the ratio at each end of the range, the
/// same products the draw forms, so a drawn orbit passes exactly.
fn check_dominant_orbit(orbit: &Orbit) -> Result<(), BuildGalaxyParamsError> {
    check_orbit(orbit)?;
    let (e_min, e_max) = d::DOMINANT_ECCENTRICITY.bounds();
    let apocentre = orbit.apocentre().value();
    check_range(
        "accretion.progenitor.orbit",
        orbit.pericentre().value(),
        d::pericentre_ratio(e_max) * apocentre,
        d::pericentre_ratio(e_min) * apocentre,
    )
}

/// An orbit's inclination, node and phase.
fn check_orientation(orbit: &Orbit) -> Result<(), BuildGalaxyParamsError> {
    let pi = core::f64::consts::PI;
    check_range(
        "accretion.progenitor.orbit",
        orbit.inclination().value(),
        0.0,
        pi,
    )?;
    check_range(
        "accretion.progenitor.orbit",
        orbit.node().value(),
        0.0,
        2.0 * pi,
    )?;
    check_range(
        "accretion.progenitor.orbit",
        orbit.phase().value(),
        0.0,
        2.0 * pi,
    )
}

/// A smooth halo component: share, flattening, core and slope by the given names and laws, and
/// its age centre by `age`.
fn check_halo_component(
    component: &HaloComponentInput,
    checks: [(&'static str, Law); 4],
    age: Law,
) -> Result<(), BuildGalaxyParamsError> {
    check_all(&[
        (checks[0].0, component.share, checks[0].1),
        (checks[1].0, component.flattening, checks[1].1),
        (checks[2].0, component.core, checks[2].1),
        (checks[3].0, component.slope, checks[3].1),
        ("halo.component.age", component.age_centre, age),
    ])
}

/// The discs, bulge, bar, arms, gas, dark halo, black hole and metallicity.
fn check_structure(i: &Inputs, sizes: &[SizeRange; 4]) -> Result<(), BuildGalaxyParamsError> {
    check_all(&[
        ("stellar_mass", i.stellar_mass, d::STELLAR_MASS),
        ("share.thick", i.share_thick, d::SHARE_THICK),
        ("share.bulge_bar", i.share_bulge_bar, d::SHARE_BULGE_BAR),
        (
            "share.bar_of_bulge",
            i.share_bar_of_bulge,
            d::SHARE_BAR_OF_BULGE,
        ),
        (
            "share.nuclear_disc",
            i.share_nuclear_disc,
            d::SHARE_NUCLEAR_DISC,
        ),
        ("share.halo", i.share_halo, d::SHARE_HALO),
        ("sfh.timescale", i.sfh_timescale, d::SFH_TIMESCALE),
    ])?;
    sizes[0].check(i.thin_length)?;
    check_all(&[
        ("thin.mean_height", i.thin_mean_height, d::THIN_MEAN_HEIGHT),
        ("young.height", i.young_height, d::YOUNG_HEIGHT),
        (
            "thick.length_ratio",
            i.thick_length_ratio,
            d::THICK_LENGTH_RATIO,
        ),
        (
            "thick.height_ratio",
            i.thick_height_ratio,
            d::THICK_HEIGHT_RATIO,
        ),
    ])?;
    sizes[1].check(i.bulge_length)?;
    check_all(&[
        ("bulge.b_over_a", i.bulge_b_over_a, d::BULGE_B_OVER_A),
        ("bulge.c_over_a", i.bulge_c_over_a, d::BULGE_C_OVER_A),
        ("bulge.boxiness", i.bulge_boxiness, d::BULGE_BOXINESS),
    ])?;
    sizes[2].check(i.bar_length)?;
    check_all(&[
        ("bar.width_ratio", i.bar_width_ratio, d::BAR_WIDTH_RATIO),
        ("bar.height", i.bar_height, d::BAR_HEIGHT),
        (
            "bar.corotation_ratio",
            i.bar_corotation_ratio,
            d::BAR_COROTATION_RATIO,
        ),
    ])?;
    sizes[3].check(i.nuclear_length)?;
    check_all(&[
        (
            "nuclear.height_ratio",
            i.nuclear_height_ratio,
            d::NUCLEAR_HEIGHT_RATIO,
        ),
        (
            "nuclear_cluster.mass.scatter",
            i.nuclear_cluster_mass_scatter,
            d::NUCLEAR_CLUSTER_MASS_SCATTER,
        ),
        ("arms.pitch", i.arm_pitch, d::ARMS_PITCH_DEGREES),
        ("arms.young_width", i.arm_young_width, d::ARMS_YOUNG_WIDTH),
        (
            "arms.young_fraction",
            i.arm_young_fraction,
            d::ARMS_YOUNG_FRACTION,
        ),
        (
            "arms.old_amplitude",
            i.arm_old_amplitude,
            d::ARMS_OLD_AMPLITUDE,
        ),
        (
            "gas.mass_fraction",
            i.gas_mass_fraction,
            d::GAS_MASS_FRACTION,
        ),
        ("gas.length_ratio", i.gas_length_ratio, d::GAS_LENGTH_RATIO),
        ("dark.f_star", i.dark_f_star, d::DARK_F_STAR),
        (
            "dark.concentration.scatter",
            i.dark_concentration_scatter,
            d::DARK_CONCENTRATION_SCATTER,
        ),
        ("bh.scatter", i.bh_scatter, d::BH_SCATTER),
        (
            "metallicity.gradient",
            i.metallicity_gradient,
            d::METALLICITY_GRADIENT,
        ),
    ])
}

/// The halo's components.
///
/// The in-situ and dominant components' age centres are checked against the range that the last
/// major merger leaves them ([`heated_age_centre`](d::heated_age_centre)), and each lesser
/// progenitor's accretion against its own stars ([`lesser_accretion`](d::lesser_accretion)), so
/// the merger is checked first, by [`validate`].
fn check_halo(i: &Inputs) -> Result<(), BuildGalaxyParamsError> {
    let heated = d::heated_age_centre(i.last_major_merger);
    check_halo_component(
        &i.halo_in_situ,
        [
            ("halo.in_situ.share", d::HALO_IN_SITU_SHARE),
            ("halo.in_situ.flattening", d::HALO_IN_SITU_FLATTENING),
            ("halo.in_situ.core", d::HALO_IN_SITU_CORE),
            ("halo.component.slope", d::HALO_COMPONENT_SLOPE),
        ],
        heated,
    )?;
    check_halo_component(
        &i.halo_dominant,
        [
            ("halo.dominant.share", d::HALO_DOMINANT_SHARE),
            ("halo.dominant.flattening", d::HALO_DOMINANT_FLATTENING),
            ("halo.dominant.core", d::HALO_DOMINANT_CORE),
            ("halo.component.slope", d::HALO_COMPONENT_SLOPE),
        ],
        heated,
    )?;
    check_all(&[
        (
            "halo.dominant.break_radius",
            i.halo_dominant_break_radius,
            d::HALO_DOMINANT_BREAK_RADIUS,
        ),
        (
            "halo.dominant.break_steepening",
            i.halo_dominant_break_steepening,
            d::HALO_DOMINANT_BREAK_STEEPENING,
        ),
    ])?;
    let (fewest, most) = d::HALO_LESSER_COUNT;
    if !(fewest..=most).contains(&i.halo_lesser.len()) {
        return Err(BuildGalaxyParamsError::LesserProgenitorCount {
            count: i.halo_lesser.len(),
        });
    }
    check_law(
        "halo.lesser.share_total",
        i.halo_lesser_share_total,
        d::HALO_LESSER_SHARE_TOTAL,
    )?;
    for lesser in &i.halo_lesser {
        check_range("halo.lesser.split", lesser.weight, 0.0, f64::MAX)?;
        check_all(&[
            (
                "halo.lesser.flattening",
                lesser.flattening,
                d::HALO_LESSER_FLATTENING,
            ),
            (
                "halo.component.slope",
                lesser.slope,
                d::HALO_COMPONENT_SLOPE,
            ),
            (
                "halo.component.age",
                lesser.age_centre,
                d::HALO_COMPONENT_AGE_CENTRE,
            ),
            ("halo.component.feh", lesser.feh_mean, d::HALO_LESSER_FEH),
            (
                "accretion.progenitor.time",
                lesser.accreted,
                d::lesser_accretion(lesser.age_centre),
            ),
        ])?;
        check_orbit(&lesser.orbit)?;
    }
    if i.halo_lesser.iter().fold(0.0, |sum, l| sum + l.weight) <= 0.0 {
        return Err(BuildGalaxyParamsError::LesserWeightsZero);
    }
    let spherical = Law::Uniform {
        lo: d::DEBRIS_FLATTENING,
        hi: d::DEBRIS_FLATTENING,
    };
    check_halo_component(
        &i.halo_debris,
        [
            ("halo.debris.share", d::HALO_DEBRIS_SHARE),
            ("halo.debris.flattening", spherical),
            ("halo.debris.core", d::HALO_DEBRIS_CORE),
            ("halo.debris.slope", d::HALO_DEBRIS_SLOPE),
        ],
        d::HALO_COMPONENT_AGE_CENTRE,
    )?;
    check_law(
        "halo.discrete_share",
        i.halo_discrete_share,
        d::HALO_DISCRETE_SHARE,
    )
}

/// The accretion history beyond the last major merger and the lesser progenitors.
fn check_accretion(i: &Inputs) -> Result<(), BuildGalaxyParamsError> {
    check_dominant_orbit(&i.dominant_orbit)?;
    for recent in &i.recent {
        let (lightest, heaviest) = d::RECENT_MASS;
        check_range("accretion.progenitor.mass", recent.mass, lightest, heaviest)?;
        check_law(
            "accretion.progenitor.time",
            recent.accreted,
            d::RECENT_ACCRETED,
        )?;
        check_orbit(&recent.orbit)?;
    }
    check_law(
        "accretion.globular_count.scatter",
        i.globular_count_scatter,
        d::GLOBULAR_COUNT_SCATTER,
    )
}

/// Checks every primary value against its range; `sizes` are the thin disc's, the bulge's, the
/// bar's and the nuclear disc's.
pub(super) fn validate(i: &Inputs, sizes: &[SizeRange; 4]) -> Result<(), BuildGalaxyParamsError> {
    check_structure(i, sizes)?;
    // Out of the table's order: the halo's ages are conditioned on the merger.
    check_law(
        "accretion.last_major_merger",
        i.last_major_merger,
        d::LAST_MAJOR_MERGER,
    )?;
    check_halo(i)?;
    check_accretion(i)
}
