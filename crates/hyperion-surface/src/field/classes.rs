//! The codes a cell's [`SurfaceClass`] and its crater state carry (plan R09, R09.T15;
//! `decision-r09-t15-classes.md`).
//!
//! # The classes
//!
//! A class's byte is in one of three ranges (decision-composition §1.7): 0 unclassified, 1 to 63 a
//! Köppen–Geiger class, 64 to 255 a surface-state form whose substance is the cell's palette entry.
//! The coarse pass's last step (the sim's `steps::classes`) assigns:
//!
//! - codes 1 to 30, [`KoppenGeiger`]'s thirty classes, numbered as Beck et al. 2018's legend
//!   numbers them (Sci. Data 5, 180214; `legend.txt` of their data release), on the land of a world
//!   whose climate is a seasonal water cycle (an energy-balance model, twelve months and water as
//!   its condensable);
//! - codes 64 to 72, [`SurfaceForm`]'s nine forms, under liquid on such a world, as its melt, and
//!   on every other world: a form names no substance of its own, which is the cell's ice, liquid,
//!   crust or cover entry as each form says.
//!
//! The classes classify climate and surface state, not life: no code is a vegetation class, and a
//! lifeless Earth-like world keeps its Köppen–Geiger class with bare ground (Design note 11). The
//! classes are named by their symbols, Af to EF, with no biome's words.
//!
//! Every other code in each range is unassigned: an open code, which a reader names `None`
//! ([`SurfaceClass::koppen_geiger`], [`SurfaceClass::surface_form`]) and never refuses, and which a
//! later task appends to. No code is ever renumbered, reused or removed (the wire's rule for open
//! codes, [`crate::wire`]).
//!
//! # The crater state
//!
//! [`CraterState`] is the crater density, N(>1 km) in lunar-equivalent craters a square kilometre,
//! that a cell's own surface has seen since it formed, on a scale of eight codes an octave: from
//! it and the header's crater contract a reader has the cell's saturation at any diameter and its
//! retention age (decision-r09-t15-classes §6).

use hyperion_base::math;
use hyperion_base::units::PerSquareKilometre;

use super::{QuantiseValueError, SurfaceClass};

coded_enum! {
    /// Köppen–Geiger's thirty climate classes by Peel, Finlayson and McMahon 2007's Table 1
    /// (Hydrol. Earth Syst. Sci. 11, 1633) with Beck et al. 2018's rules (Sci. Data 5, 180214,
    /// Table 2), each its [`SurfaceClass`] code: Beck et al.'s legend number, 1 to 30.
    ///
    /// The first letter is the main group ([`KoppenGeigerGroup`]); the second a C or D class's
    /// dry season (s, its summer; w, its winter; f, none) or an A class's driest month (f, m,
    /// w), a B class's (W, desert; S, the drier margin's) or an E class's warmest month (T, above
    /// 0 °C; F, at or below it); the third a B class's annual mean (h, at least 18 °C; k, below) or
    /// a C or D class's summer heat or winter cold (a, b, c, d). The coarse pass applies the table
    /// to rates, on each cell's own summer (the sim's `steps::classes`).
    KoppenGeiger {
        /// `Af`: tropical, its driest month at least 60 mm.
        Af = 1,
        /// `Am`: tropical, its driest month under 60 mm and at least 100 − MAP ÷ 25 mm.
        Am = 2,
        /// `Aw`: tropical, its driest month under 100 − MAP ÷ 25 mm.
        Aw = 3,
        /// `BWh`: arid, MAP under five times `P_th`, its annual mean at least 18 °C.
        BWh = 4,
        /// `BWk`: arid, MAP under five times `P_th`, its annual mean under 18 °C.
        BWk = 5,
        /// `BSh`: arid, MAP at least five times `P_th`, its annual mean at least 18 °C.
        BSh = 6,
        /// `BSk`: arid, MAP at least five times `P_th`, its annual mean under 18 °C.
        BSk = 7,
        /// `Csa`: temperate, a dry summer, its warmest month at least 22 °C.
        Csa = 8,
        /// `Csb`: temperate, a dry summer, four months or more above 10 °C.
        Csb = 9,
        /// `Csc`: temperate, a dry summer, fewer than four months above 10 °C.
        Csc = 10,
        /// `Cwa`: temperate, a dry winter, its warmest month at least 22 °C.
        Cwa = 11,
        /// `Cwb`: temperate, a dry winter, four months or more above 10 °C.
        Cwb = 12,
        /// `Cwc`: temperate, a dry winter, fewer than four months above 10 °C.
        Cwc = 13,
        /// `Cfa`: temperate, no dry season, its warmest month at least 22 °C.
        Cfa = 14,
        /// `Cfb`: temperate, no dry season, four months or more above 10 °C.
        Cfb = 15,
        /// `Cfc`: temperate, no dry season, fewer than four months above 10 °C.
        Cfc = 16,
        /// `Dsa`: continental, a dry summer, its warmest month at least 22 °C.
        Dsa = 17,
        /// `Dsb`: continental, a dry summer, four months or more above 10 °C.
        Dsb = 18,
        /// `Dsc`: continental, a dry summer, fewer than four months above 10 °C.
        Dsc = 19,
        /// `Dsd`: continental, a dry summer, its coldest month under −38 °C.
        Dsd = 20,
        /// `Dwa`: continental, a dry winter, its warmest month at least 22 °C.
        Dwa = 21,
        /// `Dwb`: continental, a dry winter, four months or more above 10 °C.
        Dwb = 22,
        /// `Dwc`: continental, a dry winter, fewer than four months above 10 °C.
        Dwc = 23,
        /// `Dwd`: continental, a dry winter, its coldest month under −38 °C.
        Dwd = 24,
        /// `Dfa`: continental, no dry season, its warmest month at least 22 °C.
        Dfa = 25,
        /// `Dfb`: continental, no dry season, four months or more above 10 °C.
        Dfb = 26,
        /// `Dfc`: continental, no dry season, fewer than four months above 10 °C.
        Dfc = 27,
        /// `Dfd`: continental, no dry season, its coldest month under −38 °C.
        Dfd = 28,
        /// `ET`: polar, its warmest month above 0 °C.
        ET = 29,
        /// `EF`: polar, its warmest month at or below 0 °C.
        EF = 30,
    }
}

/// Köppen–Geiger's five main groups, the classes' first letters (Peel et al. 2007, Table 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KoppenGeigerGroup {
    /// A: the coldest month at least 18 °C.
    Tropical,
    /// B: the year's precipitation under ten times the arid threshold `P_th`, tested first.
    Arid,
    /// C: the warmest month above 10 °C and the coldest between 0 and 18 °C.
    Temperate,
    /// D, Peel et al.'s "cold": the warmest month above 10 °C and the coldest at or below 0 °C.
    Continental,
    /// E: the warmest month at or below 10 °C (Beck et al. 2018's bound).
    Polar,
}

impl KoppenGeiger {
    /// The class's main group.
    #[must_use]
    pub const fn group(self) -> KoppenGeigerGroup {
        match self {
            Self::Af | Self::Am | Self::Aw => KoppenGeigerGroup::Tropical,
            Self::BWh | Self::BWk | Self::BSh | Self::BSk => KoppenGeigerGroup::Arid,
            Self::Csa
            | Self::Csb
            | Self::Csc
            | Self::Cwa
            | Self::Cwb
            | Self::Cwc
            | Self::Cfa
            | Self::Cfb
            | Self::Cfc => KoppenGeigerGroup::Temperate,
            Self::Dsa
            | Self::Dsb
            | Self::Dsc
            | Self::Dsd
            | Self::Dwa
            | Self::Dwb
            | Self::Dwc
            | Self::Dwd
            | Self::Dfa
            | Self::Dfb
            | Self::Dfc
            | Self::Dfd => KoppenGeigerGroup::Continental,
            Self::ET | Self::EF => KoppenGeigerGroup::Polar,
        }
    }
}

coded_enum! {
    /// A cell's surface-state form, its [`SurfaceClass`] code from 64 (decision-r09-t15-classes
    /// §3; Design note 11's "liquid, ice or frost of a named species, rock, regolith, melt, organic
    /// sediment"), whose substance is the cell's palette entry that the form names.
    ///
    /// A regime's named zones are these forms under the header's climate model, with no codes of
    /// their own: a locked world's substellar ocean is [`SurfaceForm::Sea`] and its nightside
    /// glacier [`SurfaceForm::Ice`].
    SurfaceForm {
        /// The sea: under liquid whose surface is the sea level; of the cell's liquid entry.
        Sea = 64,
        /// A lake: under liquid whose surface is not the sea level; of the cell's liquid entry.
        Lake = 65,
        /// A frozen sea or lake: under liquid, at least half under ice; of the cell's ice entry.
        FrozenSea = 66,
        /// A glacier, ice sheet or ice cap: dry, at least half under ice; of the cell's ice entry.
        Ice = 67,
        /// Frost: dry, under ice on less than half of it; of the cell's ice entry.
        Frost = 68,
        /// Melt: the ground above its crust's solidus, or a magma ocean; of the crust's entry.
        Melt = 69,
        /// Organic sediment: dry ground under an atmosphere where the palette holds an organic
        /// cover; of that cover's entry.
        OrganicSediment = 70,
        /// Regolith: other dry ground under less than 30 kPa of air, the photometric threshold
        /// reused, a labelled choice (decision-r09-t15-classes, deferred finding 8); of the crust's
        /// entry.
        Regolith = 71,
        /// Rock: other dry ground, under 30 kPa of air or more; of the crust's entry.
        Rock = 72,
    }
}

impl From<KoppenGeiger> for SurfaceClass {
    fn from(class: KoppenGeiger) -> Self {
        Self::new(u8::from(class))
    }
}

impl From<SurfaceForm> for SurfaceClass {
    fn from(form: SurfaceForm) -> Self {
        Self::new(u8::from(form))
    }
}

impl SurfaceClass {
    /// The Köppen–Geiger class of the code, or `None` where it is no assigned Köppen–Geiger code
    /// (code 0, an open code of codes 31 to 63, or a surface-state code).
    #[must_use]
    pub fn koppen_geiger(self) -> Option<KoppenGeiger> {
        // An open code is no class yet: a reader names it `None`, never refuses it.
        KoppenGeiger::try_from(self.get()).ok()
    }

    /// The surface-state form of the code, or `None` where it is no assigned form (code 0, a
    /// Köppen–Geiger code, or an open code of codes 73 to 255).
    #[must_use]
    pub fn surface_form(self) -> Option<SurfaceForm> {
        // An open code is no form yet: a reader names it `None`, never refuses it.
        SurfaceForm::try_from(self.get()).ok()
    }
}

/// A cell's crater state, one byte: the crater density its own surface has seen since it formed,
/// N(>1 km) in lunar-equivalent craters a square kilometre, the header's crater contract's unit
/// (`decision-r09-t15-classes.md` §6).
///
/// - Code 0, [`CraterState::UNCLASSIFIED`]: not classified; only fixtures carry it, never the pass.
/// - Code 1, [`CraterState::NONE_RETAINED`]: none retained, N = 0, or a surface that shows none (a
///   liquid, a melt, or an ice share of at least half that flows or renews).
/// - Code c from 2 to 255: N = 1 km⁻² × 2^((c − 216) ÷ 8), eight codes an octave, a step of 9%,
///   from 8.9 × 10⁻⁹ km⁻² (a surface about 10 kyr old on the lunar chronology) to 29.3 km⁻²,
///   past which every diameter below the coarse craters' is saturated on an unscreened body: the
///   clamp moves no saturation there, and the retention age is read only up to 29.3 km⁻² (a belt
///   scaling about 9.6 times the lunar maximum, or a thick air's screening, would reach it).
///
/// The pass writes `N_c` = n₁ C(`a_c`) ÷ C(S): the header's N☾(>1 km) n₁ scaled by the lunar
/// chronology C ([`lunar_chronology`](crate::craters::lunar_chronology)) at the cell's own age
/// `a_c` against the surface age S, so a cell as old as the surface carries n₁ itself. Its
/// saturation at a diameter D is then min(production(D; `N_c`), saturation(D)), and its retention
/// age C⁻¹(`N_c` C(S) ÷ n₁). Nothing reads it yet: the synthesis's small craters keep the body's
/// one density (Design note 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CraterState(u8);

/// [`CraterState`]'s codes an octave.
const CRATER_STATE_PER_OCTAVE: f64 = 8.0;

/// The [`CraterState`] code of a density of 1 km⁻².
const CRATER_STATE_UNIT_CODE: f64 = 216.0;

/// The first [`CraterState`] code that carries a density.
const FIRST_DENSITY_CODE: u8 = 2;

impl CraterState {
    /// Not classified: only fixtures carry it.
    pub const UNCLASSIFIED: Self = Self(0);

    /// None retained: a density of zero, or a surface that shows no craters.
    pub const NONE_RETAINED: Self = Self(1);

    /// The state of code `code`.
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The state of a density `n`: [`CraterState::NONE_RETAINED`] at zero (of either sign), else
    /// the nearest code, held to 2–255.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError::NotFinite`] if `n` is not finite, and
    /// [`QuantiseValueError::OutOfRange`] if it is negative.
    pub fn from_density(n: PerSquareKilometre) -> Result<Self, QuantiseValueError> {
        let n = n.value();
        if !n.is_finite() {
            return Err(QuantiseValueError::NotFinite(n));
        }
        if n < 0.0 {
            return Err(QuantiseValueError::OutOfRange(n));
        }
        // Zero, of either sign, has no logarithm.
        if n <= 0.0 {
            return Ok(Self::NONE_RETAINED);
        }
        let code = ((CRATER_STATE_PER_OCTAVE * math::log2(n)).round() + CRATER_STATE_UNIT_CODE)
            .clamp(f64::from(FIRST_DENSITY_CODE), f64::from(u8::MAX));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "code is an integer from 2 to 255, held above"
        )]
        Ok(Self(code as u8))
    }

    /// The density the state carries: `None` unclassified, zero where none is retained, else its
    /// code's.
    #[must_use]
    pub fn density(self) -> Option<PerSquareKilometre> {
        match self.0 {
            0 => None,
            1 => Some(PerSquareKilometre::ZERO),
            code => Some(PerSquareKilometre::new(math::exp2(
                (f64::from(code) - CRATER_STATE_UNIT_CODE) / CRATER_STATE_PER_OCTAVE,
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{DecodeFieldCodeError, SurfaceClassRange};
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn koppen_geiger_codes_are_beck_s_legend_in_the_koppen_geiger_range() {
        let symbols: Vec<String> = KoppenGeiger::ALL.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(
            symbols.join(" "),
            "Af Am Aw BWh BWk BSh BSk Csa Csb Csc Cwa Cwb Cwc Cfa Cfb Cfc Dsa Dsb Dsc Dsd Dwa \
             Dwb Dwc Dwd Dfa Dfb Dfc Dfd ET EF"
        );
        for (&class, code) in KoppenGeiger::ALL.iter().zip(1_u8..) {
            assert_eq!(u8::from(class), code);
            let surface = SurfaceClass::from(class);
            assert_eq!(surface.range(), SurfaceClassRange::KoppenGeiger);
            assert_eq!(surface.koppen_geiger(), Some(class));
            assert_eq!(surface.surface_form(), None);
        }
    }

    #[test]
    fn each_class_s_group_is_its_first_letter() {
        for &class in KoppenGeiger::ALL {
            let letter = format!("{class:?}").chars().next();
            let expected = match class.group() {
                KoppenGeigerGroup::Tropical => 'A',
                KoppenGeigerGroup::Arid => 'B',
                KoppenGeigerGroup::Temperate => 'C',
                KoppenGeigerGroup::Continental => 'D',
                KoppenGeigerGroup::Polar => 'E',
            };
            assert_eq!(letter, Some(expected), "{class:?}");
        }
    }

    #[test]
    fn surface_forms_are_codes_64_to_72_in_the_surface_state_range() {
        for (&form, code) in SurfaceForm::ALL.iter().zip(64_u8..) {
            assert_eq!(u8::from(form), code);
            let surface = SurfaceClass::from(form);
            assert_eq!(surface.range(), SurfaceClassRange::SurfaceState);
            assert_eq!(surface.surface_form(), Some(form));
            assert_eq!(surface.koppen_geiger(), None);
        }
        assert_eq!(SurfaceForm::ALL.len(), 9);
    }

    #[test]
    fn open_codes_are_named_none_and_their_conversions_refused() {
        for code in [0_u8, 31, 63, 73, 255] {
            let class = SurfaceClass::new(code);
            assert_eq!(class.koppen_geiger(), None, "{code}");
            assert_eq!(class.surface_form(), None, "{code}");
        }
        assert_eq!(
            KoppenGeiger::try_from(31),
            Err(DecodeFieldCodeError {
                kind: "KoppenGeiger",
                code: 31
            })
        );
    }

    #[test]
    fn no_code_names_a_vegetation_class() {
        let names: Vec<String> = KoppenGeiger::ALL
            .iter()
            .map(|c| format!("{c:?}"))
            .chain(SurfaceForm::ALL.iter().map(|f| format!("{f:?}")))
            .collect();
        for word in [
            "forest",
            "rainforest",
            "savanna",
            "steppe",
            "tundra",
            "grass",
            "vegetation",
            "biome",
            "jungle",
        ] {
            assert!(
                names.iter().all(|n| !n.to_lowercase().contains(word)),
                "a class is named for life: {word}"
            );
        }
    }

    #[test]
    fn the_crater_state_scale_is_eight_codes_an_octave_from_one_per_square_kilometre_at_216() {
        let state = |n: f64| CraterState::from_density(PerSquareKilometre::new(n)).unwrap();
        assert_eq!(state(0.0), CraterState::NONE_RETAINED);
        assert_eq!(state(-0.0), CraterState::NONE_RETAINED);
        assert_eq!(state(1.0).get(), 216);
        assert_eq!(state(2.0).get(), 224);
        assert_eq!(state(0.5).get(), 208);
        assert_same_bits(CraterState::new(216).density().unwrap().value(), 1.0);
        assert_same_bits(CraterState::new(224).density().unwrap().value(), 2.0);
        // The scale's ends: 8.9 × 10⁻⁹ and 29.3 km⁻², a tiny density held to the first code and a
        // great one to the last.
        let first = CraterState::new(2).density().unwrap().value();
        let last = CraterState::new(255).density().unwrap().value();
        assert!((first - 8.9e-9).abs() < 0.05e-9, "{first}");
        assert!((last - 29.3).abs() < 0.05, "{last}");
        assert_eq!(state(1e-30).get(), 2);
        assert_eq!(state(1e6).get(), 255);
        assert_eq!(CraterState::UNCLASSIFIED.density(), None);
        assert_eq!(
            CraterState::NONE_RETAINED.density(),
            Some(PerSquareKilometre::ZERO)
        );
    }

    #[test]
    fn every_density_code_reads_back_within_half_a_step() {
        for code in 2..=u8::MAX {
            let n = CraterState::new(code).density().unwrap();
            assert_eq!(CraterState::from_density(n).unwrap().get(), code);
        }
    }

    #[test]
    fn a_crater_state_of_no_finite_non_negative_density_is_refused() {
        assert_eq!(
            CraterState::from_density(PerSquareKilometre::new(f64::NAN)).map_err(|e| e.to_string()),
            Err(QuantiseValueError::NotFinite(f64::NAN).to_string())
        );
        assert_eq!(
            CraterState::from_density(PerSquareKilometre::new(-1.0)),
            Err(QuantiseValueError::OutOfRange(-1.0))
        );
    }
}
