//! The binary engine's parameters: Hurley, Tout and Pols's (2002, "BSE") inputs (their table 3),
//! with the generator's defaults (plan 11, design note 14).

use super::star::Kind;

/// The parameters of the binary engine (BSE table 3). [`Default`] is the generator version's.
///
/// Every field is a figure of BSE's own text unless its documentation says otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one of BSE table 3's independent switches"
)]
pub struct BinaryParams {
    /// The common-envelope efficiency `α_CE` (BSE equation 71): 1, the standard of Claeys et al.
    /// (2014, A&A 563, A83), with λ = 0.5 (plan 11's design note 14, settled by ruling 108.2).
    /// BSE's table 3 default and its preferred Model A take 3.0, and its section 3.2 cataclysmic
    /// variable 1. BSE's published code takes 3 with a λ from the envelope's structure and half
    /// its ionisation energy (`celamf`, Claeys et al.'s appendix A), about 1–2 for giants, so
    /// `α_CE` λ ≈ 3–6 there against 0.5 here. Zorotovic et al. (2010) fit α ≈ 0.2–0.3 to
    /// post-common-envelope binaries only with such a structure λ; that λ, with `α_CE` = 0.25,
    /// is a later refinement.
    pub alpha_ce: f64,
    /// The envelope's binding parameter λ (BSE equation 69): 0.5, the paper's (ruling 108.2).
    pub lambda: f64,
    /// `β_W` of the wind speed `v_W²` = 2 `β_W` G M ÷ R (BSE equation 9): `StarTrack`'s table by the
    /// wind-losing star's type and mass, as COSMIC uses it (ruling 108.3; [`WindSpeedFactor`]).
    pub wind_speed_factor: WindSpeedFactor,
    /// `α_W` of Bondi–Hoyle accretion (BSE equation 6): 3/2.
    pub bondi_hoyle: f64,
    /// The largest share of a wind a companion accretes (BSE equation 10): 0.8.
    pub max_wind_accretion: f64,
    /// ε, the share of accreted hydrogen a white dwarf keeps through its novae (BSE equation 66):
    /// 0.001.
    pub nova_retention: f64,
    /// Whether accretion onto a white dwarf, neutron star or black hole is held to the Eddington
    /// rate (BSE equations 67 and 68): off, BSE's table 3 default (its section 4.3.3 finds the
    /// limit leaves too few persistent low-mass X-ray binaries).
    pub eddington_limit: bool,
    /// Whether tides act (BSE section 2.3): on.
    pub tides: bool,
    /// Whether gravitational radiation takes angular momentum (BSE section 2.4): on.
    pub gravitational_radiation: bool,
    /// Whether magnetic braking takes angular momentum (BSE section 2.4): on.
    pub magnetic_braking: bool,
    /// Whether winds take angular momentum from the orbit and the spins (BSE sections 2.1 and
    /// 2.2): on. Off only for the tests of conservation; the stars' own tracks still lose the mass.
    pub wind_angular_momentum: bool,
    /// Whether remnants take their natal kicks (plan 06's law, P06.T19): on. BSE's comparisons with
    /// Portegies Zwart and Verbunt (1996) are run without (their section 3.4).
    pub natal_kicks: bool,
}

impl BinaryParams {
    /// The generator's parameters.
    pub const GENERATOR: Self = Self {
        alpha_ce: 1.0,
        lambda: 0.5,
        wind_speed_factor: WindSpeedFactor::StarTrack,
        bondi_hoyle: 1.5,
        max_wind_accretion: 0.8,
        nova_retention: 0.001,
        eddington_limit: false,
        tides: true,
        gravitational_radiation: true,
        magnetic_braking: true,
        wind_angular_momentum: true,
        natal_kicks: true,
    };

    /// These parameters with the common-envelope efficiency `alpha_ce`, as BSE's worked examples
    /// set it (their section 3).
    #[must_use]
    pub const fn with_alpha_ce(self, alpha_ce: f64) -> Self {
        Self { alpha_ce, ..self }
    }
}

impl Default for BinaryParams {
    fn default() -> Self {
        Self::GENERATOR
    }
}

/// `β_W`, the factor of the wind speed `v_W²` = 2 `β_W` G M ÷ R (BSE equation 9), which sets how
/// much of a star's wind its companion accretes (roughly as `β_W`⁻² for a fast wind).
///
/// BSE's section 2.1 gives `β_W` from about 7 for O stars down to about 0.5 for A and F stars
/// (Lamers, Snow and Lindholm 1995) and argues for 1/8 from cool supergiants' 5–35 km s⁻¹ winds;
/// its table 3 takes 0.5 for every star and its published code 1/8. The generator takes the
/// table by type of `StarTrack` (Belczynski et al. 2008) that COSMIC offers as `beta = -1`
/// (`evolv2.f`), ruling 108.3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindSpeedFactor {
    /// One figure for every star: BSE's table 3 takes 0.5 and its published code 1/8.
    Fixed(f64),
    /// By the wind-losing star: a main-sequence star 0.5 up to 1.4 M☉, rising linearly to 7 at
    /// 120 M☉; a naked helium star 0.125 up to 10 M☉, rising linearly to 7 at 120 M☉; every
    /// other star 0.125; 7 above 120 M☉ for both rising kinds. Ruling 108.3 states the rises
    /// to 7 at 120 M☉; COSMIC's code adds them to the floor, reaching 7.5 and 7.125 there and
    /// stepping down to 7 above, which the ruling's continuous form leaves out.
    StarTrack,
}

impl WindSpeedFactor {
    /// The main-sequence mass up to which `StarTrack`'s `β_W` is 0.5, M☉.
    const MAIN_SEQUENCE_KNEE: f64 = 1.4;
    /// The helium-star mass up to which `StarTrack`'s `β_W` is 0.125, M☉.
    const HELIUM_KNEE: f64 = 10.0;
    /// The mass at which both rises reach [`Self::HOT`], M☉.
    const TOP: f64 = 120.0;
    /// `β_W` of the hottest stars (BSE section 2.1, O stars).
    const HOT: f64 = 7.0;
    /// `β_W` of a main-sequence star up to 1.4 M☉ (BSE section 2.1, A and F stars).
    const WARM: f64 = 0.5;
    /// `β_W` of cool and evolved stars (BSE section 2.1, cool supergiants).
    const COOL: f64 = 0.125;

    /// `β_W` of a star of type `kind` and mass `mass`, M☉, losing its wind.
    #[must_use]
    pub(crate) fn of(self, kind: Kind, mass: f64) -> f64 {
        let rise = |knee: f64, floor: f64| {
            if mass <= knee {
                floor
            } else if mass >= Self::TOP {
                Self::HOT
            } else {
                floor + (Self::HOT - floor) * (mass - knee) / (Self::TOP - knee)
            }
        };
        match self {
            Self::Fixed(beta) => beta,
            Self::StarTrack => {
                if kind.is_main_sequence() {
                    rise(Self::MAIN_SEQUENCE_KNEE, Self::WARM)
                } else if kind.is_helium_star() {
                    rise(Self::HELIUM_KNEE, Self::COOL)
                } else {
                    Self::COOL
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    /// Ruling 108.3: `StarTrack`'s `β_W` by type, as COSMIC uses it, continuous in mass.
    #[test]
    fn the_wind_speed_factor_follows_star_tracks_table() {
        let beta = WindSpeedFactor::StarTrack;
        let ms = |m: f64| beta.of(Kind::MainSequence, m);
        let he = |m: f64| beta.of(Kind::HeliumMainSequence, m);
        assert!(close(beta.of(Kind::LowMassMainSequence, 0.5), 0.5));
        assert!(close(ms(1.4), 0.5));
        assert!(close(ms(60.7), 0.5 + 6.5 * 0.5));
        assert!(close(ms(120.0), 7.0));
        assert!(close(ms(150.0), 7.0));
        assert!(close(he(10.0), 0.125));
        assert!(close(he(65.0), 0.125 + 6.875 * 0.5));
        assert!(close(beta.of(Kind::HeliumGiant, 150.0), 7.0));
        for kind in [Kind::HertzsprungGap, Kind::GiantBranch, Kind::PulsingAgb] {
            assert!(close(beta.of(kind, 30.0), 0.125), "{kind:?}");
        }
        for knee in [1.4, 10.0, 120.0] {
            for kind in [Kind::MainSequence, Kind::HeliumGap] {
                let (below, above) = (beta.of(kind, knee - 1e-9), beta.of(kind, knee + 1e-9));
                assert!((below - above).abs() < 1e-8, "{kind:?} at {knee}");
            }
        }
        assert!(close(
            WindSpeedFactor::Fixed(0.5).of(Kind::GiantBranch, 1.0),
            0.5
        ));
    }

    /// BSE section 2.1: at `β_W` = 1/8 a 1 M☉ giant of 100 R☉ blows its wind at 21.8 km s⁻¹,
    /// inside cool supergiants' observed 5–35 km s⁻¹.
    #[test]
    fn a_cool_giants_wind_is_slow() {
        let beta = WindSpeedFactor::StarTrack.of(Kind::GiantBranch, 1.0);
        let v = (2.0 * beta * GM_SUN / (100.0 * SOLAR_RADIUS_M)).sqrt();
        assert!((v / 1e3 - 21.8).abs() < 0.1, "{v} m/s");
    }
}
