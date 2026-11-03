//! A body's resource abundances (plan 14, P14.T26; the brainstorm's "Hooks for the layers
//! above"): a mass fraction of its accessible outer layers for each [`Resource`] of a closed list,
//! a deterministic function of its bulk composition, its differentiation, the side of its host's
//! snow line it formed on, its atmosphere, its host's \[Fe/H\] and \[α/Fe\] and its age, times one
//! log-normal multiple per resource for what the model leaves out
//! ([`ResourceAbundances::derive`]).
//!
//! # The accessible outer layers
//!
//! What an operation at the surface or in orbit reaches ([`OuterLayer`]), read off the record's
//! crust and envelope ([`OuterLayer::of_lid`]):
//!
//! - **The bulk** of an undifferentiated body, whose lid is its own material (a chondrite, a
//!   comet's ice and dust) or a frozen ocean's rock-laden shell (`phyllosilicate`, Ceres): its
//!   iron, rock and ices as its mass fractions give them.
//! - **A silicate crust** on a body whose rock melted: its metal sank into its core, taking the
//!   platinum group with it, so the layer is its silicate, at the bulk silicate Earth's
//!   composition, with its water.
//! - **An ice shell** over a body whose ocean froze (an `H2O` lid): its ices alone.
//! - **A gas envelope's gas at 1 bar** (P14.T52.d's composition), which a scoop takes.
//!
//! The layer's depth or mass is open (the life brainstorm's request 18): every fraction is
//! per unit mass of the layer, whatever its depth.
//!
//! # The mean
//!
//! A solid layer is a mix of reservoirs, each with a column of concentrations at the solar
//! \[Fe/H\] and \[α/Fe\] and the present Solar System's age:
//!
//! - an undifferentiated body's metal, CI chondrites' siderophiles (`METAL`): the iron group and
//!   the platinum group in CI's proportion;
//! - an undifferentiated body's rock, CI chondrites' rock less their metal, water and nitrogen
//!   (`CI_ROCK`), whose carbon is organic matter;
//! - a differentiated body's silicate, the bulk silicate Earth (`BULK_SILICATE`), its rare
//!   earths CI's in proportion to its aluminium, as refractory lithophiles are.
//!
//! Each metal group is its reservoirs' shares times their concentrations, scaled by its host's
//! abundances by each element's ratio to the rock's α elements (P14.T26's ratios): a host's
//! \[Fe/H\] sets how much rock forms, not what a kilogram of it holds, so no 10^\[Fe/H\] is applied,
//! as the crust's own bulk iron has it
//! ([`bulk_iron_mass_fraction`](crate::planetary::hooks::crust::bulk_iron_mass_fraction)). With
//! s = 10^(−\[α/Fe\]): a bulk's iron and platinum groups are its own metal's; the α lithophiles,
//! uranium and thorium keep the column's ratio to the rock, the actinides being r-process
//! elements, which run with the α elements in the disc as europium does (Frank, Meyer and Mojzsis
//! 2014, Icarus 243, 274, Eu as their proxy; not read); the other light lithophiles, the rare
//! earths (with Al) and ⁴⁰K (with K) take s. A crust's iron is its own mantle's `FeO`
//! ([`mantle_iron_oxide_mass_fraction`]), and its V, Cr, Mn, Co, Ni and platinum group, the bulk
//! silicate Earth's, take s. The plan's own scale, times 10^\[Fe/H\] and for the α elements among
//! the light lithophiles (Mg, Ca, Ti) 10^(\[Fe/H\] + \[α/Fe\]), was version 26's and before.
//!
//! The radioactives' isotopes are each decayed from the present Solar System's age to the body's,
//! by its own half-life ([`RadiogenicIsotope`]). The light lithophiles, a rock's major metals, take
//! the scale on their odds against the rest of the rock rather than on their share: ℓ = `ℓ_S` ÷
//! (1 + `ℓ_S` − ℓ₀), `ℓ_S` the scaled share and ℓ₀ the solar one, which a trace group would not
//! tell apart: only their non-α tenth is scaled, and the odds move the group by under 2%.
//! Silicates are the rock's rest, its silicon, oxygen and the elements the list omits, so that a
//! metal-poor host's rock is poorer in metals and richer in silicates and the layer's parts sum to
//! it. Where a metal-rich host's metals would exceed the layer's solids, they are scaled down
//! together to fill them, which no host's metals reach.
//!
//! The ices are the bulk's water, with, for a body formed beyond the snow line, its carbon dioxide
//! and its nitrogen as ammonia in the proportion its element inventory holds them to its water.
//! Deuterium is the water's at Vienna Standard Mean Ocean Water's D/H, counted apart from the
//! water ice. Hydrocarbons are only an undifferentiated body's formed beyond the snow line: CI's
//! carbon per unit of its rock, times its host's C/O over the Sun's.
//!
//! A gas layer is its species by mass: the iron-group and light lithophile elements of its species
//! by element, its water, its methane, ammonia and carbon dioxide, and its hydrocarbons of two or
//! more carbon atoms by species; helium-3 its helium's at the protosolar ³He/⁴He, and deuterium its
//! molecular hydrogen's at the protosolar D/H with its water's at the ocean's. Its metallicity is
//! already the envelope's ([`EnvelopeAbundances`](crate::planetary::chem::envelope_abundances::EnvelopeAbundances)),
//! so no scale is applied.
//!
//! # The draws
//!
//! One rank per resource on [`tags::BODY_RESOURCES`], keyed by the body ([`ResourceDraws`]), gives
//! a multiple w = 10^(σ z) of median 1, σ [`RESOURCE_SCATTER_DEX`] and z the rank's standard
//! normal. It multiplies the resource's odds against the rest of the layer, x ÷ (1 − x) = w m ÷
//! (1 − m), so x = m w ÷ (1 + m (w − 1)): a minor resource scatters by the full σ, x ≈ m w, while
//! a major one, the silicates or a shell's water, moves less and stays below 1 (to first order its
//! scatter is σ (1 − m)). Where the drawn fractions sum to more than 1, each is divided by their
//! sum. Each fraction lies in 0–1 and they sum to at most 1, to rounding; at the median ranks they
//! are the mean's, to rounding (exactly, unless the mean's own parts sum past 1 by rounding).
//!
//! Multiplying the fractions themselves and closing them over the layer (the compositional
//! closure) was set aside: it scatters a major part against another by σ√2, so that a crust's
//! light lithophile metals, 28% of its rock, reached 69% of it at 2σ, which no silicate rock holds.
//!
//! # Not modelled
//!
//! - The crust's enrichment in incompatible elements: a differentiated body's layer is its whole
//!   silicate, the bulk silicate Earth, whose uranium is 20.3 ppb against the continental crust's
//!   part per million.
//! - An airless surface's implanted helium-3 (the life brainstorm's request 18): zero on
//!   every solid layer, where [`ResourceAbundances::modelled_fraction`] gives none.
//! - The ice giants' deuterium enrichment by their ices: every envelope's hydrogen is protosolar.
//! - Life's organic matter, the abiotic organics of a body whose rock melted, an ordinary
//!   chondrite's carbon, and a Titan's hydrocarbons, which its photochemistry makes at the surface.
//! - The surface's own volatiles, which the climate and escape set: the layer's water is the
//!   bulk's.
//!
//! # Determinism
//!
//! Pure functions of their inputs and the body's [`tags::BODY_RESOURCES`] stream: every sum runs
//! in a fixed order, [`Resource::ALL`]'s and [`RadiogenicIsotope::ALL`]'s, a gas's in its
//! composition's.

#[cfg(test)]
mod tests;

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::planetary::chem::solar_carbon_to_oxygen;
use crate::planetary::derive::SnowLineSide;
use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::hooks::BulkComposition;
use crate::planetary::hooks::crust::history::{
    CI_ALUMINIUM_MASS_FRACTION, CI_POTASSIUM_MASS_FRACTION, CI_THORIUM_MASS_FRACTION,
    CI_URANIUM_MASS_FRACTION,
};
use crate::planetary::hooks::crust::{
    CrustLithology, core_share, iron_oxide_per_iron, mantle_iron_oxide_mass_fraction,
};
use crate::planetary::hooks::figures::interior::{
    BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION, BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION,
    BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION, RADIOGENIC_REFERENCE_AGE, RadiogenicIsotope,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::substance::{Element, Substance, SubstanceId};
use crate::units::{Dex, Years};

/// The log-normal scatter of each resource's multiple about its mean, dex: 0.3, plan 14's
/// (P14.T26), for what the model leaves out.
pub const RESOURCE_SCATTER_DEX: f64 = 0.3;

/// Words of [`tags::BODY_RESOURCES`] a body reads or reserves: words 0–10, one rank per resource
/// in [`Resource::ALL`]'s order, then five reserved, for the base and precious metals the life
/// brainstorm's request 17 asks for.
pub const RESOURCE_WORDS: u64 = 16;

/// CI chondrites' hydrogen, kg per kg: 19,700 ppm (Lodders 2010, arXiv:1010.2746, Table 2,
/// Orgueil; read), all counted as water.
pub const CI_HYDROGEN_MASS_FRACTION: f64 = 19_700e-6;

/// CI chondrites' carbon, kg per kg: 34,800 ppm (Lodders 2010, Table 2; read), counted as organic
/// matter's.
pub const CI_CARBON_MASS_FRACTION: f64 = 34_800e-6;

/// CI chondrites' nitrogen, kg per kg: 2,950 ppm (Lodders 2010, Table 2; read), counted with the
/// water as volatile.
pub const CI_NITROGEN_MASS_FRACTION: f64 = 2_950e-6;

/// CI chondrites' iron group, kg per kg: V 54.3, Cr 2,650, Mn 1,930, Fe 185,000, Co 506 and Ni
/// 10,800 ppm (Lodders 2010, Table 2; read).
pub const CI_IRON_GROUP: [f64; 6] = [54.3e-6, 2_650e-6, 1_930e-6, 185_000e-6, 506e-6, 10_800e-6];

/// CI chondrites' platinum group, kg per kg: Ru 0.686, Rh 0.139, Pd 0.558, Os 0.493, Ir 0.469 and
/// Pt 0.947 ppm (Lodders 2010, Table 2; read).
pub const CI_PLATINUM_GROUP: [f64; 6] =
    [0.686e-6, 0.139e-6, 0.558e-6, 0.493e-6, 0.469e-6, 0.947e-6];

/// CI chondrites' α elements among the light lithophile metals, kg per kg: Mg 95,800, Ca 9,220 and
/// Ti 451 ppm (Lodders 2010, Table 2; read).
pub const CI_ALPHA_LITHOPHILES: [f64; 3] = [95_800e-6, 9_220e-6, 451e-6];

/// CI chondrites' other light lithophile metals, kg per kg: Li 1.47, Be 0.0210 and Na 4,990 ppm
/// (Lodders 2010, Table 2; read), with Al and K ([`CI_ALUMINIUM_MASS_FRACTION`],
/// [`CI_POTASSIUM_MASS_FRACTION`]), K less its ⁴⁰K.
pub const CI_OTHER_LITHOPHILES: [f64; 3] = [1.47e-6, 0.0210e-6, 4_990e-6];

/// CI chondrites' rare earths, kg per kg, IUPAC's seventeen less promethium: Sc 5.9, Y 1.53, La
/// 0.242, Ce 0.622, Pr 0.0946, Nd 0.471, Sm 0.152, Eu 0.0578, Gd 0.205, Tb 0.0384, Dy 0.255, Ho
/// 0.0572, Er 0.163, Tm 0.0261, Yb 0.169 and Lu 0.0253 ppm (Lodders 2010, Table 2; read).
pub const CI_RARE_EARTHS: [f64; 16] = [
    5.9e-6, 1.53e-6, 0.242e-6, 0.622e-6, 0.0946e-6, 0.471e-6, 0.152e-6, 0.0578e-6, 0.205e-6,
    0.0384e-6, 0.255e-6, 0.0572e-6, 0.163e-6, 0.0261e-6, 0.169e-6, 0.0253e-6,
];

/// The bulk silicate Earth's iron group, kg per kg: V 82, Cr 2,625, Mn 1,045, Fe 62,600, Co 105
/// and Ni 1,960 ppm, McDonough and Sun's (1995, Chem. Geol. 120, 223) pyrolite; Cr, Mn, Fe and Ni
/// as the `ultramafic` row's citation quotes the `pyrolite` package's table of it, V and Co from
/// the lane's memory of their Table 5, not re-read.
pub const BULK_SILICATE_EARTH_IRON_GROUP: [f64; 6] =
    [82e-6, 2_625e-6, 1_045e-6, 62_600e-6, 105e-6, 1_960e-6];

/// The bulk silicate Earth's iron group less its iron, kg per kg: its V, Cr, Mn, Co and Ni
/// ([`BULK_SILICATE_EARTH_IRON_GROUP`]), 5,817 ppm, which a silicate crust holds beside its own
/// mantle's iron.
const BULK_SILICATE_EARTH_IRON_TRACES: f64 = {
    let [v, cr, mn, _, co, ni] = BULK_SILICATE_EARTH_IRON_GROUP;
    v + cr + mn + co + ni
};

/// The bulk silicate Earth's platinum group, kg per kg: Ru 5.0, Rh 0.9, Pd 3.9, Os 3.4, Ir 3.2 and
/// Pt 7.1 ppb (McDonough and Sun 1995, Table 5; the lane's memory, not re-read), about 1/140 of
/// CI's: mostly the late veneer accreted after the core formed (Day, Pearson and Taylor 2007,
/// Science 315, 217; not read), which this model gives every differentiated body as Earth's.
pub const BULK_SILICATE_EARTH_PLATINUM_GROUP: [f64; 6] =
    [5.0e-9, 0.9e-9, 3.9e-9, 3.4e-9, 3.2e-9, 7.1e-9];

/// The bulk silicate Earth's α elements among the light lithophile metals, kg per kg: Mg 228,000,
/// Ca 25,300 and Ti 1,205 ppm (McDonough and Sun 1995, as the `ultramafic` row's citation quotes
/// the `pyrolite` package's table).
pub const BULK_SILICATE_EARTH_ALPHA_LITHOPHILES: [f64; 3] = [228_000e-6, 25_300e-6, 1_205e-6];

/// The bulk silicate Earth's other light lithophile metals, kg per kg: Li 1.6 and Be 0.068 ppm
/// (McDonough and Sun 1995, Table 5; the lane's memory, not re-read), and Na 2,670 and Al 23,500
/// ppm (as the `ultramafic` row's citation quotes them), with K
/// ([`BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION`]) less its ⁴⁰K.
pub const BULK_SILICATE_EARTH_OTHER_LITHOPHILES: [f64; 4] = [1.6e-6, 0.068e-6, 2_670e-6, 23_500e-6];

/// The bulk silicate Earth's aluminium, kg per kg: 23,500 ppm (McDonough and Sun 1995, as the
/// `ultramafic` row's citation quotes it), the scale of its rare earths to CI's.
pub const BULK_SILICATE_EARTH_ALUMINIUM_MASS_FRACTION: f64 = 23_500e-6;

/// The protosolar share of helium atoms that are ³He: 0.0166% (Lodders 2010, Table 8, "Solar
/// system nuclide abundances 4.56 Gy ago"; read), Jupiter's ³He/⁴He of 1.66 × 10⁻⁴ (Mahaffy et al.
/// 1998, via the life brainstorm's "Gas-giant fuel").
pub const PROTOSOLAR_HELIUM_3_ATOM_SHARE: f64 = 0.0166e-2;

/// The protosolar share of hydrogen atoms that are deuterium: 0.00194% (Lodders 2010, Table 8;
/// read). Jupiter's 2.6 × 10⁻⁵ (Mahaffy et al. 1998) agrees within its error; Uranus's 4.4 and
/// Neptune's 4.1 × 10⁻⁵ (Feuchtgruber et al. 2013) are not reached ([module](self)'s "Not
/// modelled").
pub const PROTOSOLAR_DEUTERIUM_ATOM_SHARE: f64 = 0.001_94e-2;

/// Vienna Standard Mean Ocean Water's D/H by number: 155.76 × 10⁻⁶ (Hagemann, Nief and Roth 1970,
/// Tellus 22, 712; the lane's memory, not re-read), the water's.
pub const OCEAN_DEUTERIUM_TO_HYDROGEN: f64 = 155.76e-6;

/// The atomic mass of ¹H, g mol⁻¹: 1.007 825 (AME2020, Wang et al. 2021, Chin. Phys. C 45,
/// 030003; the lane's memory, not re-read).
const PROTIUM_G_PER_MOL: f64 = 1.007_825;

/// The atomic mass of deuterium, g mol⁻¹: 2.014 102 (AME2020; the lane's memory, not re-read).
const DEUTERIUM_G_PER_MOL: f64 = 2.014_102;

/// The atomic mass of ³He, g mol⁻¹: 3.016 029 (AME2020; the lane's memory, not re-read).
const HELIUM_3_G_PER_MOL: f64 = 3.016_029;

/// The atomic mass of ⁴He, g mol⁻¹: 4.002 603 (AME2020; the lane's memory, not re-read).
const HELIUM_4_G_PER_MOL: f64 = 4.002_603;

/// One resource of the closed list (plan 14, P14.T26), each a mass fraction of a body's accessible
/// outer layers. The groups are disjoint, so that their fractions sum to at most 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resource {
    /// The iron group, the iron-peak metals V, Cr, Mn, Fe, Co and Ni, as elements.
    IronGroup,
    /// The light lithophile metals Li, Be, Na, Mg, Al, K (less its ⁴⁰K), Ca and Ti, as elements.
    LightLithophiles,
    /// The rare earths, Sc, Y and the lanthanides, as elements.
    RareEarths,
    /// The platinum group, Ru, Rh, Pd, Os, Ir and Pt, as elements.
    PlatinumGroup,
    /// The radioactives, ²³⁸U, ²³⁵U, ²³²Th and ⁴⁰K, as isotopes ([`RadiogenicIsotope`]).
    Radioactives,
    /// The silicates: a rock's mass less the metals the other groups count, its silicon, its
    /// oxygen and the elements the list omits.
    Silicates,
    /// Water, frozen or liquid, less its deuterium.
    WaterIce,
    /// The other ices, CO₂, NH₃ and CH₄.
    OtherIces,
    /// The hydrocarbons: a rock's organic matter, by its carbon, or a gas's hydrocarbons of two or
    /// more carbon atoms.
    Hydrocarbons,
    /// Helium-3, as the isotope.
    Helium3,
    /// Deuterium, as the isotope.
    Deuterium,
}

impl Resource {
    /// The list, in the order the draws and every sum over it run.
    pub const ALL: [Self; 11] = [
        Self::IronGroup,
        Self::LightLithophiles,
        Self::RareEarths,
        Self::PlatinumGroup,
        Self::Radioactives,
        Self::Silicates,
        Self::WaterIce,
        Self::OtherIces,
        Self::Hydrocarbons,
        Self::Helium3,
        Self::Deuterium,
    ];

    /// Its name, for tests and goldens.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::IronGroup => "iron_group",
            Self::LightLithophiles => "light_lithophiles",
            Self::RareEarths => "rare_earths",
            Self::PlatinumGroup => "platinum_group",
            Self::Radioactives => "radioactives",
            Self::Silicates => "silicates",
            Self::WaterIce => "water_ice",
            Self::OtherIces => "other_ices",
            Self::Hydrocarbons => "hydrocarbons",
            Self::Helium3 => "helium_3",
            Self::Deuterium => "deuterium",
        }
    }

    /// Its place in [`ALL`](Self::ALL), and its word of [`tags::BODY_RESOURCES`].
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::IronGroup => 0,
            Self::LightLithophiles => 1,
            Self::RareEarths => 2,
            Self::PlatinumGroup => 3,
            Self::Radioactives => 4,
            Self::Silicates => 5,
            Self::WaterIce => 6,
            Self::OtherIces => 7,
            Self::Hydrocarbons => 8,
            Self::Helium3 => 9,
            Self::Deuterium => 10,
        }
    }
}

/// The number of resources.
const RESOURCES: usize = Resource::ALL.len();

/// What a body's accessible outer layers are (the [module](self)'s list).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OuterLayer<'a> {
    /// An undifferentiated body's bulk: its iron, rock and ices.
    Bulk,
    /// A differentiated body's silicate, with its water: its metal is in its core.
    SilicateCrust,
    /// A frozen ocean's shell: its ices alone.
    IceShell,
    /// A gas envelope's gas at 1 bar, by mole fraction summing to 1, as
    /// [`EnvelopeAtmosphere::composition`](crate::planetary::hooks::EnvelopeAtmosphere::composition)
    /// gives it.
    Gas(&'a [(SubstanceId, f64)]),
}

impl OuterLayer<'_> {
    /// The layer of a solid body whose lid, the lithology at its surface, is `lid` (P14.T51.c's
    /// [`CrustComposition::lid_lithology`](crate::planetary::hooks::CrustComposition::lid_lithology)),
    /// by an exhaustive match with no wildcard, so that a new lithology fails to compile until it is
    /// placed: an ice shell for `H2O`; the bulk for an undifferentiated body's own material and a
    /// frozen ocean's rock-laden shell (`phyllosilicate`), whose rock never melted; a silicate crust
    /// for every crust of a body whose rock melted.
    #[must_use]
    pub const fn of_lid(lid: CrustLithology) -> Self {
        match lid {
            CrustLithology::Water => Self::IceShell,
            CrustLithology::Phyllosilicate
            | CrustLithology::ChondriteCi
            | CrustLithology::ChondriteCm
            | CrustLithology::ChondriteCv
            | CrustLithology::ChondriteH
            | CrustLithology::ChondriteL
            | CrustLithology::ChondriteLl
            | CrustLithology::ChondriteE
            | CrustLithology::Cometary => Self::Bulk,
            CrustLithology::Graphite
            | CrustLithology::Feldspathic
            | CrustLithology::Basalt
            | CrustLithology::IronRichBasalt
            | CrustLithology::Komatiite
            | CrustLithology::ReducedBasalt
            | CrustLithology::Granite
            | CrustLithology::Ultramafic => Self::SilicateCrust,
        }
    }
}

/// Everything [`ResourceAbundances::derive`] reads of a body, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceInputs<'a> {
    /// Its bulk composition (P14.T23): its mass fractions, its element inventory, whose carriers
    /// give its other ices, and its host's \[Fe/H\] and \[α/Fe\], an absent \[α/Fe\] read as 0.
    pub bulk: &'a BulkComposition,
    /// The side of its host's snow line it formed on, which decides whether its rock holds organic
    /// matter and its ices carbon dioxide and ammonia.
    pub formed: SnowLineSide,
    /// Its accessible outer layers.
    pub layer: OuterLayer<'a>,
    /// Its age at the time, to which its radioactives are decayed.
    pub age: Years,
    /// Its host's Mg/Si by number (P14.T50.a's
    /// [`HostAbundances::magnesium_to_silicon`](crate::planetary::chem::HostAbundances::magnesium_to_silicon),
    /// as its crust reads it), positive: a silicate crust's iron is its mantle's `FeO`, which reads
    /// it.
    pub host_magnesium_to_silicon: f64,
    /// Its host's C/O by number
    /// ([`HostAbundances::carbon_to_oxygen`](crate::planetary::chem::HostAbundances::carbon_to_oxygen)),
    /// positive: a bulk's hydrocarbons scale with it over the Sun's.
    pub host_carbon_to_oxygen: f64,
}

/// The ranks of a body's resource multiples, as drawn from [`tags::BODY_RESOURCES`] by
/// [`ResourceDraws::for_body`], or given explicitly by a test or a tool.
///
/// The ranks are plain values with no invariant between them, so they are public, as the
/// volatiles' draws are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceDraws {
    /// One rank per resource, in [`Resource::ALL`]'s order: word i of the stream.
    pub ranks: [UnitUniform; RESOURCES],
}

impl ResourceDraws {
    /// Every rank at its median, every multiple 1: the mean.
    pub const MEDIAN: Self = Self {
        ranks: [UnitUniform::HALF; RESOURCES],
    };

    /// The draws of the body `body` in the universe of `seed`: words 0–10 of its
    /// [`tags::BODY_RESOURCES`] stream, one open uniform each, in turn.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly between 0 and 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::{BodyId, SystemId};
    /// use hyperion_sim::planetary::hooks::resources::ResourceDraws;
    ///
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let (b, c) = (BodyId::new(system, 0x0100), BodyId::new(system, 0x0200));
    /// let seed = Seed::new(7);
    /// assert_eq!(ResourceDraws::for_body(seed, b), ResourceDraws::for_body(seed, b));
    /// assert_ne!(ResourceDraws::for_body(seed, b), ResourceDraws::for_body(seed, c));
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    #[must_use]
    pub fn for_body(seed: Seed, body: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::BODY_RESOURCES, ObjectKey::from(body));
        let mut ranks = [UnitUniform::HALF; RESOURCES];
        for rank in &mut ranks {
            *rank = UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1");
        }
        Self { ranks }
    }

    /// The multiple of `resource`'s mean: 10^(σ z), σ [`RESOURCE_SCATTER_DEX`] and z its rank's
    /// standard normal; 1 at the median rank.
    #[must_use]
    pub fn multiple(&self, resource: Resource) -> f64 {
        let rank = self.ranks[resource.index()].value();
        math::exp10(RESOURCE_SCATTER_DEX * math::normal_quantile(rank))
    }
}

/// A body's resource abundances (P14.T26): a mass fraction of its accessible outer layers for
/// each [`Resource`], each in 0–1 and summing to at most 1 (the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ResourceAbundances {
    fractions: [f64; RESOURCES],
    radioactive: [f64; 4],
    phase: LayerPhase,
}

/// Whether a body's accessible outer layers are solid or a gas envelope's gas, which decides what
/// the model leaves out of them (the [module](self)'s "Not modelled").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum LayerPhase {
    /// An undifferentiated body's bulk, a silicate crust or an ice shell.
    Solid,
    /// A gas envelope's gas at 1 bar.
    Gas,
}

impl ResourceAbundances {
    /// The abundances of the body `inputs` describe, at the ranks `draws`: the [module](self)'s
    /// mean, each resource's odds against the rest of the layer times its multiple, divided by
    /// their sum where they exceed 1.
    ///
    /// Finite inputs give finite fractions; an empty gas, or a solid body of no mass, gives none.
    ///
    /// # Examples
    ///
    /// A differentiated rocky body keeps a few parts per billion of platinum-group metals in its
    /// crust, where an undifferentiated one of the same composition holds them at a chondrite's
    /// parts per million:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::inventory::{InventoryInputs, element_inventory};
    /// use hyperion_sim::planetary::chem::{solar_carbon_to_oxygen, solar_magnesium_to_silicon};
    /// use hyperion_sim::planetary::derive::atmosphere::VolatileDraws;
    /// use hyperion_sim::planetary::derive::{SnowLineSide, composition};
    /// use hyperion_sim::planetary::hooks::BulkComposition;
    /// use hyperion_sim::planetary::hooks::resources::{
    ///     OuterLayer, Resource, ResourceAbundances, ResourceDraws, ResourceInputs,
    /// };
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    /// use hyperion_sim::units::{Dex, EarthFluxes, EarthMasses, EarthRadii, Metres, Years};
    ///
    /// let mass = EarthMasses::new(0.01);
    /// let side = SnowLineSide::Inside;
    /// let fractions = composition(mass, EarthRadii::new(0.24), side, EarthFluxes::new(0.2))?
    ///     .fractions();
    /// let elements = element_inventory(&InventoryInputs {
    ///     mass,
    ///     fractions,
    ///     formed: side,
    ///     formation_distance: Metres::new(2.4 * METRES_PER_AU),
    ///     snow_line: Metres::new(2.7 * METRES_PER_AU),
    ///     draws: VolatileDraws::MEDIAN,
    ///     age: Years::new(4.57e9),
    /// });
    /// let bulk = BulkComposition::new(fractions, elements, Dex::ZERO, Some(Dex::ZERO));
    /// let of = |layer| {
    ///     let inputs = ResourceInputs {
    ///         bulk: &bulk,
    ///         formed: side,
    ///         layer,
    ///         age: Years::new(4.57e9),
    ///         host_magnesium_to_silicon: solar_magnesium_to_silicon(),
    ///         host_carbon_to_oxygen: solar_carbon_to_oxygen(),
    ///     };
    ///     ResourceAbundances::derive(&inputs, &ResourceDraws::MEDIAN)
    /// };
    /// let crust = of(OuterLayer::SilicateCrust).fraction(Resource::PlatinumGroup);
    /// let undifferentiated = of(OuterLayer::Bulk).fraction(Resource::PlatinumGroup);
    /// assert!((1e-9..1e-7).contains(&crust));
    /// assert!((1e-7..1e-5).contains(&undifferentiated));
    /// # Ok::<(), hyperion_sim::planetary::derive::composition::SolveCompositionError>(())
    /// ```
    #[must_use]
    pub fn derive(inputs: &ResourceInputs<'_>, draws: &ResourceDraws) -> Self {
        let (mean, phase) = match inputs.layer {
            OuterLayer::Bulk => (solid_mean(inputs, Solid::Bulk), LayerPhase::Solid),
            OuterLayer::SilicateCrust => {
                (solid_mean(inputs, Solid::SilicateCrust), LayerPhase::Solid)
            }
            OuterLayer::IceShell => (solid_mean(inputs, Solid::IceShell), LayerPhase::Solid),
            OuterLayer::Gas(composition) => (gas_mean(composition), LayerPhase::Gas),
        };
        mean.closed(draws, phase)
    }

    /// The fraction of `resource`, kg per kg of the layer, 0 to 1.
    #[must_use]
    pub const fn fraction(&self, resource: Resource) -> f64 {
        self.fractions[resource.index()]
    }

    /// The fraction of `resource` as [`fraction`](Self::fraction) gives it where the model
    /// computes it, and `None` where the layer's 0 stands for a source the model leaves out (the
    /// [module](self)'s "Not modelled"). So far that is helium-3 on a solid layer, whose implanted
    /// helium the model does not compute (the life brainstorm's request 18). A reader that shows
    /// the fraction shows that one as unknown, never as none.
    #[must_use]
    pub const fn modelled_fraction(&self, resource: Resource) -> Option<f64> {
        match (resource, self.phase) {
            (Resource::Helium3, LayerPhase::Solid) => None,
            (Resource::Helium3, LayerPhase::Gas)
            | (
                Resource::IronGroup
                | Resource::LightLithophiles
                | Resource::RareEarths
                | Resource::PlatinumGroup
                | Resource::Radioactives
                | Resource::Silicates
                | Resource::WaterIce
                | Resource::OtherIces
                | Resource::Hydrocarbons
                | Resource::Deuterium,
                LayerPhase::Solid | LayerPhase::Gas,
            ) => Some(self.fraction(resource)),
        }
    }

    /// Every resource with its fraction, in [`Resource::ALL`]'s order.
    pub fn iter(&self) -> impl Iterator<Item = (Resource, f64)> {
        Resource::ALL.into_iter().zip(self.fractions)
    }

    /// The fraction of the radioactive isotope `isotope`, kg per kg of the layer: its share of
    /// [`Resource::Radioactives`], whose fraction the four sum to, to rounding.
    #[must_use]
    pub const fn radioactive(&self, isotope: RadiogenicIsotope) -> f64 {
        self.radioactive[isotope_index(isotope)]
    }

    /// The fractions' sum, in [`Resource::ALL`]'s order: at most 1, to rounding.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.fractions.iter().fold(0.0, |sum, &x| sum + x)
    }
}

/// A [`RadiogenicIsotope`]'s place in [`RadiogenicIsotope::ALL`].
#[must_use]
const fn isotope_index(isotope: RadiogenicIsotope) -> usize {
    match isotope {
        RadiogenicIsotope::Uranium238 => 0,
        RadiogenicIsotope::Uranium235 => 1,
        RadiogenicIsotope::Thorium232 => 2,
        RadiogenicIsotope::Potassium40 => 3,
    }
}

/// The fractions before the draws: the [module](self)'s mean.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mean {
    fractions: [f64; RESOURCES],
    radioactive: [f64; 4],
}

impl Mean {
    /// No resource at all: an empty layer.
    const NONE: Self = Self {
        fractions: [0.0; RESOURCES],
        radioactive: [0.0; 4],
    };

    /// The mean with each resource's odds against the rest of the layer times its multiple, x = m
    /// w ÷ (1 + m (w − 1)), each isotope of the radioactives as its group; then, where the
    /// fractions sum to more than 1, each over their sum (the [module](self)'s "The draws"). At w
    /// = 1 each is m to the bit, its denominator 1 exactly. The abundances are of a layer of `phase`.
    #[must_use]
    fn closed(&self, draws: &ResourceDraws, phase: LayerPhase) -> ResourceAbundances {
        let mut fractions = [0.0; RESOURCES];
        let mut denominators = [1.0; RESOURCES];
        for (resource, (x, denominator)) in Resource::ALL
            .into_iter()
            .zip(fractions.iter_mut().zip(denominators.iter_mut()))
        {
            let (m, w) = (self.fractions[resource.index()], draws.multiple(resource));
            // m in [0, 1] and w > 0, so the denominator lies between w and 1, and is positive.
            *denominator = 1.0 + m * (w - 1.0);
            *x = m * w / *denominator;
        }
        let total = fractions.iter().fold(0.0, |sum, &x| sum + x);
        let closure = if total > 1.0 { total } else { 1.0 };
        let index = Resource::Radioactives.index();
        let (w, denominator) = (draws.multiple(Resource::Radioactives), denominators[index]);
        ResourceAbundances {
            fractions: fractions.map(|x| x / closure),
            radioactive: self.radioactive.map(|m| m * w / denominator / closure),
            phase,
        }
    }
}

/// A reservoir's concentrations of the list's metals and carbon, kg per kg of the reservoir, at
/// the solar \[Fe/H\] and \[α/Fe\] and, for its radioactive elements, the present Solar System's
/// age ([`RADIOGENIC_REFERENCE_AGE`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct Column {
    iron_group: f64,
    platinum_group: f64,
    alpha_lithophiles: f64,
    other_lithophiles: f64,
    rare_earths: f64,
    uranium: f64,
    thorium: f64,
    potassium: f64,
    carbon: f64,
}

impl Column {
    /// The radioactive isotope `isotope`'s concentration at the present Solar System's age.
    #[must_use]
    pub(crate) const fn isotope(&self, isotope: RadiogenicIsotope) -> f64 {
        let element = match isotope {
            RadiogenicIsotope::Uranium238 | RadiogenicIsotope::Uranium235 => self.uranium,
            RadiogenicIsotope::Thorium232 => self.thorium,
            RadiogenicIsotope::Potassium40 => self.potassium,
        };
        element * isotope.share_of_element()
    }
}

/// The values' sum, in their order.
#[must_use]
const fn sum(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut i = 0;
    while i < values.len() {
        total += values[i];
        i += 1;
    }
    total
}

/// CI chondrites' water, kg per kg: their hydrogen ([`CI_HYDROGEN_MASS_FRACTION`]) all as H₂O, by
/// the registry's atomic weights.
pub const CI_WATER_MASS_FRACTION: f64 = {
    let hydrogen = 2.0 * Element::H.standard_atomic_weight_g_per_mol();
    CI_HYDROGEN_MASS_FRACTION * (hydrogen + Element::O.standard_atomic_weight_g_per_mol())
        / hydrogen
};

/// CI chondrites' metal, kg per kg: their iron and platinum groups.
pub const CI_METAL_MASS_FRACTION: f64 = sum(&CI_IRON_GROUP) + sum(&CI_PLATINUM_GROUP);

/// CI chondrites' rock, kg per kg: what is left of them less their water, their nitrogen and their
/// metal, 0.620.
pub const CI_ROCK_MASS_FRACTION: f64 =
    1.0 - CI_WATER_MASS_FRACTION - CI_NITROGEN_MASS_FRACTION - CI_METAL_MASS_FRACTION;

/// The mass share of potassium that is not ⁴⁰K, about 0.999 88, which the light lithophiles keep:
/// its ⁴⁰K goes to the radioactives.
const POTASSIUM_STABLE_SHARE: f64 = 1.0 - RadiogenicIsotope::Potassium40.share_of_element();

/// An undifferentiated body's metal: CI chondrites' iron and platinum groups in their proportion,
/// summing to 1.
pub(crate) const METAL: Column = Column {
    iron_group: sum(&CI_IRON_GROUP) / CI_METAL_MASS_FRACTION,
    platinum_group: sum(&CI_PLATINUM_GROUP) / CI_METAL_MASS_FRACTION,
    alpha_lithophiles: 0.0,
    other_lithophiles: 0.0,
    rare_earths: 0.0,
    uranium: 0.0,
    thorium: 0.0,
    potassium: 0.0,
    carbon: 0.0,
};

/// An undifferentiated body's rock: CI chondrites' lithophiles and carbon per unit of their rock
/// ([`CI_ROCK_MASS_FRACTION`]), with their uranium, thorium and potassium as measured today.
pub(crate) const CI_ROCK: Column = Column {
    iron_group: 0.0,
    platinum_group: 0.0,
    alpha_lithophiles: sum(&CI_ALPHA_LITHOPHILES) / CI_ROCK_MASS_FRACTION,
    other_lithophiles: (sum(&CI_OTHER_LITHOPHILES)
        + CI_ALUMINIUM_MASS_FRACTION
        + CI_POTASSIUM_MASS_FRACTION * POTASSIUM_STABLE_SHARE)
        / CI_ROCK_MASS_FRACTION,
    rare_earths: sum(&CI_RARE_EARTHS) / CI_ROCK_MASS_FRACTION,
    uranium: CI_URANIUM_MASS_FRACTION / CI_ROCK_MASS_FRACTION,
    thorium: CI_THORIUM_MASS_FRACTION / CI_ROCK_MASS_FRACTION,
    potassium: CI_POTASSIUM_MASS_FRACTION / CI_ROCK_MASS_FRACTION,
    carbon: CI_CARBON_MASS_FRACTION / CI_ROCK_MASS_FRACTION,
};

/// A differentiated body's silicate: the bulk silicate Earth, its rare earths CI's times its
/// aluminium over theirs (refractory lithophiles in chondritic proportion, McDonough and Sun 1995),
/// and no carbon.
pub(crate) const BULK_SILICATE: Column = Column {
    iron_group: sum(&BULK_SILICATE_EARTH_IRON_GROUP),
    platinum_group: sum(&BULK_SILICATE_EARTH_PLATINUM_GROUP),
    alpha_lithophiles: sum(&BULK_SILICATE_EARTH_ALPHA_LITHOPHILES),
    other_lithophiles: sum(&BULK_SILICATE_EARTH_OTHER_LITHOPHILES)
        + BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION * POTASSIUM_STABLE_SHARE,
    rare_earths: sum(&CI_RARE_EARTHS) * BULK_SILICATE_EARTH_ALUMINIUM_MASS_FRACTION
        / CI_ALUMINIUM_MASS_FRACTION,
    uranium: BULK_SILICATE_EARTH_URANIUM_MASS_FRACTION,
    thorium: BULK_SILICATE_EARTH_THORIUM_MASS_FRACTION,
    potassium: BULK_SILICATE_EARTH_POTASSIUM_MASS_FRACTION,
    carbon: 0.0,
};

/// The share of an isotope's atoms `atom_share` of mass `isotope_g_per_mol` beside the rest's,
/// `rest_g_per_mol`, by mass.
#[must_use]
fn mass_share(atom_share: f64, isotope_g_per_mol: f64, rest_g_per_mol: f64) -> f64 {
    let isotope = atom_share * isotope_g_per_mol;
    isotope / (isotope + (1.0 - atom_share) * rest_g_per_mol)
}

/// Deuterium's share of water's mass at the ocean's D/H ([`OCEAN_DEUTERIUM_TO_HYDROGEN`]):
/// 3.48 × 10⁻⁵.
#[must_use]
fn water_deuterium_share() -> f64 {
    let d = OCEAN_DEUTERIUM_TO_HYDROGEN / (1.0 + OCEAN_DEUTERIUM_TO_HYDROGEN);
    let deuterium = 2.0 * d * DEUTERIUM_G_PER_MOL;
    let hydrogen = 2.0 * (1.0 - d) * PROTIUM_G_PER_MOL;
    deuterium / (deuterium + hydrogen + Element::O.standard_atomic_weight_g_per_mol())
}

/// A solid [`OuterLayer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Solid {
    /// [`OuterLayer::Bulk`].
    Bulk,
    /// [`OuterLayer::SilicateCrust`].
    SilicateCrust,
    /// [`OuterLayer::IceShell`].
    IceShell,
}

/// The mean of the solid layer `layer` of the body `inputs` describe (the [module](self)'s "The
/// mean").
#[must_use]
fn solid_mean(inputs: &ResourceInputs<'_>, layer: Solid) -> Mean {
    let bulk = inputs.bulk;
    let fractions = bulk.fractions();
    let solids = fractions.iron() + fractions.rock() + fractions.water();
    if solids.is_nan() || solids <= 0.0 {
        return Mean::NONE;
    }
    let beyond = inputs.formed == SnowLineSide::Beyond;
    // The bulk's ices, and their split between water and the other ices.
    let ices = fractions.water() / solids;
    let others_per_water = if beyond {
        other_ices_per_water(bulk)
    } else {
        0.0
    };
    let (metal, rock, column, ices) = match layer {
        Solid::Bulk => (
            fractions.iron() / solids,
            fractions.rock() / solids,
            &CI_ROCK,
            ices,
        ),
        Solid::SilicateCrust => (0.0, 1.0 - ices, &BULK_SILICATE, ices),
        Solid::IceShell => (0.0, 0.0, &BULK_SILICATE, 1.0),
    };
    let reservoirs = Reservoirs {
        layer,
        metal,
        rock,
        column,
        since: RADIOGENIC_REFERENCE_AGE.value() - inputs.age.value(),
        organic: beyond && layer == Solid::Bulk,
    };
    let Metals {
        mut iron_group,
        mut platinum_group,
        mut lithophiles,
        mut rare_earths,
        mut radioactive,
        hydrocarbons,
    } = ratio_metals(inputs, &reservoirs);

    // The solids the metals and the silicates share; a metal-rich host's metals fill them at most.
    let available = metal + rock - hydrocarbons;
    let radioactives = radioactive.iter().fold(0.0, |sum, &x| sum + x);
    let metals = iron_group + platinum_group + lithophiles + rare_earths + radioactives;
    if metals > available {
        let fill = available / metals;
        iron_group *= fill;
        platinum_group *= fill;
        lithophiles *= fill;
        rare_earths *= fill;
        radioactive = radioactive.map(|x| x * fill);
    }
    let radioactives = radioactive.iter().fold(0.0, |sum, &x| sum + x);
    let rest = available - (iron_group + platinum_group + lithophiles + rare_earths + radioactives);
    // Written as a comparison rather than `max`, so that a signed zero or a NaN gives +0 on every
    // target (the sim-determinism skill's "min and max are not exact at a signed zero").
    let silicates = if rest > 0.0 { rest } else { 0.0 };

    let water = ices / (1.0 + others_per_water);
    let other_ices = ices - water;
    let deuterium = water * water_deuterium_share();

    let mut mean = Mean::NONE;
    mean.fractions[Resource::IronGroup.index()] = iron_group;
    mean.fractions[Resource::LightLithophiles.index()] = lithophiles;
    mean.fractions[Resource::RareEarths.index()] = rare_earths;
    mean.fractions[Resource::PlatinumGroup.index()] = platinum_group;
    mean.fractions[Resource::Radioactives.index()] = radioactives;
    mean.fractions[Resource::Silicates.index()] = silicates;
    mean.fractions[Resource::WaterIce.index()] = water - deuterium;
    mean.fractions[Resource::OtherIces.index()] = other_ices;
    mean.fractions[Resource::Hydrocarbons.index()] = hydrocarbons;
    mean.fractions[Resource::Deuterium.index()] = deuterium;
    mean.radioactive = radioactive;
    mean
}

/// A solid layer's reservoirs, as [`solid_mean`] reads them.
#[derive(Debug, Clone, Copy)]
struct Reservoirs<'c> {
    /// The layer.
    layer: Solid,
    /// Its metal, kg per kg of the layer: an undifferentiated body's iron.
    metal: f64,
    /// Its rock, kg per kg of the layer.
    rock: f64,
    /// The rock's concentrations.
    column: &'c Column,
    /// The years from the body's age to the present Solar System's, over which the radioactives
    /// are grown back.
    since: f64,
    /// Whether its rock holds organic matter: an undifferentiated body's formed beyond the snow
    /// line.
    organic: bool,
}

/// A solid layer's metal groups and hydrocarbons before the fill cap, kg per kg of the layer.
#[derive(Debug, Clone, Copy)]
struct Metals {
    iron_group: f64,
    platinum_group: f64,
    lithophiles: f64,
    rare_earths: f64,
    radioactive: [f64; 4],
    hydrocarbons: f64,
}

/// The metals of `of`, each element at its ratio to the rock's α elements (the [module](self)'s
/// "The mean"), with s = 10^(−\[α/Fe\]) of the host of `inputs` on those not made with them;
/// \[Fe/H\] is not read.
#[must_use]
fn ratio_metals(inputs: &ResourceInputs<'_>, of: &Reservoirs<'_>) -> Metals {
    let bulk = inputs.bulk;
    let (metal, rock, column) = (of.metal, of.rock, of.column);
    let alpha_fe = bulk.alpha_fe().unwrap_or(Dex::ZERO);
    let s = math::exp10(-alpha_fe.value());
    let (iron_group, platinum_group) = match of.layer {
        // Its metal is its own, as its radius gives it.
        Solid::Bulk => (metal * METAL.iron_group, metal * METAL.platinum_group),
        // Its iron is the `FeO` its core left in its mantle; the bulk silicate Earth's other
        // iron-peak metals and its late veneer's platinum group follow the host's iron.
        Solid::SilicateCrust => {
            let feo = mantle_iron_oxide_mass_fraction(
                core_share(&bulk.fractions()),
                inputs.host_magnesium_to_silicon,
                alpha_fe,
            );
            let iron = feo / iron_oxide_per_iron();
            (
                rock * (iron + BULK_SILICATE_EARTH_IRON_TRACES * s),
                rock * column.platinum_group * s,
            )
        }
        Solid::IceShell => (0.0, 0.0),
    };
    // The odds form, on the light lithophiles' non-α share alone: the host's scale multiplies their
    // odds against the rest of the rock, not their share. The difference is written so that at the
    // solar scale the denominator is 1 exactly.
    let solar = column.alpha_lithophiles + column.other_lithophiles;
    let scaled = column.alpha_lithophiles + column.other_lithophiles * s;
    Metals {
        iron_group,
        platinum_group,
        lithophiles: rock * scaled / (1.0 + (scaled - solar)),
        rare_earths: rock * column.rare_earths * s,
        radioactive: RadiogenicIsotope::ALL.map(|isotope| {
            // ⁴⁰K follows its potassium; uranium and thorium keep their ratio to the rock.
            let scale = match isotope {
                RadiogenicIsotope::Potassium40 => s,
                RadiogenicIsotope::Uranium238
                | RadiogenicIsotope::Uranium235
                | RadiogenicIsotope::Thorium232 => 1.0,
            };
            rock * column.isotope(isotope)
                * math::exp(of.since / isotope.mean_life().value())
                * scale
        }),
        hydrocarbons: if of.organic {
            rock * column.carbon * (inputs.host_carbon_to_oxygen / solar_carbon_to_oxygen())
        } else {
            0.0
        },
    }
}

/// The mass of a body's other ices per unit mass of its water: its element inventory's carbon
/// dioxide, its stored carrier, and its nitrogen as ammonia, over its water; 0 for a body with no
/// water.
#[must_use]
fn other_ices_per_water(bulk: &BulkComposition) -> f64 {
    let carriers = bulk.inventory();
    let water = carriers.water().value();
    if water.is_nan() || water <= 0.0 {
        return 0.0;
    }
    let w = Element::standard_atomic_weight_g_per_mol;
    let ammonia_per_nitrogen = (w(Element::N) + 3.0 * w(Element::H)) / w(Element::N);
    (carriers.carbon_dioxide().value() + carriers.nitrogen().value() * ammonia_per_nitrogen) / water
}

/// Which of the list's metal groups an element's mass counts in, in a gas.
#[must_use]
const fn gas_metal(element: Element) -> Option<Resource> {
    match element {
        Element::V | Element::Cr | Element::Mn | Element::Fe | Element::Ni => {
            Some(Resource::IronGroup)
        }
        Element::Na | Element::Mg | Element::Al | Element::K | Element::Ca | Element::Ti => {
            Some(Resource::LightLithophiles)
        }
        Element::H
        | Element::He
        | Element::C
        | Element::N
        | Element::O
        | Element::Ne
        | Element::Si
        | Element::P
        | Element::S
        | Element::Cl
        | Element::Ar
        | Element::Zn
        | Element::Kr
        | Element::Xe => None,
    }
}

/// Whether `substance` is a hydrocarbon of two or more carbon atoms: carbon and hydrogen alone.
#[must_use]
fn is_heavier_hydrocarbon(substance: &Substance) -> bool {
    let elements = substance.elements();
    let carbon = elements
        .iter()
        .find(|(element, _)| *element == Element::C)
        .map_or(0, |&(_, n)| n);
    carbon >= 2
        && elements
            .iter()
            .all(|(element, _)| matches!(element, Element::C | Element::H))
}

/// The mean of a gas layer of composition `composition` by mole fraction (the [module](self)'s
/// "The mean"): no layer at all for an empty gas.
#[must_use]
fn gas_mean(composition: &[(SubstanceId, f64)]) -> Mean {
    let mut mean = Mean::NONE;
    let (mut total, mut hydrogen, mut helium, mut water) = (0.0, 0.0, 0.0, 0.0);
    for &(id, x) in composition {
        let substance = id.substance();
        let mut molar = 0.0;
        for &(element, n) in substance.elements() {
            let mass = f64::from(n) * element.standard_atomic_weight_g_per_mol();
            molar += mass;
            if let Some(group) = gas_metal(element) {
                mean.fractions[group.index()] += x * mass;
            }
        }
        let mass = x * molar;
        total += mass;
        match id.gas() {
            Some(Gas::Hydrogen) => hydrogen += mass,
            Some(Gas::Helium) => helium += mass,
            Some(Gas::Water) => water += mass,
            Some(Gas::Methane | Gas::Ammonia | Gas::CarbonDioxide) => {
                mean.fractions[Resource::OtherIces.index()] += mass;
            }
            Some(Gas::Nitrogen | Gas::Oxygen | Gas::Argon) => {}
            None => {
                if is_heavier_hydrocarbon(substance) {
                    mean.fractions[Resource::Hydrocarbons.index()] += mass;
                }
            }
        }
    }
    if total.is_nan() || total <= 0.0 {
        return Mean::NONE;
    }
    let water_deuterium = water * water_deuterium_share();
    let hydrogen_deuterium = hydrogen
        * mass_share(
            PROTOSOLAR_DEUTERIUM_ATOM_SHARE,
            DEUTERIUM_G_PER_MOL,
            PROTIUM_G_PER_MOL,
        );
    let helium_3 = helium
        * mass_share(
            PROTOSOLAR_HELIUM_3_ATOM_SHARE,
            HELIUM_3_G_PER_MOL,
            HELIUM_4_G_PER_MOL,
        );
    mean.fractions[Resource::WaterIce.index()] = water - water_deuterium;
    mean.fractions[Resource::Helium3.index()] = helium_3;
    mean.fractions[Resource::Deuterium.index()] = hydrogen_deuterium + water_deuterium;
    mean.fractions = mean.fractions.map(|mass| mass / total);
    mean
}
