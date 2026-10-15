//! A body's material palette: the substances its field names, each with its values resolved by
//! the server from plan 14's registry, so that the field stays the client's only input (plan R09,
//! T2's follow-up B, Design note 17; decision-composition §1.7).
//!
//! The header carries the palette ([`FieldHeaderParts::palette`](super::FieldHeaderParts::palette)),
//! the entry of each [`Crust`](super::Crust) variant, whose lithology it names
//! ([`crust_palette`](super::FieldHeaderParts::crust_palette)), and the body's main liquid
//! ([`main_liquid`](super::FieldHeaderParts::main_liquid)); each cell's substance byte
//! ([`SynthesisCell::substances`](super::SynthesisCell::substances)) names its ice entry in the
//! high nibble and its liquid entry in the low, [`NO_ENTRY`] for none. Fifteen entries is the most a
//! nibble addresses beside its "none".
//!
//! A liquid entry, and a crust entry whose rock the body melts, carries its liquid's record
//! ([`LiquidProperties`]): what the liquid is to light and to waves at the body's temperature, by
//! R08's fifteen bake bins ([`LIQUID_BINS`]) for the optical members (R11.T8;
//! decision-r11-palette-liquids §1).

use hyperion_base::units::Kelvin;

use crate::substance_key::SubstanceKey;

/// The nibble of a cell's substance byte that names no entry: 0xF.
pub const NO_ENTRY: u8 = 0xF;

coded_enum! {
    /// What a palette entry is to the body, one byte, append-only (decision-composition §1.7).
    ///
    /// The three crusts are Taylor's classes of planetary crust: primary from the crystallisation
    /// of the first melt, secondary from partial melting of the mantle, tertiary from the
    /// reprocessing of secondary crust (Taylor 1989, Tectonophysics 161, 147; Taylor and McLennan
    /// 2009, Planetary Crusts, Cambridge University Press; not read).
    PaletteRole {
        /// A primary crust, crystallised from the body's first global liquid: a magma ocean's
        /// flotation crust, as the lunar highlands' feldspathic crust
        /// (science-composition-crusts.md §7), or a frozen ocean's shell with what it trapped, as
        /// Ceres's ice-, salt- and phyllosilicate-bearing crust (Castillo-Rogez et al. 2018, MAPS
        /// 53, 1820, p. 1838; science-r09-drainage.md §5).
        PrimaryCrust = 0,
        /// A secondary crust, a solid mantle's partial melt, as basalt: the ocean floors, a Mars's
        /// crust. A body may have none (a Ceres, whose rock "avoided igneous differentiation", Fu
        /// et al. 2017, EPSL 476, 153).
        SecondaryCrust = 1,
        /// A tertiary crust, as the granite of Earth's continents.
        TertiaryCrust = 2,
        /// The lithology of a stagnant lid's volcanic provinces where it is not the lid's own, as
        /// the lunar maria's basalt on a primary crust.
        Province = 3,
        /// A condensate on the ground as ice or frost.
        Ice = 4,
        /// A liquid on the ground: a sea's or a lake's.
        Liquid = 5,
        /// A cover over the ground that is not its rock: an organic cover, or, reserved for a life
        /// plan, a biological one.
        Cover = 6,
        /// The source of a body's sand and dust where it is not the crust's, as a Mars's dust.
        Deposit = 7,
        /// An undifferentiated body's own material where no crust covers it: its accreted,
        /// never-melted rock or ice–dust, as a chondrite or a comet's dust mantle (or, on a
        /// fragment of a differentiated body, its exposed mantle or core).
        ///
        /// It is not a crust in Taylor's sense (science-composition-crusts.md §1).
        Bulk = 8,
    }
}

impl PaletteRole {
    /// Whether the role is one a [`Crust`](super::Crust) variant's entry may take: the three
    /// crusts, a province's and an undifferentiated body's bulk.
    #[must_use]
    pub const fn is_crust(self) -> bool {
        match self {
            Self::PrimaryCrust
            | Self::SecondaryCrust
            | Self::TertiaryCrust
            | Self::Province
            | Self::Bulk => true,
            Self::Ice | Self::Liquid | Self::Cover | Self::Deposit => false,
        }
    }
}

coded_enum! {
    /// The family whose mechanical rows (strength, friction) a substance takes in R10's material
    /// table, one byte (decision-composition §1.7).
    MechanicsFamily {
        /// Silicate rock, regolith and dust.
        Silicate = 0,
        /// Metal, as an iron-nickel surface.
        Metal = 1,
        /// Water ice, and a water liquid's frozen form.
        WaterIce = 2,
        /// An ice of a volatile other than water: CO₂, N₂, CH₄, CO.
        VolatileIce = 3,
        /// A salt or evaporite.
        Salt = 4,
        /// An organic solid, as tholin.
        Organic = 5,
    }
}

/// The number of spectral bins of a liquid's optical members: R08's fifteen bake bins, 25.33 nm
/// wide from 380 to 760 nm (R08's Design note 5), the grid each star's light is sent on
/// (`HostDiscDto.bake_spectrum`), so that a client builds each sun's transmittance of a liquid
/// column from them (decision-r11-palette-liquids §1.2).
///
/// A new bin set changes the payload's layout, so it takes a new `SURFACE_PAYLOAD_FORMAT`.
pub const LIQUID_BINS: usize = 15;

/// The centres of the liquid bins, nm: 392.67 + 25.33 k for k = 0 to 14.
///
/// They are R08's `BAKE_WAVELENGTHS_NM` and the sim's `sky::colour::BAKE_WAVELENGTHS_NM`, which a
/// sim test holds equal to these to the bit, and which
/// `packages/protocol/fixtures/bake_wavelengths_nm.json` pins for all three to 10⁻⁹ nm. Each bin's
/// value is its member's uniform mean over the bin, from 380 + 25.33 k to 380 + 25.33 (k + 1) nm.
pub const LIQUID_BIN_CENTRES_NM: [f64; LIQUID_BINS] = [
    392.666_666_666_666_7,
    418.0,
    443.333_333_333_333_3,
    468.666_666_666_666_7,
    494.0,
    519.333_333_333_333_3,
    544.666_666_666_666_7,
    570.0,
    595.333_333_333_333_3,
    620.666_666_666_666_7,
    646.0,
    671.333_333_333_333_3,
    696.666_666_666_666_7,
    722.0,
    747.333_333_333_333_3,
];

/// The effective wavelengths of the bands of [`PaletteEntry::normal_albedo_bvr`], nm: 440, 550
/// and 640.
///
/// They are Johnson B and V and Cousins R as the sim's extinction bands give them (the SVO Filter
/// Profile Service's to two figures; `galaxy::gas::ccm::Band`), the wavelengths the reference
/// rows' liquid water was read at. A liquid entry's normal albedo is its record's Fresnel
/// reflectance there ([`LiquidProperties::fresnel_normal_albedo_bvr`]).
pub const BVR_EFFECTIVE_WAVELENGTHS_NM: [f64; 3] = [440.0, 550.0, 640.0];

coded_enum! {
    /// How directly a liquid record's members were obtained: the least direct of them, one byte,
    /// append-only (decision-r11-palette-liquids §1.1).
    ///
    /// A client labels a record that is not [`Measured`](Self::Measured) as approximate, as R08
    /// labels an approximate atmosphere.
    ///
    /// Exact relations applied to measured inputs keep their inputs' basis: Lorentz–Lorenz over one
    /// substance's liquid range, Einstein–Smoluchowski–Cabannes for a pure liquid, and a = 4πk ÷ λ.
    LiquidBasis {
        /// Every member from measurements of this liquid, or from a reference correlation fitted to
        /// them within its range.
        Measured = 0,
        /// At least one member from a mixing rule, an estimation method (the parachor,
        /// Grunberg–Nissan, a group contribution), a reference composition in place of the
        /// body's, or an extrapolation beyond a source's range.
        Derived = 1,
        /// At least one member is a named analogue's or a stated constant.
        StandIn = 2,
    }
}

/// What a liquid on the ground is to light and to waves, resolved by the server for its body at
/// [`temperature`](Self::temperature) (R11.T8; decision-r11-palette-liquids §1.1).
///
/// Unknown is absent, never 0 or NaN: each optional member is `None` where it is not modelled
/// (a presence byte of 0 on the wire, a key left off the DTO), and a by-bin member is whole or
/// absent. The refractive index, the density and the temperature are computable for any liquid,
/// and so required. No member is scaled by R10's albedo scale. Each value carries its source's
/// precision, as its IEEE 754 `f64`.
///
/// Plain data with public fields: [`MaterialPalette::new`] checks them. It is about 430 bytes, so
/// it is not `Copy`.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidProperties {
    /// The temperature every other member is resolved at, kelvin: finite and positive. The
    /// server's rule is decision-r11-palette-liquids §1.7's (the liquid's own area-mean surface
    /// temperature, or a melt's own, clamped into the liquid's range at the surface pressure).
    pub temperature: Kelvin,
    /// The liquid's density at that temperature and the surface pressure, kg m⁻³: finite and
    /// positive. On a [`PaletteRole::Liquid`] entry it is the entry's
    /// [`grain_density_kg_m3`](PaletteEntry::grain_density_kg_m3) to the bit; a melt's record on a
    /// crust entry holds the melt's density beside the solid's grain density.
    pub density_kg_m3: f64,
    /// How directly the members were obtained: the least direct of them.
    pub basis: LiquidBasis,
    /// The real refractive index n relative to vacuum, each bin's uniform mean over
    /// [`LIQUID_BIN_CENTRES_NM`]'s bins, dimensionless: finite and positive.
    pub refractive_index: [f64; LIQUID_BINS],
    /// The absorption coefficient a = 4πk ÷ λ, m⁻¹, Napierian (the intensity's e-folding per
    /// metre), each bin's uniform mean: finite and at least +0 (a −0 is refused), or `None` where
    /// not modelled. A solution's is modelled only when every component above 1 per cent by volume
    /// has its own.
    pub absorption_per_m: Option<[f64; LIQUID_BINS]>,
    /// The pure liquid's scattering coefficient b over 4π, m⁻¹, from its density and, for a
    /// solution, concentration fluctuations, each bin's uniform mean: finite and at least +0 (a −0
    /// is refused), or `None` where not modelled. Its phase function is symmetric (the depolarised
    /// Rayleigh form, Morel 1974), so its backscattering is b ÷ 2. Suspended and dissolved matter
    /// (sediment, organics, life) is not in it.
    pub scattering_per_m: Option<[f64; LIQUID_BINS]>,
    /// The surface tension, N m⁻¹, against the body's air where a source gives it, else against
    /// the liquid's own vapour: finite and positive, or `None` where not modelled.
    pub surface_tension_n_per_m: Option<f64>,
    /// The dynamic viscosity μ, Pa s (R11 takes ν = μ ÷ ρ): finite and positive, or `None` where
    /// not modelled.
    pub viscosity_pa_s: Option<f64>,
}

impl LiquidProperties {
    /// The real refractive index at `wavelength_nm`, nm: linear between the bins' centres, and
    /// the first or last bin's beyond them.
    #[must_use]
    pub fn refractive_index_at(&self, wavelength_nm: f64) -> f64 {
        let (c, n) = (&LIQUID_BIN_CENTRES_NM, &self.refractive_index);
        if wavelength_nm <= c[0] {
            return n[0];
        }
        for k in 1..LIQUID_BINS {
            if wavelength_nm <= c[k] {
                let t = (wavelength_nm - c[k - 1]) / (c[k] - c[k - 1]);
                return n[k - 1] + t * (n[k] - n[k - 1]);
            }
        }
        n[LIQUID_BINS - 1]
    }

    /// The Fresnel reflectance of the liquid's surface at normal incidence from vacuum,
    /// ((n − 1) ÷ (n + 1))², in B, V and R, at [`BVR_EFFECTIVE_WAVELENGTHS_NM`].
    ///
    /// It is what a [`PaletteRole::Liquid`] entry's
    /// [`normal_albedo_bvr`](PaletteEntry::normal_albedo_bvr) holds (decision-r11-palette-liquids
    /// §2, item 7). Against a body's air the reflectance is a little lower, since the air's index
    /// is above 1.
    #[must_use]
    pub fn fresnel_normal_albedo_bvr(&self) -> [f64; 3] {
        BVR_EFFECTIVE_WAVELENGTHS_NM.map(|wavelength| {
            let n = self.refractive_index_at(wavelength);
            let r = (n - 1.0) / (n + 1.0);
            r * r
        })
    }

    /// The first member out of its range, by name, or `None`.
    #[must_use]
    fn out_of_range(&self) -> Option<&'static str> {
        let finite_non_negative = |a: &f64| a.is_finite() && *a >= 0.0 && a.is_sign_positive();
        let bins_ok = |bins: Option<&[f64; LIQUID_BINS]>| {
            bins.is_none_or(|b| b.iter().all(finite_non_negative))
        };
        if !positive(self.temperature.value()) {
            Some("temperature")
        } else if !positive(self.density_kg_m3) {
            Some("density")
        } else if !self.refractive_index.iter().all(|n| positive(*n)) {
            Some("refractive index")
        } else if !bins_ok(self.absorption_per_m.as_ref()) {
            Some("absorption")
        } else if !bins_ok(self.scattering_per_m.as_ref()) {
            Some("scattering")
        } else if !self.surface_tension_n_per_m.is_none_or(positive) {
            Some("surface tension")
        } else if !self.viscosity_pa_s.is_none_or(positive) {
            Some("viscosity")
        } else {
            None
        }
    }
}

/// One substance of a body's palette, every value resolved by the server from plan 14's registry
/// (P14.T49.c's optics at the body's grain class, the phase column's transition, the mechanics
/// family, P14.T49.h's liquid record), so that the client draws from the field alone (Design note
/// 17).
///
/// Plain data with public fields: [`MaterialPalette::new`] checks them. An entry with its liquid
/// record is about 500 bytes, so it is not `Copy`.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteEntry {
    /// The substance, by its registry key.
    pub substance: SubstanceKey,
    /// What the substance is to the body.
    pub role: PaletteRole,
    /// The normal albedo `A_N`, the radiance factor at i = e = α = 0 (Hapke 2012, §10; R10's
    /// Design note 8), in the Johnson B, V and R bands, dimensionless: finite and at least +0 (a
    /// −0 is refused, so that a black band has one wire form). It may exceed 1 on a strongly
    /// backscattering surface.
    ///
    /// A [`PaletteRole::Liquid`] entry's is its record's Fresnel reflectance at normal incidence,
    /// ((n − 1) ÷ (n + 1))², from the record's n at the bands' effective wavelengths
    /// ([`LiquidProperties::fresnel_normal_albedo_bvr`]; decision-r11-palette-liquids §2, item
    /// 7): a specular surface's, never a Lambertian albedo, which no reader takes it for. A sea's
    /// diffuse colour is R11's, from the record's absorption and scattering.
    pub normal_albedo_bvr: [f64; 3],
    /// The row of R10's phase table, its law's f(α) and L(α), whose codes R10 assigns; 0 in every
    /// field until it does.
    pub phase_row: u8,
    /// The substance's grain density, kg m⁻³: the mass of its solid or liquid over that solid's or
    /// liquid's own volume, without pore space (a rock's grain density, an ice's crystal density at
    /// the temperature where it lies on the body, a liquid's density); finite and positive. A
    /// layer's bulk density is this times one less its porosity, which the reader takes from how
    /// the material lies (R10's forms by depth), never from the palette (science-r09-drainage.md
    /// §3; science-composition-crusts.md §3).
    pub grain_density_kg_m3: f64,
    /// The temperature above which the substance is melt or liquid: the solidus of a lithology,
    /// the melting point or eutectic of an ice or a liquid. Finite and positive.
    pub transition: Kelvin,
    /// The family of its mechanical rows in R10's table; a liquid's is its frozen form's.
    pub mechanics: MechanicsFamily,
    /// The liquid's record (decision-r11-palette-liquids §1.3), or `None` where the entry is not
    /// liquid on this body (`null` in the DTO), which is not "not modelled":
    ///
    /// - **required** on a [`PaletteRole::Liquid`] entry, a sea's or a lake's;
    /// - **present** on a crust-role entry ([`PaletteRole::is_crust`]) whose substance the body
    ///   melts, its melt's (a lava lake, a lava world's day side, a magma ocean: R10's `Melt`),
    ///   where plan 14's crust gives a melt area and the entry is the lid's, or the entry's
    ///   `transition` lies below the body's highest surface temperature; absent on every other
    ///   crust entry;
    /// - **absent** on an [`Ice`](PaletteRole::Ice), [`Cover`](PaletteRole::Cover) or
    ///   [`Deposit`](PaletteRole::Deposit) entry, which [`MaterialPalette::new`] refuses: an ice's
    ///   liquid is its own `Liquid` entry where the body has it.
    pub liquid: Option<LiquidProperties>,
}

impl PaletteEntry {
    /// The entry's place in a palette's order: by role, then by key.
    #[must_use]
    fn order_key(&self) -> (PaletteRole, SubstanceKey) {
        (self.role, self.substance)
    }
}

/// A body's palette: at most [`MAX_ENTRIES`](Self::MAX_ENTRIES) substances, in strictly
/// increasing (role, key), so that a palette has one order and no entry twice (decision-composition
/// §1.7).
///
/// A cell, the header's `crust_palette` and its `main_liquid` name an entry by its index here. An
/// empty palette is a body whose composition is not modelled, which a client labels so.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MaterialPalette {
    entries: Vec<PaletteEntry>,
}

/// Why [`MaterialPalette::new`] refused its entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPaletteError {
    /// More entries than [`MaterialPalette::MAX_ENTRIES`].
    TooManyEntries(usize),
    /// An entry is not strictly after its predecessor in (role, key).
    Unsorted {
        /// The entry's index.
        entry: u8,
    },
    /// An entry's normal albedo is not finite and at least +0 in every band.
    Albedo {
        /// The entry's index.
        entry: u8,
    },
    /// An entry's grain density is not finite and positive.
    Density {
        /// The entry's index.
        entry: u8,
    },
    /// An entry's transition temperature is not finite and positive.
    Transition {
        /// The entry's index.
        entry: u8,
    },
    /// A [`PaletteRole::Liquid`] entry has no liquid record.
    LiquidMissing {
        /// The entry's index.
        entry: u8,
    },
    /// An [`Ice`](PaletteRole::Ice), [`Cover`](PaletteRole::Cover) or
    /// [`Deposit`](PaletteRole::Deposit) entry carries a liquid record.
    LiquidMisplaced {
        /// The entry's index.
        entry: u8,
        /// Its role.
        role: PaletteRole,
    },
    /// A [`PaletteRole::Liquid`] entry's grain density is not its record's density to the bit.
    LiquidDensity {
        /// The entry's index.
        entry: u8,
    },
    /// A member of an entry's liquid record is out of its range: a temperature, density,
    /// refractive index, surface tension or viscosity that is not finite and positive, or an
    /// absorption or scattering that is not finite and at least +0.
    LiquidMember {
        /// The entry's index.
        entry: u8,
        /// The member's name.
        member: &'static str,
    },
}

impl std::fmt::Display for BuildPaletteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyEntries(n) => write!(
                f,
                "a palette of {n} entries is more than {}",
                MaterialPalette::MAX_ENTRIES
            ),
            Self::Unsorted { entry } => write!(
                f,
                "palette entry {entry} is not after its predecessor in (role, key)"
            ),
            Self::Albedo { entry } => write!(
                f,
                "palette entry {entry}'s normal albedo is not finite and non-negative"
            ),
            Self::Density { entry } => {
                write!(
                    f,
                    "palette entry {entry}'s grain density is not finite and positive"
                )
            }
            Self::Transition { entry } => write!(
                f,
                "palette entry {entry}'s transition temperature is not finite and positive"
            ),
            Self::LiquidMissing { entry } => {
                write!(f, "palette entry {entry} is a liquid without its record")
            }
            Self::LiquidMisplaced { entry, role } => write!(
                f,
                "palette entry {entry}, of role {role:?}, carries a liquid record"
            ),
            Self::LiquidDensity { entry } => write!(
                f,
                "palette entry {entry}'s grain density is not its liquid record's density"
            ),
            Self::LiquidMember { entry, member } => write!(
                f,
                "palette entry {entry}'s liquid {member} is out of its range"
            ),
        }
    }
}

impl std::error::Error for BuildPaletteError {}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Checks entry `entry`'s liquid record against its role (decision-r11-palette-liquids §1.3 and
/// §1.6): required on a liquid, with the liquid's grain density, absent on an ice, a cover and a
/// deposit, and each member in its range.
fn check_liquid(entry: u8, e: &PaletteEntry) -> Result<(), BuildPaletteError> {
    let record = match (e.role, &e.liquid) {
        (PaletteRole::Liquid, None) => return Err(BuildPaletteError::LiquidMissing { entry }),
        (PaletteRole::Ice | PaletteRole::Cover | PaletteRole::Deposit, Some(_)) => {
            return Err(BuildPaletteError::LiquidMisplaced {
                entry,
                role: e.role,
            });
        }
        (_, None) => return Ok(()),
        (
            PaletteRole::Liquid
            | PaletteRole::PrimaryCrust
            | PaletteRole::SecondaryCrust
            | PaletteRole::TertiaryCrust
            | PaletteRole::Province
            | PaletteRole::Bulk,
            Some(record),
        ) => record,
    };
    if let Some(member) = record.out_of_range() {
        return Err(BuildPaletteError::LiquidMember { entry, member });
    }
    // Both are finite and positive, checked above, so equal bits are equal values.
    if e.role == PaletteRole::Liquid
        && !e
            .grain_density_kg_m3
            .total_cmp(&record.density_kg_m3)
            .is_eq()
    {
        return Err(BuildPaletteError::LiquidDensity { entry });
    }
    Ok(())
}

impl MaterialPalette {
    /// The most entries a palette holds: 15, the most a nibble addresses beside [`NO_ENTRY`].
    pub const MAX_ENTRIES: usize = 15;

    /// The palette of `entries`, which must be in strictly increasing (role, key).
    ///
    /// # Errors
    ///
    /// [`BuildPaletteError::TooManyEntries`] past 15 entries, [`BuildPaletteError::Unsorted`] for
    /// an entry not after its predecessor, [`BuildPaletteError::Albedo`],
    /// [`BuildPaletteError::Density`] or [`BuildPaletteError::Transition`] for a value out of its
    /// range, [`BuildPaletteError::LiquidMissing`] for a liquid without its record,
    /// [`BuildPaletteError::LiquidMisplaced`] for a record on an ice, a cover or a deposit,
    /// [`BuildPaletteError::LiquidDensity`] for a liquid whose grain density is not its record's,
    /// and [`BuildPaletteError::LiquidMember`] for a record's member out of its range, the first
    /// entry that breaks a rule named.
    pub fn new(entries: Vec<PaletteEntry>) -> Result<Self, BuildPaletteError> {
        if entries.len() > Self::MAX_ENTRIES {
            return Err(BuildPaletteError::TooManyEntries(entries.len()));
        }
        for (entry, e) in (0_u8..).zip(&entries) {
            if entry > 0 && entries[usize::from(entry - 1)].order_key() >= e.order_key() {
                return Err(BuildPaletteError::Unsorted { entry });
            }
            if !e
                .normal_albedo_bvr
                .iter()
                .all(|a| a.is_finite() && *a >= 0.0 && a.is_sign_positive())
            {
                return Err(BuildPaletteError::Albedo { entry });
            }
            if !positive(e.grain_density_kg_m3) {
                return Err(BuildPaletteError::Density { entry });
            }
            if !positive(e.transition.value()) {
                return Err(BuildPaletteError::Transition { entry });
            }
            check_liquid(entry, e)?;
        }
        Ok(Self { entries })
    }

    /// The entries, in (role, key) order.
    #[must_use]
    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }

    /// The bytes the entries take on the heap, by the vector's capacity
    /// ([`CoarseField::heap_bytes`](super::CoarseField::heap_bytes)).
    #[must_use]
    pub(super) fn heap_bytes(&self) -> usize {
        super::vec_heap_bytes(&self.entries)
    }

    /// The entry of index `index`, or `None` past the palette.
    #[must_use]
    pub fn get(&self, index: u8) -> Option<&PaletteEntry> {
        self.entries.get(usize::from(index))
    }

    /// The number of entries, 0 to 15.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the palette has no entry: the body's composition is not modelled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The index of the entry of `substance` in `role`, if the palette has it.
    ///
    /// # Panics
    ///
    /// Never: a palette holds at most 15 entries, whose indices fit a `u8`.
    #[must_use]
    pub fn find(&self, role: PaletteRole, substance: SubstanceKey) -> Option<u8> {
        // The entries are strictly sorted by (role, key), so a search finds the one entry of
        // that pair or the place it would go, which is "absent".
        let at = self
            .entries
            .binary_search_by(|e| e.order_key().cmp(&(role, substance)))
            .ok()?;
        Some(u8::try_from(at).expect("a palette holds at most 15 entries"))
    }

    /// Whether `index` names an entry of `role`.
    #[must_use]
    pub fn has_role(&self, index: u8, role: PaletteRole) -> bool {
        self.get(index).is_some_and(|e| e.role == role)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn entry(key: &str, role: PaletteRole) -> PaletteEntry {
        let liquid = (role == PaletteRole::Liquid).then(crate::testing::liquid_water);
        PaletteEntry {
            substance: SubstanceKey::new(key).unwrap(),
            role,
            normal_albedo_bvr: [0.1, 0.1, 0.1],
            phase_row: 0,
            grain_density_kg_m3: liquid.as_ref().map_or(2_900.0, |l| l.density_kg_m3),
            transition: Kelvin::new(1_300.0),
            mechanics: MechanicsFamily::Silicate,
            liquid,
        }
    }

    /// A palette is in strictly increasing (role, key): by role first, then by key, with no entry
    /// twice; the same key in two roles is two entries.
    #[test]
    fn a_field_palette_is_sorted_by_role_then_key() {
        let water_ice = entry("H2O", PaletteRole::Ice);
        let water = entry("H2O", PaletteRole::Liquid);
        let basalt = entry("basalt", PaletteRole::SecondaryCrust);
        let dry_ice = entry("CO2", PaletteRole::Ice);
        let palette = MaterialPalette::new(vec![
            basalt.clone(),
            dry_ice.clone(),
            water_ice.clone(),
            water,
        ])
        .unwrap();
        assert_eq!(palette.len(), 4);
        assert_eq!(
            palette.find(PaletteRole::Ice, SubstanceKey::new("H2O").unwrap()),
            Some(2)
        );
        assert_eq!(
            palette.find(PaletteRole::Liquid, SubstanceKey::new("H2O").unwrap()),
            Some(3)
        );
        assert_eq!(
            palette.find(PaletteRole::Deposit, SubstanceKey::new("H2O").unwrap()),
            None
        );
        assert!(palette.has_role(1, PaletteRole::Ice));
        assert!(!palette.has_role(1, PaletteRole::Liquid));
        assert!(!palette.has_role(4, PaletteRole::Liquid));
        assert_eq!(palette.get(4), None);
        // By role first: an ice before a crust is out of order, though "CO2" < "basalt".
        assert_eq!(
            MaterialPalette::new(vec![dry_ice.clone(), basalt.clone()]),
            Err(BuildPaletteError::Unsorted { entry: 1 })
        );
        // Then by key, strictly.
        assert_eq!(
            MaterialPalette::new(vec![water_ice, dry_ice]),
            Err(BuildPaletteError::Unsorted { entry: 1 })
        );
        assert_eq!(
            MaterialPalette::new(vec![basalt.clone(), basalt]),
            Err(BuildPaletteError::Unsorted { entry: 1 })
        );
        assert!(MaterialPalette::new(vec![]).unwrap().is_empty());
        assert_eq!(
            MaterialPalette::default(),
            MaterialPalette::new(vec![]).unwrap()
        );
    }

    /// Fifteen entries fit, sixteen do not, and each value out of its range is refused.
    #[test]
    fn a_field_palette_refuses_more_than_fifteen_entries_and_values_out_of_range() {
        let names = [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
        ];
        let all: Vec<PaletteEntry> = names
            .iter()
            .map(|n| entry(n, PaletteRole::Deposit))
            .collect();
        assert_eq!(MaterialPalette::new(all[..15].to_vec()).unwrap().len(), 15);
        assert_eq!(
            MaterialPalette::new(all),
            Err(BuildPaletteError::TooManyEntries(16))
        );
        let refuse = |edit: fn(&mut PaletteEntry)| {
            let mut e = entry("basalt", PaletteRole::SecondaryCrust);
            edit(&mut e);
            MaterialPalette::new(vec![e]).unwrap_err()
        };
        assert_eq!(
            refuse(|e| e.normal_albedo_bvr[2] = -0.1),
            BuildPaletteError::Albedo { entry: 0 }
        );
        assert_eq!(
            refuse(|e| e.normal_albedo_bvr[0] = f64::NAN),
            BuildPaletteError::Albedo { entry: 0 }
        );
        assert_eq!(
            refuse(|e| e.normal_albedo_bvr[1] = -0.0),
            BuildPaletteError::Albedo { entry: 0 }
        );
        assert_eq!(
            refuse(|e| e.grain_density_kg_m3 = 0.0),
            BuildPaletteError::Density { entry: 0 }
        );
        assert_eq!(
            refuse(|e| e.transition = Kelvin::new(f64::INFINITY)),
            BuildPaletteError::Transition { entry: 0 }
        );
        // A bright backscatterer's normal albedo above 1 is a value, not an error.
        let mut bright = entry("H2O", PaletteRole::Ice);
        bright.normal_albedo_bvr = [1.2, 1.1, 1.0];
        assert!(MaterialPalette::new(vec![bright]).is_ok());
    }

    /// A liquid entry carries its record, with the record's density as its grain density; a melt's
    /// record may sit on a crust entry beside the solid's grain density; an ice, a cover and a
    /// deposit carry none (decision-r11-palette-liquids §1.3 and §1.6).
    #[test]
    fn a_field_palette_holds_each_liquid_record_where_its_role_takes_one() {
        let sea = entry("H2O", PaletteRole::Liquid);
        assert_eq!(
            sea.liquid.as_ref().map(|l| l.density_kg_m3),
            Some(sea.grain_density_kg_m3)
        );
        let mut lava = entry("basalt", PaletteRole::SecondaryCrust);
        lava.liquid = Some(crate::testing::basaltic_melt());
        assert!(MaterialPalette::new(vec![lava.clone(), sea.clone()]).is_ok());
        let mut dry = sea.clone();
        dry.liquid = None;
        assert_eq!(
            MaterialPalette::new(vec![lava, dry]),
            Err(BuildPaletteError::LiquidMissing { entry: 1 })
        );
        for role in [PaletteRole::Ice, PaletteRole::Cover, PaletteRole::Deposit] {
            let mut wet = entry("H2O", role);
            wet.liquid = Some(crate::testing::liquid_water());
            assert_eq!(
                MaterialPalette::new(vec![wet]),
                Err(BuildPaletteError::LiquidMisplaced { entry: 0, role })
            );
        }
        // The grain density is the record's to the bit, not to a rounding.
        let mut denser = sea.clone();
        denser.grain_density_kg_m3 = sea.grain_density_kg_m3.next_up();
        assert_eq!(
            MaterialPalette::new(vec![denser]),
            Err(BuildPaletteError::LiquidDensity { entry: 0 })
        );
        // Every crust role may carry a melt's.
        for role in PaletteRole::ALL.iter().copied().filter(|r| r.is_crust()) {
            let mut melt = entry("basalt", role);
            melt.liquid = Some(crate::testing::basaltic_melt());
            assert!(MaterialPalette::new(vec![melt]).is_ok(), "{role:?}");
        }
    }

    /// Each member of a record out of its range is refused by name: a temperature, density, index,
    /// surface tension or viscosity that is not finite and positive, and an absorption or
    /// scattering that is not finite and at least +0, a −0 among them.
    #[test]
    fn a_field_palette_refuses_liquid_members_out_of_their_ranges() {
        let refuse = |edit: fn(&mut LiquidProperties)| {
            let mut e = entry("H2O", PaletteRole::Liquid);
            let record = e.liquid.as_mut().unwrap();
            record.scattering_per_m = Some([1e-3; LIQUID_BINS]);
            edit(record);
            // A member out of its range is named before the grain density is matched to it.
            match MaterialPalette::new(vec![e]) {
                Err(BuildPaletteError::LiquidMember { entry: 0, member }) => member,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(refuse(|l| l.temperature = Kelvin::new(0.0)), "temperature");
        assert_eq!(refuse(|l| l.density_kg_m3 = f64::NAN), "density");
        assert_eq!(
            refuse(|l| l.refractive_index[14] = -1.33),
            "refractive index"
        );
        assert_eq!(
            refuse(|l| l.absorption_per_m.as_mut().unwrap()[3] = -0.0),
            "absorption"
        );
        assert_eq!(
            refuse(|l| l.absorption_per_m.as_mut().unwrap()[0] = f64::INFINITY),
            "absorption"
        );
        assert_eq!(
            refuse(|l| l.scattering_per_m.as_mut().unwrap()[7] = -1e-4),
            "scattering"
        );
        assert_eq!(
            refuse(|l| l.surface_tension_n_per_m = Some(0.0)),
            "surface tension"
        );
        assert_eq!(refuse(|l| l.viscosity_pa_s = Some(f64::NAN)), "viscosity");
        // A transparent bin, +0, is a value; an absent member is "not modelled".
        let mut clear = entry("H2O", PaletteRole::Liquid);
        let record = clear.liquid.as_mut().unwrap();
        record.absorption_per_m = Some([0.0; LIQUID_BINS]);
        record.surface_tension_n_per_m = None;
        assert!(MaterialPalette::new(vec![clear]).is_ok());
    }

    /// A liquid's normal albedo is its Fresnel reflectance at normal incidence from its record's n,
    /// read between the bins' centres at B, V and R (decision-r11-palette-liquids §2, item 7). The
    /// index here falls linearly across the bins, 1.34 less 0.000 5 a bin, so that the test reads
    /// no material file and a change to `water.json`'s n does not move it.
    #[test]
    fn a_liquid_record_gives_its_fresnel_normal_albedo() {
        let mut liquid = crate::testing::liquid_water();
        for (k, n) in (0_u8..).zip(&mut liquid.refractive_index) {
            *n = 1.34 - 0.000_5 * f64::from(k);
        }
        let width = LIQUID_BIN_CENTRES_NM[1] - LIQUID_BIN_CENTRES_NM[0];
        let albedo = liquid.fresnel_normal_albedo_bvr();
        for (band, wavelength) in BVR_EFFECTIVE_WAVELENGTHS_NM.iter().enumerate() {
            let n = 1.34 - 0.000_5 * (wavelength - LIQUID_BIN_CENTRES_NM[0]) / width;
            let r = (n - 1.0) / (n + 1.0);
            assert!((albedo[band] - r * r).abs() < 1e-12, "{wavelength} nm");
        }
        // About 0.0210, 0.0208 and 0.0206 here, water's order.
        assert!(
            albedo.iter().all(|a| (0.020..0.0215).contains(a)),
            "{albedo:?}"
        );
        // At a bin's centre the index is the bin's, and beyond the bins the end bins'.
        assert_same_bits(
            liquid.refractive_index_at(LIQUID_BIN_CENTRES_NM[6]),
            liquid.refractive_index[6],
        );
        assert_same_bits(
            liquid.refractive_index_at(300.0),
            liquid.refractive_index[0],
        );
        assert_same_bits(
            liquid.refractive_index_at(900.0),
            liquid.refractive_index[14],
        );
    }

    /// The liquid bins are R08's bake bins, as the cross-language fixture pins them: fifteen,
    /// 25.33 nm wide from 380 to 760 nm (decision-r11-palette-liquids §1.2).
    #[test]
    fn the_liquid_bins_are_r08s_bake_bins() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/bake_wavelengths_nm.json"
        ));
        let list = fixture
            .split("\"wavelengths_nm\":")
            .nth(1)
            .and_then(|rest| rest.split('[').nth(1))
            .and_then(|rest| rest.split(']').next())
            .expect("the fixture lists wavelengths_nm");
        let values: Vec<f64> = list
            .split(',')
            .map(|v| v.trim().parse().expect("a wavelength is a number"))
            .collect();
        assert_eq!(values.len(), LIQUID_BINS);
        for (k, (a, b)) in values.iter().zip(LIQUID_BIN_CENTRES_NM).enumerate() {
            assert!((a - b).abs() < 1e-9, "bin {k}: {a} against {b}");
            let centre = 380.0 + (f64::from(u8::try_from(k).unwrap()) + 0.5) * 380.0 / 15.0;
            assert!((b - centre).abs() < 1e-9, "bin {k}: {b} against {centre}");
        }
    }

    /// The crust roles are the five a `Crust` variant's entry may take: the three crusts, a
    /// province's and an undifferentiated body's bulk (science-composition-crusts.md §1).
    #[test]
    fn field_palette_crust_roles_are_the_crusts_and_the_provinces() {
        let crusts: Vec<PaletteRole> = PaletteRole::ALL
            .iter()
            .copied()
            .filter(|r| r.is_crust())
            .collect();
        assert_eq!(
            crusts,
            [
                PaletteRole::PrimaryCrust,
                PaletteRole::SecondaryCrust,
                PaletteRole::TertiaryCrust,
                PaletteRole::Province,
                PaletteRole::Bulk
            ]
        );
    }
}
