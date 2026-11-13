//! Test streams like three of the Milky Way's (plan 10's Provides; P10.T5.b is their first user):
//! a living globular's, an orphan's and a dwarf's, each a [`StreamSpec`] in a galaxy's potential
//! tables, which only the in-plane part of the tables reads.
//!
//! Their progenitors' masses and turning points are the named streams', from the sources each
//! helper cites; their orbital planes and phases are chosen near the real ones, not fitted. Each
//! specification follows the generator's own rules from there: the orbit in the mid-plane
//! potential taken as spherical, the stripping time of Design note 5 at the Milky Way fixture's
//! last major merger, 10 Gyr ago, and the mass loss of P10.T3. So a helper's stream is what the
//! generator would make of such a progenitor, and its figures move with the potential.

use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::PointLy;
use crate::galaxy::features::FeatureId;
use crate::galaxy::features::interior::counts::CANONICAL_SLOPE;
use crate::galaxy::features::testing::{PAL_5, named_cluster};
use crate::galaxy::params::{GalaxyParams, Orbit};
use crate::galaxy::potential::PotentialTables;
use crate::id::FeatureCell;
use crate::math;
use crate::units::{Dex, Kiloparsecs, LightYears, Radians, SolarMasses, SolarMassesPerYear, Years};

use super::dwarfs::{epoch_state, pericentres_within, remaining};
use super::spec::{ClusterStars, MassLoss, SphericalOrbit, StreamSpec, stripping_time};
use super::{StreamNumber, StreamOrigin};

/// A living globular's stream like Palomar 5's, stream number 0, in `tables`' potential.
///
/// The cluster is the Baumgardt–Hilker catalogue's Pal 5 ([`named_cluster`]): 1.34 × 10⁴ M☉ now,
/// 10^4.78 M☉ at birth, on an orbit between 8.88 and 17.54 kpc (Vasiliev and Baumgardt 2021), with
/// Harris's [Fe/H] of −1.41 (1996, 2010 edition) and an age of 11.5 Gyr, inside plan 09's
/// accreted globulars' 10.5–13 Gyr. It has lost mass at a constant rate, what it lost since birth
/// over its age, `4.1 × 10⁻⁶` M☉ a year, and is just past apocentre, falling inwards, as Pal 5 is,
/// on an orbit inclined 70° to the disc (the catalogue's state gives 67°). Its tube holds what it
/// lost within the stripping time.
///
/// # Panics
///
/// If Pal 5's turning points bound no orbit in `tables`' mid-plane potential, which a Milky
/// Way-like galaxy's always do.
#[must_use]
pub fn pal5_like(tables: &PotentialTables) -> StreamSpec {
    let cluster = named_cluster(PAL_5);
    let orbit = orbit_between(tables, cluster.pericentre_kpc, cluster.apocentre_kpc);
    let mass = SolarMasses::new(cluster.mass);
    let age = Years::new(11.5e9);
    let rate = (math::exp10(cluster.log_initial_mass) - cluster.mass) / age.value();
    let tidal = tidal_radius(tables, mass, &orbit);
    let window = stripping_time(&orbit, tidal, [last_major_merger(), age]);
    let period = orbit.radial_period().value();
    let (position, velocity) = place(tables, &orbit, [70.0, 0.0, 0.0], 0.55 * period);
    let cell = FeatureCell::new([0, 0, 0]).expect("the origin's cell is inside the root cube");
    StreamSpec {
        number: StreamNumber(0),
        origin: StreamOrigin::LivingGlobular(
            FeatureId::of(cell, 0).expect("index 0 is a candidate index"),
        ),
        position,
        velocity,
        orbit,
        stripping_time: window,
        mass: SolarMasses::new(rate * window.value()),
        progenitor_mass: mass,
        tidal_radius: tidal,
        mass_loss: MassLoss::Steady(SolarMassesPerYear::new(rate)),
        // A test stream's figures (lean, 2026-10-30), where a drawn globular takes plan 09's from
        // its model: it keeps its present mass's share of its birth mass, and a dynamically old
        // cluster's flat slope.
        stars: Some(ClusterStars {
            fe_h: Dex::new(-1.41),
            age,
            kept: cluster.mass / math::exp10(cluster.log_initial_mass),
            depleted_slope: 0.0,
        }),
    }
}

/// An orphan's stream like GD-1's, stream number 1, in `tables`' potential.
///
/// GD-1's orbit is retrograde, between 14 and 26 kpc, inclined 39° to the disc, and the stream is
/// near its pericentre, with an age of 9 Gyr and a photometric [Fe/H] of −1.9 (Koposov, Rix and
/// Hogg 2010, ApJ 712, 260, §§2, 5.1); its stream holds 7 × 10³ M☉ (Bonaca and Price-Whelan 2024,
/// arXiv:2405.19410, Table A.1). Its cluster dissolved halfway through the stripping time, the
/// orphans' mean (P10.T3.b's dissolution share is uniform), and its tidal radius is at the tube's
/// mass, as an orphan's is.
///
/// # Panics
///
/// If GD-1's turning points bound no orbit in `tables`' mid-plane potential, which a Milky
/// Way-like galaxy's always do.
#[must_use]
pub fn gd1_like(tables: &PotentialTables) -> StreamSpec {
    let orbit = orbit_between(tables, 14.0, 26.0);
    let mass = SolarMasses::new(7e3);
    let age = Years::new(9e9);
    let tidal = tidal_radius(tables, mass, &orbit);
    let window = stripping_time(&orbit, tidal, [last_major_merger(), age]);
    let period = orbit.radial_period().value();
    let (position, velocity) = place(tables, &orbit, [141.0, 115.0, 0.0], 0.02 * period);
    StreamSpec {
        number: StreamNumber(1),
        origin: StreamOrigin::Orphan,
        position,
        velocity,
        orbit,
        stripping_time: window,
        mass,
        progenitor_mass: SolarMasses::new(0.0),
        tidal_radius: tidal,
        mass_loss: MassLoss::Dissolved {
            ago: Years::new(0.5 * window.value()),
        },
        stars: Some(ClusterStars {
            fe_h: Dex::new(-1.9),
            age,
            kept: 0.0,
            depleted_slope: CANONICAL_SLOPE,
        }),
    }
}

/// A dwarf's stream like Sagittarius's, stream number 2, of recent progenitor 0, in `tables`'
/// potential.
///
/// Sagittarius's orbit runs between 19 and 59 kpc on average, its original mass was 6.4 × 10⁸ M☉,
/// it is slightly past a pericentre, and its orbit's pole lies at (l, b) = (273.8°, −14.5°), a
/// plane inclined 76° to the disc (Law and Majewski 2010, ApJ 714, 229, §§3.2, 3.4, 4.2, 5.1).
/// Their mass is the total mass of a model whose mass follows its light; here it is the stellar
/// mass at accretion, plan 02's, which draws no dark mass (P10.T3.c's finding). The dwarf was
/// accreted 6 Gyr ago, plan 02's oldest recent progenitor, and loses
/// 0.3 of its bound mass at each pericentre, the middle of Design note 11's 0.1–0.5. The tube is
/// what it lost at the pericentres within the stripping time, and the core what is left.
///
/// # Panics
///
/// If Sagittarius's turning points bound no orbit in `tables`' mid-plane potential, which a Milky
/// Way-like galaxy's always do.
#[must_use]
pub fn sagittarius_like(tables: &PotentialTables) -> StreamSpec {
    let orbit = orbit_between(tables, 19.0, 59.0);
    let accreted_mass = SolarMasses::new(6.4e8);
    let accreted = Years::new(6e9);
    let fraction = 0.3;
    let tidal = tidal_radius(tables, accreted_mass, &orbit);
    let window = stripping_time(&orbit, tidal, [last_major_merger(), accreted]);
    let period = orbit.radial_period().value();
    let last = 0.04 * period;
    let since_accretion = pericentres_within(accreted.value(), last, period);
    let in_window = pericentres_within(window.value(), last, period).min(since_accretion);
    let bound_after = |n: u32| accreted_mass.value() * remaining(fraction, n);
    let core = bound_after(since_accretion);
    let (position, velocity) = place(tables, &orbit, [76.0, 286.0, 60.0], last);
    StreamSpec {
        number: StreamNumber(2),
        origin: StreamOrigin::Dwarf(0),
        position,
        velocity,
        orbit,
        stripping_time: window,
        mass: SolarMasses::new(bound_after(since_accretion - in_window) - core),
        progenitor_mass: SolarMasses::new(core),
        tidal_radius: tidal,
        mass_loss: MassLoss::Pulsed {
            fraction,
            last_pericentre: Years::new(last),
            pericentres: in_window,
        },
        stars: None,
    }
}

/// The orbit between turning points given in kpc, in `tables`' mid-plane potential.
#[must_use]
fn orbit_between(tables: &PotentialTables, pericentre: f64, apocentre: f64) -> SphericalOrbit {
    let ly = |kpc: f64| LightYears::from(Kiloparsecs::new(kpc));
    SphericalOrbit::between(tables, ly(pericentre), ly(apocentre))
        .expect("the named stream's turning points bound an orbit in a Milky Way-like potential")
}

/// The tidal radius of `mass` at `orbit`'s pericentre, as P10.T3 takes it.
#[must_use]
fn tidal_radius(tables: &PotentialTables, mass: SolarMasses, orbit: &SphericalOrbit) -> LightYears {
    let at = PointLy::new(orbit.pericentre().value(), 0.0, 0.0);
    LightYears::from(tables.tidal_radius(mass, &at))
}

/// The Milky Way fixture's last major merger, before the epoch.
#[must_use]
fn last_major_merger() -> Years {
    GalaxyParams::milky_way_like()
        .accretion()
        .last_major_merger()
}

/// The epoch state `since` years after a pericentre of `orbit`, in the plane of inclination and
/// ascending node `[i, Ω, ..]` with the pericentre `[.., .., ω]` from the node, all in degrees:
/// P10.T3.c's construction of a dwarf's.
#[must_use]
fn place(
    tables: &PotentialTables,
    orbit: &SphericalOrbit,
    [inclination, node, omega]: [f64; 3],
    since: f64,
) -> (GalacticDisplacement, GalacticVelocity) {
    let elements = Orbit::new(
        orbit.apocentre(),
        orbit.pericentre(),
        Radians::new(inclination.to_radians()),
        Radians::new(node.to_radians()),
        Radians::new(0.0),
    );
    let phi = |r: f64| tables.potential_in_plane(LightYears::new(r));
    epoch_state(orbit, &phi, &elements, omega.to_radians(), since)
}
