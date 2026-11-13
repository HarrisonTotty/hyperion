//! The warm envelopes' bright haze on its fit's fixture (P14.T24.h.b, from generator version 32;
//! research-leans-50's ruling, 2026-11-10): GJ 1214 b as the snapshot path's closed form reads it,
//! for `hyperion-fit`'s `warm_haze` task, which fits the τ(550) of the bright haze's coagulated
//! mode ([`COAGULATED_HAZE_OPTICAL_DEPTH`](crate::tables::warm_haze::COAGULATED_HAZE_OPTICAL_DEPTH);
//! research-leans-52's ruling, 2026-11-13) on the planet's measured Bond albedo, and for the sim's
//! tests. Not a part of the generator's API.
//!
//! The fixture is read under 32's rules whatever the version's (`envelope_bond`'s "The column read
//! whole"), its bright haze's slab at
//! [`BRIGHT_HAZE_OPTICAL_DEPTH`](crate::planetary::chem::photochem::BRIGHT_HAZE_OPTICAL_DEPTH) and
//! its coagulated mode at the optical depth it is given, its `T_eq` iterated with its own Bond
//! albedo (`FIXED_POINT_STEPS` steps from 0.3), about a host whose light is a black body's of its
//! effective temperature (`area_mix::HostShade::of_host`).
//!
//! # Determinism
//!
//! A fixed number of steps, every transcendental [`math`]'s: the same bits on every target.

use crate::math;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::RunawayDeck;
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::derive::atmosphere::{
    AtmosphereInputs, Crust, EnvelopeHistory, Insolation, SurfaceMaterial, VolatileInventory,
};
use crate::planetary::derive::composition::{MassFractions, SnowLineSide, composition};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::derive::photometry::area_mix::OPTICS_BANDS;
use crate::planetary::derive::photometry::area_mix::{ColourBand, HostShade};
use crate::planetary::derive::photometry::envelope_bond::EnvelopeColumnRules;
use crate::planetary::derive::photometry::envelope_bond::column::{
    ClosedReading, snapshot_light_under,
};
use crate::planetary::derive::photometry::{
    Bands, BodyPhotometry, BrightShare, DiscColour, PhotometryInputs,
};
use crate::units::MetresPerSecondSquared;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::consts::{METRES_PER_AU, SOLAR_RADIUS_M, STEFAN_BOLTZMANN};
use crate::units::{
    EarthFluxes, EarthMasses, EarthRadii, JoulesPerSquareMetre, Kelvin, Kilograms, Metres,
    MetresPerSecond, WattsPerSquareMetre, Years,
};

/// The steps of the fixture's `T_eq` against its own Bond albedo: 24, from 0.3.
pub(crate) const FIXED_POINT_STEPS: u32 = 24;

/// A warm envelope and its host, as the fit's fixture states them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarmFixture {
    /// The planet's mass, M⊕.
    pub mass_earth: f64,
    /// Its radius, R⊕.
    pub radius_earth: f64,
    /// Its orbit's semi-major axis, au, circular.
    pub semi_major_axis_au: f64,
    /// Its age, years.
    pub age_years: f64,
    /// The host's effective temperature, K.
    pub host_temperature_k: f64,
    /// The host's radius, R☉.
    pub host_radius_solar: f64,
}

/// GJ 1214 b and its host (Cloutier et al. 2021, AJ 162, 174, recalled): 8.17 M⊕, 2.742 R⊕ at
/// 0.01490 au, about an M4.5 dwarf of 0.178 M☉, 0.215 R☉ and 3,250 K, whose zero-albedo `T_eq`
/// is 595 K; 5 Gyr old (the builder's, inside the system's several Gyr, labelled).
pub const GJ_1214_B: WarmFixture = WarmFixture {
    mass_earth: 8.17,
    radius_earth: 2.742,
    semi_major_axis_au: 0.0149,
    age_years: 5.0e9,
    host_temperature_k: 3_250.0,
    host_radius_solar: 0.215,
};

/// A warm envelope's light as the closed form gives it: its zero-albedo and own `T_eq`, its Bond
/// albedo, and its column's spherical albedo in the B, V and R bins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarmLight {
    /// Its equilibrium temperature at zero albedo, K.
    pub zero_albedo_temperature_k: f64,
    /// Its equilibrium temperature at its own Bond albedo, K.
    pub equilibrium_temperature_k: f64,
    /// Its Bond albedo under its host's light.
    pub bond_albedo: f64,
    /// Its column's spherical albedo in the B, V and R bins.
    pub spherical_albedo: [f64; 3],
    /// Its bright haze's share of its hazes' scattering in those bins.
    pub bright_share: [f64; 3],
    /// Its geometric albedo p in B, V and R, as its record's photometry draws it.
    pub geometric_albedo: [f64; 3],
    /// The phase integral q in B, V and R that p was drawn with.
    pub phase_integral: [f64; 3],
}

impl WarmFixture {
    /// The flux it receives, its host's black body of its radius and temperature at its distance.
    #[must_use]
    pub(crate) fn flux(&self) -> EarthFluxes {
        let radius = self.host_radius_solar * SOLAR_RADIUS_M;
        let distance = self.semi_major_axis_au * METRES_PER_AU;
        let ratio = radius / distance;
        EarthFluxes::from(WattsPerSquareMetre::new(
            STEFAN_BOLTZMANN * math::powi(self.host_temperature_k, 4) * ratio * ratio,
        ))
    }

    /// Its mass fractions, the generator's composition for its mass and radius formed inside the
    /// snow line; `None` where the composition refuses them.
    #[must_use]
    pub(crate) fn fractions(&self) -> Option<MassFractions> {
        let (mass, radius) = (
            EarthMasses::new(self.mass_earth),
            EarthRadii::new(self.radius_earth),
        );
        composition(mass, radius, SnowLineSide::Inside, self.flux())
            .ok()
            .map(|solved| solved.fractions())
    }

    /// The pass's inputs at equilibrium temperature `equilibrium`: what the closed form reads (its
    /// mass, radius, fractions, class, `T_eq`, flux, age and formation inside the snow line), the
    /// rest at neutral values, which no gas envelope's albedo reads. `None` where the generator's
    /// composition refuses its mass and radius.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn inputs(&self, equilibrium: Kelvin) -> Option<AtmosphereInputs> {
        Some(self.inputs_of(equilibrium, self.fractions()?))
    }

    /// [`inputs`](Self::inputs) of the fractions `fractions`.
    #[must_use]
    fn inputs_of(&self, equilibrium: Kelvin, fractions: MassFractions) -> AtmosphereInputs {
        let (mass, radius) = (
            EarthMasses::new(self.mass_earth),
            EarthRadii::new(self.radius_earth),
        );
        let flux = self.flux();
        AtmosphereInputs {
            mass: Kilograms::from(mass),
            radius: Metres::from(radius),
            material: SurfaceMaterial::of(&fractions),
            envelope_fraction: fractions.envelope(),
            class: PlanetClass::of(mass, &fractions),
            inventory: VolatileInventory::default(),
            formed: SnowLineSide::Inside,
            equilibrium,
            irradiation_temperature: equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0)),
            heated: equilibrium,
            xuv_fluence: JoulesPerSquareMetre::new(0.0),
            saturated_xuv_flux: WattsPerSquareMetre::new(0.0),
            insolation: Insolation::BeyondMaximumGreenhouse,
            present_insolation: Insolation::BeyondMaximumGreenhouse,
            crust: Crust::Solid,
            molten_for: Years::ZERO,
            age: Years::new(self.age_years),
            hottest_host: Kelvin::new(self.host_temperature_k),
            impact_speed: MetresPerSecond::new(0.0),
            xuv_saturation_time: Years::ZERO,
            envelope_history: EnvelopeHistory::NONE,
            flux,
            maximum_greenhouse_flux_ratio: 0.0,
            envelope_helium: None,
        }
    }

    /// Its light under 32's rules with its bright haze's coagulated mode of τ(550)
    /// `coagulated_haze_optical_depth` at its whole share, the slab at the record's, its `T_eq` its
    /// own Bond albedo's after `FIXED_POINT_STEPS` steps; `None` for an optical depth that is not
    /// finite and positive, and where the closed form gives it no column.
    #[must_use]
    pub fn light(&self, coagulated_haze_optical_depth: f64) -> Option<WarmLight> {
        if !(coagulated_haze_optical_depth.is_finite() && coagulated_haze_optical_depth > 0.0) {
            return None;
        }
        self.light_under(
            ClosedReading {
                rules: EnvelopeColumnRules::Whole,
                bright_haze_optical_depth:
                    crate::planetary::chem::photochem::BRIGHT_HAZE_OPTICAL_DEPTH,
                coagulated_haze_optical_depth,
            },
            BondAlbedo::from_fraction(0.3),
        )
    }

    /// [`light`](Self::light) read as `reading` says, from the Bond albedo `start`.
    #[must_use]
    pub(crate) fn light_under(
        &self,
        reading: ClosedReading,
        start: BondAlbedo,
    ) -> Option<WarmLight> {
        let flux = self.flux();
        let shade = HostShade::of_host(Kelvin::new(self.host_temperature_k));
        let fractions = self.fractions()?;
        let zero_albedo = equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0)).value();
        let mut albedo = start;
        let mut light = None;
        for _ in 0..FIXED_POINT_STEPS {
            let equilibrium = equilibrium_temperature(flux, albedo);
            let inputs = self.inputs_of(equilibrium, fractions);
            let closed = snapshot_light_under(reading, &inputs, &shade)?;
            let bands = |bins: &[f64; OPTICS_BANDS]| Bands {
                b: bins[ColourBand::B.bin()],
                v: bins[ColourBand::V.bin()],
                r: bins[ColourBand::R.bin()],
            };
            let (spherical, bright) = (bands(&closed.bins), bands(&closed.bright_share));
            let bond = BondAlbedo::from_fraction(closed.bond);
            // The photometry as a record of the fixture draws it, from its column's own colour.
            let photometry = BodyPhotometry::derive(&PhotometryInputs {
                state: SurfaceState::GasEnvelope,
                runaway_deck: RunawayDeck::Kept,
                material: inputs.material,
                surface_pressure: None,
                cloud_fraction: 1.0,
                surface_temperature: equilibrium,
                equilibrium_temperature: equilibrium,
                surface_gravity: MetresPerSecondSquared::new(
                    GRAVITATIONAL_CONSTANT * inputs.mass.value()
                        / (inputs.radius.value() * inputs.radius.value()),
                ),
                class: inputs.class,
                fractions,
                bond,
                band_albedo: DiscColour::new(spherical),
                bright_share: BrightShare::new(bright),
            });
            light = Some(WarmLight {
                zero_albedo_temperature_k: zero_albedo,
                equilibrium_temperature_k: equilibrium.value(),
                bond_albedo: closed.bond,
                spherical_albedo: [spherical.b, spherical.v, spherical.r],
                bright_share: [bright.b, bright.v, bright.r],
                geometric_albedo: {
                    let p = photometry.geometric_albedo();
                    [p.b, p.v, p.r]
                },
                phase_integral: {
                    let q = photometry.phase_integral();
                    [q.b, q.v, q.r]
                },
            });
            albedo = bond;
        }
        light
    }
}

#[cfg(test)]
mod tests;
