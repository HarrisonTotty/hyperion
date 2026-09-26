//! The binary engine's parameters: Hurley, Tout and Pols's (2002, "BSE") inputs (their table 3),
//! with the generator's defaults (plan 11, design note 14).

/// The parameters of the binary engine (BSE table 3). [`Default`] is the generator version's.
///
/// Every field is a figure of BSE's own text unless its documentation says otherwise; the plan's
/// common-envelope efficiency is marked provisional where it departs from the paper's default.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one of BSE table 3's independent switches"
)]
pub struct BinaryParams {
    /// The common-envelope efficiency `α_CE` (BSE equation 71): 1, plan 11's design note 14
    /// (provisional). BSE's table 3 default and its preferred Model A take 3.0; its section 3.2
    /// cataclysmic variable takes 1.
    pub alpha_ce: f64,
    /// The envelope's binding parameter λ (BSE equation 69): 0.5.
    pub lambda: f64,
    /// `β_W` of the wind speed `v_W²` = 2 `β_W` G M ÷ R (BSE equation 9): 0.5, BSE's table 3 default
    /// and section 4.1.4's value for every population. Section 2.1's argument from cool
    /// supergiants' winds gives 1/8, which the published code's input file uses.
    pub wind_speed_factor: f64,
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
        wind_speed_factor: 0.5,
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
