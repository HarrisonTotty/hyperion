//! The material classes, the forms in which the ground lies, and the material, a form of one of the
//! body's substances (plan R10, T1.a; decision-composition §1.8).

use crate::field::{MaterialPalette, coded_enum};

coded_enum! {
    /// How the ground lies: the form of its material, one byte, append-only.
    ///
    /// A class is a form, never a substance: what the ground is made of is the palette entry a
    /// [`Material`] names beside it, so one form serves every substance the generator can produce
    /// (decision-composition §1.8). Its mechanical rows are the form's for the entry's
    /// [`MechanicsFamily`](crate::field::MechanicsFamily) ([`form_row`](super::form_row)).
    ///
    /// The codes are wire- and golden-stable, and from R10.T1.b, which first generates the class,
    /// any change to the set, the property table or the classifier is a generator-version change; a
    /// new class appends a code. The set is closed, fifteen forms, for the composition audit (plan
    /// R10, Risks): a form no class names (a biosphere's cover, a liquid's frozen spray) is a new
    /// code, not a stand-in.
    MaterialClass {
        /// Exposed rock of the crust's entry: ground too steep for loose material to rest on (above
        /// [`CLIFF_SLOPE`](super::CLIFF_SLOPE)), or a surface that weathering and impacts have not
        /// broken up.
        Bedrock = 0,
        /// The fragmental layer impacts make on an airless body, of the crust's entry or a
        /// `Deposit` entry: the lunar soil, a well-graded silty sand to sandy silt (Carrier,
        /// Olhoeft and Mendell 1991, Lunar Sourcebook, ch. 9, p. 478).
        Regolith = 1,
        /// Weathered, fine-grained ground under an atmosphere, dry: the classifier has no wet state
        /// until R09's climate gives one (plan R10, Design note 8).
        Soil = 2,
        /// Loose, sand-sized grains sorted by wind or water: dunes, beaches and sand seas.
        Sand = 3,
        /// Material laid down by flowing liquid: along channels and in the filled basins (plan R10,
        /// Design note 1).
        Sediment = 4,
        /// Volcanic rock of a province's entry, in its older provinces (plan R10, Design note 1).
        VolcanicRock = 5,
        /// Lithology above its transition temperature: a magma ocean or a young province's lava,
        /// emissive (R08.T18's thermal emission; plan R10, Design note 8).
        Melt = 6,
        /// Massive ice of the cell's ice entry, of whatever species: a glacier, an ice sheet or an
        /// ice cap, which stands in cliffs of its own.
        ///
        /// Plan R10, Design note 1; decision-composition §1.8's "frost or glacier of the cell's
        /// ice entry". Named `WaterIce` in the plan's sketch; the code is the sketch's.
        Glacier = 7,
        /// A fresh, porous deposit of fine grains of the cell's ice entry, as fallen snow.
        Snow = 8,
        /// Ice of the cell's ice entry frozen on its liquid: a sea's or a lake's ice cover.
        SeaIce = 9,
        /// A thin condensate of the cell's ice entry on the ground, of whatever species: a seasonal
        /// frost, or a cold trap's exposed ice.
        ///
        /// Named `VolatileFrost` in the plan's sketch; the code is the sketch's.
        Frost = 10,
        /// An organic sediment of a `Cover` entry: a Titan-like haze's deposit (plan R10, Design
        /// note 1).
        OrganicSediment = 11,
        /// The ground under the liquid entry, which the ocean pass draws (R11; plan R10, Design
        /// note 8).
        Seabed = 12,
        /// Salts a drying liquid left in an arid basin it filled: a playa, among the brightest land
        /// surfaces (plan R10, Design note 1).
        Evaporite = 13,
        /// Fine material a thin atmosphere's wind deposits on an arid world, of the crust's entry
        /// or a `Deposit` entry: Mars's drift (plan R10, Design note 1).
        Dust = 14,
    }
}

/// How a form's ground scatters light: the kind that chooses its photometric law
/// ([`PhaseLaw`](super::PhaseLaw); plan R10, Design note 8; decision-r10-limb §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PhotometricForm {
    /// A smooth, consolidated or near-conservative medium, in which multiple scattering dominates
    /// and shadow hiding is small or absent: glacier ice and sea ice, which scatter inside a
    /// continuous matrix, firn and snow, and a recrystallised salt crust. Its zero-phase limb
    /// profile is multiple scattering's, which a lunar-Lambert fit puts at L(0) ≈ 0.04–0.05 for
    /// snow, so it takes the Lambert law ([`PhaseLaw::Lambert`](super::PhaseLaw::Lambert)) whatever
    /// its entry's code.
    Consolidated,
    /// A porous particulate regolith, which shadow-hides: lunar-like, with no limb darkening at
    /// zero phase up to a normal albedo of about 0.60, and limb-darkened, but never Lambert, above
    /// it ([`limb_parameter`](super::limb_parameter())). It takes its entry's law, or its family's
    /// where the entry names none; an entry that names Lambert's gives it Lambert's.
    Particulate,
}

impl MaterialClass {
    /// The form's photometric kind: [`Glacier`](Self::Glacier), [`Snow`](Self::Snow),
    /// [`SeaIce`](Self::SeaIce) and [`Evaporite`](Self::Evaporite) are consolidated, of whatever
    /// species or salt, and every other form is particulate, [`Frost`](Self::Frost) of any species
    /// included (decision-r10-limb §2).
    ///
    /// [`Bedrock`](Self::Bedrock), [`VolcanicRock`](Self::VolcanicRock), [`Melt`](Self::Melt) and
    /// [`Seabed`](Self::Seabed) are particulate by default: no source read gives a bright rock's
    /// limb profile.
    #[must_use]
    pub const fn photometric_form(self) -> PhotometricForm {
        match self {
            Self::Glacier | Self::Snow | Self::SeaIce | Self::Evaporite => {
                PhotometricForm::Consolidated
            }
            Self::Bedrock
            | Self::Regolith
            | Self::Soil
            | Self::Sand
            | Self::Sediment
            | Self::VolcanicRock
            | Self::Melt
            | Self::Frost
            | Self::OrganicSediment
            | Self::Seabed
            | Self::Dust => PhotometricForm::Particulate,
        }
    }
}

/// A material: a form of one substance of the body's palette (decision-composition §1.8).
///
/// `entry` is the index of the [`PaletteEntry`](crate::field::PaletteEntry) in the body's
/// [`MaterialPalette`] that the ground is made of. Whether the entry exists is the palette's to say
/// ([`properties`](super::properties()) refuses one past it); a form and a substance that do not
/// usually meet (a regolith of ice, as a Europa's) are a material all the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Material {
    class: MaterialClass,
    entry: u8,
}

impl Material {
    /// The material of form `class` and palette entry `entry`, or `None` for an entry no palette
    /// can hold: 15 or more ([`MaterialPalette::MAX_ENTRIES`], the cell's "none" nibble among
    /// them).
    #[must_use]
    pub fn new(class: MaterialClass, entry: u8) -> Option<Self> {
        (usize::from(entry) < MaterialPalette::MAX_ENTRIES).then_some(Self { class, entry })
    }

    /// The form.
    #[must_use]
    pub const fn class(self) -> MaterialClass {
        self.class
    }

    /// The palette entry's index.
    #[must_use]
    pub const fn entry(self) -> u8 {
        self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::NO_ENTRY;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The class codes are the plan's table, in order, and round-trip through their byte: the
    /// payload and the goldens carry them (plan R10, Provides).
    #[test]
    fn material_class_discriminants_are_stable() {
        let table: [(MaterialClass, u8); 15] = [
            (MaterialClass::Bedrock, 0),
            (MaterialClass::Regolith, 1),
            (MaterialClass::Soil, 2),
            (MaterialClass::Sand, 3),
            (MaterialClass::Sediment, 4),
            (MaterialClass::VolcanicRock, 5),
            (MaterialClass::Melt, 6),
            (MaterialClass::Glacier, 7),
            (MaterialClass::Snow, 8),
            (MaterialClass::SeaIce, 9),
            (MaterialClass::Frost, 10),
            (MaterialClass::OrganicSediment, 11),
            (MaterialClass::Seabed, 12),
            (MaterialClass::Evaporite, 13),
            (MaterialClass::Dust, 14),
        ];
        assert_eq!(MaterialClass::ALL.len(), table.len());
        for (&class, (want, code)) in MaterialClass::ALL.iter().zip(table) {
            assert_eq!(class, want, "ALL is in code order");
            assert_eq!(u8::from(class), code, "{class:?}'s code");
            assert_eq!(MaterialClass::try_from(code), Ok(class));
        }
        let unknown = MaterialClass::try_from(15).unwrap_err();
        assert_eq!(unknown.code, 15);
        assert_eq!(unknown.kind, "MaterialClass");
    }

    /// Glacier, snow, sea ice and evaporite are consolidated, and every other form, frost among
    /// them, is particulate (decision-r10-limb §2).
    #[test]
    fn material_photometric_form_is_consolidated_for_the_four_bright_forms() {
        let consolidated = [
            MaterialClass::Glacier,
            MaterialClass::Snow,
            MaterialClass::SeaIce,
            MaterialClass::Evaporite,
        ];
        for &class in MaterialClass::ALL {
            let want = if consolidated.contains(&class) {
                PhotometricForm::Consolidated
            } else {
                PhotometricForm::Particulate
            };
            assert_eq!(class.photometric_form(), want, "{class:?}");
        }
        assert_eq!(
            MaterialClass::Frost.photometric_form(),
            PhotometricForm::Particulate
        );
    }

    /// A material names an entry a palette can hold: 0 to 14, never the cell's "none" nibble.
    #[test]
    fn material_entry_is_a_palette_index() {
        let m = Material::new(MaterialClass::Snow, 14).unwrap();
        assert_eq!(m.class(), MaterialClass::Snow);
        assert_eq!(m.entry(), 14);
        assert_eq!(Material::new(MaterialClass::Snow, NO_ENTRY), None);
        assert_eq!(Material::new(MaterialClass::Bedrock, 255), None);
    }
}
