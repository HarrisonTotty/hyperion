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
        /// A primary crust, as the lunar highlands' anorthosite.
        PrimaryCrust = 0,
        /// A secondary crust, as basalt: the ocean floors, a Mars's crust.
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
    }
}

impl PaletteRole {
    /// Whether the role is a crust's, the roles a [`Crust`](super::Crust) variant's entry may
    /// take: the three crusts and a province's.
    #[must_use]
    pub const fn is_crust(self) -> bool {
        match self {
            Self::PrimaryCrust | Self::SecondaryCrust | Self::TertiaryCrust | Self::Province => {
                true
            }
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

/// One substance of a body's palette, every value resolved by the server from plan 14's registry
/// (P14.T49.c's optics at the body's grain class, the phase column's transition, the mechanics
/// family), so that the client draws from the field alone (Design note 17).
///
/// Plain data with public fields: [`MaterialPalette::new`] checks them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteEntry {
    /// The substance, by its registry key.
    pub substance: SubstanceKey,
    /// What the substance is to the body.
    pub role: PaletteRole,
    /// The normal albedo `A_N`, the radiance factor at i = e = α = 0 (Hapke 2012, §10; R10's
    /// Design note 8), in the Johnson B, V and R bands, dimensionless: finite and at least +0 (a
    /// −0 is refused, so that a black band has one wire form). It may exceed 1 on a strongly
    /// backscattering surface.
    pub normal_albedo_bvr: [f64; 3],
    /// The row of R10's phase table, its law's f(α) and L(α), whose codes R10 assigns; 0 in every
    /// field until it does.
    pub phase_row: u8,
    /// The substance's bulk density, kg m⁻³: finite and positive.
    pub density_kg_m3: f64,
    /// The temperature above which the substance is melt or liquid: the solidus of a lithology,
    /// the melting point or eutectic of an ice or a liquid. Finite and positive.
    pub transition: Kelvin,
    /// The family of its mechanical rows in R10's table; a liquid's is its frozen form's.
    pub mechanics: MechanicsFamily,
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
    /// An entry's density is not finite and positive.
    Density {
        /// The entry's index.
        entry: u8,
    },
    /// An entry's transition temperature is not finite and positive.
    Transition {
        /// The entry's index.
        entry: u8,
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
                    "palette entry {entry}'s density is not finite and positive"
                )
            }
            Self::Transition { entry } => write!(
                f,
                "palette entry {entry}'s transition temperature is not finite and positive"
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

impl MaterialPalette {
    /// The most entries a palette holds: 15, the most a nibble addresses beside [`NO_ENTRY`].
    pub const MAX_ENTRIES: usize = 15;

    /// The palette of `entries`, which must be in strictly increasing (role, key).
    ///
    /// # Errors
    ///
    /// [`BuildPaletteError::TooManyEntries`] past 15 entries, [`BuildPaletteError::Unsorted`] for
    /// an entry not after its predecessor, and [`BuildPaletteError::Albedo`],
    /// [`BuildPaletteError::Density`] or [`BuildPaletteError::Transition`] for a value out of its
    /// range, the first entry that breaks a rule named.
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
            if !positive(e.density_kg_m3) {
                return Err(BuildPaletteError::Density { entry });
            }
            if !positive(e.transition.value()) {
                return Err(BuildPaletteError::Transition { entry });
            }
        }
        Ok(Self { entries })
    }

    /// The entries, in (role, key) order.
    #[must_use]
    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
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
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn entry(key: &str, role: PaletteRole) -> PaletteEntry {
        PaletteEntry {
            substance: SubstanceKey::new(key).unwrap(),
            role,
            normal_albedo_bvr: [0.1, 0.1, 0.1],
            phase_row: 0,
            density_kg_m3: 2_900.0,
            transition: Kelvin::new(1_300.0),
            mechanics: MechanicsFamily::Silicate,
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
        let palette = MaterialPalette::new(vec![basalt, dry_ice, water_ice, water]).unwrap();
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
            MaterialPalette::new(vec![dry_ice, basalt]),
            Err(BuildPaletteError::Unsorted { entry: 1 })
        );
        // Then by key, strictly.
        assert_eq!(
            MaterialPalette::new(vec![water_ice, dry_ice]),
            Err(BuildPaletteError::Unsorted { entry: 1 })
        );
        assert_eq!(
            MaterialPalette::new(vec![basalt, basalt]),
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
            refuse(|e| e.density_kg_m3 = 0.0),
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

    /// The crust roles are the four a `Crust` variant's entry may take.
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
                PaletteRole::Province
            ]
        );
    }
}
