//! The hold on what the engine's classes count as an X-ray binary (plan 11, P11.T5's and P11.T8.b's
//! fix, at [`XRB_CLASS_VERSION`]; research-p15-t10b-rates).
//!
//! As built, `classify` makes any Roche-lobe overflow onto a neutron star or a black hole an X-ray
//! binary, in quiescence too; a black hole beside a massive star an X-ray binary wherever the
//! star's wind gives it 10³⁵ erg s⁻¹ by Bondi and Hoyle's capture; and a Be/X-ray binary's donor a
//! Be star by its own single-star rotation draw alone. research-p15-t10b-rates found, in the fit's
//! census: 1,607 of the slab's 2,212 members neutron stars and black holes fed by white dwarfs or
//! helium stars at a median 4.8 × 10⁻¹⁶ M☉ yr⁻¹, 89% of them under 10⁻¹³; 420 of the young thin
//! disc's 552 high-mass hosts black holes fed by a wind; and Be/X hosts 55 of its 132
//! neutron-star high-mass hosts.
//!
//! From 27:
//!
//! - **A floor.** Roche-lobe overflow onto a compact star is an X-ray binary only at
//!   [`XRB_MIN_TRANSFER_RATE`] or more, about 10³⁴ erg s⁻¹ time-averaged onto a neutron star: the
//!   time-averaged rate of the very faint X-ray transients, the faintest X-ray binaries known (King
//!   and Wijnands 2006, MNRAS 366, L31; Wijnands et al. 2006, A&A 449, 1117; research-leans-30
//!   §4, which keeps 75 of the census's ultracompacts, all under 80 minutes, so that no bound on
//!   the period is needed). Below it a neutron star beside a
//!   white dwarf is a radio pulsar, its companion's matter blown away (radio ejection, Burderi et
//!   al. 2001, ApJ 560, L71), not a catalogued X-ray binary.
//! - **Be donors spun up.** A main-sequence B star without a fossil field that has gained
//!   [`SPUN_UP_GAINED_SHARE`] of its mass in stable transfer rotates at the Be threshold, whatever
//!   its own draw: a tenth of its mass spins an accretor to its critical speed (Packet 1981, A&A
//!   102, 17), and the mass gainers of stable transfer are the Be/X-ray binaries' channel (Pols et
//!   al. 1991, A&A 241, 419).
//! - **A disc for a wind-fed black hole.** A black hole fed by a massive star's wind is an X-ray
//!   binary only if the star fills [`WIND_DISC_ROCHE_FILLING`] of its Roche lobe, where its wind
//!   is slow and focused enough to form a disc about the hole (Hirai and Mandel 2021, PASA 38,
//!   e56); without one the hole's flow is radiatively inefficient. A neutron star's field takes a
//!   wind with no disc (the wind-fed supergiant systems, Vela X-1 among them), so its rule is
//!   unchanged: the brief named both, and the neutron stars are left as they are (a deviation).
//! - **A Be star feeds a neutron star alone.** Beside a black hole a Be star makes no X-ray binary:
//!   the hole truncates the Be star's disc and accretes inefficiently (Zhang, Li and Wang 2004, ApJ
//!   603, 663; most OB stars beside black holes are X-ray quiet, Langer et al. 2020, A&A 638, A39),
//!   and MWC 656, the one claimed (Casares et al. 2014, Nature 505, 378), shines at about
//!   10³¹ erg s⁻¹ (Munar-Adrover et al. 2014, ApJ 786, L11; Ribó et al. 2017, ApJ 835, L33), far
//!   under the catalogues' reach. Main's ruling of 2026-11-01
//!   made the black holes' share of the young disc's high-mass hosts, 0.145 against at most 0.01,
//!   a wrong state: 62 of its 64 black holes were beside Be stars, most spun up by transfer, and
//!   none fed by a wind filled the Roche share its disc needs.
//!
//! Every figure is recalled, not read at source (the science checks of 2026-11-01 reached none);
//! the floor sits at the very faint transients' upper bound, so it may cut the faintest of them.
//! The rules' one `match` is [`XrbClassRules::carves`], which each of the four reads in
//! `classify` through the pair's own rules, and the X-ray binaries' scan reads for the floor
//! through the galaxy's, so that it keeps exactly the class's entries whichever rules the class's
//! test reads (its timelines, built from marks, take the version's). A timeline built from marks
//! has no history, so a Be star spun up by transfer is one in the engine's timeline alone (a
//! deviation: the catalogue's Be/X donors are Be stars by their own draws). The census's members
//! each have one of the nine kinds (`testing::XrayMarks::member_kind`), which the catalogue draws
//! from 27.

use crate::GeneratorVersion;
use crate::units::SolarMassesPerYear;

/// The generator version whose stellar rules carve the X-ray binaries anew (P11.T5's and P11.T8.b's
/// fix): [`XrbClassRules::Carved`].
///
/// Below it every pair takes the held arm, [`XrbClassRules::Held`], today's code verbatim, so
/// every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module's version and
/// [`XrbClassRules`] (its constants move to `classify`), the stellar rules' `xrb_class` field,
/// the calls of [`XrbClassRules::carves`] (`classify`'s four and the X-ray binaries' scan's) and
/// the context's gainer masses, and the tripwire (`the_xrb_class_is_held_only_until_the_27_bump`).
pub(crate) const XRB_CLASS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the engine's classes carve the X-ray binaries anew (the stellar rules' `xrb_class`
/// field, the hold of [`XRB_CLASS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum XrbClassRules {
    /// Versions 24–26's: every Roche-lobe overflow onto a compact star, every wind-fed black hole
    /// above the luminosity floor, and a Be star by its own draw beside a neutron star or a black
    /// hole.
    Held,
    /// Version 27's: the floor on the transfer rate, the spun-up Be donors, the wind-fed black
    /// holes' disc and Be/X-ray binaries of neutron stars alone (module documentation).
    Carved,
}

impl XrbClassRules {
    /// Whether the rules carve anew: the hold's one `match`.
    #[must_use]
    pub(crate) const fn carves(self) -> bool {
        match self {
            Self::Held => false,
            Self::Carved => true,
        }
    }
}

/// The least rate of Roche-lobe overflow onto a neutron star or a black hole at which the pair is an
/// X-ray binary from 27, M☉ yr⁻¹: 10⁻¹², about 10³⁴ erg s⁻¹ time-averaged onto a 1.4 M☉ neutron
/// star of 12 km (G M Ṁ ÷ R), the very faint X-ray transients' time-averaged rate (King and
/// Wijnands 2006, MNRAS 366, L31; recalled; research-leans-30 §4).
pub(crate) const XRB_MIN_TRANSFER_RATE: SolarMassesPerYear = SolarMassesPerYear::new(1.0e-12);

/// The least share of its mass a main-sequence B star must have gained in stable transfer to rotate
/// at the Be threshold from 27: 0.1, what spins an accretor to its critical speed (Packet 1981,
/// A&A 102, 17; recalled), above the Be stars' 0.7 of it.
pub(crate) const SPUN_UP_GAINED_SHARE: f64 = 0.1;

/// The least share of its Roche lobe a massive star must fill for its wind to form a disc about a
/// black hole, from 27: 0.9 (Hirai and Mandel 2021, PASA 38, e56; recalled, provisional).
pub(crate) const WIND_DISC_ROCHE_FILLING: f64 = 0.9;
