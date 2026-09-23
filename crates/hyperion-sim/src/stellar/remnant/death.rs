//! How a star dies: the age, the kind of death, and the progenitor at its last living instant
//! (plan 06, P06.T10.e), which the remnant's type and mass (P06.T18) and the natal kick (P06.T19)
//! read.

use crate::units::{SolarMasses, Years};

/// A star's death: when, how, and what it was at its last living instant.
///
/// Built by the track integrator ([`Track::death`](crate::stellar::sse::Track::death)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Death {
    age: Years,
    kind: DeathKind,
    progenitor: ProgenitorAtDeath,
}

impl Death {
    /// A death at `age` of `kind`, from `progenitor`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `age` is not finite and non-negative.
    #[must_use]
    pub(crate) fn new(age: Years, kind: DeathKind, progenitor: ProgenitorAtDeath) -> Self {
        debug_assert!(
            age.value().is_finite() && age.value() >= 0.0,
            "a death has a finite, non-negative age: {age:?}"
        );
        Self {
            age,
            kind,
            progenitor,
        }
    }

    /// The age of death, Julian years since the onset of collapse (plan 06, design note 4): the
    /// end of the last living phase, which is the star's lifetime.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// How the star died.
    #[must_use]
    pub const fn kind(&self) -> DeathKind {
        self.kind
    }

    /// The star at its last living instant.
    #[must_use]
    pub const fn progenitor(&self) -> ProgenitorAtDeath {
        self.progenitor
    }
}

/// How a star dies.
///
/// Only [`DeathKind::EnvelopeLoss`] is continuous in state: the star becomes a white dwarf, which
/// its last living phase approaches (plan 06, design note 3). Every other kind is an explosion or
/// a collapse, a discontinuity with a clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeathKind {
    /// The envelope is lost and the core is left as a white dwarf (through the post-AGB crossing
    /// where it applies, P06.T16).
    EnvelopeLoss,
    /// An oxygen–neon core collapses by electron capture (HPT section 6: a core at the base of
    /// the AGB of 1.6–2.25 M☉ reaching `Mc,SN` on the thermally pulsing AGB).
    ElectronCapture,
    /// An iron core collapses, in a supernova of the type the envelope sets.
    CoreCollapse {
        /// The supernova's spectral type.
        supernova: SupernovaType,
    },
    /// The core collapses to a black hole with complete fallback and no supernova (P06.T18).
    DirectCollapse,
    /// Pair instability disrupts the star and leaves no remnant (plan 06, design note 10; P06.T18.c).
    PairInstability,
    /// Carbon ignites in a degenerate carbon–oxygen core that reaches the Chandrasekhar mass, and
    /// the explosion leaves no remnant (HPT section 6, after equation 75: a core at the base of the
    /// AGB below 1.6 M☉, or a helium star below 1.6 M☉, whose core reaches 1.44 M☉ before its
    /// envelope is gone).
    ThermonuclearDisruption,
}

impl DeathKind {
    /// Whether the death is sudden, a discontinuity in the star's state at a clock time: every
    /// kind but [`DeathKind::EnvelopeLoss`].
    #[must_use]
    pub const fn is_sudden(self) -> bool {
        match self {
            Self::EnvelopeLoss => false,
            Self::ElectronCapture
            | Self::CoreCollapse { .. }
            | Self::DirectCollapse
            | Self::PairInstability
            | Self::ThermonuclearDisruption => true,
        }
    }
}

/// The spectral type of a core-collapse supernova, set by what the progenitor kept of its
/// envelopes: IIP above 2 M☉ of hydrogen envelope, IIL from 0.1 M☉, IIb below that, Ib with no
/// hydrogen and more than 0.14 M☉ of helium outside the carbon–oxygen core, and Ic with less (plan
/// 06, P06.T10.e; the thresholds are generator defaults, after Heger et al. 2003, ApJ 591, 288,
/// and Hachinger et al. 2012, MNRAS 422, 70).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SupernovaType {
    /// A plateau light curve from a massive hydrogen envelope.
    IIP,
    /// A linear decline from a smaller hydrogen envelope.
    IIL,
    /// Hydrogen early, helium later: a thin hydrogen layer.
    IIb,
    /// No hydrogen, helium present.
    Ib,
    /// Neither hydrogen nor much helium.
    Ic,
    /// No supernova.
    None,
}

/// The hydrogen envelope, M☉, above which a core collapse is a Type IIP supernova: 2 M☉, the
/// generator's default, after Heger et al. (2003, ApJ 591, 288, section 4.1 and figure 2), who
/// assume that an envelope above about 2 M☉ sustains the plateau and a smaller one gives a linear
/// decline.
pub(crate) const PLATEAU_ENVELOPE: SolarMasses = SolarMasses::new(2.0);

/// The hydrogen envelope, M☉, below which a core collapse is a Type IIb supernova: 0.1 M☉, plan
/// 06's generator default (P06.T10.e).
///
/// It is the envelope of about 0.1 M☉ that Bersten et al. (2012, ApJ 757, 31) find for SN
/// 2011dh, whose hydrogen itself is about 0.02 M☉. The prototype, SN 1993J, kept an envelope of
/// 0.20 ± 0.05 M☉ (Woosley et al. 1994, ApJ 429, 300), which this threshold calls Type IIL, so the
/// threshold is a lower bound of the observed envelopes rather than their boundary.
pub(crate) const THIN_HYDROGEN_ENVELOPE: SolarMasses = SolarMasses::new(0.1);

/// The helium outside the carbon–oxygen core, M☉, below which a stripped star's supernova is Type
/// Ic: 0.14 M☉, the upper end of the 0.06–0.14 M☉ of helium that Hachinger et al. (2012, MNRAS 422,
/// 70, section 4.3) find can stay hidden in the spectrum of a Type Ic supernova. Their models have
/// 2–3 M☉ of ejecta, and the threshold is extrapolated to heavier cores.
pub(crate) const HIDDEN_HELIUM: SolarMasses = SolarMasses::new(0.14);

impl SupernovaType {
    /// The type of a core-collapse supernova whose progenitor kept `hydrogen` M☉ of hydrogen
    /// envelope and `helium` M☉ of helium outside its carbon–oxygen core: IIP above 2 M☉ of
    /// hydrogen, IIL from 0.1 to 2, IIb below 0.1, Ib with no hydrogen and more than 0.14 M☉ of
    /// helium, Ic with less (plan 06's T10.e; the thresholds are the generator's defaults, with
    /// their sources at [`PLATEAU_ENVELOPE`], [`THIN_HYDROGEN_ENVELOPE`] and [`HIDDEN_HELIUM`]).
    #[must_use]
    pub(crate) fn of_envelopes(hydrogen: SolarMasses, helium: SolarMasses) -> Self {
        if hydrogen > PLATEAU_ENVELOPE {
            Self::IIP
        } else if hydrogen >= THIN_HYDROGEN_ENVELOPE {
            Self::IIL
        } else if hydrogen.value() > 0.0 {
            Self::IIb
        } else if helium > HIDDEN_HELIUM {
            Self::Ib
        } else {
            Self::Ic
        }
    }
}

/// A star at its last living instant: the masses the remnant and the kick depend on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgenitorAtDeath {
    co_core_mass: SolarMasses,
    helium_core_mass: SolarMasses,
    envelope_mass: SolarMasses,
    stripping: Stripping,
}

impl ProgenitorAtDeath {
    /// A progenitor with a carbon–oxygen core of `co_core_mass`, a helium core (everything below
    /// the hydrogen envelope) of `helium_core_mass` and a hydrogen envelope of `envelope_mass`, all
    /// M☉, stripped as `stripping` says.
    ///
    /// # Panics
    ///
    /// In debug builds, if a mass is negative or not finite, or the cores are not nested.
    #[must_use]
    pub(crate) fn new(
        co_core_mass: SolarMasses,
        helium_core_mass: SolarMasses,
        envelope_mass: SolarMasses,
        stripping: Stripping,
    ) -> Self {
        debug_assert!(
            [co_core_mass, helium_core_mass, envelope_mass]
                .iter()
                .all(|m| m.value().is_finite() && m.value() >= 0.0),
            "masses at death are finite and non-negative"
        );
        debug_assert!(
            co_core_mass.value() <= helium_core_mass.value() * (1.0 + 1e-12),
            "the carbon–oxygen core lies inside the helium core: {co_core_mass:?} of {helium_core_mass:?}"
        );
        Self {
            co_core_mass,
            helium_core_mass,
            envelope_mass,
            stripping,
        }
    }

    /// The carbon–oxygen core, M☉; zero for a star that dies before it has one (a helium white
    /// dwarf's progenitor).
    #[must_use]
    pub const fn co_core_mass(&self) -> SolarMasses {
        self.co_core_mass
    }

    /// The helium core, M☉: everything inside the hydrogen envelope, the whole star for a naked
    /// helium star.
    #[must_use]
    pub const fn helium_core_mass(&self) -> SolarMasses {
        self.helium_core_mass
    }

    /// The hydrogen envelope, M☉.
    #[must_use]
    pub const fn envelope_mass(&self) -> SolarMasses {
        self.envelope_mass
    }

    /// How the progenitor lost its hydrogen envelope, if it did.
    #[must_use]
    pub const fn stripping(&self) -> Stripping {
        self.stripping
    }
}

/// How a progenitor lost its hydrogen envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stripping {
    /// It kept it.
    None,
    /// Its own wind removed it, leaving a naked helium star.
    Wind,
    /// A companion removed it (plan 06, design note 11; the provisional mark of P06.T19, which
    /// plan 11 replaces). The track never sets it: a companion-stripped star follows an unstripped
    /// track until plan 11.
    Companion,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: f64) -> SolarMasses {
        SolarMasses::new(v)
    }

    #[test]
    fn the_envelopes_set_the_supernova_type() {
        assert_eq!(
            SupernovaType::of_envelopes(m(8.0), m(3.0)),
            SupernovaType::IIP
        );
        assert_eq!(
            SupernovaType::of_envelopes(m(2.0), m(3.0)),
            SupernovaType::IIL
        );
        assert_eq!(
            SupernovaType::of_envelopes(m(0.1), m(3.0)),
            SupernovaType::IIL
        );
        assert_eq!(
            SupernovaType::of_envelopes(m(0.05), m(3.0)),
            SupernovaType::IIb
        );
        assert_eq!(
            SupernovaType::of_envelopes(m(0.0), m(0.5)),
            SupernovaType::Ib
        );
        assert_eq!(
            SupernovaType::of_envelopes(m(0.0), m(0.1)),
            SupernovaType::Ic
        );
    }

    #[test]
    fn only_envelope_loss_is_gradual() {
        let kinds = [
            DeathKind::EnvelopeLoss,
            DeathKind::ElectronCapture,
            DeathKind::CoreCollapse {
                supernova: SupernovaType::IIP,
            },
            DeathKind::DirectCollapse,
            DeathKind::PairInstability,
            DeathKind::ThermonuclearDisruption,
        ];
        let gradual = kinds.iter().filter(|k| !k.is_sudden()).count();
        assert_eq!(gradual, 1);
    }
}
