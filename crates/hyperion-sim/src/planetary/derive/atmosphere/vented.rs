//! A secondary air's carriers by its vent (plan 14, P14.T55.a.c; research-leans-58), from generator
//! version 36 (`OUTGASSING_SPECIATION_VERSION`) under its hold ([`OutgassingRules`]): the air
//! publishes its vent's gas, quenched as it erupts, in place of P14.T24.f's insolation rule.
//!
//! # The route (ruling 1)
//!
//! Gas-phase CH₄ ⇌ CO conversion freezes below about 900–1,050 K at 1 bar over 10⁴–10⁶ yr (Zahnle
//! and Marley 2014, ApJ 797, 41, eqs. 12–14, as `chem::quench` holds them; fitted on near-solar
//! H₂ gas, extrapolated to a secondary air), so a cool air keeps the carbon shares its vent
//! quenched, and is not re-equilibrated at its surface: Woitke et al.'s types (2021, A&A 646, A43)
//! are equilibrium's answer, which the census names from each air's elements (`chem::secondary`),
//! not an air's. Every bulk air above the quench is a hot air, which keeps P14.T24.f's carriers
//! whole (ruling 4): no vent, no shift and no vent hydrogen. A body takes the route where it is
//! none of:
//!
//! - **on P14.T24.f's methane arm** (formed beyond the snow line, beyond the maximum-greenhouse
//!   limit, with a solid crust): Titan's, Triton's and Pluto's carbon is their ices', not a melt's;
//! - **a hot air, by its recorded state** ([`VentRoute::Hot`]): its climate's division a runaway or
//!   a magma ocean (its water lost, its melt the air's), or its class one, a hothouse's re-solve
//!   (P14.T53.b part 1 (c)). The state a pass carries to the next by P14.T53.e's rules 5–7 is not
//!   read: a route that flipped with it would couple the passes, a runaway's T24.f air carrying
//!   back a temperate class and its vent's air a runaway again (the census found such 2-cycles
//!   publishing a runaway its own figures do not make). A rule-6 runaway lies below the CH₄–CO
//!   quench, so its vent's quenched gas stands, as does a surface its own greenhouse holds
//!   molten, whose state is its pass's output (leans, 2026-11-25; the census lists them);
//! - **an ice bulk** ([`SurfaceMaterial::Ice`](super::SurfaceMaterial::Ice)), whose silicate vent
//!   lies on its seafloor, its gas reaching the air only through its ocean, which is not modelled
//!   (P14.T55.a.b; a lean, 2026-11-25).
//!
//! # The steps, in order ([`outgassed`])
//!
//! 1. **The vent's carbon shares** (part b, [`VentGas::speciate`]): the vent's gas of the body's
//!    drawn hydrogen, carbon and nitrogen ([`vent_elements`]) at its mantle's ΔIW, its vent's
//!    temperature ([`VentConditions`]) and its pressure. The pressure is the air's own, which no
//!    derivation knows before its atmosphere: the weight, M g ÷ 4πR², of what retention and the
//!    cosmic shoreline (escape's step 3) keep of the gases that do not condense, its nitrogen and
//!    argon and its carbon at the least share its climate's division leaves airborne,
//!    [`TEMPERATE_AIRBORNE_CARBON`](super::TEMPERATE_AIRBORNE_CARBON) on a temperate division that
//!    holds water (the thermostat's stored state, a lower bound where it keeps more) and all of it
//!    elsewhere, the carbon as the route publishes it, its CO₂ and CO as carbon dioxide and its
//!    CH₄ as methane, after graphite's cap: so the pressure is solved with the cap, as a fixed
//!    point of at most [`VENT_PRESSURE_STEPS`] trials from the drawn carbon's weight (a lean,
//!    2026-11-25). Earth's comes out at 0.77 bar, its nitrogen and argon. A vent the gas refuses (an
//!    oxidised mantle under so thin an air that its f(O₂) reaches the pressure) keeps P14.T24.f's
//!    carriers ([`VentRoute::Refused`]).
//! 2. **Graphite's cap** (ruling 5): the air's CO, as the column weight of its airborne carbon (the
//!    division's least airborne share of it, less the shoreline's), is at most `K_f(CO)` f(O₂)^½
//!    ([`VentGas::carbon_monoxide_at_graphite`]), its CO₂ and CH₄ following by the vent's ratios;
//!    the carbon above the cap stays in the crust as graphite, for P14.T55.d. A closed-form min, so
//!    it is continuous. On a temperate world whose thermostat stores its carbon the vent's gas
//!    under its own thin air cannot reach the cap, which binds where the carbon stays airborne
//!    beside the vent (a lean, 2026-11-25, of the ruling's whole-inventory comparison).
//! 3. **The vent's methane takes its hydrogen from the water**, two H₂O a CH₄, the oxygen to the
//!    melt; the water inventory otherwise stays water (the hydrogen guard, a lean of 2026-11-25: the
//!    vent's H₂O/H₂ enters methane's ratio alone). Where the water is short, the rest is CO.
//! 4. **The shift where the air holds water** (ruling 2): CO + H₂O → CO₂ + H₂, the oxygen from the
//!    airborne water, by water's OH (McElroy and Donahue 1972, Science 177, 986; Parkinson and
//!    Hunten 1972, J. Atmos. Sci. 29, 1380, recalled), downhill below about 1,100 K. "Holds water"
//!    is a recorded state: retention or the ice keeps some (a lean, 2026-11-25: the water its step
//!    2 keeps, whose vapour any published air holds). A dry air keeps its CO, published as
//!    P14.T24.f's CO₂ stand-in until the CO row (ruling 4), and counted; where the CO outnumbers
//!    the water, the shift takes all of it, and the air is dried.
//! 5. **The shift's hydrogen to escape**: none where Jeans's test does not retain hydrogen, and
//!    otherwise what the diffusion limit through nitrogen leaves over the age, on
//!    `methane_outcome`'s pattern ([`diffusion_limited_flux_per_m2_s`], Hunten 1973, J. Atmos. Sci.
//!    30, 1481, recalled; b [`HYDROGEN_NITROGEN_DIFFUSION`] at the irradiation temperature, the
//!    homopause's stand-in). Its background is nitrogen whatever the air (in carbon dioxide the
//!    flux is some 1.4 times more), its mixing ratio the shift's at the start and its loss linear
//!    at it, not decaying: none changes its outcome while the e-folding time, tens of kyr on an
//!    Earth, lies far under the age. Escape's own steps (the shoreline, the thin envelope's) then
//!    read it.
//!
//! The air's H₂O, CH₄, N₂ and CO₂ come from it, with the H₂ escape keeps; no NH₃ or O₂ from the
//! vent (P14.T24.f's runaway oxygen is a hot air's, and stays). Its surface's water is the water
//! the route leaves ([`Atmosphere::surface_water_inventory`](super::Atmosphere::surface_water_inventory)).
//!
//! # The balance
//!
//! The drawn carbon dioxide of the carbon published as CO₂ is published whole, by mass; the
//! vent's CH₄ takes its drawn carbon dioxide's carbon and two waters' hydrogen, their four oxygen
//! atoms the melt's; the shifted CO's water gives its hydrogen to H₂ and its oxygen, by mass, to the
//! melt (the melt kept the CO's own); graphite's drawn carbon dioxide is the crust's and the melt's.
//! So each element's outcome stays mass-conserving (escape's `Ledger`).
//!
//! # Not modelled
//!
//! - The vent's methane is the vent's supply, not a reservoir: methane's loss over the age
//!   (`methane_outcome`, P14.T53.a), a reservoir's, stays P14.T24.f's arm's (a lean, 2026-11-25:
//!   at Titan's rate scaled to 1 S⊕ it would photolyse any vent's methane many times over, where a
//!   steady supply keeps some).
//! - Methane's greenhouse: the vent's CH₄ is in no τ term, as P14.T24.f's arm's (ruling 3's
//!   stand-in, carbon dioxide's term reading it, is its own step); and the weathering thermostat
//!   stores it with the carbon
//!   dioxide, its carbon held on the carbon as CH₄ oxidised by OH would be before weathering (a
//!   lean, 2026-11-25: silicate weathering itself stores no CH₄).
//! - H₂'s steady state from the present outgassing at the vent's ratio (research-leans-58,
//!   findings).

use core::f64::consts::PI;

use crate::planetary::chem::escape::{
    HYDROGEN_NITROGEN_DIFFUSION, diffusion_limited_flux_per_m2_s,
};
use crate::planetary::chem::outgassing::{
    OutgassingRules, SpeciateVentGasError, VentElements, VentGas, VentInputs, VentSpecies,
};
use crate::substance::Element;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Kilograms, Pascals};

use super::{Airborne, AtmosphereInputs, Gas, Retention, VentConditions, VolatileInventory};

/// The most trials of the vent's pressure ([the module](self)'s step 1): 16, a fixed bound (the
/// lane's). Each reads the carriers the trial before published; the trials stop where a pressure
/// repeats within 10⁻¹² of itself, after two or three on most bodies, and a capped, methane-making
/// vent's settle within ten.
pub(crate) const VENT_PRESSURE_STEPS: u32 = 16;

/// Why a body's carriers are, or are not, its vent's (P14.T55.a.c, [the module](self)), from
/// generator version 36: [`Atmosphere::vent`](super::Atmosphere::vent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VentRoute {
    /// On P14.T24.f's methane arm: its carbon is its ices', as methane.
    MethaneArm,
    /// A hot air (ruling 4): a runaway or a magma ocean by its division or its class, which keeps
    /// P14.T24.f's carriers whole.
    Hot,
    /// An ice bulk, whose vent lies on its seafloor.
    IceShell,
    /// No carbon kept: retention left the air none, the body drew none, or its vent's gas holds
    /// none.
    NoCarbon,
    /// The vent's gas refused its inputs: P14.T24.f's carriers.
    Refused(SpeciateVentGasError),
    /// The vent's carriers.
    Vented(Outgassed),
}

/// The vent's carbon by moles of carbon, each 0 to 1, their sum 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VentShares {
    pub(crate) carbon_dioxide: f64,
    pub(crate) carbon_monoxide: f64,
    pub(crate) methane: f64,
}

impl VentShares {
    /// The carbon shares of the vent's gas `vent`; none where it holds no carbon, or its shares are
    /// not numbers.
    #[must_use]
    fn of(vent: &VentGas) -> Option<Self> {
        let [carbon_dioxide, carbon_monoxide, methane] = [
            VentSpecies::CarbonDioxide,
            VentSpecies::CarbonMonoxide,
            VentSpecies::Methane,
        ]
        .map(|species| vent.fraction(species));
        let carbon = carbon_dioxide + carbon_monoxide + methane;
        // By comparison, so that a sum that is not a number gives none.
        (carbon > 0.0 && carbon.is_finite()).then(|| Self {
            carbon_dioxide: carbon_dioxide / carbon,
            carbon_monoxide: carbon_monoxide / carbon,
            methane: methane / carbon,
        })
    }
}

/// What the vent's route made of a body's kept carbon and water ([`outgassed`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Outgassed {
    /// The vent's pressure: the published air's non-condensing weight ([the module](self)'s
    /// step 1).
    pub(crate) pressure: Pascals,
    /// The vent's carbon shares, before the cap.
    pub(crate) shares: VentShares,
    /// Whether the air holds water, which the shift needs.
    pub(crate) holds_water: bool,
    /// The carbon graphite's cap left in the crust, as carbon.
    pub(crate) graphite: Kilograms,
    /// The vent's methane in the air, and its share of the carbon the cap leaves in the gas, by
    /// moles.
    pub(crate) methane: Kilograms,
    pub(crate) methane_share: f64,
    /// The carbon dioxide the shift made of the vent's CO.
    pub(crate) shifted: Kilograms,
    /// The CO a dry air publishes as P14.T24.f's carbon dioxide stand-in, as that carbon dioxide.
    pub(crate) dry_monoxide: Kilograms,
    /// The water the vent's methane and the shift took, and whether they took it all: an air they
    /// dried, its CO beyond its water left on the stand-in.
    pub(crate) water_debit: Kilograms,
    pub(crate) dried: bool,
    /// The hydrogen the shift made, and what escape's diffusion limit left of it.
    pub(crate) hydrogen: Kilograms,
    pub(crate) hydrogen_kept: Kilograms,
}

impl Outgassed {
    /// The oxygen of the drawn carbon dioxide and water the route gave the melt: two atoms a vent
    /// CH₄'s carbon dioxide, and the water debit's.
    #[must_use]
    pub(crate) fn melt_oxygen(&self) -> Kilograms {
        let oxygen = Element::O.standard_atomic_weight_g_per_mol();
        let methane_moles = self.methane.value() / Gas::Methane.molar_mass_g_per_mol();
        let water_moles = self.water_debit.value() / Gas::Water.molar_mass_g_per_mol();
        Kilograms::new((2.0 * methane_moles + water_moles) * oxygen)
    }

    /// The drawn carbon dioxide of the carbon graphite's cap left in the crust.
    #[must_use]
    pub(crate) fn graphite_as_carbon_dioxide(&self) -> Kilograms {
        Kilograms::new(
            self.graphite.value() * Gas::CarbonDioxide.molar_mass_g_per_mol()
                / Element::C.standard_atomic_weight_g_per_mol(),
        )
    }
}

/// Whether the rules in force publish the vent's carriers: from generator version 36
/// (`OUTGASSING_SPECIATION_VERSION`).
#[must_use]
pub(crate) fn speciated() -> bool {
    OutgassingRules::in_force() == OutgassingRules::Speciated
}

/// The hydrogen, carbon and nitrogen the body of drawn volatiles `inventory` outgasses, mol of
/// atoms: its water's, its carbon dioxide's and its nitrogen's, at the registry's molar masses,
/// whose ratios are the vent's (P14.T55.a.b's `VentElements::of_exchangeable`, on the carriers as
/// drawn); no sulphur (P14.T50.a holds the exchangeable sulphur at zero).
#[must_use]
pub(crate) fn vent_elements(inventory: &VolatileInventory) -> VentElements {
    let mol = |mass: Kilograms, gas: Gas| mass.value() * 1e3 / gas.molar_mass_g_per_mol();
    VentElements::new(
        2.0 * mol(inventory.water(), Gas::Water),
        mol(inventory.carbon_dioxide(), Gas::CarbonDioxide),
        2.0 * mol(inventory.nitrogen(), Gas::Nitrogen),
        0.0,
    )
}

/// CO's molar mass, g mol⁻¹, by the element registry's standard atomic weights: CO has no gas row
/// until its follow-up (ruling 4).
#[must_use]
fn carbon_monoxide_molar_mass() -> f64 {
    Element::C.standard_atomic_weight_g_per_mol() + Element::O.standard_atomic_weight_g_per_mol()
}

/// The smaller of `a` and `b`, by comparison, `b` where `a` is not a number (the sim's hazard of
/// `f64::min` at a signed zero, whose sign either input may give).
#[must_use]
fn least(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// What escape's diffusion limit leaves of the hydrogen `made` a body of `inputs` holds at the
/// mixing ratio `mixing_ratio`, 0 to 1, where Jeans's test retains it (`retention`), over its age
/// ([the module](self)'s step 5): none where it is not retained, and otherwise the store less the
/// limiting flux through nitrogen at the irradiation temperature over the whole surface and age, as
/// `methane_outcome` takes methane's escape.
#[must_use]
fn hydrogen_kept(
    made: Kilograms,
    mixing_ratio: f64,
    inputs: &AtmosphereInputs,
    retention: Retention,
) -> Kilograms {
    let made = made.value();
    if !(made > 0.0 && retention.retains(Gas::Hydrogen)) {
        return Kilograms::ZERO;
    }
    let (r, t) = (inputs.radius.value(), inputs.irradiation_temperature);
    let hydrogen = Gas::Hydrogen.molecular_mass();
    let flux = diffusion_limited_flux_per_m2_s(
        mixing_ratio,
        hydrogen,
        Gas::Nitrogen.molecular_mass(),
        HYDROGEN_NITROGEN_DIFFUSION.per_metre_second_at(t),
        inputs.mass,
        inputs.radius,
        t,
    );
    let years = inputs.age.value();
    let age = if years > 0.0 { years } else { 0.0 } * SECONDS_PER_JULIAN_YEAR;
    let demand = flux * 4.0 * PI * r * r * hydrogen.value() * age;
    // By comparison, so that a demand that is not a number keeps none.
    Kilograms::new(if demand < made { made - demand } else { 0.0 })
}

/// One trial of the vent at a pressure ([`outgassed`]'s steps 1–3): its gas's carbon shares,
/// graphite's cap's share of the kept carbon left in the gas, the gas's carbon and methane, mol,
/// and the pressure the air they publish weighs.
#[derive(Debug, Clone, Copy)]
struct Trial {
    pressure: Pascals,
    shares: VentShares,
    kept_share: f64,
    gas: f64,
    methane: f64,
    next: Pascals,
}

/// What the vent's trials read of a body and its kept air ([`outgassed`]'s steps 1–3).
struct Vent<'a> {
    inputs: &'a AtmosphereInputs,
    elements: VentElements,
    /// The kept carbon and water, mol, and the kept nitrogen and argon, kg.
    moles: f64,
    water_moles: f64,
    background: f64,
    /// The division's least airborne share of the carbon, and the airborne air's weight per kg
    /// (the shoreline's share of it), Pa kg⁻¹.
    airborne_share: f64,
    airborne: f64,
}

impl Vent<'_> {
    /// Steps 1–3 at the pressure `pressure`: the gas, its cap and its methane, and the air they
    /// weigh; none where its gas holds no carbon.
    fn trial(&self, pressure: Pascals) -> Result<Option<Trial>, SpeciateVentGasError> {
        let VentConditions {
            delta_iw,
            temperature,
        } = self.inputs.vent;
        let vent = VentGas::speciate(
            &VentInputs {
                delta_iw,
                temperature,
                pressure,
            },
            &self.elements,
        )?;
        let Some(shares) = VentShares::of(&vent) else {
            return Ok(None);
        };
        let monoxide = self.moles
            * shares.carbon_monoxide
            * carbon_monoxide_molar_mass()
            * 1e-3
            * self.airborne_share
            * self.airborne;
        let cap = vent.carbon_monoxide_at_graphite().value();
        let kept_share = if monoxide > cap { cap / monoxide } else { 1.0 };
        let gas = self.moles * kept_share;
        let methane = least(gas * shares.methane, 0.5 * self.water_moles);
        let carbon = ((gas - methane) * Gas::CarbonDioxide.molar_mass_g_per_mol()
            + methane * Gas::Methane.molar_mass_g_per_mol())
            * 1e-3;
        Ok(Some(Trial {
            pressure,
            shares,
            kept_share,
            gas,
            methane,
            next: Pascals::new((self.background + carbon * self.airborne_share) * self.airborne),
        }))
    }

    /// The trials from the pressure `start` until one repeats within 10⁻¹² of itself, at most
    /// [`VENT_PRESSURE_STEPS`] of them: the last, none where the gas holds no carbon.
    fn solve(&self, start: Pascals) -> Result<Option<Trial>, SpeciateVentGasError> {
        let Some(mut at) = self.trial(start)? else {
            return Ok(None);
        };
        for _ in 1..VENT_PRESSURE_STEPS {
            let (p, next) = (at.pressure.value(), at.next.value());
            if (next - p).abs() <= 1e-12 * p {
                break;
            }
            let Some(next) = self.trial(at.next)? else {
                return Ok(None);
            };
            at = next;
        }
        Ok(Some(at))
    }
}

/// The vent's route on the air `air` the body of `inputs` keeps by `retention` (P14.T24.f's, its
/// carrier carbon dioxide), its column weighing `weight_pa_per_kg`, its climate's division leaving
/// `airborne_share` of its carbon airborne at the least and the cosmic shoreline `shoreline_share`
/// of its air ([the module](self)'s steps 1–5): where it vents, the air's carbon dioxide, methane
/// and water are set and what the route made returned ([`VentRoute::Vented`]); a vent whose gas
/// refuses its inputs gives [`VentRoute::Refused`], and one whose gas holds no carbon
/// [`VentRoute::NoCarbon`], `air` unchanged.
pub(crate) fn outgassed(
    air: &mut Airborne,
    inputs: &AtmosphereInputs,
    retention: Retention,
    weight_pa_per_kg: f64,
    (airborne_share, shoreline_share): (f64, f64),
) -> VentRoute {
    let carbon_dioxide = Gas::CarbonDioxide.molar_mass_g_per_mol();
    let methane_mass = Gas::Methane.molar_mass_g_per_mol();
    let water_mass = Gas::Water.molar_mass_g_per_mol();
    let vent = Vent {
        inputs,
        elements: vent_elements(&inputs.inventory),
        moles: air.carbon_dioxide * 1e3 / carbon_dioxide,
        water_moles: air.water * 1e3 / water_mass,
        background: air.nitrogen + air.argon,
        airborne_share,
        airborne: shoreline_share * weight_pa_per_kg,
    };
    let (moles, water_moles) = (vent.moles, vent.water_moles);
    let start =
        Pascals::new((vent.background + air.carbon_dioxide * airborne_share) * vent.airborne);
    let at = match vent.solve(start) {
        Ok(Some(at)) => at,
        Ok(None) => return VentRoute::NoCarbon,
        Err(error) => return VentRoute::Refused(error),
    };
    let Trial {
        pressure,
        shares,
        kept_share,
        gas,
        methane,
        ..
    } = at;
    let monoxide = gas * shares.carbon_monoxide + (gas * shares.methane - methane);
    // Step 4: the shift where the air holds water.
    let left = water_moles - 2.0 * methane;
    let wet = if left > 0.0 { left } else { 0.0 };
    let holds_water = wet > 0.0;
    let shifted = if holds_water {
        least(monoxide, wet)
    } else {
        0.0
    };
    let dry = monoxide - shifted;
    // Step 5: the shift's hydrogen, against the air's non-condensing moles.
    let made = Kilograms::new(shifted * Gas::Hydrogen.molar_mass_g_per_mol() * 1e-3);
    let others = (air.nitrogen * 1e3 / Gas::Nitrogen.molar_mass_g_per_mol())
        + (air.argon * 1e3 / Gas::Argon.molar_mass_g_per_mol())
        + (gas - methane) * airborne_share
        + methane;
    let mixing_ratio = if shifted + others > 0.0 {
        shifted / (shifted + others)
    } else {
        0.0
    };
    let hydrogen = hydrogen_kept(made, mixing_ratio, inputs, retention);

    // Unchanged to the bit where nothing moves: no cap, no methane, no shift.
    if kept_share < 1.0 || methane > 0.0 {
        let kept = air.carbon_dioxide * kept_share - methane * carbon_dioxide * 1e-3;
        air.carbon_dioxide = if kept > 0.0 { kept } else { 0.0 };
    }
    air.methane = methane * methane_mass * 1e-3;
    let water = air.water;
    if methane > 0.0 || shifted > 0.0 {
        // None where the methane and the shift took it all, whatever the rounding.
        let left = water - (2.0 * methane + shifted) * water_mass * 1e-3;
        air.water = if shifted >= wet || left <= 0.0 {
            0.0
        } else {
            left
        };
    }
    VentRoute::Vented(Outgassed {
        pressure,
        shares,
        holds_water,
        graphite: Kilograms::new(
            moles * (1.0 - kept_share) * Element::C.standard_atomic_weight_g_per_mol() * 1e-3,
        ),
        methane: Kilograms::new(air.methane),
        methane_share: if gas > 0.0 { methane / gas } else { 0.0 },
        shifted: Kilograms::new(shifted * carbon_dioxide * 1e-3),
        dry_monoxide: Kilograms::new(dry * carbon_dioxide * 1e-3),
        water_debit: Kilograms::new(water - air.water),
        dried: water > 0.0 && air.water <= 0.0,
        hydrogen: made,
        hydrogen_kept: hydrogen,
    })
}
