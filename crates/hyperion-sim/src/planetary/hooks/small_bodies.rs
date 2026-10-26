//! A small body's surface type, its taxonomic class, from plain inputs ([`TaxonomicClass::of`]).
//!
//! Plan 14's P14.T51.e, by decision-p14-t51e-contract, with its Q/S split by
//! decision-p14-t51e-qs. The class is a label and not a substance: no optics, albedo or registry
//! row moves with it.
//!
//! # The inputs
//!
//! - **The body's kind** ([`TaxonomyBodyKind`]): a planet, an asteroid belt's member, a Kuiper-belt
//!   body (a Kuiper-like belt's member or a moon of one), or another moon.
//! - **Its lid's lithology**, a P14.T51.c [`CrustLithology`], which a Kuiper-belt body need not
//!   give. The wiring reads it through `CrustComposition::lid_lithology`, P14.T54.b.c's getter.
//! - **Its veneer** ([`Veneer`]): the tholin-covered share of its surface, `s_t`, T51.d's organic
//!   ground times its organic cover, and the uncovered perennial ice share, `s_i`.
//! - **Its weathering clocks** ([`WeatheringClocks`]), which only an ordinary chondrite's split
//!   reads: P14.T24.b's surface age, T51.d's maturity, and T51.d's wind term at an unshielded share
//!   of 1 with its value at the anchor ([`WindTerms`], decision-p14-t51e-qs §1.2). The caller
//!   computes both wind terms by T51.d's rule; this module names nothing of T51.d's.
//!
//! Each is checked once where it is built, and one out of its range is refused there
//! ([`BuildTaxonomyInputsError`]).
//!
//! # The class, by the first rule that applies
//!
//! (decision-p14-t51e-contract §3.1)
//!
//! 1. **A planet** is not classed.
//! 2. **A Kuiper-belt body**, whatever its lid, takes a colour and never a Bus–`DeMeo` letter: red
//!    where `s_t` ≥ [`VENEER_SHARE`], neutral otherwise. Large and medium bodies show their present
//!    surface (Brown 2012, Annu. Rev. Earth Planet. Sci. 40, 467: Quaoar red, Orcus neutral);
//!    Fraser and Brown 2012's bimodality (ApJ 749, 33) holds only below about 400 km, the members'
//!    floor. Its moons are included (Charon is neutral water ice).
//! 3. **Another member or moon whose lid is classed** (rule 4's table) is D where `s_t` ≥
//!    [`VENEER_SHARE`], and not classed where `s_i` ≥ [`VENEER_SHARE`], an icy surface the asteroid
//!    letters do not describe (Saturn's small icy moons): a majority veneer hides the lid in the
//!    disk-integrated spectrum (Brown 2012's "frosty veneer hiding the inherent surface
//!    composition").
//! 4. **Otherwise its lid's class**, by an exhaustive match over [`CrustLithology`] with no
//!    wildcard, so that a new lithology fails to compile until it is placed
//!    (decision-p14-t51e-contract §3.2):
//!    - S, or Q where fresh ("The Q/S split"), for `chondrite_h`, `chondrite_l` and
//!      `chondrite_ll` (Nakamura et al. 2011, Itokawa's LL);
//!    - C for `chondrite_ci` and a `phyllosilicate` lid (Ceres is C in `DeMeo` et al. 2009);
//!    - Ch for `chondrite_cm`, whose clays carry the 0.7 µm band (`DeMeo` et al. 2022);
//!    - K for `chondrite_cv`, a convention (`DeMeo` et al. 2022 §4.9; Mahlke et al. 2023);
//!    - Xc for `chondrite_e` (Lutetia);
//!    - V for `basalt_fe` and `basalt` (Vesta; Binzel and Xu 1993);
//!    - A for `ultramafic` (`DeMeo` et al. 2022 §4.7), assuming an olivine-dominated lid,
//!      labelled;
//!    - P for `cometary` (`DeMeo` and Carry 2013's low-albedo X);
//!    - not classed for `H2O`, `graphite`, `feldspathic`, `komatiite`, `granite` and
//!      `basalt_reduced`: a closed set, for the composition audit.
//!
//! Each class's source is on its [`TaxonomicClass`] variant. CM → Ch rather than C, and `basalt`
//! → V beside `basalt_fe`, are labels, accepted as reversible. A letter on a body larger than any
//! classed asteroid (Ceres's 940 km) is its lid's spectral analogue, labelled.
//!
//! No lithology maps to a mixture of classes, so nothing draws: K for every `chondrite_cv`, since
//! no published criterion separates K from L (Mahlke et al. 2023, A&A 676, A94: the classes are
//! "continuous"), and L is produced by no rule (decision-p14-t51e-contract §3.5).
//!
//! # The Q/S split
//!
//! An ordinary chondrite's surface is S where either of two clocks has run, a tie included, and Q
//! otherwise (decision-p14-t51e-qs §1.2), never by T51.d's lunar m alone, whose `τ_L` ÷ k, 2.8 Gyr
//! at 2.5 au, is a gardened regolith's and would make every such member younger than 1.4 Gyr Q:
//!
//! 1. **The wind's clock**, the asteroid surface's own: it has run where `t_x` w₀ ≥ `T_Q` `w_ref`,
//!    compared without dividing. `t_x` is the exposure age, P14.T24.b's surface age; `T_Q` is
//!    [`Q_TYPE_WEATHERING_TIME`]; w₀ is the body's wind term, and `w_ref` the same term on a
//!    circular orbit at [`Q_TYPE_ANCHOR_DISTANCE`] about T51.d's reference Sun. A host with no wind
//!    (w₀ = +0) never runs it, since `w_ref` is positive. Its time, `τ_Q` = `T_Q` `w_ref` ÷ w₀
//!    ([`WeatheringClocks::q_type_time`]), is for tests and the census and is not read by the
//!    comparison.
//! 2. **The regolith's clock**: T51.d's maturity m at least [`IMMATURE_MATURITY`]. It decides only
//!    for a host with little or no wind, where the micrometeoroids still mature the ground.
//!
//! A body under air above T51.d's ion floor is S, labelled, since neither the wind nor the dust
//! reaches its ground ([`AirShield::AboveIonFloor`]). No other lithology is ever Q: Q is the
//! ordinary chondrites' fresh spectrum (Binzel et al. 2010, Nature 463, 331).
//!
//! A moon passes its planet's heliocentric orbit for w₀, unshielded even inside its planet's
//! magnetopause: the planet's plasma, which T51.d does not model, stands in at the wind's open
//! flux, labelled.
//!
//! # What is not given
//!
//! [`TaxonomicClass::of`] refuses a body whose rule reads an input it was not given
//! ([`ClassifyTaxonomyError::NotGiven`]), which the wiring maps to not modelled, never to none
//! (decision-p14-t51e-contract §1.5):
//!
//! - an asteroid belt's member or another moon with no lid;
//! - a Kuiper-belt body, or a member or moon whose lid is classed, with no veneer (the record's
//!   modifiers not modelled);
//! - an ordinary chondrite that rule 3 leaves to its lid, with no clocks.
//!
//! A planet reads nothing, and a lid of the closed not-classed set reads no veneer.
//!
//! # Not produced, recorded
//!
//! (decision-p14-t51e-contract §5.1)
//!
//! - A rubble pile's or a fragment's parent lithology: the generator records no parents.
//! - Trojans and Hildas: no co-orbital population is generated. Their D share is set at formation
//!   (Wong and Brown 2016, AJ 152, 90: H₂S retention, with no value given), which needs a sourced
//!   retention temperature against the derivation's formation temperature, a later value change in
//!   the one function that holds the cover rule (`cover_class`).
//! - Xe for an aubrite lid, which has no registry row.
//!
//! # On the record
//!
//! P14.T51.e's wiring, `planetary/system/small_bodies.rs`, fills the record's class
//! ([`record::Surface::taxonomic_class`](crate::planetary::record::Surface::taxonomic_class)) from
//! each planet's, member's and moon's built surface, behind its hold, `SMALL_BODY_TYPES_VERSION` =
//! 24, as a P14.T54.b.j rider, so that nothing on the record moves at generator version 23. The
//! tripwire test (`small_bodies_nothing_on_the_record_reads_the_types_yet`) holds every other file
//! off this module's entry points until P14.T54.b.l deletes it (decision-p14-t51e-contract §1.4,
//! §3.7 and §4).
//!
//! # Determinism
//!
//! Pure functions of their arguments, with no draw and so no domain tag (decision-p14-t51e-contract
//! §3.5). The split compares two products of `f64`s, with no division and no transcendental.

#[cfg(test)]
mod tests;

use core::fmt;
use std::error::Error;

use crate::planetary::hooks::crust::CrustLithology;
use crate::units::{AstronomicalUnits, Years};

/// The time an ordinary chondrite's fresh, Q-type surface takes to weather to S at
/// [`Q_TYPE_ANCHOR_DISTANCE`] about T51.d's reference Sun, `T_Q`: 1 Myr, 10⁶ Julian years
/// (decision-p14-t51e-qs §3.1).
///
/// Vernazza et al. 2009 (Nature 458, 993, abstract, read by the ruling through Europe PMC): "the
/// final colour of a silicate-rich asteroid is acquired shortly after its 'birth' (within 10⁶ years
/// of undergoing a catastrophic collision)", from young main-belt families, the solar wind's
/// implantation the main mechanism; Binzel et al. 2010 (Nature 463, 331, abstract, read by the
/// ruling): "in less than 10⁶ years". Vernazza's "within" makes it an upper bound, used as the
/// threshold: the largest `τ_Q` consistent with it gives the most Q, the cautious side.
/// Nesvorný et al. 2010's step-model fit to the near-Earth Q share, about 10⁶ yr at 1 au (Icarus
/// 209, 510), would be 6.25 Myr at 2.5 au and make the 5.75 Myr Karin family's weathered surface
/// (Brunetto et al. 2006, Icarus 184, 327) Q, so the belt's anchor is Vernazza's.
pub const Q_TYPE_WEATHERING_TIME: Years = Years::new(1.0e6);

/// The heliocentric distance at which [`Q_TYPE_WEATHERING_TIME`] holds: 2.5 au, a circular orbit
/// about T51.d's reference Sun (`ModifierHost::sun()`), the main belt's middle, labelled
/// (decision-p14-t51e-qs §3.1).
///
/// Vernazza et al. 2009's families lie in the main belt at about 2.2–2.9 au (*from memory*, as the
/// ruling has it), a span of 1.7 in `τ_Q` under its a² scaling, inside the bound's own looseness.
/// The caller computes the reference wind term `w_ref` there ([`WindTerms::new`]).
pub const Q_TYPE_ANCHOR_DISTANCE: AstronomicalUnits = AstronomicalUnits::new(2.5);

/// Morris's upper edge of the immature class, `I_s/FeO` (Morris 1978, Proc. LPSC 9, 2287,
/// pp. 2289–2290, as T51.d's lane read it).
const IMMATURE_IS_FEO: f64 = 30.0;

/// T51.d's stand-in for the lunar mature background's `I_s/FeO`, X: 75, inside Morris 1978's mature
/// class (the surface crate's `material/maturity.rs`, its tests' constant).
const MATURE_BACKGROUND_IS_FEO: f64 = 75.0;

/// The maturity at and above which an ordinary chondrite's regolith has weathered to S whatever
/// its wind: 0.4, dimensionless.
///
/// It is 30 ÷ 75, Morris 1978's immature edge (`I_s/FeO` 30, Proc. LPSC 9, 2287, pp. 2289–2290,
/// as T51.d's lane read it) at T51.d's mature-background stand-in X = 75 (decision-p14-t51e-qs
/// §1.2 and §3.1): the bound T51.d's Tycho test holds, not a new figure.
pub const IMMATURE_MATURITY: f64 = IMMATURE_IS_FEO / MATURE_BACKGROUND_IS_FEO;

/// The share of a surface at and above which a veneer sets the class before the lid does: 0.5,
/// dimensionless.
///
/// It is a majority of the disk, labelled, with no published value (decision-p14-t51e-contract
/// §3.3): the tholin share that makes a Kuiper-belt body red and another classed body D, and the
/// uncovered ice share that leaves a classed body unclassed. Any cover above 0 is refused, since a
/// trace patch does not redden a whole-disk spectrum (the ruling's science check). The
/// trans-Neptunian colour classes BB, BR, IR and RR are colour indices, not covered shares
/// (Fulchignoni et al. 2008, in *The Solar System Beyond Neptune*, *from memory*).
pub const VENEER_SHARE: f64 = 0.5;

/// How far a veneer's two shares may sum above 1, dimensionless: 10⁻¹², labelled.
///
/// `s_i` is the uncovered share, the caller's ice sum less `s_t`, so the two never exceed the
/// surface; the tolerance absorbs that subtraction's rounding, a few units in the last place.
const VENEER_SUM_TOLERANCE: f64 = 1.0e-12;

/// A small body's taxonomic class: a label of its surface's spectrum, not a substance (the
/// [module](self) documentation).
///
/// Closed, with all its variants at its first send (decision-p14-t51e-contract §1.6): `DeMeo` et
/// al. 2009's 24 Bus–`DeMeo` classes (Icarus 202, 160), in their S, C and X complexes and end
/// members; Tholen's P, `DeMeo` and Carry 2013's low-albedo split of X (Icarus 226, 723); and the
/// two Kuiper-belt colours. At generator version 24 the rules produce S, Q, C, Ch, K, Xc, V, A, D,
/// P, red and neutral; L and the rest are produced by no rule, so that a later rule (L by a CAI
/// criterion, Xe for an aubrite row, B or the C subclasses) needs no protocol change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaxonomicClass {
    /// A-type, an end member: [`CrustLithology::Ultramafic`]'s class.
    ///
    /// An A-type is olivine-rich, above 80% olivine (`DeMeo` et al. 2022, Icarus 380, 114971,
    /// §4.7); the rule assumes an olivine-dominated lid, labelled. Produced.
    A,
    /// B-type, of the C complex (Bennu's): produced by no rule.
    B,
    /// C-type, of the C complex: [`CrustLithology::ChondriteCi`]'s and
    /// [`CrustLithology::Phyllosilicate`]'s class.
    ///
    /// "CI samples match to C, Cb, and B-types" (`DeMeo` et al. 2022, §4.3); a phyllosilicate lid
    /// is Ceres's shell, and Ceres is C in `DeMeo` et al. 2009 (De Sanctis et al. 2015, Nature
    /// 528, 241, for its ammoniated phyllosilicates). Produced.
    C,
    /// Cb-type, of the C complex (Ryugu's): produced by no rule.
    Cb,
    /// Cg-type, of the C complex: produced by no rule.
    Cg,
    /// Cgh-type, of the C complex: produced by no rule.
    Cgh,
    /// Ch-type, of the C complex: [`CrustLithology::ChondriteCm`]'s class.
    ///
    /// The 0.7 µm band of pristine CM clays marks it (`DeMeo` et al. 2022, abstract: "pristine CM
    /// ... with Ch-type"). Produced.
    Ch,
    /// D-type, an end member: an asteroid belt's member or a moon whose lid is classed, under a
    /// tholin cover over at least [`VENEER_SHARE`] of its surface.
    ///
    /// The rule is decision-p14-t51e-contract §3.3's, after the Trojans' D (Emery et al. 2011, AJ
    /// 141, 25; Vernazza et al. 2015, ApJ 806, 204). Produced.
    D,
    /// K-type, an end member: [`CrustLithology::ChondriteCv`]'s class, a convention.
    ///
    /// "K-types matched predominantly to CV" (`DeMeo` et al. 2022, §4.9; Burbine et al. 2001,
    /// Meteorit. Planet. Sci. 36, 245). No published criterion separates K from L (Mahlke et al.
    /// 2023, A&A 676, A94). Produced.
    K,
    /// L-type, an end member: produced by no rule.
    ///
    /// Mahlke et al. 2023 find K and L "continuous", and the CV chondrites match the "Barbarian"
    /// L-types "without requiring an enrichment of CAIs"; no lithology carries a CAI share
    /// (decision-p14-t51e-contract §3.5).
    L,
    /// O-type, an end member: produced by no rule.
    O,
    /// Q-type, an end member: an ordinary chondrite's fresh surface, where neither clock has run
    /// (the [module](self)'s "The Q/S split").
    ///
    /// It is the same rock as S (Binzel et al. 2010, Nature 463, 331). No other lithology is Q.
    /// Produced.
    Q,
    /// R-type, an end member: produced by no rule.
    R,
    /// S-type, of the S complex: an ordinary chondrite's ([`CrustLithology::ChondriteH`],
    /// [`ChondriteL`](CrustLithology::ChondriteL), [`ChondriteLl`](CrustLithology::ChondriteLl))
    /// weathered surface.
    ///
    /// Itokawa's returned grains are thermally metamorphosed LL chondrite (Nakamura et al. 2011,
    /// Science 333, 1113); `DeMeo` et al. 2022 match the S complex and Q/Sq to ordinary
    /// chondrites. Produced.
    S,
    /// Sa-type, of the S complex: produced by no rule.
    Sa,
    /// Sq-type, of the S complex: produced by no rule.
    Sq,
    /// Sr-type, of the S complex: produced by no rule.
    Sr,
    /// Sv-type, of the S complex: produced by no rule.
    Sv,
    /// T-type, an end member: produced by no rule.
    T,
    /// V-type, an end member: [`CrustLithology::IronRichBasalt`]'s class, Vesta's basaltic crust,
    /// and [`CrustLithology::Basalt`]'s by its pyroxene bands, a label, labelled.
    ///
    /// Vesta is V in `DeMeo` et al. 2009 (McCord, Adams and Johnson 1970, Science 168, 1445;
    /// Binzel and Xu 1993, Science 260, 186). Produced.
    V,
    /// X-type, of the X complex: produced by no rule.
    X,
    /// Xc-type, of the X complex: [`CrustLithology::ChondriteE`]'s class.
    ///
    /// It is Lutetia's class in `DeMeo` et al. 2009 (Vernazza et al. 2011), and "the strongest
    /// association is between Xc and enstatite chondrites" (`DeMeo` et al. 2022, §6). Produced.
    Xc,
    /// Xe-type, of the X complex: produced by no rule (an aubrite lid has no registry row).
    Xe,
    /// Xk-type, of the X complex: produced by no rule.
    Xk,
    /// P-type, Tholen's, `DeMeo` and Carry 2013's low-albedo X (`p_V` < 0.075):
    /// [`CrustLithology::Cometary`] with no majority veneer.
    ///
    /// The sim's dry `cometary` bodies at 4–5 au are analogues of the Cybeles and Hildas, P by
    /// mass, 84% and 71% (`DeMeo` and Carry 2013, Table 6, as decision-p14-t51e-contract §3.6 reads
    /// it). Produced.
    P,
    /// A Kuiper-belt body's red colour, whatever its lid: a tholin cover over at least
    /// [`VENEER_SHARE`] of its surface, as Quaoar's.
    ///
    /// For the colours, Brown 2012 (Annu. Rev. Earth Planet. Sci. 40, 467) and Brown, Schaller and
    /// Fraser 2011 (ApJL 739, L60); for tholins as the outer Solar System's colouring agents,
    /// Cruikshank, Imanaka and Dalle Ore 2005 (Adv. Space Res. 36, 178). Produced.
    Red,
    /// A Kuiper-belt body's neutral colour: less of its surface under tholin, as Orcus's, Charon's
    /// and Haumea's water ice (Brown 2012).
    ///
    /// Produced.
    Neutral,
}

impl TaxonomicClass {
    /// Every class: the 24 Bus–`DeMeo` classes in alphabetical order, then P, red and neutral.
    pub const ALL: [Self; 27] = [
        Self::A,
        Self::B,
        Self::C,
        Self::Cb,
        Self::Cg,
        Self::Cgh,
        Self::Ch,
        Self::D,
        Self::K,
        Self::L,
        Self::O,
        Self::Q,
        Self::R,
        Self::S,
        Self::Sa,
        Self::Sq,
        Self::Sr,
        Self::Sv,
        Self::T,
        Self::V,
        Self::X,
        Self::Xc,
        Self::Xe,
        Self::Xk,
        Self::P,
        Self::Red,
        Self::Neutral,
    ];

    /// Its name, for the record and the census: the class's letters, or `red` and `neutral`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::Cb => "Cb",
            Self::Cg => "Cg",
            Self::Cgh => "Cgh",
            Self::Ch => "Ch",
            Self::D => "D",
            Self::K => "K",
            Self::L => "L",
            Self::O => "O",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::Sa => "Sa",
            Self::Sq => "Sq",
            Self::Sr => "Sr",
            Self::Sv => "Sv",
            Self::T => "T",
            Self::V => "V",
            Self::X => "X",
            Self::Xc => "Xc",
            Self::Xe => "Xe",
            Self::Xk => "Xk",
            Self::P => "P",
            Self::Red => "red",
            Self::Neutral => "neutral",
        }
    }

    /// The class of the body `inputs` describe, by the first rule that applies (the
    /// [module](self) documentation).
    ///
    /// `Some` class, or `None` where the rules leave it unclassed: a planet, a majority ice
    /// veneer, or a lid of the closed not-classed set.
    ///
    /// # Errors
    ///
    /// [`ClassifyTaxonomyError::NotGiven`] where the body's rule reads an input it was not given
    /// (the [module](self)'s "What is not given"), naming [`MissingTaxonomyInput::Lid`],
    /// [`MissingTaxonomyInput::Veneer`] or [`MissingTaxonomyInput::Clocks`]: the wiring's not
    /// modelled, never none.
    ///
    /// # Examples
    ///
    /// An L chondrite in a belt at the anchor's 2.5 au about a Sun-like host, where its wind term
    /// w₀ is the reference's (0.16): Q on a fresh surface, S on an old one. A Kuiper-belt body
    /// under a tholin cover is red, whatever its lid, and given none; a planet is not classed; a
    /// member given no lid is refused.
    ///
    /// ```
    /// use std::error::Error;
    ///
    /// use hyperion_sim::planetary::hooks::crust::CrustLithology;
    /// use hyperion_sim::planetary::hooks::small_bodies::{
    ///     AirShield, ClassifyTaxonomyError, MissingTaxonomyInput, TaxonomicClass,
    ///     TaxonomyBodyKind, TaxonomyInputs, Veneer, WeatheringClocks, WindTerms,
    /// };
    /// use hyperion_sim::units::Years;
    ///
    /// let bare = Veneer::new(0.0, 0.0)?;
    /// let wind = WindTerms::new(0.16, 0.16)?;
    /// let member = |age: Years, maturity: f64| -> Result<_, Box<dyn Error>> {
    ///     let clocks = WeatheringClocks::new(age, wind, maturity, AirShield::Open)?;
    ///     let lid = Some(CrustLithology::ChondriteL);
    ///     let kind = TaxonomyBodyKind::AsteroidBeltMember;
    ///     let inputs = TaxonomyInputs::new(kind, lid, Some(bare), Some(clocks));
    ///     Ok(TaxonomicClass::of(&inputs)?)
    /// };
    /// assert_eq!(member(Years::ZERO, 0.0)?, Some(TaxonomicClass::Q));
    /// assert_eq!(member(Years::new(4.567e9), 0.81)?, Some(TaxonomicClass::S));
    ///
    /// let covered = Some(Veneer::new(0.9, 0.0)?);
    /// let kuiper = TaxonomyInputs::new(TaxonomyBodyKind::KuiperBody, None, covered, None);
    /// assert_eq!(TaxonomicClass::of(&kuiper)?, Some(TaxonomicClass::Red));
    ///
    /// let planet = TaxonomyInputs::new(TaxonomyBodyKind::Planet, None, None, None);
    /// assert_eq!(TaxonomicClass::of(&planet)?, None);
    ///
    /// let unknown = TaxonomyInputs::new(TaxonomyBodyKind::OtherMoon, None, Some(bare), None);
    /// assert_eq!(
    ///     TaxonomicClass::of(&unknown),
    ///     Err(ClassifyTaxonomyError::NotGiven(MissingTaxonomyInput::Lid))
    /// );
    /// # Ok::<(), Box<dyn Error>>(())
    /// ```
    pub fn of(inputs: &TaxonomyInputs) -> Result<Option<Self>, ClassifyTaxonomyError> {
        let given_veneer = || {
            inputs.veneer.ok_or(ClassifyTaxonomyError::NotGiven(
                MissingTaxonomyInput::Veneer,
            ))
        };
        match inputs.kind {
            TaxonomyBodyKind::Planet => Ok(None),
            TaxonomyBodyKind::KuiperBody => {
                Ok(cover_class(inputs.kind, given_veneer()?.tholin_share))
            }
            TaxonomyBodyKind::AsteroidBeltMember | TaxonomyBodyKind::OtherMoon => {
                let lid = inputs
                    .lid
                    .ok_or(ClassifyTaxonomyError::NotGiven(MissingTaxonomyInput::Lid))?;
                let Some(by_lid) = lid_class(lid) else {
                    return Ok(None);
                };
                let veneer = given_veneer()?;
                if let Some(class) = cover_class(inputs.kind, veneer.tholin_share) {
                    return Ok(Some(class));
                }
                if veneer.ice_share >= VENEER_SHARE {
                    return Ok(None);
                }
                match by_lid {
                    LidClass::Fixed(class) => Ok(Some(class)),
                    LidClass::OrdinaryChondrite => {
                        let clocks = inputs.clocks.ok_or(ClassifyTaxonomyError::NotGiven(
                            MissingTaxonomyInput::Clocks,
                        ))?;
                        Ok(Some(ordinary_chondrite_class(&clocks)))
                    }
                }
            }
        }
    }
}

/// A body's kind, as its class reads it (decision-p14-t51e-contract §1.2): closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaxonomyBodyKind {
    /// A planet, bound or free-floating: not classed, whatever it holds.
    Planet,
    /// An asteroid belt's member (P14.T21.a): classed by its veneer, then its lid.
    AsteroidBeltMember,
    /// A Kuiper-like belt's member (P14.T21.b, the record's
    /// [`BeltKind::Kuiper`](crate::planetary::record::BeltKind::Kuiper)) or a moon of one: red or
    /// neutral, whatever its lid.
    KuiperBody,
    /// Any other moon: classed by its veneer, then its lid.
    OtherMoon,
}

/// Whether a body's air stops the wind's ions above its ground, T51.d's ion floor (its
/// `ION_STOPPING_COLUMN`, a column p ÷ g of 10⁻⁴ kg m⁻²), which the caller reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AirShield {
    /// No air, or an air whose column is at most the ion floor: the wind reaches the ground.
    Open,
    /// An air whose column is above the ion floor: neither the wind nor the dust reaches the
    /// ground, and an ordinary chondrite is S, labelled (decision-p14-t51e-qs §1.2).
    AboveIonFloor,
}

/// A surface's veneer, the shares of its disk that can hide its lid (decision-p14-t51e-contract
/// §1.2 and §3.3), each and their sum checked once here.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Veneer {
    tholin_share: f64,
    ice_share: f64,
}

impl Veneer {
    /// The veneer of a surface whose tholin-covered share is `tholin_share` and whose uncovered
    /// perennial ice share is `ice_share`, both dimensionless in [0, 1], each a share of the whole
    /// surface.
    ///
    /// - `tholin_share`, `s_t`: T51.d's organic ground times its organic cover, the covered
    ///   fraction of the ground that can take one (T51.d's organic cover alone is that fraction,
    ///   not this share).
    /// - `ice_share`, `s_i`: the solid, non-seasonal condensates' areas (P14.T51.a's entries, the
    ///   cold-trapped areas included), summed in their order and held to 1, less `s_t`, held to +0.
    ///   The sum is an upper bound, labelled, since the entries' areas may overlap, as T51.d's
    ///   organic ground's may. The caller computes it.
    ///
    /// `s_i` is the uncovered share, so the two never exceed the surface: `s_t` + `s_i` is at most
    /// 1, to a tolerance of 10⁻¹² that absorbs the subtraction's rounding.
    ///
    /// # Errors
    ///
    /// [`BuildTaxonomyInputsError::OutOfRange`] with [`TaxonomyInput::TholinShare`] or
    /// [`TaxonomyInput::IceShare`] for a share that is not finite or lies outside [0, 1], and with
    /// [`TaxonomyInput::VeneerSum`] for two shares that sum above 1 + 10⁻¹².
    pub fn new(tholin_share: f64, ice_share: f64) -> Result<Self, BuildTaxonomyInputsError> {
        first_out_of_range([
            (
                (0.0..=1.0).contains(&tholin_share),
                TaxonomyInput::TholinShare,
            ),
            ((0.0..=1.0).contains(&ice_share), TaxonomyInput::IceShare),
            (
                tholin_share + ice_share <= 1.0 + VENEER_SUM_TOLERANCE,
                TaxonomyInput::VeneerSum,
            ),
        ])?;
        Ok(Self {
            tholin_share,
            ice_share,
        })
    }

    /// The tholin-covered share of the surface `s_t`, dimensionless in [0, 1].
    #[must_use]
    pub const fn tholin_share(&self) -> f64 {
        self.tholin_share
    }

    /// The uncovered perennial ice share of the surface `s_i`, dimensionless in [0, 1], an upper
    /// bound.
    #[must_use]
    pub const fn ice_share(&self) -> f64 {
        self.ice_share
    }
}

/// The wind's clock's two terms, a body's wind term w₀ and the reference wind term `w_ref` at the
/// anchor (decision-p14-t51e-qs §1.2), each checked once here.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WindTerms {
    wind_term: f64,
    reference_wind_term: f64,
}

impl WindTerms {
    /// The wind terms of a body whose wind term is `wind_term` (w₀), against the reference wind
    /// term `reference_wind_term` (`w_ref`).
    ///
    /// - `wind_term`, w₀: the body's wind term, dimensionless, finite and at least 0; computed by
    ///   T51.d's rule at an unshielded share of 1 (decision-p14-t51e-qs §1.2), the host's wind mass
    ///   flux at the body's heliocentric orbit over the Moon's reference, a moon's planet's orbit
    ///   for a moon. The caller computes it.
    /// - `reference_wind_term`, `w_ref`: the same term on a circular orbit at
    ///   [`Q_TYPE_ANCHOR_DISTANCE`] about T51.d's reference Sun (0.15998 there, by the ruling's
    ///   port of the rule), computed by the same rule and never hard-coded; finite and positive.
    ///   The caller computes it.
    ///
    /// # Errors
    ///
    /// [`BuildTaxonomyInputsError::OutOfRange`] with [`TaxonomyInput::WindTerm`] or
    /// [`TaxonomyInput::ReferenceWindTerm`] for a term that is not finite or lies outside its
    /// range above.
    pub fn new(wind_term: f64, reference_wind_term: f64) -> Result<Self, BuildTaxonomyInputsError> {
        first_out_of_range([
            (
                wind_term.is_finite() && wind_term >= 0.0,
                TaxonomyInput::WindTerm,
            ),
            (
                reference_wind_term.is_finite() && reference_wind_term > 0.0,
                TaxonomyInput::ReferenceWindTerm,
            ),
        ])?;
        Ok(Self {
            wind_term,
            reference_wind_term,
        })
    }

    /// The body's wind term w₀ at an unshielded share of 1, dimensionless, finite and at least 0.
    #[must_use]
    pub const fn wind_term(&self) -> f64 {
        self.wind_term
    }

    /// The reference wind term `w_ref` at [`Q_TYPE_ANCHOR_DISTANCE`], dimensionless, finite and
    /// positive.
    #[must_use]
    pub const fn reference_wind_term(&self) -> f64 {
        self.reference_wind_term
    }
}

/// An ordinary chondrite's two weathering clocks' inputs (decision-p14-t51e-qs §1.2 and §2), each
/// checked once where it is built.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WeatheringClocks {
    exposure_age: Years,
    wind: WindTerms,
    maturity: f64,
    air: AirShield,
}

impl WeatheringClocks {
    /// The clocks of a surface of exposure age `exposure_age`, wind terms `wind` and maturity
    /// `maturity`, under the air `air`.
    ///
    /// - `exposure_age`, `t_x`: P14.T24.b's surface age
    ///   ([`GlobalFigures::surface_age`](crate::planetary::hooks::GlobalFigures::surface_age)), the
    ///   one T51.d's maturity reads; finite and at least 0.
    /// - `wind`: the body's wind term w₀ and the reference `w_ref`, checked where they were built
    ///   ([`WindTerms::new`]).
    /// - `maturity`, m: T51.d's maturity as built (its k with both terms and every shield),
    ///   dimensionless in [0, 1].
    ///
    /// # Errors
    ///
    /// [`BuildTaxonomyInputsError::OutOfRange`] with [`TaxonomyInput::ExposureAge`] or
    /// [`TaxonomyInput::Maturity`] for an input that is not finite or lies outside its range
    /// above.
    pub fn new(
        exposure_age: Years,
        wind: WindTerms,
        maturity: f64,
        air: AirShield,
    ) -> Result<Self, BuildTaxonomyInputsError> {
        let age = exposure_age.value();
        first_out_of_range([
            (age.is_finite() && age >= 0.0, TaxonomyInput::ExposureAge),
            ((0.0..=1.0).contains(&maturity), TaxonomyInput::Maturity),
        ])?;
        Ok(Self {
            exposure_age,
            wind,
            maturity,
            air,
        })
    }

    /// The exposure age `t_x`, P14.T24.b's surface age, finite and at least 0.
    #[must_use]
    pub const fn exposure_age(&self) -> Years {
        self.exposure_age
    }

    /// The body's wind term w₀ at an unshielded share of 1, dimensionless, finite and at least 0.
    #[must_use]
    pub const fn wind_term(&self) -> f64 {
        self.wind.wind_term
    }

    /// The reference wind term `w_ref` at [`Q_TYPE_ANCHOR_DISTANCE`], dimensionless, finite and
    /// positive.
    #[must_use]
    pub const fn reference_wind_term(&self) -> f64 {
        self.wind.reference_wind_term
    }

    /// T51.d's maturity m, dimensionless in [0, 1].
    #[must_use]
    pub const fn maturity(&self) -> f64 {
        self.maturity
    }

    /// Whether the body's air stops the wind above its ground.
    #[must_use]
    pub const fn air(&self) -> AirShield {
        self.air
    }

    /// The wind's clock's time `τ_Q` = `T_Q` `w_ref` ÷ w₀ ([`Q_TYPE_WEATHERING_TIME`]), Julian
    /// years, positive.
    ///
    /// It is `T_Q` at the anchor, scaling as a² √(1 − e²) about one host, and +∞ for a host with
    /// no wind (w₀ = 0). It is for tests and the census: the class compares `t_x` w₀ with `T_Q`
    /// `w_ref` and never reads it.
    #[must_use]
    pub fn q_type_time(&self) -> Years {
        let WindTerms {
            wind_term,
            reference_wind_term,
        } = self.wind;
        if wind_term > 0.0 {
            Years::new(Q_TYPE_WEATHERING_TIME.value() * reference_wind_term / wind_term)
        } else {
            Years::new(f64::INFINITY)
        }
    }

    /// Whether the wind's clock has run: `t_x` w₀ ≥ `T_Q` `w_ref`, a tie included, without
    /// dividing.
    ///
    /// Never for w₀ = 0, since `w_ref` is positive.
    #[must_use]
    fn wind_clock_has_run(&self) -> bool {
        self.exposure_age.value() * self.wind.wind_term
            >= Q_TYPE_WEATHERING_TIME.value() * self.wind.reference_wind_term
    }

    /// Whether the regolith's clock has run: m ≥ [`IMMATURE_MATURITY`].
    #[must_use]
    fn regolith_clock_has_run(&self) -> bool {
        self.maturity >= IMMATURE_MATURITY
    }
}

/// What [`TaxonomicClass::of`] reads of a body.
///
/// An input a rule does not read may be absent; one it reads and is not given is refused there
/// ([`ClassifyTaxonomyError::NotGiven`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TaxonomyInputs {
    kind: TaxonomyBodyKind,
    lid: Option<CrustLithology>,
    veneer: Option<Veneer>,
    clocks: Option<WeatheringClocks>,
}

impl TaxonomyInputs {
    /// The inputs of a body of kind `kind`, with its lid `lid`, its veneer `veneer` and its
    /// weathering clocks `clocks`.
    ///
    /// - `lid`: its lid's P14.T51.c lithology, which the wiring reads through
    ///   `CrustComposition::lid_lithology`, P14.T54.b.c's getter; `None` where the record gives
    ///   no crust.
    /// - `veneer`: `None` where the record's modifiers are not modelled.
    /// - `clocks`: `None` where the record lacks what the Q/S split reads.
    #[must_use]
    pub const fn new(
        kind: TaxonomyBodyKind,
        lid: Option<CrustLithology>,
        veneer: Option<Veneer>,
        clocks: Option<WeatheringClocks>,
    ) -> Self {
        Self {
            kind,
            lid,
            veneer,
            clocks,
        }
    }

    /// The body's kind.
    #[must_use]
    pub const fn kind(&self) -> TaxonomyBodyKind {
        self.kind
    }

    /// Its lid's lithology, if given.
    #[must_use]
    pub const fn lid(&self) -> Option<CrustLithology> {
        self.lid
    }

    /// Its veneer, if given.
    #[must_use]
    pub const fn veneer(&self) -> Option<Veneer> {
        self.veneer
    }

    /// Its weathering clocks, if given.
    #[must_use]
    pub const fn clocks(&self) -> Option<WeatheringClocks> {
        self.clocks
    }
}

/// A range-checked input of [`Veneer`], [`WindTerms`] or [`WeatheringClocks`], for a
/// [`BuildTaxonomyInputsError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaxonomyInput {
    /// [`Veneer::tholin_share`].
    TholinShare,
    /// [`Veneer::ice_share`].
    IceShare,
    /// [`Veneer::tholin_share`] plus [`Veneer::ice_share`], at most 1 to 10⁻¹².
    VeneerSum,
    /// [`WeatheringClocks::exposure_age`].
    ExposureAge,
    /// [`WindTerms::wind_term`].
    WindTerm,
    /// [`WindTerms::reference_wind_term`].
    ReferenceWindTerm,
    /// [`WeatheringClocks::maturity`].
    Maturity,
}

impl TaxonomyInput {
    /// The input's name, for a message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TholinShare => "tholin-covered share",
            Self::IceShare => "uncovered ice share",
            Self::VeneerSum => "tholin and ice shares' sum",
            Self::ExposureAge => "exposure age",
            Self::WindTerm => "wind term",
            Self::ReferenceWindTerm => "reference wind term",
            Self::Maturity => "maturity",
        }
    }
}

/// An input of [`TaxonomyInputs`] that a body's rule reads, for a [`ClassifyTaxonomyError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MissingTaxonomyInput {
    /// [`TaxonomyInputs::lid`].
    Lid,
    /// [`TaxonomyInputs::veneer`].
    Veneer,
    /// [`TaxonomyInputs::clocks`].
    Clocks,
}

impl MissingTaxonomyInput {
    /// The input's name, for a message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lid => "lid lithology",
            Self::Veneer => "veneer",
            Self::Clocks => "weathering clocks",
        }
    }
}

/// Why a small body's [`Veneer`], [`WindTerms`] or [`WeatheringClocks`] was not built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuildTaxonomyInputsError {
    /// An input was not finite or lay outside its range, as [`Veneer::new`],
    /// [`WindTerms::new`] and [`WeatheringClocks::new`] state them.
    OutOfRange(TaxonomyInput),
}

impl fmt::Display for BuildTaxonomyInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(input) => write!(
                f,
                "the body's {} is not finite or out of its range",
                input.name()
            ),
        }
    }
}

impl Error for BuildTaxonomyInputsError {}

/// Why a small body's class was not computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClassifyTaxonomyError {
    /// The body's rule reads an input it was not given: the wiring's not modelled, never none
    /// (decision-p14-t51e-contract §1.5).
    NotGiven(MissingTaxonomyInput),
}

impl fmt::Display for ClassifyTaxonomyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotGiven(input) => write!(
                f,
                "the body's {} is not given, and its class reads it",
                input.name()
            ),
        }
    }
}

impl Error for ClassifyTaxonomyError {}

/// The first input among `checks` whose check failed, refused, or `Ok` where every check holds.
///
/// Each check pairs whether the input is in its range with the input it names, in the order the
/// constructor's `# Errors` lists them.
fn first_out_of_range<const N: usize>(
    checks: [(bool, TaxonomyInput); N],
) -> Result<(), BuildTaxonomyInputsError> {
    match checks.into_iter().find(|&(in_range, _)| !in_range) {
        Some((_, input)) => Err(BuildTaxonomyInputsError::OutOfRange(input)),
        None => Ok(()),
    }
}

/// What a classed lid gives (rule 4): a fixed class, or an ordinary chondrite's S or Q.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LidClass {
    /// The lid's one class.
    Fixed(TaxonomicClass),
    /// An ordinary chondrite: S or Q by the clocks.
    OrdinaryChondrite,
}

/// The lid table (the [module](self)'s rule 4; decision-p14-t51e-contract §3.2): `None` for the
/// closed not-classed set.
///
/// Exhaustive, with no wildcard.
#[must_use]
const fn lid_class(lid: CrustLithology) -> Option<LidClass> {
    match lid {
        CrustLithology::ChondriteH | CrustLithology::ChondriteL | CrustLithology::ChondriteLl => {
            Some(LidClass::OrdinaryChondrite)
        }
        CrustLithology::ChondriteCi | CrustLithology::Phyllosilicate => {
            Some(LidClass::Fixed(TaxonomicClass::C))
        }
        CrustLithology::ChondriteCm => Some(LidClass::Fixed(TaxonomicClass::Ch)),
        CrustLithology::ChondriteCv => Some(LidClass::Fixed(TaxonomicClass::K)),
        CrustLithology::ChondriteE => Some(LidClass::Fixed(TaxonomicClass::Xc)),
        CrustLithology::IronRichBasalt | CrustLithology::Basalt => {
            Some(LidClass::Fixed(TaxonomicClass::V))
        }
        CrustLithology::Ultramafic => Some(LidClass::Fixed(TaxonomicClass::A)),
        CrustLithology::Cometary => Some(LidClass::Fixed(TaxonomicClass::P)),
        CrustLithology::Water
        | CrustLithology::Graphite
        | CrustLithology::Feldspathic
        | CrustLithology::Komatiite
        | CrustLithology::ReducedBasalt
        | CrustLithology::Granite => None,
    }
}

/// The tholin cover's class for a body of kind `kind` whose tholin-covered share is
/// `tholin_share` (rules 2 and 3's cover; decision-p14-t51e-contract §3.3, §3.4 and §3.6): a
/// Kuiper-belt body's red at [`VENEER_SHARE`] and over, else neutral; another member's or moon's D
/// at [`VENEER_SHARE`] and over, else `None`, its lid deciding; a planet's `None`.
///
/// It is the one function that holds the cover rule, so that a formation-temperature rule can
/// replace it: when an implanted population or bodies under 400 km become records, or a sourced
/// H₂S retention temperature is read (Wong and Brown 2016, AJ 152, 90, put the Trojans' and the
/// small Kuiper-belt bodies' split at H₂S retention in the primordial disc and give no value), D
/// or P and red or neutral take a line against the derivation's formation temperature, a value
/// change at a later bump.
#[must_use]
fn cover_class(kind: TaxonomyBodyKind, tholin_share: f64) -> Option<TaxonomicClass> {
    let covered = tholin_share >= VENEER_SHARE;
    match kind {
        TaxonomyBodyKind::KuiperBody => Some(if covered {
            TaxonomicClass::Red
        } else {
            TaxonomicClass::Neutral
        }),
        TaxonomyBodyKind::AsteroidBeltMember | TaxonomyBodyKind::OtherMoon => {
            covered.then_some(TaxonomicClass::D)
        }
        TaxonomyBodyKind::Planet => None,
    }
}

/// An ordinary chondrite's class under `clocks` (the [module](self)'s "The Q/S split"): S under
/// air above the ion floor, labelled; else S where either clock has run, and Q otherwise.
#[must_use]
fn ordinary_chondrite_class(clocks: &WeatheringClocks) -> TaxonomicClass {
    match clocks.air {
        AirShield::AboveIonFloor => TaxonomicClass::S,
        AirShield::Open => {
            if clocks.wind_clock_has_run() || clocks.regolith_clock_has_run() {
                TaxonomicClass::S
            } else {
                TaxonomicClass::Q
            }
        }
    }
}
