//! What the engine's classes count as an X-ray binary (plan 11, P11.T5's and P11.T8.b's fix;
//! research-p15-t10b-rates).
//!
//! As first built, `classify` made any Roche-lobe overflow onto a neutron star or a black hole an
//! X-ray binary, in quiescence too; a black hole beside a massive star an X-ray binary wherever the
//! star's wind gave it 10³⁵ erg s⁻¹ by Bondi and Hoyle's capture; and a Be/X-ray binary's donor a
//! Be star by its own single-star rotation draw alone. research-p15-t10b-rates found, in the fit's
//! census: 1,607 of the slab's 2,212 members neutron stars and black holes fed by white dwarfs or
//! helium stars at a median 4.8 × 10⁻¹⁶ M☉ yr⁻¹, 89% of them under 10⁻¹³; 420 of the young thin
//! disc's 552 high-mass hosts black holes fed by a wind; and Be/X hosts 55 of its 132
//! neutron-star high-mass hosts.
//!
//! So:
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
//! - **A symbiotic X-ray binary is a neutron star fed by a cool giant's wind.** It needs a neutron
//!   star, never a black hole: none is known, and Deng and Li (2024, ApJ 977, 95) predict at most
//!   one in six as many as of neutron stars, where the census held seven black holes to one; a
//!   black hole fed by a giant's wind is a dormant pair, as Gaia BH2 is (El-Badry et al. 2023,
//!   MNRAS 521, 4323; recalled). Its giant is a K or M
//!   giant at [`SYMBIOTIC_XRB_MAX_TEFF`] or cooler, under 8 M☉, inside
//!   [`SYMBIOTIC_XRB_MAX_FILLING`] of its Roche lobe, on an orbit of [`SYMBIOTIC_XRB_MAX_PERIOD`]
//!   or less (research's ruling of 2026-11-02): the census's wide pairs, at a median 7 × 10⁴ days,
//!   and its blue-loop giants of some 5 M☉ are no symbiotic X-ray binaries.
//! - **A helium disc's line.** A low-mass X-ray binary fed by a white dwarf, of helium or of carbon
//!   and oxygen, is persistent or transient by its irradiated helium disc's instability line
//!   ([`helium_disc_line`]), some 22–24 times the hydrogen disc's beside a 1.4 M☉ neutron star and
//!   17 beside a 7 M☉ black hole (Lasota, Dubus and Kruk 2008), not by the hydrogen disc's
//!   (research's ruling of 2026-11-02). A helium star keeps the hydrogen line: the helium line's
//!   period form holds for an ultracompact disc alone (the science check of 2026-11-02). The
//!   X-ray binaries' scan reads the line its class's test reads.
//!
//! Every figure is recalled, not read at source (the science checks of 2026-11-01 reached none);
//! the floor sits at the very faint transients' upper bound, so it may cut the faintest of them.
//! `classify` reads each of the rules, and the X-ray binaries' scan reads the floor, so that it
//! keeps exactly the class's entries. A timeline built from marks
//! has no history, so a Be star spun up by transfer is one in the engine's timeline alone (a
//! deviation: the catalogue's Be/X donors are Be stars by their own draws). The census's members
//! each have one of the nine kinds (`testing::XrayMarks::member_kind`), which the catalogue draws.

use crate::math;
use crate::units::{Days, Kelvin, Seconds, SolarMassesPerYear};

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

/// The hottest a symbiotic X-ray binary's giant may be from 27: 5,000 K, a K–M giant's, from its
/// first giant branch through the asymptotic one, which leaves out the core-helium-burning giants
/// of some 5 M☉ on their blue loops (research's ruling of 2026-11-02; the known systems' donors
/// are K and M giants, GX 1+4's an M6 III, Hinkle et al. 2006, ApJ 641, 479).
pub(crate) const SYMBIOTIC_XRB_MAX_TEFF: Kelvin = Kelvin::new(5_000.0);

/// The largest share of its Roche lobe a symbiotic X-ray binary's giant may fill from 27: 0.8, a
/// detached pair fed by the giant's wind (research's ruling of 2026-11-02).
pub(crate) const SYMBIOTIC_XRB_MAX_FILLING: f64 = 0.8;

/// The longest orbital period of a symbiotic X-ray binary from 27: 10⁴ days, past the longest
/// measured, 4U 1700+24's 4,391 d (Hinkle et al. 2019, ApJ 872, 43; research's ruling of
/// 2026-11-02).
pub(crate) const SYMBIOTIC_XRB_MAX_PERIOD: Days = Days::new(1.0e4);

/// The irradiated helium disc's instability line, a low-mass X-ray binary fed by a white dwarf of
/// orbital period `period`, from 27: 2.4 × 10⁻¹² (P ÷ 1 min)^1.67 M☉ yr⁻¹, Lasota, Dubus and Kruk's
/// (2008, A&A 486, 523) equation 6 in its period form, their equation 12 (recalled, not read at
/// source), valid for an ultracompact orbit of some 10–80 minutes, whose donor is far lighter than
/// its accretor; some 22–24 times the hydrogen disc's beside a 1.4 M☉ neutron star, it is taken as
/// a lower bound for a carbon–oxygen disc (research's ruling of 2026-11-02). A cold ultracompact
/// host then turns transient from about 20 minutes, where the hydrogen line turns it at about 32.
#[must_use]
pub(crate) fn helium_disc_line(period: Seconds) -> SolarMassesPerYear {
    let minutes = period.value() / 60.0;
    SolarMassesPerYear::new(HELIUM_DISC_LINE_AT_A_MINUTE * math::powf_positive(minutes, 1.67))
}

/// [`helium_disc_line`] at a period of one minute, M☉ yr⁻¹: 2.4 × 10⁻¹².
const HELIUM_DISC_LINE_AT_A_MINUTE: f64 = 2.4e-12;
