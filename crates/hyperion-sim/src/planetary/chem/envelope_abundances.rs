//! A gas envelope's element abundances (plan 14, P14.T50.b; decision-composition §1.2, as
//! decision-p14-t50b-envelope rules it, which upholds the model below): for a body
//! in [`SurfaceState::GasEnvelope`](crate::planetary::derive::SurfaceState::GasEnvelope), every
//! element of the registry ([`Element::ALL`]) by number relative to hydrogen, which P14.T52.a
//! speciates and P14.T52.d publishes.
//!
//! # The host
//!
//! The host's abundances are the element registry's protosolar ones (Lodders 2010, Table 6) times
//! 10^\[Fe/H\], with the α elements ([`ALPHA_ELEMENTS`]) times 10^\[α/Fe\] as well, and three
//! ratios its star sets ([`HostAbundances`], P14.T50.a): C/O, so carbon is the host's C/O times
//! its oxygen; Mg/Si, so magnesium is that times its silicon, as the crust (P14.T51.c) reads it;
//! and N/O, the protosolar ratio, so nitrogen is that times its oxygen. The envelope's helium
//! before any rain is an input ([`EnvelopeInputs::primordial_helium_per_hydrogen`]): the record
//! gives it (P14.T54.b.i), the host star's helium fractionated by P14.T53.a's escape, and the
//! tests pass the protosolar value.
//!
//! # The envelope's metals: E(M)
//!
//! Every metal is the host's times one enrichment E, the envelope's metal-to-hydrogen mass ratio
//! over the host's ([`EnvelopeAbundances::enrichment`]), with carbon, nitrogen and oxygen then
//! shared out by the ice-line factor at the same mass of the three:
//!
//! - **The trend**, E = [`ENRICHMENT_AT_ONE_JUPITER_MASS`] × (M ÷ `M_J`)^−[`ENRICHMENT_MASS_INDEX`]
//!   ([`mass_enrichment`]), is the Solar System's giants' measured carbon. Their CH₄ gives C/H
//!   "∼4, 10, 80, and 80" times solar in Jupiter, Saturn, Uranus and Neptune (Thorngren et al.
//!   2016, ApJ 831, 64, arXiv:1511.07854v2, §1, read). The four values fitted are those of Guillot
//!   et al.'s (2022, arXiv:2205.04100, Table 2, read) compilation, on this registry's protosolar
//!   C/H of 2.754 × 10⁻⁴: 4.32 (Galileo's mass spectrometer, Wong et al. 2004), 9.08 (Cassini's
//!   CIRS, Fletcher et al. 2009), 70.8 (Sromovsky et al. 2019) and 89.0 (Karkoschka and Tomasko
//!   2011). The four primary papers were not read. It is fitted from 14.5 to 318 M⊕ and
//!   extrapolated outside; it draws no scatter.
//! - **The bound** is the radius model's heavy-element mass. An envelope cannot be richer in
//!   metals than the whole planet: its metal mass fraction is the bulk's, `Z_p`, times a visible
//!   fraction f, with 0 ≤ f ≤ 1 (Thorngren and Fortney 2019, `ApJL` 874, L31, arXiv:1811.11859v2,
//!   their Eq. 1, read). `Z_p` is 1 less the body's envelope share, which for a giant is
//!   Thorngren et al.'s (2016, §5.1) 57.9 M⊕ × (M ÷ `M_J`)^0.61 of heavy elements
//!   ([`giant_heavy_elements`](crate::planetary::derive::composition::giant_heavy_elements)).
//!   - So E never exceeds f = 1. Thorngren and Fortney find Jupiter's and Saturn's atmospheres
//!     "about 20% of the limits we compute" (§3.1), against their own Jupiter's `Z_p` of 0.12.
//!   - On the radius model's `Z_p` of 0.182, a Jupiter at f = 0.2 would hold E ≈ 2.3 and at
//!     f = 1 E ≈ 13.6; the trend gives 2.65. Thorngren et al.'s `Z_planet` ÷ `Z_star` =
//!     9.7 (M ÷ `M_J`)^−0.45 (§5.3) is the bulk's enrichment, the limit and not the envelope's.
//! - **The floor**: E is never below 1, the host's own gas. The trend reaches it at 2.69 `M_J`.
//!
//! E is relative to the host, as Thorngren et al.'s `Z_planet` ÷ `Z_star` is, so an envelope's
//! metallicity follows its star's \[Fe/H\].
//!
//! # The ice-line factor
//!
//! The envelope's C/O is the host's times the C/O of the disc's gas where the body formed, over the
//! disc's own ([`ice_line_factor`]): Öberg, Murray-Clay and Bergin (2011, `ApJL` 743, L16,
//! arXiv:1110.5567, read), whose Table 1 shares the disc's oxygen and carbon among CO (20 K), CO₂
//! (47 K), water (here the generator's snow line, 170 K, for their adopted 135 K), refractory
//! carbon (Table 1's adopted 500 K, its range "> 150 K"; P14.T50.a's soot line) and silicates
//! (1,500 K) ([`DISC_CARRIERS`]). Each holds
//! its share as ice or dust where the disc is no warmer than its line, and the gas keeps the rest:
//! a gas C/O of 0.6 inside the snow line, 0.86 between the H₂O and CO₂ lines and 1.0 between the
//! CO₂ and CO lines, against the disc's 0.545 ("the maximum C/O∼1 reached between the CO₂ and CO
//! sublimation lines", their §2). Beyond the CO line the gas holds neither, and the envelope keeps
//! the host's ratio. About a host richer in carbon than 0.545, an envelope formed between the CO₂
//! and CO lines has a C/O above 1.
//!
//! This is Öberg et al.'s gas-dominated limit, "the case of completely isolated core and atmosphere
//! accretion" (their §2), applied at every E: they expect an atmosphere polluted by solids to have
//! "stellar or sub-stellar C/O" (their abstract and §4), as Espinoza et al. (2017,
//! arXiv:1611.08616) find on Thorngren et al.'s metallicities (as decision-p14-t50b-envelope §2
//! quotes them). It is calibrated on the four giants instead, which all hold more carbon than
//! sulphur (Guillot et al.'s Table 2); the solids' share as a drawn rank, which would give JWST's
//! sub-stellar hot Jupiters, is P14.T50.d's. The gas phase's ratio is used rather than their Eq. 2,
//! which mixes it with the local ices and dust towards an envelope's solid share. To reach
//! Jupiter's measured carbon (4.3 times solar at its 112 K, between the H₂O and CO₂ lines), Eq. 2
//! alone needs 14 times the disc's solids per unit of gas, which puts its oxygen at 7.9, its
//! troilite sulphur at 14 and its argon at 1 times solar with a C/O of 0.25. Held to this module's
//! shared-out carbon, nitrogen and oxygen, it needs E ≈ 5.4 for every other element, with a C/O of
//! 0.29. The gas phase's ratio keeps the pattern that was measured: with the factor of 11/7, the
//! trend gives a Jupiter about the Sun's own ratios C/H 3.7, O/H and N/H 2.3, and S/H and the heavy
//! noble gases 2.65 times this registry's protosolar values, and C/O 0.72. On the same values the
//! Galileo probe measured C/H 4.3, S/H 2.7 and Ar, Kr and Xe 2.1–2.6 (Wong et al. 2004 and Mahaffy
//! et al. 2000, as Atreya et al. 2016, arXiv:1606.04510, Table 2.1 list them, read), and Juno O/H
//! 2.4 (+2.1, −1.5) and N/H 2.5 (Li et al. 2020, Nature Astronomy 4, 609, arXiv:2012.10305, read,
//! whose own scale gives 2.7 (+2.4, −1.7) for oxygen; as Guillot et al.'s Table 2 lists them).
//!
//! # Helium rain
//!
//! He/H is the envelope's primordial value, depleted in a gas giant ([`PlanetClass::GasGiant`]) of
//! at least [`HELIUM_RAIN_LEAST_MASS`] whose interior has cooled enough for helium to separate from
//! metallic hydrogen and rain towards its core ([`helium_depletion`]). Guillot et al. (2022,
//! §2.1.2, read) put the rain in "planets with masses between about ∼ 40 and 400 M⊕ and effective
//! temperatures below about ∼ 150 K".
//!
//! - **What decides it** is where the deep adiabat meets the hydrogen–helium phase curve near
//!   2 Mbar: Mankovich and Fortney (2020, ApJ 889, 51, arXiv:1912.01009, read) draw the onset as
//!   the adiabat of one-bar temperature T1 = 149 K against Schöttler and Redmer's (2018) phase
//!   diagram (their Fig. 1), which must be warmer than computed by `ΔT_phase` = 539 ± 23 K for
//!   Galileo's helium. Schöttler and Redmer find "current Jupiter models are most likely too hot to
//!   allow demixing" without such a shift, and Brygoo et al. (2021) measure the immiscibility up to
//!   10,200 K above 1.5 Mbar (both abstracts, as decision-p14-t50b-envelope §3 quotes them).
//! - **One adiabat, whatever the gravity.** An adiabat is labelled by its one-bar temperature, and
//!   that follows T₀ = K g^−0.167 `T_eff`^1.243 (Hubbard 1977's fit to Graboske et al. 1975's
//!   atmospheres, as Fortney et al. 2011, ApJ 729, 32, arXiv:1101.0606, Eq. 1, read, give it,
//!   "below Teff of 200 K"). So the rain reads x = `T_eff` (`g_J` ÷ g)^(0.167 ÷ 1.243)
//!   ([`rain_temperature`]), the `T_eff` that a planet of Jupiter's gravity
//!   ([`JUPITER_SURFACE_GRAVITY`]) would have on the same adiabat. `T_eff` is the planet's own,
//!   `T_eff`⁴ = `T_int`⁴ + `T_eq`⁴: Fortney and Hubbard (2004, ApJ 608, 1039,
//!   arXiv:astro-ph/0402620, read) set the deep entropy from the isolated models at the planet's
//!   `T_eff` and gravity, S = S(`T_eff`, g, X) (their eq. 4, with eq. 6, for planets "at least 5
//!   AU" out). A giant within a few au "will cool to \[its] equilibrium temperatures, determined by
//!   stellar heating, before the onset of phase separation" (their abstract), and so, keyed on
//!   `T_eff`, never rains, whatever its record's `T_int`.
//! - **Calibrated on the two giants measured.** He/H over the primordial falls linearly in x,
//!   through Jupiter's [`JUPITER_HELIUM_MOLE_FRACTION`] at [`JUPITER_EFFECTIVE_TEMPERATURE`]
//!   (He/H₂ 0.157, 0.812 of protosolar) and Saturn's [`SATURN_HELIUM_PER_HYDROGEN`] at
//!   [`SATURN_EFFECTIVE_TEMPERATURE`] and [`SATURN_SURFACE_GRAVITY`] (0.407, at x = 108.22 K), from
//!   none at the 133.61 K that line reaches ([`helium_rain_onset`]), and is held at Saturn's below.
//!   - The onset is 133.61 K at Jupiter's gravity and 119.35 K at Saturn's
//!     ([`helium_rain_onset_at`]), inside the "∼100 - 120 K" that Fortney and Hubbard quote for
//!     Saturn from Stevenson and Salpeter (1977, their §1).
//!   - A heavier giant rains at a higher `T_eff` and a lighter one at a lower, as "the greater the
//!     planet's mass, the later in the planet's evolution helium phase separation begins" (their
//!     §3.1): a 3 `M_J` planet below about 153 K and a 0.15 `M_J` one below about 106 K
//!     (decision-p14-t50b-envelope §3).
//!   - On it, Saturn's T1 over Jupiter's is 0.831, against Mankovich and Fortney's 140 ÷ 166 =
//!     0.843.
//!   - Fortney and Hubbard's own phase diagram starts Jupiter's rain only "at `T_eff` below 123 K",
//!     2.6 K under its present `T_eff`, and so leaves its measured depletion unexplained (their
//!     §1): the measured giants calibrate it instead.
//! - **Not before the interior has cooled.** A giant under 0.3 `M_J` has no internal luminosity on
//!   the record, so its `T_eff` is its `T_eq` at every age. The rain therefore also waits on age,
//!   growing linearly from none at [`HELIUM_RAIN_EARLIEST_AGE`], 0.7 Gyr (Fortney and Hubbard's
//!   0.15 `M_J` model, the earliest onset of their range: "at an age of 700 million years", §3.1),
//!   to all of the line's at [`HELIUM_RAIN_SATURN_AGE`], 1.5 Gyr ("helium separation should have
//!   occurred first in Saturn after ∼ 1.5 Gyr of evolution", Guillot et al. 2022, §2.1.2). A giant
//!   with an internal luminosity of its own is still too warm at those ages to rain.
//! - **Neon** dissolves into the helium droplets (Roulston and Stevenson 1995; Wilson and
//!   Militzer 2010; as Atreya et al. 2016 and Guillot et al. 2022 cite them, read). Jupiter's
//!   neon is 0.0985 of protosolar against its argon's, krypton's and xenon's mean of 2.29 (Mahaffy
//!   et al. 2000 in Atreya et al.'s Table 2.1), so it falls with the helium, by the same fraction
//!   of what is left at each step, through [`RAINED_NEON`] of the other metals' at Jupiter's
//!   depletion, and on: to 5 × 10⁻⁵ at Saturn's. Mankovich and Fortney (2020, §4.3, read) expect
//!   Saturn's to be "even more advanced than in Jupiter's" and take its outer envelope's neon as
//!   negligible.
//! - **A sub-Neptune or an ice giant keeps its helium.** Its envelope is too warm and too light for
//!   the immiscibility: 0.15 `M_J` on a 10 M⊕ core "is approximately the lowest mass a planet can
//!   be while still having some liquid metallic hydrogen", a limit "dependent on the mass of the
//!   core", against Uranus's and Neptune's 14.5 and 17.1 M⊕ (Fortney and Hubbard 2004, §3.1).
//!   Uranus's helium mole fraction of 0.152 ± 0.033 is solar, "suggesting that helium
//!   differentiation has not occurred on Uranus" (Conrath et al. 1987, JGR 92, 15003, abstract
//!   read).
//!
//! # On the record from 24
//!
//! This module is pure. P14.T52.a speciates its abundances and P14.T52.d publishes them in the
//! record's envelope section, which the record builds behind the air's sections' hold
//! (P14.T54.b.i), so that at generator version 23 nothing on the record calls it
//! (decision-bump-plan-after-22 §1.3).
//!
//! # Determinism
//!
//! Every value is a closed form of the inputs, summed in [`Element::ALL`]'s order through the
//! crate's deterministic elementary functions. Nothing is drawn.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::redox::{SNOW_LINE_TEMPERATURE, SOOT_LINE_TEMPERATURE};
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::formation_temperature;
use crate::planetary::derive::composition::MassFractions;
use crate::planetary::params::THIN_ENVELOPE_FRACTION;
use crate::substance::Element;
use crate::units::{
    Dex, EarthMasses, JupiterMasses, Kelvin, Metres, MetresPerSecondSquared, Years,
};

/// The envelope enrichment of a body of one Jupiter mass: 2.65 (the module's trend, fitted with
/// [`ENRICHMENT_MASS_INDEX`]).
///
/// The lane's unweighted least squares in log₁₀, over the four giants' C/H of the
/// [module](self)'s documentation, of this module's own C/H at each: on a protosolar host, formed
/// at its semi-major axis on the generated zero-age Sun's disc (snow line 2.255 au), so with the
/// ice-line factor 11/7 for Jupiter, Saturn and Uranus and 11/6 for Neptune. It fits 0.4234 in
/// log₁₀, 2.651, with an rms residual of 0.086 dex: Jupiter −0.07, Saturn +0.12, Uranus +0.04
/// and Neptune −0.09. Neptune's factor rests on its formation at 46.55 K, 1% inside the 47 K CO₂
/// line, whose range is 42–52 K. Welbanks et al.'s (2019, `ApJL` 887, L20, arXiv:1912.04904,
/// read) fit to the same planets' methane, weighted by its errors, log₁₀(C/H) =
/// −1.11 log₁₀(M ÷ `M_J`) + 0.38, passes Saturn and puts Jupiter's carbon at 2.4: the recorded
/// alternative.
pub const ENRICHMENT_AT_ONE_JUPITER_MASS: f64 = 2.65;

/// How the envelope enrichment falls with mass, E ∝ M^−0.986 (the fit of
/// [`ENRICHMENT_AT_ONE_JUPITER_MASS`]).
///
/// About M⁻¹: a fixed mass of metals spread through envelopes of every mass.
pub const ENRICHMENT_MASS_INDEX: f64 = 0.986;

/// The elements a host's \[α/Fe\] raises with its oxygen: O, Ne, Mg, Si, S, Ar, Ca and Ti, in the
/// registry's order.
///
/// Bensby, Feltzing and Oey's (2014, A&A 562, A71, Fig. 15, read) α elements are O, Mg, Si, Ca
/// and Ti, and Hayden et al.'s (2015, ApJ 808, 132, Appendix B, read) O, Mg, Si, S and Ca. Neon and
/// argon are not measured in an FGK photosphere; they are made with oxygen in the α chain, and
/// go with it (the lane's; the constant Ne/O and Ar/O of H II regions, Izotov et al. 2006, A&A
/// 448, 955, were not read).
pub const ALPHA_ELEMENTS: [Element; 8] = [
    Element::O,
    Element::Ne,
    Element::Mg,
    Element::Si,
    Element::S,
    Element::Ar,
    Element::Ca,
    Element::Ti,
];

/// One carrier of a disc's oxygen and carbon, from Öberg, Murray-Clay and Bergin's (2011) Table 1.
///
/// A plain row of a cited table with no invariant between its fields, so they are public.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DiscCarrier {
    /// The disc temperature at or below which the carrier is ice or dust, K.
    pub line: Kelvin,
    /// Its oxygen by number relative to the disc's hydrogen nuclei.
    pub oxygen_per_hydrogen: f64,
    /// Its carbon by number relative to the disc's hydrogen nuclei.
    pub carbon_per_hydrogen: f64,
}

/// Öberg, Murray-Clay and Bergin's (2011, Table 1, read) carriers of a disc's oxygen and carbon,
/// coldest first: their adopted abundances and evaporation temperatures, with water's line at the
/// generator's snow line.
///
/// - CO, 1.5 × 10⁻⁴ of each per hydrogen, at 20 K (their range 18–22 K);
/// - CO₂, 0.6 × 10⁻⁴ of oxygen and 0.3 × 10⁻⁴ of carbon, at 47 K (42–52 K);
/// - water, 0.9 × 10⁻⁴ of oxygen, at [`SNOW_LINE_TEMPERATURE`], 170 K, the snow line of the
///   generator's Hayashi profile, for their adopted 135 K (120–150 K at disc midplane densities);
/// - carbon grains, 0.6 × 10⁻⁴ of carbon, at [`SOOT_LINE_TEMPERATURE`], 500 K, P14.T50.a's soot
///   line (Li et al. 2021), their adopted value ("> 150 K");
/// - silicates, 1.4 × 10⁻⁴ of oxygen, at 1,500 K.
///
/// Together 4.4 × 10⁻⁴ of oxygen and 2.4 × 10⁻⁴ of carbon: a C/O of 0.545, their disc's solar
/// ratio.
pub const DISC_CARRIERS: [DiscCarrier; 5] = [
    DiscCarrier {
        line: Kelvin::new(20.0),
        oxygen_per_hydrogen: 1.5e-4,
        carbon_per_hydrogen: 1.5e-4,
    },
    DiscCarrier {
        line: Kelvin::new(47.0),
        oxygen_per_hydrogen: 0.6e-4,
        carbon_per_hydrogen: 0.3e-4,
    },
    DiscCarrier {
        line: SNOW_LINE_TEMPERATURE,
        oxygen_per_hydrogen: 0.9e-4,
        carbon_per_hydrogen: 0.0e-4,
    },
    DiscCarrier {
        line: SOOT_LINE_TEMPERATURE,
        oxygen_per_hydrogen: 0.0e-4,
        carbon_per_hydrogen: 0.6e-4,
    },
    DiscCarrier {
        line: Kelvin::new(1_500.0),
        oxygen_per_hydrogen: 1.4e-4,
        carbon_per_hydrogen: 0.0e-4,
    },
];

/// The least mass of a giant whose helium can rain: 0.15 `M_J`, the 47.7 M⊕ that Fortney and
/// Hubbard (2004, §3.1, read) find "is approximately the lowest mass a planet can be while still
/// having some liquid metallic hydrogen in its deep interior", on a 10 M⊕ core; without one, their
/// "coreless model planets of mass 0.15 and 0.2 MJ … do not have central pressures high enough
/// for liquid metallic hydrogen to form" (their Fig. 12).
pub const HELIUM_RAIN_LEAST_MASS: JupiterMasses = JupiterMasses::new(0.15);

/// The age from which a giant's helium can begin to rain: 0.7 Gyr, the earliest onset of Fortney
/// and Hubbard's (2004, §3.1, read) range of models, their 0.15 `M_J`'s ("the onset of helium
/// separation is fairly early in the planet's evolution, at an age of 700 million years").
pub const HELIUM_RAIN_EARLIEST_AGE: Years = Years::new(7.0e8);

/// The age by which a giant's helium rains as far as its `T_eff` allows: 1.5 Gyr, Saturn's onset
/// ("helium separation should have occurred first in Saturn after ∼ 1.5 Gyr of evolution", Guillot
/// et al. 2022, §2.1.2, read, after Mankovich and Fortney 2020).
pub const HELIUM_RAIN_SATURN_AGE: Years = Years::new(1.5e9);

/// Jupiter's effective temperature, 125.57 ± 0.07 K (Li et al. 2012, as Mankovich and Fortney
/// 2020, Table 1, list it, read): the (P ÷ σ)^¼ of the 14.098 ± 0.031 W m⁻² it emits (Li et al.
/// 2018, Nature Communications 9, 3709, Table 1, read). Its helium is the Galileo probe's here.
pub const JUPITER_EFFECTIVE_TEMPERATURE: Kelvin = Kelvin::new(125.57);

/// The Galileo probe's helium mole fraction in Jupiter, 0.1359 ± 0.0027 from 2 to 12 bar (von
/// Zahn, Hunten and Lehmacher 1998, JGR 103, 22815, abstract read). Over the rest of the gas,
/// He/H₂ = 0.157 by number: the plan's "about 0.16". The same measurement is Mankovich and
/// Fortney's (2020, Table 1, read) Y ÷ (X + Y) of 0.238 ± 0.005 and Guillot et al.'s (2022,
/// Table 2, read) He/H of 7.88 ± 0.16 × 10⁻².
pub const JUPITER_HELIUM_MOLE_FRACTION: f64 = 0.1359;

/// Saturn's effective temperature, 96.67 ± 0.17 K (Li et al. 2010, JGR 115, E11002, abstract
/// read; Mankovich and Fortney 2020, Table 1). Its helium is [`SATURN_HELIUM_PER_HYDROGEN`] here,
/// at its [`SATURN_SURFACE_GRAVITY`].
pub const SATURN_EFFECTIVE_TEMPERATURE: Kelvin = Kelvin::new(96.67);

/// Saturn's He/H by number: 0.0394, the mean of the three Cassini measurements Guillot et al.
/// (2022, Table 2, read) list, 2.87 ± 0.87 (Achterberg and Flasar 2020), 6.20 ± 1.25 (Koskinen
/// and Guerlet 2018) and 2.75 (+0.50, −0.75) × 10⁻² (Sromovsky et al. 2016), which disagree by
/// more than their errors: 0.407 of this registry's protosolar He/H.
pub const SATURN_HELIUM_PER_HYDROGEN: f64 = 0.0394;

/// Jupiter's surface gravity, GM over its mean radius squared: 25.92 m s⁻², from the nominal
/// GM = 1.266 865 3 × 10¹⁷ m³ s⁻² (IAU 2015 Resolution B3,
/// [`GM_JUPITER`](crate::units::consts::GM_JUPITER)) and the mean radius of 69,911 km (Seidelmann
/// et al. 2007, as Mankovich and Fortney 2020, Table 1, list it, read).
pub const JUPITER_SURFACE_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(25.92);

/// Saturn's surface gravity, GM over its mean radius squared: 11.19 m s⁻², from the Solar System
/// table's mass of 568.32 × 10²⁴ kg (NASA's planetary fact sheet) times G (CODATA 2018), GM =
/// 3.7931 × 10¹⁶ m³ s⁻², and the mean radius of 58,232 km (Seidelmann et al. 2007, as Mankovich
/// and Fortney 2020, Table 1, list it, read).
pub const SATURN_SURFACE_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(11.19);

/// The exponents of Hubbard's (1977) fit, T₀ = K g^−0.167 `T_eff`^1.243 (Fortney et al. 2011, ApJ
/// 729, 32, arXiv:1101.0606, Eq. 1, read): gravity's, 0.167, and the effective temperature's,
/// 1.243. Their ratio is how a planet's `T_eff` maps onto Jupiter's gravity at one adiabat
/// ([`rain_temperature`]).
pub const HUBBARD_INDICES: (f64, f64) = (0.167, 1.243);

/// The share of its neon a raining giant keeps against its other metals once its helium is at
/// Jupiter's depletion: 0.043, Jupiter's neon over its heavy noble gases' mean enrichment.
///
/// Jupiter's Ne/H is 1.24 × 10⁻⁵, Ar/H 9.10 × 10⁻⁶, Kr/H 4.65 × 10⁻⁹ and Xe/H 4.45 × 10⁻¹⁰ (Mahaffy
/// et al. 2000, JGR 105, 15061, as Atreya et al. 2016, Table 2.1, list them, read; Mahaffy et
/// al.'s abstract, read: argon, krypton and xenon "2.5±0.5, 2.7±0.5, and 2.6±0.5 times the solar
/// ratios", neon "0.1 times solar"). Over this registry's protosolar values that is 0.0985 for
/// neon against 2.565, 2.175 and 2.130, whose mean is 2.290.
pub const RAINED_NEON: f64 = 0.043;

/// How far from solar an \[Fe/H\] or an \[α/Fe\] may lie, dex: 10, far outside any population the
/// generator places, and near enough that every abundance and mass fraction stays finite.
pub const METALLICITY_BOUND: Dex = Dex::new(10.0);

/// Why [`EnvelopeAbundances::derive`] could not give an envelope's abundances.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum DeriveEnvelopeAbundancesError {
    /// A mass that is not finite and positive.
    Mass(EarthMasses),
    /// No gas envelope: an envelope share that is not finite, below [`THIN_ENVELOPE_FRACTION`]
    /// (a body P14.T13's atmosphere does not make a
    /// [`SurfaceState::GasEnvelope`](crate::planetary::derive::SurfaceState::GasEnvelope)), or
    /// above 1.
    NotAnEnvelope(f64),
    /// A formation distance that is not finite and positive.
    FormationDistance(Metres),
    /// A snow line that is not finite and positive.
    SnowLine(Metres),
    /// An \[Fe/H\] that is not finite or lies beyond ±[`METALLICITY_BOUND`].
    FeH(Dex),
    /// An \[α/Fe\] that is not finite or lies beyond ±[`METALLICITY_BOUND`].
    AlphaFe(Dex),
    /// An effective temperature that is not finite and non-negative.
    EffectiveTemperature(Kelvin),
    /// An age that is not finite and non-negative.
    Age(Years),
    /// A surface gravity that is not finite and positive.
    SurfaceGravity(MetresPerSecondSquared),
    /// A primordial He/H that is not finite and positive.
    PrimordialHelium(f64),
}

impl fmt::Display for DeriveEnvelopeAbundancesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mass(m) => write!(f, "mass {} M⊕ is not finite and positive", m.value()),
            Self::NotAnEnvelope(share) => write!(
                f,
                "an envelope share of {share} is not a gas envelope's, from \
                 {THIN_ENVELOPE_FRACTION} to 1"
            ),
            Self::FormationDistance(d) => write!(
                f,
                "formation distance {} m is not finite and positive",
                d.value()
            ),
            Self::SnowLine(d) => write!(f, "snow line {} m is not finite and positive", d.value()),
            Self::FeH(x) => write!(
                f,
                "[Fe/H] {} is not a finite value within ±{} dex",
                x.value(),
                METALLICITY_BOUND.value()
            ),
            Self::AlphaFe(x) => write!(
                f,
                "[α/Fe] {} is not a finite value within ±{} dex",
                x.value(),
                METALLICITY_BOUND.value()
            ),
            Self::EffectiveTemperature(t) => write!(
                f,
                "effective temperature {} K is not finite and non-negative",
                t.value()
            ),
            Self::Age(t) => write!(f, "age {} yr is not finite and non-negative", t.value()),
            Self::SurfaceGravity(g) => write!(
                f,
                "surface gravity {} m s⁻² is not finite and positive",
                g.value()
            ),
            Self::PrimordialHelium(x) => {
                write!(f, "primordial He/H {x} is not finite and positive")
            }
        }
    }
}

impl Error for DeriveEnvelopeAbundancesError {}

/// Everything [`EnvelopeAbundances::derive`] reads of a body and its host, as plain values.
///
/// The fields are public, as P14.T50.a's inputs are.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EnvelopeInputs {
    /// The body's mass at the time.
    pub mass: EarthMasses,
    /// Its bulk composition, whose envelope share is at least [`THIN_ENVELOPE_FRACTION`]: what
    /// bounds the envelope's metals, and with the mass its class.
    pub fractions: MassFractions,
    /// Where it formed from its host.
    pub formation_distance: Metres,
    /// Its host's snow line, against which its disc temperature at formation is taken
    /// ([`formation_temperature`]).
    pub snow_line: Metres,
    /// Its host's \[Fe/H\], dex.
    pub fe_h: Dex,
    /// Its host's \[α/Fe\], dex
    /// ([`SystemContext::alpha_fe`](crate::planetary::context::SystemContext::alpha_fe)).
    pub alpha_fe: Dex,
    /// Its host's C/O and Mg/Si (P14.T50.a).
    pub host: HostAbundances,
    /// Its effective temperature, `T_eff`⁴ = `T_eq`⁴ + `T_int`⁴: (`T_eq`⁴ + `T_int`⁴)^¼ with
    /// P14.T24.d's `T_int`
    /// ([`envelope_internal_temperature`](crate::planetary::hooks::envelope_internal_temperature)),
    /// as P14.T54.b.i passes it (decision-p14-t24d-profile §3.5), the envelope's profile's own
    /// temperature, not the bulk section's `T_eff`, which below 0.414 Jupiter masses has no
    /// internal heat.
    pub effective_temperature: Kelvin,
    /// Its age at the time, which the helium rain waits on.
    pub age: Years,
    /// Its surface gravity at the time, G M ÷ R² (the record's
    /// [`surface_gravity`](crate::planetary::derive::DerivedBody::surface_gravity)), which sets the
    /// adiabat its `T_eff` lies on.
    pub surface_gravity: MetresPerSecondSquared,
    /// The envelope's He/H by number before any helium rain: the derivation's
    /// [`EnvelopeHelium::helium_to_hydrogen`](crate::planetary::chem::escape::EnvelopeHelium::helium_to_hydrogen),
    /// fractionated by P14.T53.a's escape from the host star's start (P14.T54.b.i). The tests pass
    /// the protosolar value, [`Element::He`]'s.
    pub primordial_helium_per_hydrogen: f64,
}

/// A gas envelope's element abundances by number relative to hydrogen (P14.T50.b; the
/// [module](self) documentation).
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct EnvelopeAbundances {
    per_hydrogen: [f64; Element::ALL.len()],
    enrichment: f64,
    ice_line_factor: f64,
    helium_depletion: f64,
}

impl EnvelopeAbundances {
    /// The abundances of the envelope `inputs` describe (the [module](self)'s model).
    ///
    /// # Errors
    ///
    /// [`DeriveEnvelopeAbundancesError`] for a mass that is not finite and positive, an envelope
    /// share outside [`THIN_ENVELOPE_FRACTION`] to 1, a distance that is not finite and positive,
    /// a metallicity that is not a finite value within ±[`METALLICITY_BOUND`], an effective
    /// temperature or an age that is not finite and non-negative, or a surface gravity or a
    /// primordial He/H that is not finite and positive.
    ///
    /// # Examples
    ///
    /// A sub-Neptune of 10 M⊕ and 3 R⊕ formed inside its star's snow line holds about 80 times its
    /// star's metals in its envelope, with a C/O a tenth above its star's and the star's helium:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::envelope_abundances::{
    ///     EnvelopeAbundances, EnvelopeInputs,
    /// };
    /// use hyperion_sim::planetary::chem::{AbundanceDraws, HostAbundances};
    /// use hyperion_sim::planetary::derive::{SnowLineSide, composition};
    /// use hyperion_sim::substance::Element;
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{
    ///     Dex, EarthFluxes, EarthMasses, EarthRadii, Kelvin, Metres, MetresPerSecondSquared,
    ///     Years,
    /// };
    ///
    /// let (mass, side) = (EarthMasses::new(10.0), SnowLineSide::Inside);
    /// let solved = composition(mass, EarthRadii::new(3.0), side, EarthFluxes::new(10.0))?;
    /// let host = HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN);
    /// let envelope = EnvelopeAbundances::derive(&EnvelopeInputs {
    ///     mass,
    ///     fractions: solved.fractions(),
    ///     formation_distance: Metres::new(0.3 * METRES_PER_AU),
    ///     snow_line: Metres::new(2.255 * METRES_PER_AU),
    ///     fe_h: Dex::ZERO,
    ///     alpha_fe: Dex::ZERO,
    ///     host,
    ///     effective_temperature: Kelvin::new(700.0),
    ///     age: Years::new(4.6e9),
    ///     surface_gravity: MetresPerSecondSquared::new(10.9),
    ///     primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
    /// })?;
    /// assert!((70.0..90.0).contains(&envelope.enrichment()));
    /// assert!((envelope.carbon_to_oxygen() / host.carbon_to_oxygen() - 1.1).abs() < 1e-12);
    /// let helium = Element::He.protosolar_abundance_per_hydrogen();
    /// assert!((envelope.per_hydrogen(Element::He) - helium).abs() < 1e-15);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn derive(inputs: &EnvelopeInputs) -> Result<Self, DeriveEnvelopeAbundancesError> {
        let mass = inputs.mass.value();
        if !(mass.is_finite() && mass > 0.0) {
            return Err(DeriveEnvelopeAbundancesError::Mass(inputs.mass));
        }
        let share = inputs.fractions.envelope();
        if !(share.is_finite() && (THIN_ENVELOPE_FRACTION..=1.0).contains(&share)) {
            return Err(DeriveEnvelopeAbundancesError::NotAnEnvelope(share));
        }
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(inputs.formation_distance.value()) {
            return Err(DeriveEnvelopeAbundancesError::FormationDistance(
                inputs.formation_distance,
            ));
        }
        if !positive(inputs.snow_line.value()) {
            return Err(DeriveEnvelopeAbundancesError::SnowLine(inputs.snow_line));
        }
        let bounded = |x: Dex| x.value().abs() <= METALLICITY_BOUND.value();
        if !bounded(inputs.fe_h) {
            return Err(DeriveEnvelopeAbundancesError::FeH(inputs.fe_h));
        }
        if !bounded(inputs.alpha_fe) {
            return Err(DeriveEnvelopeAbundancesError::AlphaFe(inputs.alpha_fe));
        }
        let t_eff = inputs.effective_temperature.value();
        if !(t_eff.is_finite() && t_eff >= 0.0) {
            return Err(DeriveEnvelopeAbundancesError::EffectiveTemperature(
                inputs.effective_temperature,
            ));
        }
        let age = inputs.age.value();
        if !(age.is_finite() && age >= 0.0) {
            return Err(DeriveEnvelopeAbundancesError::Age(inputs.age));
        }
        if !positive(inputs.surface_gravity.value()) {
            return Err(DeriveEnvelopeAbundancesError::SurfaceGravity(
                inputs.surface_gravity,
            ));
        }
        let primordial = inputs.primordial_helium_per_hydrogen;
        if !positive(primordial) {
            return Err(DeriveEnvelopeAbundancesError::PrimordialHelium(primordial));
        }

        let host = host_per_hydrogen(inputs.fe_h, inputs.alpha_fe, &inputs.host);
        let class = PlanetClass::of(inputs.mass, &inputs.fractions);
        let depletion = helium_depletion(
            class,
            inputs.mass,
            rain_temperature(inputs.effective_temperature, inputs.surface_gravity),
            inputs.age,
        );
        let helium = primordial * depletion;
        let enrichment = enrichment(inputs.mass, share, helium, metal_mass_per_hydrogen(&host));
        let factor = ice_line_factor(formation_temperature(
            inputs.formation_distance,
            inputs.snow_line,
        ));

        let mut per_hydrogen = host.map(|n| enrichment * n);
        per_hydrogen[index(Element::H)] = 1.0;
        per_hydrogen[index(Element::He)] = helium;
        per_hydrogen[index(Element::Ne)] *= neon_kept(depletion);
        // Carbon, nitrogen and oxygen at E times the host's mass of the three, carbon at the
        // host's C/O times the factor and nitrogen at the protosolar N/O to the oxygen.
        let weight = |e: Element| e.standard_atomic_weight_g_per_mol();
        let (c, n, o) = (index(Element::C), index(Element::N), index(Element::O));
        let carbon_to_oxygen = inputs.host.carbon_to_oxygen() * factor;
        let nitrogen_to_oxygen = solar_nitrogen_to_oxygen();
        let mass_of_three = weight(Element::C) * host[c]
            + weight(Element::N) * host[n]
            + weight(Element::O) * host[o];
        let oxygen = enrichment * mass_of_three
            / (weight(Element::C) * carbon_to_oxygen
                + weight(Element::N) * nitrogen_to_oxygen
                + weight(Element::O));
        per_hydrogen[o] = oxygen;
        per_hydrogen[c] = carbon_to_oxygen * oxygen;
        per_hydrogen[n] = nitrogen_to_oxygen * oxygen;
        Ok(Self {
            per_hydrogen,
            enrichment,
            ice_line_factor: factor,
            helium_depletion: depletion,
        })
    }

    /// The element's abundance by number relative to hydrogen, `N_X` ÷ `N_H`; hydrogen's is 1.
    #[must_use]
    pub fn per_hydrogen(&self, element: Element) -> f64 {
        self.per_hydrogen[index(element)]
    }

    /// Every element with its abundance relative to hydrogen, in [`Element::ALL`]'s order.
    pub fn elements(&self) -> impl Iterator<Item = (Element, f64)> {
        Element::ALL.into_iter().zip(self.per_hydrogen)
    }

    /// The envelope's enrichment E: its metal-to-hydrogen mass ratio over its host's, before the
    /// rain takes neon: the trend ([`mass_enrichment`]), held at most to the bulk's metal fraction
    /// (f = 1) and at least to 1.
    #[must_use]
    pub const fn enrichment(&self) -> f64 {
        self.enrichment
    }

    /// The envelope's C/O by number: the host's times the
    /// [`ice_line_factor`](Self::ice_line_factor).
    #[must_use]
    pub fn carbon_to_oxygen(&self) -> f64 {
        self.per_hydrogen(Element::C) / self.per_hydrogen(Element::O)
    }

    /// The factor by which the envelope's C/O exceeds its host's
    /// ([`ice_line_factor`](fn@ice_line_factor)).
    #[must_use]
    pub const fn ice_line_factor(&self) -> f64 {
        self.ice_line_factor
    }

    /// The envelope's He/H over the protosolar value: 1 where no helium has rained out, 0.812 at
    /// Jupiter's measured depletion and 0.407 at Saturn's
    /// ([`helium_depletion`](fn@helium_depletion)).
    #[must_use]
    pub const fn helium_depletion(&self) -> f64 {
        self.helium_depletion
    }

    /// He/H₂ by number, twice He/H, for an envelope whose hydrogen is molecular.
    #[must_use]
    pub fn helium_to_molecular_hydrogen(&self) -> f64 {
        2.0 * self.per_hydrogen(Element::He)
    }

    /// The envelope's mass fractions of hydrogen, helium and the metals (X, Y and Z), summing to 1.
    #[must_use]
    pub fn mass_fractions(&self) -> (f64, f64, f64) {
        let weight = |e: Element| e.standard_atomic_weight_g_per_mol();
        let hydrogen = weight(Element::H);
        let helium = weight(Element::He) * self.per_hydrogen(Element::He);
        let metals = metal_mass_per_hydrogen(&self.per_hydrogen);
        let total = hydrogen + helium + metals;
        (hydrogen / total, helium / total, metals / total)
    }
}

/// The trend of the envelope enrichment E with a body's mass `mass`:
/// [`ENRICHMENT_AT_ONE_JUPITER_MASS`] × (M ÷ `M_J`)^−[`ENRICHMENT_MASS_INDEX`], before the
/// [module](self)'s bound and floor.
///
/// It is fitted from 14.5 M⊕ (Uranus) to 318 M⊕ (Jupiter) and extrapolated outside: 80 at 10 M⊕,
/// 160 at 5 M⊕ and 390 at 2 M⊕.
///
/// # Examples
///
/// Jupiter's envelope holds about 2.7 times the metals of the Sun's gas, a Neptune's about 50:
///
/// ```
/// use hyperion_sim::planetary::chem::envelope_abundances::mass_enrichment;
/// use hyperion_sim::units::EarthMasses;
///
/// assert!((mass_enrichment(EarthMasses::new(317.83)) - 2.65).abs() < 0.01);
/// assert!((45.0..50.0).contains(&mass_enrichment(EarthMasses::new(17.15))));
/// ```
#[must_use]
pub fn mass_enrichment(mass: EarthMasses) -> f64 {
    let jupiters = JupiterMasses::from(mass).value();
    ENRICHMENT_AT_ONE_JUPITER_MASS * math::powf(jupiters, -ENRICHMENT_MASS_INDEX)
}

/// The envelope enrichment E of a body of mass `mass` whose envelope is `envelope_share` of its
/// mass, with `helium_per_hydrogen` of helium, about a host whose metals weigh
/// `host_metals_per_hydrogen` atomic mass units per hydrogen atom: the trend
/// ([`mass_enrichment`]), held at most to the bulk's metal fraction (f = 1) and at least to 1.
///
/// With X, Y and Z the envelope's mass fractions, Z ≤ `Z_p` = 1 − `envelope_share` is
/// Z ÷ X ≤ `Z_p` (1 + Y ÷ X) ÷ (1 − `Z_p`).
#[must_use]
fn enrichment(
    mass: EarthMasses,
    envelope_share: f64,
    helium_per_hydrogen: f64,
    host_metals_per_hydrogen: f64,
) -> f64 {
    let hydrogen = Element::H.standard_atomic_weight_g_per_mol();
    let helium_over_hydrogen =
        Element::He.standard_atomic_weight_g_per_mol() * helium_per_hydrogen / hydrogen;
    let bulk = 1.0 - envelope_share;
    let most = bulk * (1.0 + helium_over_hydrogen)
        / envelope_share
        / (host_metals_per_hydrogen / hydrogen);
    mass_enrichment(mass).min(most).max(1.0)
}

/// The factor by which an envelope's C/O exceeds its host's, for a body formed where its disc was
/// at `formation_temperature`: the disc gas's C/O there over the disc's ([`DISC_CARRIERS`]; the
/// [module](self) documentation).
///
/// Each carrier is ice or dust where the disc is at or below its line. The factor is 1 above
/// 1,500 K, where nothing has condensed, 22/15 inside the soot line, 11/10 inside the snow line,
/// 11/7 to the CO₂ line, 11/6 to the CO line, and 1 beyond it, where the gas holds neither.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::envelope_abundances::ice_line_factor;
/// use hyperion_sim::units::Kelvin;
///
/// // Jupiter formed at 5.2 au, where the zero-age Sun's disc was 112 K, between the H₂O and CO₂
/// // lines.
/// assert!((ice_line_factor(Kelvin::new(112.0)) - 11.0 / 7.0).abs() < 1e-12);
/// assert!((ice_line_factor(Kelvin::new(10.0)) - 1.0).abs() < 1e-15);
/// ```
#[must_use]
pub fn ice_line_factor(formation_temperature: Kelvin) -> f64 {
    let t = formation_temperature.value();
    let (mut oxygen, mut carbon, mut gas_oxygen, mut gas_carbon) = (0.0, 0.0, 0.0, 0.0);
    for carrier in DISC_CARRIERS {
        oxygen += carrier.oxygen_per_hydrogen;
        carbon += carrier.carbon_per_hydrogen;
        if t > carrier.line.value() {
            gas_oxygen += carrier.oxygen_per_hydrogen;
            gas_carbon += carrier.carbon_per_hydrogen;
        }
    }
    if gas_oxygen > 0.0 {
        (gas_carbon / gas_oxygen) / (carbon / oxygen)
    } else {
        1.0
    }
}

/// The temperature the helium rain reads: the `T_eff` that a planet of Jupiter's gravity would
/// have on the adiabat of a planet of effective temperature `effective_temperature` and surface
/// gravity `surface_gravity`, x = `T_eff` (`g_J` ÷ g)^(0.167 ÷ 1.243) ([`HUBBARD_INDICES`],
/// [`JUPITER_SURFACE_GRAVITY`]).
///
/// # Examples
///
/// Saturn's 96.67 K at its 11.19 m s⁻² lies on the adiabat of a Jupiter at 108.22 K:
///
/// ```
/// use hyperion_sim::planetary::chem::envelope_abundances::{
///     SATURN_EFFECTIVE_TEMPERATURE, SATURN_SURFACE_GRAVITY, rain_temperature,
/// };
///
/// let x = rain_temperature(SATURN_EFFECTIVE_TEMPERATURE, SATURN_SURFACE_GRAVITY);
/// assert!((x.value() - 108.22).abs() < 0.01);
/// ```
#[must_use]
pub fn rain_temperature(
    effective_temperature: Kelvin,
    surface_gravity: MetresPerSecondSquared,
) -> Kelvin {
    let (gravity, temperature) = HUBBARD_INDICES;
    let ratio = JUPITER_SURFACE_GRAVITY.value() / surface_gravity.value();
    Kelvin::new(effective_temperature.value() * math::powf(ratio, gravity / temperature))
}

/// The He/H over its primordial value of a body of class `class` and mass `mass`, whose helium
/// rain reads the temperature `rain_temperature` ([`rain_temperature`](fn@rain_temperature)) at
/// age `age` (the [module](self)'s helium rain): 1 for a body that is not a
/// [`PlanetClass::GasGiant`] of at least [`HELIUM_RAIN_LEAST_MASS`]; for one, linear in the
/// temperature through Jupiter's measured depletion and Saturn's, 1 above [`helium_rain_onset`]
/// and Saturn's below its temperature, and of that depletion none before
/// [`HELIUM_RAIN_EARLIEST_AGE`], rising linearly to all of it at [`HELIUM_RAIN_SATURN_AGE`].
///
/// # Examples
///
/// A Jupiter at its measured `T_eff` takes the Galileo probe's helium; a hot Jupiter and an ice
/// giant keep their helium:
///
/// ```
/// use hyperion_sim::planetary::chem::envelope_abundances::{
///     JUPITER_EFFECTIVE_TEMPERATURE, helium_depletion, jupiter_helium_to_molecular_hydrogen,
/// };
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::substance::Element;
/// use hyperion_sim::units::{EarthMasses, Kelvin, Years};
///
/// let (giant, jupiters) = (PlanetClass::GasGiant, EarthMasses::new(317.83));
/// let now = Years::new(4.57e9);
/// let jupiter = helium_depletion(giant, jupiters, JUPITER_EFFECTIVE_TEMPERATURE, now);
/// let he_h2 = 2.0 * Element::He.protosolar_abundance_per_hydrogen() * jupiter;
/// assert!((he_h2 - jupiter_helium_to_molecular_hydrogen()).abs() < 1e-12);
/// let hot = helium_depletion(giant, jupiters, Kelvin::new(1_300.0), now);
/// assert!((hot - 1.0).abs() < 1e-15);
/// let neptune = EarthMasses::new(17.15);
/// let neptune = helium_depletion(PlanetClass::IceGiant, neptune, Kelvin::new(59.0), now);
/// assert!((neptune - 1.0).abs() < 1e-15);
/// ```
#[must_use]
pub fn helium_depletion(
    class: PlanetClass,
    mass: EarthMasses,
    rain_temperature: Kelvin,
    age: Years,
) -> f64 {
    let rains = match class {
        PlanetClass::GasGiant => {
            JupiterMasses::from(mass).value() >= HELIUM_RAIN_LEAST_MASS.value()
        }
        PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune | PlanetClass::IceGiant => {
            false
        }
    };
    if !rains {
        return 1.0;
    }
    let (jupiter, saturn) = (jupiter_helium_depletion(), saturn_helium_depletion());
    let (x_jupiter, x_saturn) = calibration_temperatures();
    let slope = (jupiter - saturn) / (x_jupiter - x_saturn);
    let by_temperature =
        (jupiter + slope * (rain_temperature.value() - x_jupiter)).clamp(saturn, 1.0);
    let (earliest, saturn_age) = (
        HELIUM_RAIN_EARLIEST_AGE.value(),
        HELIUM_RAIN_SATURN_AGE.value(),
    );
    let by_age = ((age.value() - earliest) / (saturn_age - earliest)).clamp(0.0, 1.0);
    1.0 - by_age * (1.0 - by_temperature)
}

/// Jupiter's and Saturn's rain temperatures ([`rain_temperature`](fn@rain_temperature)), K:
/// Jupiter's `T_eff` itself, 125.57 K, and Saturn's 96.67 K at its gravity, 108.22 K.
#[must_use]
fn calibration_temperatures() -> (f64, f64) {
    (
        rain_temperature(JUPITER_EFFECTIVE_TEMPERATURE, JUPITER_SURFACE_GRAVITY).value(),
        rain_temperature(SATURN_EFFECTIVE_TEMPERATURE, SATURN_SURFACE_GRAVITY).value(),
    )
}

/// The rain temperature above which no helium rains, where [`helium_depletion`]'s line through
/// Jupiter and Saturn reaches the primordial value: 133.61 K, the onset in `T_eff` at Jupiter's
/// gravity ([`helium_rain_onset_at`] gives it at another).
#[must_use]
pub fn helium_rain_onset() -> Kelvin {
    let (jupiter, saturn) = (jupiter_helium_depletion(), saturn_helium_depletion());
    let (x_jupiter, x_saturn) = calibration_temperatures();
    Kelvin::new(x_jupiter + (1.0 - jupiter) * (x_jupiter - x_saturn) / (jupiter - saturn))
}

/// The effective temperature below which helium begins to rain in a gas giant of surface gravity
/// `surface_gravity`: [`helium_rain_onset`] × (g ÷ `g_J`)^(0.167 ÷ 1.243), 119.35 K at Saturn's.
#[must_use]
pub fn helium_rain_onset_at(surface_gravity: MetresPerSecondSquared) -> Kelvin {
    let (gravity, temperature) = HUBBARD_INDICES;
    let ratio = surface_gravity.value() / JUPITER_SURFACE_GRAVITY.value();
    Kelvin::new(helium_rain_onset().value() * math::powf(ratio, gravity / temperature))
}

/// The share of its neon an envelope of helium depletion `depletion` keeps against its other
/// metals: [`RAINED_NEON`]^p, where p is how far the helium has gone in units of Jupiter's
/// depletion: 0 with none, 1 at Jupiter's, 3.16 at Saturn's.
///
/// Each step of the rain takes the same fraction of the neon left, as droplets that dissolve it
/// in proportion would: so neon falls fast once the rain begins, as Jupiter's did ("in spite of
/// helium immiscibility having set in only recently", Mankovich and Fortney 2020; the lane's
/// form).
#[must_use]
fn neon_kept(depletion: f64) -> f64 {
    let progress = (1.0 - depletion) / (1.0 - jupiter_helium_depletion());
    math::powf(RAINED_NEON, progress)
}

/// Jupiter's He/H₂ by number from the Galileo probe's mole fraction: x ÷ (1 − x) = 0.157
/// ([`JUPITER_HELIUM_MOLE_FRACTION`]), counting the rest of the gas as hydrogen.
#[must_use]
pub fn jupiter_helium_to_molecular_hydrogen() -> f64 {
    JUPITER_HELIUM_MOLE_FRACTION / (1.0 - JUPITER_HELIUM_MOLE_FRACTION)
}

/// Jupiter's He/H over the protosolar value: 0.812.
#[must_use]
fn jupiter_helium_depletion() -> f64 {
    jupiter_helium_to_molecular_hydrogen() / 2.0 / Element::He.protosolar_abundance_per_hydrogen()
}

/// Saturn's He/H over the protosolar value: 0.407 ([`SATURN_HELIUM_PER_HYDROGEN`]).
#[must_use]
fn saturn_helium_depletion() -> f64 {
    SATURN_HELIUM_PER_HYDROGEN / Element::He.protosolar_abundance_per_hydrogen()
}

/// The protosolar N/O by number: 0.135 (Lodders 2010, Table 6).
#[must_use]
pub fn solar_nitrogen_to_oxygen() -> f64 {
    math::exp10(
        (Element::N.protosolar_log_abundance() - Element::O.protosolar_log_abundance()).value(),
    )
}

/// The position of `element` in [`Element::ALL`].
#[must_use]
fn index(element: Element) -> usize {
    Element::ALL
        .iter()
        .position(|&e| e == element)
        .expect("every element is in the registry's list")
}

/// The host's abundances by number relative to hydrogen, in [`Element::ALL`]'s order: protosolar
/// times 10^\[Fe/H\], the α elements times 10^\[α/Fe\] too, carbon and magnesium at the host's
/// C/O and Mg/Si, nitrogen at the protosolar N/O, and helium protosolar.
#[must_use]
fn host_per_hydrogen(fe_h: Dex, alpha_fe: Dex, host: &HostAbundances) -> [f64; Element::ALL.len()] {
    let scaled = |e: Element| {
        let alpha = if ALPHA_ELEMENTS.contains(&e) {
            alpha_fe.value()
        } else {
            0.0
        };
        math::exp10(e.protosolar_log_abundance().value() - 12.0 + fe_h.value() + alpha)
    };
    let mut per_hydrogen = Element::ALL.map(scaled);
    per_hydrogen[index(Element::H)] = 1.0;
    per_hydrogen[index(Element::He)] = Element::He.protosolar_abundance_per_hydrogen();
    let oxygen = per_hydrogen[index(Element::O)];
    per_hydrogen[index(Element::C)] = host.carbon_to_oxygen() * oxygen;
    per_hydrogen[index(Element::N)] = solar_nitrogen_to_oxygen() * oxygen;
    per_hydrogen[index(Element::Mg)] =
        host.magnesium_to_silicon() * per_hydrogen[index(Element::Si)];
    per_hydrogen
}

/// The metals' mass per hydrogen atom, atomic mass units, of abundances `per_hydrogen` in
/// [`Element::ALL`]'s order: every element but hydrogen and helium, summed in that order.
#[must_use]
fn metal_mass_per_hydrogen(per_hydrogen: &[f64; Element::ALL.len()]) -> f64 {
    Element::ALL
        .into_iter()
        .zip(per_hydrogen)
        .filter(|(e, _)| !matches!(e, Element::H | Element::He))
        .fold(0.0, |sum, (e, &n)| {
            sum + e.standard_atomic_weight_g_per_mol() * n
        })
}

#[cfg(test)]
pub(crate) mod tests;
