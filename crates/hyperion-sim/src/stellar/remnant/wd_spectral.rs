//! White-dwarf spectral types after Sion et al. (1983, ApJ 269, 253): `DA`, `DB`, `DO`, `DC`, `DQ`
//! and `DZ`, a `Z` for metal lines, and the temperature index 50,400 K ÷ T<sub>eff</sub> (plan 06,
//! P06.T20.b).
//!
//! A white dwarf's type follows from its effective temperature and three fixed marks, each
//! compared with a threshold that moves with the temperature, the pattern of the brainstorm's
//! "Random streams, not a random sequence": one mark answers at every age.
//!
//! 1. **Atmosphere.** The mark `star.wd.atmosphere` below the helium-atmosphere fraction
//!    f<sub>He</sub>(T<sub>eff</sub>) gives a helium atmosphere, otherwise a hydrogen one. The
//!    fraction is measured, not modelled, and it is not monotone in temperature:
//!    - from 75,000 K to 30,000 K it falls as the star cools, from 24% to 8%: the
//!      volume-corrected fraction of Bédard et al. (2020, ApJ 901, 93, Fig. 19, 1,467 hot white
//!      dwarfs of SDSS, bins of 0.05 dex). Residual hydrogen floats up, and about two thirds of the
//!      helium-rich stars turn into DAs (their section 6). Above 75,000 K it is held at 24%, which
//!      they find is the share of all white dwarfs "born with hydrogen-deficient atmospheres":
//!      their fraction rises to 87% above 90,000 K only because hydrogen-rich stars cross those
//!      temperatures faster, which is no change in any one star;
//!    - below 25,000 K it rises as the star cools, from 9% at 15,000–25,000 K to 20% at
//!      11,000–13,000 K (convective dilution of thin hydrogen layers) and 32% at 5,500–7,000 K
//!      (convective mixing): Kilic et al. (2025, ApJ 979, 157, Table 4, the 100 pc sample in the
//!      SDSS footprint).
//!
//!    Both are made monotone within their range by pooling adjacent bins that reverse, weighted by
//!    the inverse variance of their errors (Bédard et al.'s 53,000–63,000 K and 32,000–40,000 K;
//!    Kilic et al.'s 10,000–13,000 K and 5,500–7,000 K). So a star that has cooled below
//!    30,000 K never turns from a helium type back to DA, and above it the one change the
//!    measurements show, helium to hydrogen, is the only one possible. The fraction is linear in
//!    log₁₀ T<sub>eff</sub> between bin centres and held beyond the first and last.
//! 2. **Type.** A hydrogen atmosphere is DA down to 5,000 K, where the Balmer lines vanish, and
//!    DC below (the coolest DAs of the 40 pc and 100 pc samples are at 4,906 and 4,675 K). A
//!    helium atmosphere is DO above 45,000 K (He II) and DB down to 11,000 K (He I), where Kilic et
//!    al. find the DBs giving way (44% of helium atmospheres at 11,000–12,000 K, 18% at
//!    10,000–11,000 K). Below that it is DQ when the mark `star.wd.carbon` lies below the DQ
//!    fraction of helium atmospheres, and DC otherwise. That fraction rises from 5% at
//!    10,000–12,000 K to a peak of 38% at 8,000–9,000 K and falls to 13% at 5,000–7,000 K and 1%
//!    below, as carbon dredged up by the convection zone first appears and then sinks out of view
//!    (Kilic et al. 2025, binned here from their Table 2 catalogue in 1,000 K bins, the two hottest
//!    pooled).
//! 3. **Metals.** The mark `star.wd.metals` below the share of each atmosphere with metal lines
//!    in the Gaia 40 pc sample (O'Brien et al. 2024, MNRAS 527, 8687, Table 2) adds a `Z`: 8.0% of
//!    DAs (53 DAZ of 659) and 15.8% of the rest (66 of 417). A featureless (DC) atmosphere with
//!    metals is a DZ. The combined share shows no trend with cooling age in that sample (6–15% in
//!    every bin from 0.1 to 8 Gyr, as binned here from their catalogue), so it is one number for
//!    all ages, and needs no cooling age. Among the DAs alone it falls from 12–13% at 0.5–2 Gyr to
//!    4% at 4–8 Gyr (6 of 141), about two standard deviations, which is left for when the
//!    classification reads a cooling age.
//!
//! The index is 50,400 ÷ T<sub>eff</sub> written to one decimal, as Gianninas et al. (2011, ApJ
//! 743, 138) do (Sion et al. gave it as an integer).

use core::fmt;

use crate::math;
use crate::rng::{Mark, Threshold};
use crate::stellar::draws::StarDraws;
use crate::units::Kelvin;

/// A white dwarf's atmosphere: which element dominates its photosphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WhiteDwarfAtmosphere {
    /// Hydrogen-dominated: DA, or DC below 5,000 K.
    Hydrogen,
    /// Helium-dominated: DO, DB, DQ, DC or DZ.
    Helium,
}

/// The primary spectral class of a white dwarf, from the strongest features of its spectrum
/// (Sion et al. 1983).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WhiteDwarfClass {
    /// DA: Balmer lines only.
    Da,
    /// DB: He I lines.
    Db,
    /// DO: He II lines.
    Do,
    /// DC: a continuous spectrum without lines.
    Dc,
    /// DQ: carbon features.
    Dq,
    /// DZ: metal lines only.
    Dz,
}

impl WhiteDwarfClass {
    /// The class as written: `DA`, `DB`, `DO`, `DC`, `DQ`, `DZ`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Da => "DA",
            Self::Db => "DB",
            Self::Do => "DO",
            Self::Dc => "DC",
            Self::Dq => "DQ",
            Self::Dz => "DZ",
        }
    }
}

/// A white dwarf's spectral type: its class, any metal lines, and its temperature index.
///
/// Its `Display` is the type as written: `DA4.2`, `DAZ3.1`, `DB2.3`, `DQ6.2`, `DZ9.8`, `DO0.8`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhiteDwarfType {
    class: WhiteDwarfClass,
    atmosphere: WhiteDwarfAtmosphere,
    metal_lines: bool,
    temperature_index: Option<f64>,
}

impl WhiteDwarfType {
    /// The primary class.
    #[must_use]
    pub const fn class(&self) -> WhiteDwarfClass {
        self.class
    }

    /// The dominant element of the atmosphere.
    #[must_use]
    pub const fn atmosphere(&self) -> WhiteDwarfAtmosphere {
        self.atmosphere
    }

    /// Whether the spectrum shows metal lines: a `Z` after the class letter, or the class DZ.
    #[must_use]
    pub const fn has_metal_lines(&self) -> bool {
        self.metal_lines
    }

    /// The temperature index 50,400 K ÷ T<sub>eff</sub>, unrounded; `None` for a white dwarf
    /// without a temperature.
    #[must_use]
    pub const fn temperature_index(&self) -> Option<f64> {
        self.temperature_index
    }

    /// A type from its parts, for the classification's test parser.
    #[cfg(test)]
    pub(crate) const fn from_parts(
        class: WhiteDwarfClass,
        atmosphere: WhiteDwarfAtmosphere,
        metal_lines: bool,
        temperature_index: Option<f64>,
    ) -> Self {
        Self {
            class,
            atmosphere,
            metal_lines,
            temperature_index,
        }
    }
}

impl fmt::Display for WhiteDwarfType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.class.as_str())?;
        if self.metal_lines && self.class != WhiteDwarfClass::Dz {
            f.write_str("Z")?;
        }
        match self.temperature_index {
            Some(index) => write!(f, "{index:.1}"),
            None => Ok(()),
        }
    }
}

/// The numerator of Sion et al.'s temperature index, K: θ<sub>eff</sub> = 5,040 K ÷
/// T<sub>eff</sub>, times ten.
const TEMPERATURE_INDEX_K: f64 = 50_400.0;

/// The hottest a DB is, K: above it a helium atmosphere shows He II and is a DO (the hot edge of
/// the historical DB gap, 45,000–30,000 K, "between the hot DO stars and cooler DB stars":
/// Bédard et al. 2020, section 1).
const DO_MIN_TEFF_K: f64 = 45_000.0;

/// The coolest a DB is, K: below it He I fades and a helium atmosphere is DC or DQ (Kilic et al.
/// 2025).
const DB_MIN_TEFF_K: f64 = 11_000.0;

/// The coolest a DA is, K: below it the Balmer lines vanish and a hydrogen atmosphere is DC (the
/// coolest DAs of the Gaia 40 pc sample, O'Brien et al. 2024, and of the 100 pc SDSS sample, Kilic
/// et al. 2025, are at 4,906 and 4,675 K).
const DA_MIN_TEFF_K: f64 = 5_000.0;

/// The share of hydrogen atmospheres with metal lines: 53 DAZ of 659 DAs in the Gaia 40 pc sample
/// (O'Brien et al. 2024, Table 2).
const HYDROGEN_METAL_SHARE: f64 = 53.0 / 659.0;

/// The share of the other white dwarfs with metal lines: 66 of 417 in the Gaia 40 pc sample
/// (O'Brien et al. 2024, Table 2).
const HELIUM_METAL_SHARE: f64 = 66.0 / 417.0;

/// A fraction against temperature: (T<sub>eff</sub> in K, fraction), hottest first.
type Curve = [(f64, f64)];

/// f<sub>He</sub>(T<sub>eff</sub>), the fraction of white dwarfs with a helium atmosphere (see the
/// [module](self) documentation): Bédard et al. (2020, Fig. 19) at the centres of their 0.05 dex
/// bins from 75,000 to 30,000 K, Kilic et al. (2025, Table 4) at the centres of theirs below 25,000
/// K, each pooled where adjacent bins reverse.
const HELIUM_FRACTION: [(f64, f64); 19] = [
    (74_989.0, 0.239),
    (66_834.0, 0.203),
    (59_566.0, 0.1512),
    (53_088.0, 0.1512),
    (47_315.0, 0.147),
    (42_170.0, 0.138),
    (37_584.0, 0.109),
    (33_497.0, 0.109),
    (29_854.0, 0.077),
    (22_500.0, 0.087),
    (17_500.0, 0.094),
    (14_000.0, 0.167),
    (12_000.0, 0.202),
    (10_500.0, 0.202),
    (9_500.0, 0.243),
    (8_500.0, 0.254),
    (7_500.0, 0.288),
    (6_500.0, 0.3166),
    (5_750.0, 0.3166),
];

/// The fraction of helium atmospheres below 11,000 K that are DQ: Kilic et al. (2025, Table 2)
/// in 1,000 K bins, the 10,000–12,000 K pair pooled (2 of 40); the coolest bin, 4,000–5,000 K, is
/// of a part of their sample not targeted for spectroscopy.
const DQ_FRACTION: [(f64, f64); 8] = [
    (11_500.0, 0.05),
    (10_500.0, 0.05),
    (9_500.0, 0.122),
    (8_500.0, 0.377),
    (7_500.0, 0.257),
    (6_500.0, 0.129),
    (5_500.0, 0.129),
    (4_500.0, 0.014),
];

/// A curve's value at `log_teff`: linear in log₁₀ T<sub>eff</sub> between its points, held beyond
/// them.
#[must_use]
fn curve_at(curve: &Curve, log_teff: f64) -> f64 {
    let mut previous: Option<(f64, f64)> = None;
    for &(t, value) in curve {
        let x = math::log10(t);
        if log_teff >= x {
            return match previous {
                None => value,
                Some((x0, v0)) => v0 + (x0 - log_teff) / (x0 - x) * (value - v0),
            };
        }
        previous = Some((x, value));
    }
    previous.map_or(0.0, |(_, value)| value)
}

/// Whether `mark` lies below probability `p`.
#[must_use]
fn below(mark: Mark, p: f64) -> bool {
    mark.is_below(Threshold::from_probability(p.clamp(0.0, 1.0)))
}

/// The spectral type of a white dwarf of effective temperature `teff` with the marks of `draws`
/// (see the [module](self) documentation).
///
/// A white dwarf without a temperature (none has one below zero, and the cooling laws never
/// reach zero) is read as a featureless DC with no index.
///
/// # Examples
///
/// The median star (every mark one half) has a hydrogen atmosphere below 90,000 K, where fewer
/// than half of white dwarfs have helium ones, and is a DA until 5,000 K:
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::remnant::wd_spectral::white_dwarf_type;
/// use hyperion_sim::units::Kelvin;
///
/// let median = StarDraws::median();
/// assert_eq!(white_dwarf_type(Kelvin::new(12_000.0), &median).to_string(), "DA4.2");
/// assert_eq!(white_dwarf_type(Kelvin::new(4_200.0), &median).to_string(), "DC12.0");
/// ```
#[must_use]
pub fn white_dwarf_type(teff: Kelvin, draws: &StarDraws) -> WhiteDwarfType {
    let t = teff.value();
    if !(t.is_finite() && t > 0.0) {
        return WhiteDwarfType {
            class: WhiteDwarfClass::Dc,
            atmosphere: WhiteDwarfAtmosphere::Hydrogen,
            metal_lines: false,
            temperature_index: None,
        };
    }
    let log_teff = math::log10(t);
    let atmosphere = if below(draws.wd_atmosphere(), curve_at(&HELIUM_FRACTION, log_teff)) {
        WhiteDwarfAtmosphere::Helium
    } else {
        WhiteDwarfAtmosphere::Hydrogen
    };
    let (lines, metal_share) = match atmosphere {
        WhiteDwarfAtmosphere::Hydrogen if t >= DA_MIN_TEFF_K => {
            (WhiteDwarfClass::Da, HYDROGEN_METAL_SHARE)
        }
        WhiteDwarfAtmosphere::Hydrogen => (WhiteDwarfClass::Dc, HYDROGEN_METAL_SHARE),
        WhiteDwarfAtmosphere::Helium if t >= DO_MIN_TEFF_K => {
            (WhiteDwarfClass::Do, HELIUM_METAL_SHARE)
        }
        WhiteDwarfAtmosphere::Helium if t >= DB_MIN_TEFF_K => {
            (WhiteDwarfClass::Db, HELIUM_METAL_SHARE)
        }
        WhiteDwarfAtmosphere::Helium
            if below(draws.wd_carbon(), curve_at(&DQ_FRACTION, log_teff)) =>
        {
            (WhiteDwarfClass::Dq, HELIUM_METAL_SHARE)
        }
        WhiteDwarfAtmosphere::Helium => (WhiteDwarfClass::Dc, HELIUM_METAL_SHARE),
    };
    let metal_lines = below(draws.wd_metals(), metal_share);
    let class = if metal_lines && lines == WhiteDwarfClass::Dc {
        WhiteDwarfClass::Dz
    } else {
        lines
    };
    WhiteDwarfType {
        class,
        atmosphere,
        metal_lines,
        temperature_index: Some(TEMPERATURE_INDEX_K / t),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::Seed;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};
    use crate::rng::{ObjectKey, Stream, tags};
    use crate::stellar::draws::StarDrawsParts;

    /// T<sub>eff</sub> against cooling age of a 0.6 M☉ carbon–oxygen white dwarf with a thick
    /// hydrogen layer: the Montreal sequence of Bédard et al. (2020), as tabulated in
    /// `CoolingModels/Tables/Table_Mass_0.6` (pure-hydrogen block, file of 13 January 2021; the
    /// models start at 100,000 K, which is age zero here). (age in years, T<sub>eff</sub> in K).
    const MONTREAL_0_6_DA: [(f64, f64); 56] = [
        (0.0, 100_000.0),
        (1.029e5, 90_000.0),
        (1.977e5, 85_000.0),
        (3.165e5, 80_000.0),
        (4.625e5, 75_000.0),
        (6.414e5, 70_000.0),
        (8.627e5, 65_000.0),
        (1.145e6, 60_000.0),
        (1.515e6, 55_000.0),
        (2.005e6, 50_000.0),
        (2.690e6, 45_000.0),
        (3.732e6, 40_000.0),
        (5.466e6, 35_000.0),
        (8.900e6, 30_000.0),
        (1.818e7, 25_000.0),
        (5.866e7, 20_000.0),
        (1.234e8, 17_000.0),
        (1.386e8, 16_500.0),
        (1.555e8, 16_000.0),
        (1.743e8, 15_500.0),
        (1.954e8, 15_000.0),
        (2.189e8, 14_500.0),
        (2.451e8, 14_000.0),
        (2.745e8, 13_500.0),
        (3.076e8, 13_000.0),
        (3.450e8, 12_500.0),
        (3.874e8, 12_000.0),
        (4.358e8, 11_500.0),
        (4.917e8, 11_000.0),
        (5.567e8, 10_500.0),
        (6.328e8, 10_000.0),
        (7.230e8, 9_500.0),
        (8.315e8, 9_000.0),
        (9.637e8, 8_500.0),
        (1.128e9, 8_000.0),
        (1.334e9, 7_500.0),
        (1.599e9, 7_000.0),
        (1.950e9, 6_500.0),
        (2.418e9, 6_000.0),
        (3.367e9, 5_500.0),
        (4.688e9, 5_250.0),
        (6.390e9, 5_000.0),
        (7.619e9, 4_750.0),
        (8.517e9, 4_500.0),
        (9.254e9, 4_250.0),
        (9.886e9, 4_000.0),
        (1.050e10, 3_750.0),
        (1.106e10, 3_500.0),
        (1.157e10, 3_250.0),
        (1.208e10, 3_000.0),
        (1.259e10, 2_750.0),
        (1.312e10, 2_500.0),
        (1.368e10, 2_250.0),
        (1.426e10, 2_000.0),
        (1.488e10, 1_750.0),
        (1.559e10, 1_500.0),
    ];

    /// T<sub>eff</sub> at cooling age `age` (years) on the Montreal sequence: linear in age from
    /// zero to the first cooled model, then linear in log age against log T<sub>eff</sub>.
    fn montreal_teff(age: f64) -> Kelvin {
        let rows = &MONTREAL_0_6_DA;
        let after = rows
            .partition_point(|&(a, _)| a <= age)
            .clamp(1, rows.len() - 1);
        let ((a0, t0), (a1, t1)) = (rows[after - 1], rows[after]);
        let fraction = if a0 <= 0.0 {
            (age / a1).clamp(0.0, 1.0)
        } else {
            ((math::log10(age) - math::log10(a0)) / (math::log10(a1) - math::log10(a0)))
                .clamp(0.0, 1.0)
        };
        let log_t = math::log10(t0) + fraction * (math::log10(t1) - math::log10(t0));
        Kelvin::new(math::exp10(log_t))
    }

    /// The draws of star `i` of a sample: body 0 of system 0 in a cell of its own.
    fn draws(i: u32) -> StarDraws {
        let x = i32::try_from(i % 8_000).unwrap() - 4_000;
        let y = i32::try_from(i / 8_000).unwrap();
        let cell = GenCell::new(CellSize::Ly8, [x, y, 0]).unwrap();
        let star = BodyId::new(SystemId::from_parts(Layer::A, cell, 0).unwrap(), 0);
        StarDraws::for_star(Seed::new(0x0d0e_5ee0), star)
    }

    fn draws_with(atmosphere: u64, carbon: u64, metals: u64) -> StarDraws {
        StarDraws::from_parts(StarDrawsParts {
            wd_atmosphere: Mark::from_word(atmosphere),
            wd_carbon: Mark::from_word(carbon),
            wd_metals: Mark::from_word(metals),
            ..StarDrawsParts::MEDIAN
        })
    }

    fn written(teff: f64, d: &StarDraws) -> String {
        white_dwarf_type(Kelvin::new(teff), d).to_string()
    }

    /// Types are written as catalogues write them, with the index to one decimal.
    #[test]
    fn types_are_written_with_their_index() {
        let hydrogen = draws_with(u64::MAX, u64::MAX, u64::MAX);
        let helium = draws_with(0, u64::MAX, u64::MAX);
        let carbon = draws_with(0, 0, u64::MAX);
        let metals = draws_with(u64::MAX, u64::MAX, 0);
        let helium_metals = draws_with(0, u64::MAX, 0);
        assert_eq!(written(12_000.0, &hydrogen), "DA4.2");
        assert_eq!(written(4_800.0, &hydrogen), "DC10.5");
        assert_eq!(written(60_000.0, &helium), "DO0.8");
        assert_eq!(written(20_000.0, &helium), "DB2.5");
        assert_eq!(written(8_000.0, &helium), "DC6.3");
        assert_eq!(written(8_000.0, &carbon), "DQ6.3");
        assert_eq!(written(16_000.0, &metals), "DAZ3.1");
        assert_eq!(written(4_000.0, &metals), "DZ12.6");
        assert_eq!(written(7_000.0, &helium_metals), "DZ7.2");
        assert_eq!(written(15_000.0, &helium_metals), "DBZ3.4");
        let dark = white_dwarf_type(Kelvin::ZERO, &hydrogen);
        assert_eq!(dark.to_string(), "DC");
        assert_eq!(dark.temperature_index(), None);
        let t = white_dwarf_type(Kelvin::new(8_000.0), &carbon);
        assert_eq!(t.atmosphere(), WhiteDwarfAtmosphere::Helium);
        assert!(!t.has_metal_lines());
        assert!((t.temperature_index().unwrap() - 6.3).abs() < 1e-12);
    }

    /// The helium fraction falls as a star cools from the hottest bin to 29,854 K and rises from
    /// there to the coolest; at 20,000 K it is about the plan's 10%, below 10,000 K about 30%.
    #[test]
    fn the_helium_fraction_has_one_minimum() {
        let minimum = HELIUM_FRACTION
            .iter()
            .position(|&(t, _)| (t - 29_854.0).abs() < 0.5)
            .unwrap();
        for pair in HELIUM_FRACTION[..=minimum].windows(2) {
            assert!(pair[0].1 >= pair[1].1, "{pair:?}");
        }
        for pair in HELIUM_FRACTION[minimum..].windows(2) {
            assert!(pair[0].1 <= pair[1].1, "{pair:?}");
        }
        let at = |t: f64| curve_at(&HELIUM_FRACTION, math::log10(t));
        assert!((0.08..0.11).contains(&at(20_000.0)), "{}", at(20_000.0));
        assert!((0.24..0.32).contains(&at(9_000.0)), "{}", at(9_000.0));
        assert!((at(3_000.0) - 0.3166).abs() < 1e-12);
        assert!((at(200_000.0) - 0.239).abs() < 1e-12);
        // The DQ fraction rises to one peak at 8,500 K and falls away from it.
        let peak = DQ_FRACTION
            .iter()
            .position(|&(t, _)| (t - 8_500.0).abs() < 0.5)
            .unwrap();
        assert!(DQ_FRACTION[..=peak].windows(2).all(|p| p[0].1 <= p[1].1));
        assert!(DQ_FRACTION[peak..].windows(2).all(|p| p[0].1 >= p[1].1));
    }

    /// The plan's test: a fixed star's type over 10 Gyr of cooling never turns from a helium type
    /// back to DA. It holds from the minimum of the helium fraction, 29,854 K, down; above it
    /// helium-rich stars do turn into DAs as hydrogen floats up (Bédard et al. 2020), and some of
    /// the sample's do.
    #[test]
    fn no_star_turns_from_helium_back_to_da_below_30000_k() {
        let mut floated_up = 0;
        let mut mixed = 0;
        for i in 0..2_000 {
            let d = draws(i);
            let mut helium_seen = false;
            let mut hot_helium = false;
            let mut age = 1e3_f64;
            while age < 1e10 {
                let teff = montreal_teff(age);
                let wd = white_dwarf_type(teff, &d);
                let helium = wd.atmosphere() == WhiteDwarfAtmosphere::Helium;
                if teff.value() > 29_854.0 {
                    hot_helium |= helium;
                    if hot_helium && !helium && wd.class() == WhiteDwarfClass::Da {
                        floated_up += 1;
                        hot_helium = false;
                    }
                } else {
                    if helium_seen {
                        assert_ne!(wd.class(), WhiteDwarfClass::Da, "star {i} at {teff:?}");
                        assert!(helium, "star {i} back to hydrogen at {teff:?}");
                    }
                    if helium && !helium_seen && teff.value() < 20_000.0 {
                        mixed += 1;
                    }
                    helium_seen |= helium;
                }
                age *= 1.02;
            }
        }
        assert!(floated_up > 50, "{floated_up} DO → DA");
        assert!(mixed > 50, "{mixed} DA → helium");
    }

    /// The plan's test, on a documented cooling sample: over a thin disc forming white dwarfs at
    /// a constant rate for 9 Gyr (cooling ages uniform over 0–9 Gyr on the Montreal 0.6 M☉
    /// sequence), the types of the white dwarfs above 5,000 K come out DA 65–85%, DC, DQ and DZ
    /// together 10–30%, and DB 1–3%.
    ///
    /// The volume-limited censuses are spectroscopically complete above 5,000 K only (O'Brien et
    /// al. 2024; Kilic et al. 2025), and below it every white dwarf is DC or DZ whatever its
    /// atmosphere, so the comparison is made above it, where they give DA 72%, DC + DQ + DZ 26%
    /// and DB 1.7–1.9%. The plan's DB 2–10% is below every measured sample (1.1–2.0%) and is
    /// corrected to 1–3%.
    #[test]
    fn a_thin_disc_sample_matches_the_local_census() {
        let mut stream = Stream::open(
            Seed::new(0x7417_d15c),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let mut counts = BTreeMap::<&str, u32>::new();
        let mut total = 0_u32;
        for i in 0..100_000 {
            let teff = montreal_teff(9e9 * stream.uniform());
            if teff.value() < 5_000.0 {
                continue;
            }
            let wd = white_dwarf_type(teff, &draws(i));
            *counts.entry(wd.class().as_str()).or_default() += 1;
            total += 1;
        }
        let share = |classes: &[&str]| {
            let n: u32 = classes
                .iter()
                .map(|c| counts.get(c).copied().unwrap_or(0))
                .sum();
            f64::from(n) / f64::from(total)
        };
        let (da, db, dc_dq_dz) = (share(&["DA"]), share(&["DB"]), share(&["DC", "DQ", "DZ"]));
        assert!((0.65..=0.85).contains(&da), "DA {da}: {counts:?}");
        assert!(
            (0.10..=0.30).contains(&dc_dq_dz),
            "DC+DQ+DZ {dc_dq_dz}: {counts:?}"
        );
        assert!((0.01..=0.03).contains(&db), "DB {db}: {counts:?}");
        assert!(share(&["DQ"]) > 0.02 && share(&["DZ"]) > 0.02, "{counts:?}");
        assert!(total > 60_000, "{total}");
    }

    /// Metal lines at the 40 pc sample's rates, whatever the temperature.
    #[test]
    fn metal_lines_follow_the_40_pc_rates() {
        assert!((HYDROGEN_METAL_SHARE - 0.0804).abs() < 1e-4);
        assert!((HELIUM_METAL_SHARE - 0.1583).abs() < 1e-4);
        let mut stream = Stream::open(Seed::new(3), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let (mut hydrogen, mut with_metals) = (0_u32, 0_u32);
        for _ in 0..20_000 {
            let d = draws_with(u64::MAX, u64::MAX, stream.next_u64());
            let wd = white_dwarf_type(Kelvin::new(15_000.0), &d);
            hydrogen += 1;
            with_metals += u32::from(wd.has_metal_lines());
        }
        let rate = f64::from(with_metals) / f64::from(hydrogen);
        assert!((rate - HYDROGEN_METAL_SHARE).abs() < 0.006, "{rate}");
    }
}
