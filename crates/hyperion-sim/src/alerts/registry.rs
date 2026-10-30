//! The registry of transient kinds: one row per [`TransientKind`] with its classes, how its events
//! are found, the bound on its peak luminosity in each band, its longest duration and its light
//! curve (plan 12, P12.T5.a, P12.T5.a.2 and P12.T5.b).
//!
//! The rows read their sources' own constants where the sources state them. Plan 09's supernova
//! templates and plan 06's giant eruptions and magnetar outbursts state no peak, so their bounds
//! are restated here and held to their sources by tests. The nine rows that are built:
//!
//! | Kind                | Route     | Photometric     | X-ray           | D        | Curve      |
//! | ------------------- | --------- | --------------- | --------------- | -------- | ---------- |
//! | `CoreCollapse`      | one-shot  | per band        | 8.0931 × 10³⁶ W | unending | supernova  |
//! | `TypeIa`            | one-shot  | per band        | none            | unending | supernova  |
//! | `GiantEruption`     | recurrent | 1.2106 × 10³⁴ W | none            | 20 yr    | eruption   |
//! | `TidalDisruption`   | centre    | `L_Edd` of M•   | `L_Edd` ÷ 15.8  | unending | tail       |
//! | `CentralFlare`      | centre    | none            | 3.5965 × 10²⁸ W | 8,175 s  | flare      |
//! | `Flare`             | local     | per host        | none            | 1 day    | star flare |
//! | `Glitch`            | local     | none            | none            | instant  | none       |
//! | `MagnetarBurst`     | recurrent | none            | 5 × 10⁴⁰ W      | 3 s      | top hat    |
//! | `FuOrionisOutburst` | local     | per host        | none            | 500 yr   | outburst   |
//!
//! The supernovae's bound in V is 2.1922 × 10³⁴ W for a core collapse and 1.4081 × 10³⁵ W for a
//! Type Ia ([`CORE_COLLAPSE_BAND_PEAKS`], [`TYPE_IA_BAND_PEAKS`]). The other seven rows wait on
//! plan 11's sources, P11.T9 (below): `LuminousRedNova` and `Kilonova` (one-shot), `Nova`,
//! `XrayTransient`, `XrayBurst` and `BeXOutburst` (recurrent) and `DwarfNova` (local). X-ray bursts
//! are seen across the Galaxy, so they are walked over the X-ray binaries, by the reasoning of the
//! magnetars' ruling (below; plan 12's Risks, "P12.T5.b as built").
//!
//! A kind's hosts are a class's ([`TransientRow::classes`]): the supernovae are the two supernova
//! classes', plan 11's kinds its classes' (novae and dwarf novae both accreting white dwarf
//! classes'), the giant eruptions the luminous blue variables', the magnetar outbursts the core
//! collapses', whose neutron stars the magnetars are, and the disruptions their victims'. The
//! centre's flares and the local kinds of ordinary stars have none.
//!
//! # Rows whose source is not built
//!
//! Plan 11's light curves and X-ray luminosities (P11.T9) do not exist yet. Their rows keep their
//! kind, classes and route, and every other part is `None`, with [`TransientRow::pending`] naming
//! the task whose source fills it (the galaxy plans' slice rule). Such a kind has no bound, so it
//! adds nothing to its classes' reach, which is true while no event of it can be generated; the
//! task that fills the row raises the reach.
//!
//! The giant eruptions' row is built from its source, plan 06's (P06.T28.e), though their class's
//! process (P09.T43) is not. So the luminous blue variables have a reach, and the walk (P12.T5.c)
//! finds no hosts in it until the process is built (plan 12's Risks, "P12.T5.a.2 as built").
//!
//! # Magnetars
//!
//! A magnetar's short bursts and giant flares are seen across the Galaxy, so their kind is walked
//! with the core collapses that leave the neutron stars, not found locally (main's ruling of
//! 2026-10-30; plan 12's Risks, "P12.T5.a.2 as built"). A magnetar's events are keyed by its body,
//! so a detection is the same event whichever query finds it. Giant flares take no kind of their
//! own; the event's series tells them from short bursts. A magnetar is active for under 227,273
//! years after its birth, so the route is complete for observers within 34,871 ly whatever the
//! entry's window. Beyond that an entry with a short window can leave the class while its magnetar
//! is still active, a recorded finding. Plan 06's thermal pulses take no kind: their light rises
//! a tenth of a dex over 0.5% of the interpulse period ([`thermal_pulse::RAPID_PEAK_DEX`] over
//! half of [`thermal_pulse::RAPID_PEAK_SHARE`], the module's provisional figures), half a year
//! for a 100-year period, which a sensor sees through the star's own observed state. Shorter
//! periods, which the census has, rise faster but no higher.
//!
//! # The bands
//!
//! A bound is on the kind's light in the band, its in-band power through the band's top hat
//! ([`light`](super::light)), and a row has one per band ([`TransientRow::peak_bound`]). The
//! supernovae's are their templates' light band by band, each term weighted by its spectrum's share
//! (P12.T5.b): their shock breakouts, bright but far in the ultraviolet, give V little, and the
//! stripped stars' gives the core collapses their X-rays. Every other row bounds the photometric
//! bands alike. Where its source is bolometric (the tails' bolometric light, the giant eruptions,
//! the star flares and the FU Orionis outbursts) that is the bolometric bound, an upper bound on
//! every band, so a reach from it errs long and the flux decides (Design note 5). The X-ray bound
//! is also filled where the source models the 2–10 keV band itself, the centre's flares and its
//! tails, and for the magnetars, whose energies are the event's whole output, so that their bound
//! bounds the band too.
//!
//! # The centre's tails
//!
//! P09.T30.b's tails radiate `min(η Ṁ c², L_Edd)`, with η fixed at 0.1, which keeps the Milky
//! Way's centre far above its observed quiescence. P09.T30.c replaces the law, behind a hold at
//! generator version 25, with an efficiency that falls with the fallback rate. The bound here is
//! the Eddington cap, which holds under either law, and the light curve is the tail's own, so the
//! row reads whichever law the galaxy's rules hold. The black hole's quiescence is no transient and
//! has no row.

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::features::centre::events::flares as centre_flares;
use crate::galaxy::features::centre::events::luminosity::{XRAY_SHARE, eddington_luminosity_watts};
use crate::stellar::events::{flares, fu_orionis, lbv, magnetar};
use crate::time::Span;
use crate::units::Watts;

use super::{AlertBand, BAND_COUNT, ClassSet};

#[cfg(doc)]
use crate::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
#[cfg(doc)]
use crate::galaxy::features::centre::events::{CentreFlare, DisruptionTail};
#[cfg(doc)]
use crate::stellar::events::{
    flares::Flare,
    fu_orionis::Outburst,
    lbv::Eruption,
    magnetar::{Burst, GiantFlare},
    thermal_pulse,
};

/// What went off: the kinds of event an alert reports (plan 12's Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransientKind {
    /// A core-collapse supernova, the explosion of a [`ClassId::CORE_COLLAPSE`] entry (plan 09,
    /// P09.T33).
    CoreCollapse,
    /// A Type Ia supernova, the explosion of a [`ClassId::TYPE_IA`] entry (plan 09, P09.T34).
    TypeIa,
    /// A luminous red nova, a stellar merger (plan 11).
    LuminousRedNova,
    /// A kilonova, a neutron-star merger (plan 11).
    Kilonova,
    /// A classical or recurrent nova of an accreting white dwarf (plan 11).
    Nova,
    /// A dwarf nova, an accretion-disc outburst of an accreting white dwarf (plan 11).
    DwarfNova,
    /// An outburst of a transient X-ray binary (plan 11).
    XrayTransient,
    /// A thermonuclear burst on an accreting neutron star (plan 11).
    XrayBurst,
    /// A Be/X-ray binary's outburst (plan 11).
    BeXOutburst,
    /// A luminous blue variable's giant eruption (plan 06's event, plan 09's class).
    GiantEruption,
    /// The central black hole's disruption of a star, and the tail of its debris falling back
    /// (plan 09, P09.T30.b).
    TidalDisruption,
    /// One of the central black hole's X-ray flares (plan 09, P09.T30.a).
    CentralFlare,
    /// A white-light flare of a star with a convective envelope (plan 06, P06.T28.a).
    Flare,
    /// A pulsar glitch (plan 06, P06.T28.b).
    Glitch,
    /// A magnetar's short burst or giant flare, of a core collapse's neutron star (plan 06,
    /// P06.T28.c).
    MagnetarBurst,
    /// An FU Orionis outburst of a young star (plan 06, P06.T28.d).
    FuOrionisOutburst,
}

impl TransientKind {
    /// Every kind, in the registry's order.
    pub const ALL: [Self; 16] = [
        Self::CoreCollapse,
        Self::TypeIa,
        Self::LuminousRedNova,
        Self::Kilonova,
        Self::Nova,
        Self::DwarfNova,
        Self::XrayTransient,
        Self::XrayBurst,
        Self::BeXOutburst,
        Self::GiantEruption,
        Self::TidalDisruption,
        Self::CentralFlare,
        Self::Flare,
        Self::Glitch,
        Self::MagnetarBurst,
        Self::FuOrionisOutburst,
    ];

    /// The kind's row of the registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::alerts::{AlertBand, LongestDuration, PeakBound, TransientKind};
    /// use hyperion_sim::galaxy::features::centre::events::flares;
    ///
    /// let row = TransientKind::CentralFlare.row();
    /// let bound = hyperion_sim::units::Watts::new(flares::MAX_PEAK_LUMINOSITY_WATTS);
    /// assert_eq!(row.peak_bound(AlertBand::XRay), Some(PeakBound::Watts(bound)));
    /// assert_eq!(row.duration(), Some(LongestDuration::AtMost(flares::MAX_DURATION)));
    /// // Plan 11's novae are not built yet: the row says which task fills it.
    /// assert_eq!(TransientKind::Nova.row().pending(), Some("P11.T9"));
    /// ```
    #[must_use]
    pub fn row(self) -> &'static TransientRow {
        &ROWS[self.index()]
    }

    /// The kind's place in [`ALL`](Self::ALL) and the registry.
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::CoreCollapse => 0,
            Self::TypeIa => 1,
            Self::LuminousRedNova => 2,
            Self::Kilonova => 3,
            Self::Nova => 4,
            Self::DwarfNova => 5,
            Self::XrayTransient => 6,
            Self::XrayBurst => 7,
            Self::BeXOutburst => 8,
            Self::GiantEruption => 9,
            Self::TidalDisruption => 10,
            Self::CentralFlare => 11,
            Self::Flare => 12,
            Self::Glitch => 13,
            Self::MagnetarBurst => 14,
            Self::FuOrionisOutburst => 15,
        }
    }
}

/// How a kind's events are found (plan 12, P12.T5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Route {
    /// One event per catalogue entry, at its time T, whose arrival is compared with the window
    /// directly (P12.T5.b).
    OneShot,
    /// The recurrent events of a catalogue class's hosts, through plan 11's scan marks or plan
    /// 09's class (P12.T5.c), plan 06's events on the core collapses' neutron stars among them.
    Recurrent,
    /// The galactic centre's: its black hole on plan 10's global list and its disruptions on the
    /// centre's feature-level list (P12.T5.d).
    Centre,
    /// A system's own events, found with the range query over each system's retarded interval
    /// (P12.T5.e). Too faint for a distant scan, so they add nothing to their classes' reach.
    Local,
}

/// What bounds a kind's peak luminosity in one band.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum PeakBound {
    /// At most this luminosity, a constant of the source with its own citation.
    Watts(Watts),
    /// At most this share of the galaxy's central black hole's Eddington luminosity
    /// ([`eddington_luminosity_watts`] of its mass), which depends on the galaxy.
    EddingtonShare(f64),
    /// No bound across hosts: each event's luminosity is a factor of its own star's, read per
    /// event (a local event, P12.T5.e).
    PerHost,
}

impl PeakBound {
    /// The bound in `galaxy`, W, or `None` for [`PerHost`](Self::PerHost).
    ///
    /// The black hole's mass is solved on first use ([`BlackHoleParams`]), about a hundred
    /// milliseconds once per galaxy.
    ///
    /// [`BlackHoleParams`]: crate::galaxy::params::BlackHoleParams
    #[must_use]
    pub fn in_galaxy(self, galaxy: &Galaxy) -> Option<Watts> {
        match self {
            Self::Watts(watts) => Some(watts),
            Self::EddingtonShare(share) => Some(Watts::new(
                share * eddington_luminosity_watts(galaxy.params().black_hole().mass()),
            )),
            Self::PerHost => None,
        }
    }
}

/// The longest a kind's event lasts: Design note 6's D, by which the emission interval reaches
/// back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LongestDuration {
    /// No event lasts longer: its source's own look-back.
    AtMost(Span),
    /// The light curve has no end. A one-shot event is found from its time T (Design note 6),
    /// and the centre's tails over the whole source horizon (P09.T30.b's `TAIL_LOOK_BACK_BINS`).
    Unending,
}

/// The function that gives a kind's luminosity at a time: the light curve of its row.
///
/// Each source's events carry what their curves read, so a walk calls the named function on the
/// event it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LightCurveSource {
    /// Plan 09's supernova templates (P09.T19.b), the entry's [`SupernovaKind`] at its age since
    /// T, in each band by its terms' shares ([`supernova_luminosity`](super::supernova_luminosity),
    /// P12.T5.b); [`LightCurve::luminosity`] is their bolometric light.
    SupernovaTemplate,
    /// [`DisruptionTail::luminosity_watts_at`] in X-rays, and
    /// [`DisruptionTail::bolometric_watts_at`] as the bound on every other band (P09.T30.b).
    DisruptionTail,
    /// [`CentreFlare::luminosity_watts_at`]: unabsorbed 2–10 keV, above the black hole's
    /// quiescence (P09.T30.a).
    CentreFlare,
    /// The flare's own light, ([`Flare::luminosity_factor_at`] − 1) times the star's quiescent
    /// luminosity at the epoch: bolometric, above the star's, as the centre's flares are above its
    /// quiescence, so that a detection clears when the flare fades (P06.T28.a).
    StarFlare,
    /// The outburst's own light, ([`Outburst::luminosity_factor_at`] − 1) times the star's
    /// quiescent luminosity: bolometric, above the star's (P06.T28.d).
    FuOrionisOutburst,
    /// A giant eruption's own light, ([`Eruption::luminosity_factor_at`] − 1) times the star's
    /// quiescent luminosity at the onset ([`Eruption::quiescent_luminosity`]): bolometric, above
    /// the star's (P06.T28.e).
    GiantEruption,
    /// A magnetar's short burst or giant flare, a top hat at its mean luminosity over its duration
    /// ([`Burst::mean_luminosity_watts`], [`GiantFlare::mean_luminosity_watts`]), since plan 06
    /// gives no shape: the event's whole output, in no one band (P06.T28.c).
    MagnetarBurst,
}

/// One kind's row of the registry (plan 12, P12.T5.a): its classes, how its events are found,
/// the bound on its peak luminosity in each band, its longest duration and its light curve.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TransientRow {
    kind: TransientKind,
    classes: ClassSet,
    route: Route,
    source: RowSource,
}

/// What a row knows of its kind's source.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum RowSource {
    /// The source is built.
    Built {
        /// The bound in each band, in [`AlertBand::ALL`]'s order.
        bounds: [Option<PeakBound>; BAND_COUNT],
        /// The longest an event lasts.
        duration: LongestDuration,
        /// The light curve, if the kind gives light.
        light_curve: Option<LightCurveSource>,
    },
    /// The source is not built: the task that builds it.
    Pending(&'static str),
}

impl TransientRow {
    /// The row's kind.
    #[must_use]
    pub const fn kind(&self) -> TransientKind {
        self.kind
    }

    /// The catalogue classes whose hosts have events of the kind: empty for the centre's flares
    /// and for the local kinds of ordinary stars. A local event of a class's host (a pulsar of a
    /// supernova entry) takes its class from the host (P12.T5.e).
    #[must_use]
    pub const fn classes(&self) -> ClassSet {
        self.classes
    }

    /// How the kind's events are found.
    #[must_use]
    pub const fn route(&self) -> Route {
        self.route
    }

    /// The bound on the kind's peak luminosity in `band`, its in-band power; `None` where the
    /// source models no light in the band, or is not built ([`pending`](Self::pending)).
    #[must_use]
    pub const fn peak_bound(&self, band: AlertBand) -> Option<PeakBound> {
        match self.source {
            RowSource::Built { bounds, .. } => bounds[band.index()],
            RowSource::Pending(_) => None,
        }
    }

    /// The longest an event of the kind lasts; `None` while the source is not built.
    #[must_use]
    pub const fn duration(&self) -> Option<LongestDuration> {
        match self.source {
            RowSource::Built { duration, .. } => Some(duration),
            RowSource::Pending(_) => None,
        }
    }

    /// The function that gives an event's luminosity at a time; `None` for a kind that gives no
    /// light (a glitch changes a pulsar's spin, which a sensor reads by timing) or whose source
    /// is not built.
    #[must_use]
    pub const fn light_curve(&self) -> Option<LightCurveSource> {
        match self.source {
            RowSource::Built { light_curve, .. } => light_curve,
            RowSource::Pending(_) => None,
        }
    }

    /// The task whose source fills the row's bounds, duration and light curve, while it is not
    /// built: `P11.T9`, for plan 11's kinds.
    #[must_use]
    pub const fn pending(&self) -> Option<&'static str> {
        match self.source {
            RowSource::Built { .. } => None,
            RowSource::Pending(task) => Some(task),
        }
    }

    /// The row of a kind whose source is not built: its classes and route, and the task that
    /// fills it.
    #[must_use]
    const fn pending_on(
        kind: TransientKind,
        classes: ClassSet,
        route: Route,
        task: &'static str,
    ) -> Self {
        Self {
            kind,
            classes,
            route,
            source: RowSource::Pending(task),
        }
    }
}

#[cfg(test)]
impl TransientRow {
    /// A row of `kind` with `classes`, `route` and a photometric bound of `photometric`, for the
    /// tests of what reads rows.
    #[must_use]
    pub(super) const fn for_test(
        kind: TransientKind,
        classes: ClassSet,
        route: Route,
        photometric: Watts,
    ) -> Self {
        Self {
            kind,
            classes,
            route,
            source: RowSource::Built {
                bounds: bands_alike(Some(PeakBound::Watts(photometric)), None),
                duration: LongestDuration::AtMost(Span::ZERO),
                light_curve: None,
            },
        }
    }
}

/// The bound on a core collapse's bolometric luminosity, W: 10³⁸ (10⁴⁵ erg/s), the shock
/// breakout's peak in plan 09's brightest templates (IIP, IIL and IIb; [`LightCurve::of`]) at the
/// instant of the explosion, before the plateau and the radioactive heating begin, after which
/// every term falls or rises far below it. Ib and Ic break out at a tenth of it. The IIP and IIL
/// breakouts are typical (Nakar and Sari 2010, ApJ 725, 904, equation A-8 and §4.1: about 9 ×
/// 10⁴⁷ erg over some 20 minutes at 500 R☉); IIb's was not read at source. It bounds every band's
/// bound ([`CORE_COLLAPSE_BAND_PEAKS`]), which the row takes.
pub const CORE_COLLAPSE_PEAK: Watts = Watts::new(1e38);

/// The bound on a Type Ia's bolometric luminosity, W: 1.0730 × 10³⁶ (1.0730 × 10⁴³ erg/s), the
/// peak of plan 09's template for the merger, double-detonation and hydrogen-donor channels,
/// 1.07291 × 10³⁶ W at 15.13 days ([`LightCurve::of`]), rounded up at the fifth figure. The Type
/// Iax template peaks at 1.345 × 10³⁵ W. It bounds every band's bound ([`TYPE_IA_BAND_PEAKS`]),
/// which the row takes.
pub const TYPE_IA_PEAK: Watts = Watts::new(1.0730e36);

/// The bound on a core collapse's luminosity in each band, W, in [`AlertBand::ALL`]'s order: the
/// greatest of the IIP, IIL, IIb, Ib and Ic templates' light in the band
/// ([`supernova_luminosity`](super::supernova_luminosity)) at any age, rounded up at the fifth
/// figure (P12.T5.b).
///
/// | Band  | Bound (W)       | Set by                                              |
/// | ----- | --------------- | --------------------------------------------------- |
/// | U     | 1.8819 × 10³⁴   | the Ib's and Ic's radioactive light, 14.6 days in   |
/// | B     | 2.6861 × 10³⁴   | the same                                            |
/// | V     | 2.1922 × 10³⁴   | a IIL's plateau, 8.9 days in                        |
/// | R     | 3.5972 × 10³⁴   | the same, 8.8 days in                               |
/// | I     | 2.5631 × 10³⁴   | the same, 8.75 days in                              |
/// | J     | 1.4989 × 10³⁴   | the same, 8.6 days in                               |
/// | H     | 7.3777 × 10³³   | the same, 8.6 days in                               |
/// | K     | 4.1255 × 10³³   | the same, 8.55 days in                              |
/// | N     | 1.9515 × 10³²   | the same, 8.5 days in                               |
/// | radio | none            |                                                     |
/// | X     | 8.0931 × 10³⁶   | the stripped stars' breakout at the explosion       |
///
/// A test holds each at or above a fine search about its templates' peaks, and within 10⁻⁴ of
/// it.
pub const CORE_COLLAPSE_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(1.8819e34),
    Watts::new(2.6861e34),
    Watts::new(2.1922e34),
    Watts::new(3.5972e34),
    Watts::new(2.5631e34),
    Watts::new(1.4989e34),
    Watts::new(7.3777e33),
    Watts::new(4.1255e33),
    Watts::new(1.9515e32),
    Watts::ZERO,
    Watts::new(8.0931e36),
];

/// The bound on a Type Ia's luminosity in each band, W, in [`AlertBand::ALL`]'s order: the greatest
/// of the four channels' templates' light in the band
/// ([`supernova_luminosity`](super::supernova_luminosity)) at any age, rounded up at the fifth
/// figure (P12.T5.b).
///
/// | Band  | Bound (W)       |
/// | ----- | --------------- |
/// | U     | 2.1374 × 10³⁵   |
/// | B     | 2.2751 × 10³⁵   |
/// | V     | 1.4081 × 10³⁵   |
/// | R     | 1.8823 × 10³⁵   |
/// | I     | 1.0445 × 10³⁵   |
/// | J     | 4.3014 × 10³⁴   |
/// | H     | 1.8505 × 10³⁴   |
/// | K     | 9.3988 × 10³³   |
/// | N     | 3.6453 × 10³²   |
/// | radio | none            |
/// | X     | none            |
///
/// The normal template sets every band at its peak, 15.15 days in, with SN 2011fe's V share,
/// 0.131; the Type Iax's peaks some ten times fainter. A test holds each at or above a fine search
/// about its templates' peaks, and within 10⁻⁴ of it.
pub const TYPE_IA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(2.1374e35),
    Watts::new(2.2751e35),
    Watts::new(1.4081e35),
    Watts::new(1.8823e35),
    Watts::new(1.0445e35),
    Watts::new(4.3014e34),
    Watts::new(1.8505e34),
    Watts::new(9.3988e33),
    Watts::new(3.6453e32),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a giant eruption's own light in any band, W: 1.2106 × 10³⁴, 10^7.5 L☉, the top of
/// plan 06's law for its peak ([`lbv::LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES`]) with the IAU's
/// nominal solar luminosity, 3.828 × 10²⁶ W, rounded up at the fifth figure. The eruption's own
/// light is the drawn peak less the star's quiescent luminosity, so it stays under the bound. A
/// constant, not a factor of the host's, since the peak law is absolute. Bolometric.
pub const GIANT_ERUPTION_PEAK: Watts = Watts::new(1.2106e34);

/// The bound on a magnetar outburst's luminosity in X-rays, W: 5 × 10⁴⁰ (5 × 10⁴⁷ erg/s), a giant
/// flare's spike at the top of its energy law, 10⁴⁷ erg over [`magnetar::GIANT_FLARE_SPIKE`]'s 0.2
/// s. A short burst reaches at most 2.512 × 10³⁶ W, 10⁴¹ erg over the 3.981 ms floor of its
/// duration law. Plan 06's energies are the whole event's (main's ruling of 2026-10-30), so the
/// bound bounds the 2–10 keV band too. The row has no photometric bound, since plan 06 models no
/// optical light for them.
pub const MAGNETAR_BURST_PEAK: Watts = Watts::new(5e40);

/// The bounds of a row that bounds every photometric band alike by `photometric`, and X-rays by
/// `xray`.
const fn bands_alike(
    photometric: Option<PeakBound>,
    xray: Option<PeakBound>,
) -> [Option<PeakBound>; BAND_COUNT] {
    let mut bounds = [photometric; BAND_COUNT];
    bounds[AlertBand::XRay.index()] = xray;
    bounds
}

/// The bounds of a row with one constant bound per band, `peaks`: a band of no light, whose peak
/// is zero, has none.
const fn band_by_band(peaks: [Watts; BAND_COUNT]) -> [Option<PeakBound>; BAND_COUNT] {
    let mut bounds = [None; BAND_COUNT];
    let mut i = 0;
    while i < BAND_COUNT {
        if peaks[i].value() > 0.0 {
            bounds[i] = Some(PeakBound::Watts(peaks[i]));
        }
        i += 1;
    }
    bounds
}

/// Both classes of accreting white dwarf, the hosts of novae and dwarf novae.
const ACCRETING_WHITE_DWARFS: ClassSet =
    ClassSet::of(ClassId::ACCRETING_WHITE_DWARF).with(ClassId::ACCRETING_WHITE_DWARF_SLOW);

/// The registry, in [`TransientKind::ALL`]'s order (module documentation).
static ROWS: [TransientRow; 16] = [
    TransientRow {
        kind: TransientKind::CoreCollapse,
        classes: ClassSet::of(ClassId::CORE_COLLAPSE),
        route: Route::OneShot,
        source: RowSource::Built {
            bounds: band_by_band(CORE_COLLAPSE_BAND_PEAKS),
            duration: LongestDuration::Unending,
            light_curve: Some(LightCurveSource::SupernovaTemplate),
        },
    },
    TransientRow {
        kind: TransientKind::TypeIa,
        classes: ClassSet::of(ClassId::TYPE_IA),
        route: Route::OneShot,
        source: RowSource::Built {
            bounds: band_by_band(TYPE_IA_BAND_PEAKS),
            duration: LongestDuration::Unending,
            light_curve: Some(LightCurveSource::SupernovaTemplate),
        },
    },
    TransientRow::pending_on(
        TransientKind::LuminousRedNova,
        ClassSet::of(ClassId::STELLAR_MERGER),
        Route::OneShot,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::Kilonova,
        ClassSet::of(ClassId::NEUTRON_STAR_MERGER),
        Route::OneShot,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::Nova,
        ACCRETING_WHITE_DWARFS,
        Route::Recurrent,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::DwarfNova,
        ACCRETING_WHITE_DWARFS,
        Route::Local,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::XrayTransient,
        ClassSet::of(ClassId::XRAY_BINARY),
        Route::Recurrent,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::XrayBurst,
        ClassSet::of(ClassId::XRAY_BINARY),
        Route::Recurrent,
        "P11.T9",
    ),
    TransientRow::pending_on(
        TransientKind::BeXOutburst,
        ClassSet::of(ClassId::XRAY_BINARY),
        Route::Recurrent,
        "P11.T9",
    ),
    TransientRow {
        kind: TransientKind::GiantEruption,
        classes: ClassSet::of(ClassId::LUMINOUS_BLUE_VARIABLE),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: bands_alike(Some(PeakBound::Watts(GIANT_ERUPTION_PEAK)), None),
            duration: LongestDuration::AtMost(lbv::MAX_DURATION),
            light_curve: Some(LightCurveSource::GiantEruption),
        },
    },
    TransientRow {
        kind: TransientKind::TidalDisruption,
        classes: ClassSet::of(ClassId::TIDAL_DISRUPTION_VICTIM),
        route: Route::Centre,
        source: RowSource::Built {
            bounds: bands_alike(
                Some(PeakBound::EddingtonShare(1.0)),
                Some(PeakBound::EddingtonShare(XRAY_SHARE)),
            ),
            duration: LongestDuration::Unending,
            light_curve: Some(LightCurveSource::DisruptionTail),
        },
    },
    TransientRow {
        kind: TransientKind::CentralFlare,
        classes: ClassSet::EMPTY,
        route: Route::Centre,
        source: RowSource::Built {
            bounds: bands_alike(
                None,
                Some(PeakBound::Watts(Watts::new(
                    centre_flares::MAX_PEAK_LUMINOSITY_WATTS,
                ))),
            ),
            duration: LongestDuration::AtMost(centre_flares::MAX_DURATION),
            light_curve: Some(LightCurveSource::CentreFlare),
        },
    },
    TransientRow {
        kind: TransientKind::Flare,
        classes: ClassSet::EMPTY,
        route: Route::Local,
        source: RowSource::Built {
            bounds: bands_alike(Some(PeakBound::PerHost), None),
            duration: LongestDuration::AtMost(flares::MAX_DURATION),
            light_curve: Some(LightCurveSource::StarFlare),
        },
    },
    TransientRow {
        kind: TransientKind::Glitch,
        classes: ClassSet::EMPTY,
        route: Route::Local,
        source: RowSource::Built {
            bounds: bands_alike(None, None),
            duration: LongestDuration::AtMost(Span::ZERO),
            light_curve: None,
        },
    },
    TransientRow {
        kind: TransientKind::MagnetarBurst,
        classes: ClassSet::of(ClassId::CORE_COLLAPSE),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: bands_alike(None, Some(PeakBound::Watts(MAGNETAR_BURST_PEAK))),
            duration: LongestDuration::AtMost(magnetar::MAX_BURST_DURATION),
            light_curve: Some(LightCurveSource::MagnetarBurst),
        },
    },
    TransientRow {
        kind: TransientKind::FuOrionisOutburst,
        classes: ClassSet::EMPTY,
        route: Route::Local,
        source: RowSource::Built {
            bounds: bands_alike(Some(PeakBound::PerHost), None),
            duration: LongestDuration::AtMost(fu_orionis::MAX_DURATION),
            light_curve: Some(LightCurveSource::FuOrionisOutburst),
        },
    },
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
    use crate::galaxy::catalogue_classes::type_ia::IaChannel;
    use crate::galaxy::features::centre::events::CentreVictims;
    use crate::galaxy::features::centre::events::DisruptionTail;
    use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
    use crate::galaxy::gas::ccm::Band;
    use crate::galaxy::rules::GalaxyRules;
    use crate::math;
    use crate::stellar::remnant::SupernovaType;
    use crate::time::{SourceHorizon, UniverseTime};
    use crate::units::consts::SOLAR_LUMINOSITY_W;
    use crate::units::{LightYears, SolarMasses, SolarRadii, WattsPerSquareMetre, Years};

    use super::super::light;
    use super::super::reach::{ROOT_CUBE_DIAGONAL, class_reach, kind_reach, reach};
    use super::super::{BandLimits, SensorHorizon, supernova_luminosity, supernova_peak};

    #[test]
    fn every_kind_has_its_own_row_in_order() {
        for (i, kind) in TransientKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(), i);
            assert_eq!(kind.row().kind(), kind);
        }
    }

    /// A row is either built, with a duration and (but for a glitch) a bound and a light curve, or
    /// pending, with nothing but its kind, classes and route and the task that fills it.
    #[test]
    fn a_row_without_its_source_names_the_task_that_fills_it() {
        for kind in TransientKind::ALL {
            let row = kind.row();
            let bounds = AlertBand::ALL.map(|band| row.peak_bound(band));
            if let Some(task) = row.pending() {
                assert_eq!(task, "P11.T9", "{kind:?}");
                assert!(bounds.iter().all(Option::is_none), "{kind:?}");
                assert_eq!(row.duration(), None, "{kind:?}");
                assert_eq!(row.light_curve(), None, "{kind:?}");
            } else {
                assert!(row.duration().is_some(), "{kind:?}");
                let lit = kind != TransientKind::Glitch;
                assert_eq!(row.light_curve().is_some(), lit, "{kind:?}");
                assert_eq!(bounds.iter().any(Option::is_some), lit, "{kind:?}");
            }
        }
        assert_eq!(
            TransientKind::ALL
                .into_iter()
                .filter(|kind| kind.row().pending().is_some())
                .count(),
            7
        );
    }

    /// A row bounded alike bounds every photometric band by one bound; the supernovae's rows take
    /// one bound per band, none where a band has no light.
    #[test]
    fn a_row_takes_one_bound_per_band() {
        let row = TransientKind::Flare.row();
        for band in Band::ALL {
            assert_eq!(
                row.peak_bound(AlertBand::Photometric(band)),
                Some(PeakBound::PerHost)
            );
        }
        assert_eq!(row.peak_bound(AlertBand::XRay), None);
        for (kind, peaks) in [
            (TransientKind::CoreCollapse, CORE_COLLAPSE_BAND_PEAKS),
            (TransientKind::TypeIa, TYPE_IA_BAND_PEAKS),
        ] {
            for (band, peak) in AlertBand::ALL.into_iter().zip(peaks) {
                let expected = (peak.value() > 0.0).then_some(PeakBound::Watts(peak));
                assert_eq!(
                    kind.row().peak_bound(band),
                    expected,
                    "{kind:?} in {band:?}"
                );
            }
        }
        let radio = AlertBand::Photometric(Band::Radio);
        assert_eq!(TransientKind::CoreCollapse.row().peak_bound(radio), None);
        assert_eq!(
            TransientKind::TypeIa.row().peak_bound(AlertBand::XRay),
            None
        );
    }

    /// Plan 09's templates never exceed the rows' bounds over the first three years, sampled every
    /// hour for the first day and every 0.05 day after it, and the bounds are reached: the core
    /// collapse's at the instant of the explosion, the Type Ia's to its fourth figure.
    #[test]
    fn the_supernova_bounds_hold_over_their_templates() {
        let collapses = [
            SupernovaType::IIP,
            SupernovaType::IIL,
            SupernovaType::IIb,
            SupernovaType::Ib,
            SupernovaType::Ic,
        ]
        .map(SupernovaKind::CoreCollapse);
        let thermonuclear = [
            IaChannel::Merger,
            IaChannel::DoubleDetonation,
            IaChannel::HydrogenDonor,
            IaChannel::Iax,
        ]
        .map(SupernovaKind::TypeIa);
        let days = (0..24)
            .map(|hour| f64::from(hour) / 24.0)
            .chain((20..21_920).map(|twentieth| f64::from(twentieth) / 20.0));
        let groups = [
            (CORE_COLLAPSE_PEAK, &collapses[..]),
            (TYPE_IA_PEAK, &thermonuclear[..]),
        ];
        let mut brightest = [0.0_f64; 2];
        for day in days {
            let age = Years::new(day / 365.25);
            for ((bound, kinds), most) in groups.iter().zip(&mut brightest) {
                for &kind in *kinds {
                    let watts = LightCurve::luminosity(kind, age).value();
                    assert!(watts <= bound.value(), "{kind:?} at {day} d: {watts:e} W");
                    *most = most.max(watts);
                }
            }
        }
        assert_same_bits(brightest[0], CORE_COLLAPSE_PEAK.value());
        assert!(
            brightest[1] / TYPE_IA_PEAK.value() > 0.9999,
            "{:e}",
            brightest[1]
        );
    }

    /// Each supernova row's bound in each band holds over its templates' light in the band, at and
    /// above a fine search about each template's peak on the grid (steps of 10⁻⁴ day over a fifth
    /// of a day), within 10⁻⁴ of the brightest, and under the bolometric bound; a band no template
    /// lights has none.
    #[test]
    fn the_band_bounds_hold_over_their_templates() {
        let collapses = [
            SupernovaType::IIP,
            SupernovaType::IIL,
            SupernovaType::IIb,
            SupernovaType::Ib,
        ]
        .map(SupernovaKind::CoreCollapse);
        let thermonuclear = [IaChannel::Merger, IaChannel::Iax].map(SupernovaKind::TypeIa);
        for (peaks, bolometric, kinds) in [
            (CORE_COLLAPSE_BAND_PEAKS, CORE_COLLAPSE_PEAK, &collapses[..]),
            (TYPE_IA_BAND_PEAKS, TYPE_IA_PEAK, &thermonuclear[..]),
        ] {
            for (band, bound) in AlertBand::ALL.into_iter().zip(peaks) {
                let mut brightest = 0.0_f64;
                for &kind in kinds {
                    let mut on_grid = (0.0_f64, Years::ZERO);
                    for age in light::peak_grid() {
                        let watts = supernova_luminosity(kind, age, band).value();
                        if watts > on_grid.0 {
                            on_grid = (watts, age);
                        }
                    }
                    assert_same_bits(on_grid.0, supernova_peak(kind, band).value());
                    let day = on_grid.1.value() * 365.25;
                    let fine = (-1_000..=1_000)
                        .map(|step| day + f64::from(step) * 1e-4)
                        .filter(|&day| day >= 0.0)
                        .map(|day| {
                            supernova_luminosity(kind, Years::new(day / 365.25), band).value()
                        })
                        .fold(on_grid.0, f64::max);
                    brightest = brightest.max(fine);
                }
                let bound = bound.value();
                assert!(brightest <= bound, "{band:?}: {brightest:e} over {bound:e}");
                if brightest > 0.0 {
                    assert!(bound / brightest - 1.0 < 1e-4, "{band:?}: {bound:e}");
                } else {
                    assert_same_bits(bound, 0.0);
                }
                assert!(bound < bolometric.value(), "{band:?}");
            }
        }
    }

    /// The centre's rows read plan 09's own figures: the flares' bound and duration, and the tails'
    /// Eddington share, which is a Sun-like victim's peak at the Milky Way's black hole.
    #[test]
    fn the_centre_rows_read_plan_09s_figures() {
        let flare = TransientKind::CentralFlare.row();
        assert_eq!(
            flare.peak_bound(AlertBand::XRay),
            Some(PeakBound::Watts(Watts::new(
                centre_flares::MAX_PEAK_LUMINOSITY_WATTS
            )))
        );
        let look_back = Span::from_seconds(
            centre_flares::BIN_SECONDS * i64::from(centre_flares::LOOK_BACK_BINS),
        );
        let Some(LongestDuration::AtMost(longest)) = flare.duration() else {
            panic!("a flare has an end");
        };
        assert!(longest <= look_back);

        let galaxy = milky_way_galaxy();
        let mass = galaxy.params().black_hole().mass();
        // About Sgr A*'s 4.3 × 10⁶ M☉, at which a Sun-like victim's fallback is above Eddington's
        // rate, so that its peak is the cap.
        assert!((3e6..6e6).contains(&mass.value()), "{mass:?}");
        let tail = DisruptionTail::new(
            UniverseTime::EPOCH,
            mass,
            SolarMasses::new(1.0),
            SolarRadii::new(1.0),
        );
        let row = TransientKind::TidalDisruption.row();
        let xray = row.peak_bound(AlertBand::XRay).unwrap().in_galaxy(galaxy);
        let peak = tail.peak_luminosity_watts();
        assert!(
            (xray.unwrap().value() / peak - 1.0).abs() < 1e-12,
            "{peak:e} W"
        );
        let light = row
            .peak_bound(AlertBand::Photometric(Band::V))
            .unwrap()
            .in_galaxy(galaxy)
            .unwrap();
        assert!((light.value() * XRAY_SHARE / peak - 1.0).abs() < 1e-12);
        assert_eq!(PeakBound::PerHost.in_galaxy(galaxy), None);
    }

    /// The galaxy's own tails stay under the row's bounds through the source horizon, under the
    /// rules of the generator version in force and under every hold at 25, where P09.T30.c's law
    /// joins: the tripwire on the row's reading whichever law the galaxy's rules hold. Each tail is
    /// sampled from its fallback time at quarter-octave steps to the horizon's end.
    #[test]
    fn the_galaxys_own_tails_stay_under_the_rows_bounds_under_each_rule_set() {
        let row = TransientKind::TidalDisruption.row();
        let now = milky_way_galaxy().clone();
        let next = now.clone().with_rules(GalaxyRules::every_25_hold());
        for galaxy in [now, next] {
            let bound = |band| {
                row.peak_bound(band)
                    .unwrap()
                    .in_galaxy(&galaxy)
                    .unwrap()
                    .value()
            };
            let (xray, light) = (
                bound(AlertBand::XRay),
                bound(AlertBand::Photometric(Band::V)),
            );
            let victims = CentreVictims::new(&galaxy, milky_way_centre()).unwrap();
            let mut tails = 0;
            for tail in victims.victims().iter().filter_map(|victim| victim.tail()) {
                tails += 1;
                assert!(tail.peak_luminosity_watts() <= xray, "{tail:?}");
                for step in 0..=120 {
                    let after = tail.fallback_time().value() * math::exp2(f64::from(step) / 4.0);
                    let Some(t) = Span::from_seconds_f64(after)
                        .and_then(|span| tail.disruption().checked_add(span))
                        .filter(|&t| t <= SourceHorizon::END)
                    else {
                        break;
                    };
                    assert!(tail.luminosity_watts_at(t) <= xray, "{tail:?} at {t}");
                    assert!(tail.bolometric_watts_at(t) <= light, "{tail:?} at {t}");
                }
            }
            assert!(tails > 10, "{tails} tails");
        }
    }

    /// The magnetar row bounds every short burst and giant flare. A burst's mean luminosity is at
    /// most the energy law's top over the duration law's floor, 2.512 × 10³⁶ W; a giant flare's
    /// spike at the top of its law is the bound, which its mean meets within rounding; and D
    /// covers both.
    #[test]
    fn the_magnetar_row_bounds_every_burst_and_giant_flare() {
        let row = TransientKind::MagnetarBurst.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::CORE_COLLAPSE));
        assert_eq!(
            row.peak_bound(AlertBand::XRay),
            Some(PeakBound::Watts(MAGNETAR_BURST_PEAK))
        );
        assert_eq!(row.peak_bound(AlertBand::Photometric(Band::V)), None);
        let bound = MAGNETAR_BURST_PEAK.value();

        // `Burst::mean_luminosity_watts` at the energy law's top and the shortest duration, which
        // `Span::from_seconds_f64` floors to the nanosecond as a burst's is.
        let floor = magnetar::BURST_DURATION_MEDIAN_SECONDS
            * math::exp10(
                -magnetar::BURST_DURATION_SIGMA_DEX * magnetar::BURST_DURATION_TRUNCATION,
            );
        let shortest = Span::from_seconds_f64(floor).unwrap();
        let burst = magnetar::BURST_ENERGY_JOULES.1 / shortest.as_seconds_f64();
        assert!((burst / 2.512e36 - 1.0).abs() < 1e-3, "{burst:e} W");
        assert!(burst <= bound, "{burst:e} W");

        // `GiantFlare::mean_luminosity_watts` at the energy law's top: `1e40 / 0.2` in f64.
        let giant =
            magnetar::GIANT_FLARE_ENERGY_JOULES.1 / magnetar::GIANT_FLARE_SPIKE.as_seconds_f64();
        assert!(giant <= bound, "{giant:e} W");
        assert!(giant / bound > 0.999_999, "{giant:e} W");

        assert_eq!(
            row.duration(),
            Some(LongestDuration::AtMost(magnetar::MAX_BURST_DURATION))
        );
        assert!(magnetar::GIANT_FLARE_SPIKE <= magnetar::MAX_BURST_DURATION);
        assert_eq!(row.light_curve(), Some(LightCurveSource::MagnetarBurst));
    }

    /// The giant-eruption row bounds the eruption's own light in every photometric band: the top of
    /// plan 06's peak law, 10^7.5 L☉, is under the bound and within its fifth figure, and D is the
    /// eruptions' longest.
    #[test]
    fn the_giant_eruption_row_bounds_its_own_light() {
        let row = TransientKind::GiantEruption.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::LUMINOUS_BLUE_VARIABLE));
        let top = math::exp10(lbv::LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES.1) * SOLAR_LUMINOSITY_W;
        let bound = GIANT_ERUPTION_PEAK.value();
        assert!(top <= bound && top / bound > 0.999_9, "{top:e} W");
        for band in Band::ALL {
            assert_eq!(
                row.peak_bound(AlertBand::Photometric(band)),
                Some(PeakBound::Watts(GIANT_ERUPTION_PEAK))
            );
        }
        assert_eq!(row.peak_bound(AlertBand::XRay), None);
        assert_eq!(
            row.duration(),
            Some(LongestDuration::AtMost(lbv::MAX_DURATION))
        );
        assert_eq!(row.light_curve(), Some(LightCurveSource::GiantEruption));
    }

    /// An X-ray sensor at 10⁻¹³ W m⁻² sees the core collapses' class to the cube's diagonal, through
    /// its magnetars, even a short burst at its bound, and through the stripped stars' breakout,
    /// whose bound is 8.0931 × 10³⁶ W, which at 10⁻⁶ W m⁻² still reaches 84,826 ly. A visual sensor
    /// sees no magnetar.
    #[test]
    fn the_core_collapses_reach_the_diagonal_in_x_rays() {
        let w = WattsPerSquareMetre::new;
        let limits = BandLimits::new(w(1e-13), w(1e-13)).unwrap();
        let xray = SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap();
        assert_eq!(
            class_reach(ClassId::CORE_COLLAPSE, &xray),
            ROOT_CUBE_DIAGONAL
        );
        assert_eq!(
            kind_reach(TransientKind::MagnetarBurst, &xray),
            ROOT_CUBE_DIAGONAL
        );
        assert_eq!(
            kind_reach(TransientKind::CoreCollapse, &xray),
            ROOT_CUBE_DIAGONAL
        );
        let bright = BandLimits::new(w(1e-6), w(1e-6)).unwrap();
        let breakout = kind_reach(
            TransientKind::CoreCollapse,
            &SensorHorizon::new([(AlertBand::XRay, bright)]).unwrap(),
        );
        assert!((breakout.value() - 84_826.0).abs() < 1.0, "{breakout:?}");
        assert_eq!(reach(Watts::new(2.512e36), limits), ROOT_CUBE_DIAGONAL);
        let visual = SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap();
        assert_eq!(
            kind_reach(TransientKind::MagnetarBurst, &visual),
            LightYears::ZERO
        );
    }

    /// Every row a walk takes by reach, one-shot or recurrent, has constant bounds only, so that
    /// `class_reach` needs no galaxy: the bounds that need the galaxy or the host are the centre's
    /// and the local rows', which reach does not walk.
    #[test]
    fn walked_rows_have_constant_bounds() {
        let mut walked = Vec::new();
        for kind in TransientKind::ALL {
            let row = kind.row();
            match row.route() {
                Route::OneShot | Route::Recurrent => {
                    for band in AlertBand::ALL {
                        let bound = row.peak_bound(band);
                        assert!(
                            matches!(bound, None | Some(PeakBound::Watts(_))),
                            "{kind:?} in {band:?}: {bound:?}"
                        );
                    }
                    if row.pending().is_none() {
                        walked.push(kind);
                    }
                }
                Route::Centre | Route::Local => {}
            }
        }
        assert_eq!(
            walked,
            [
                TransientKind::CoreCollapse,
                TransientKind::TypeIa,
                TransientKind::GiantEruption,
                TransientKind::MagnetarBurst,
            ]
        );
    }

    /// X-ray bursts are walked over the X-ray binaries' class, as the magnetars over the core
    /// collapses', not found locally; their row waits on P11.T9, so they reach nothing yet.
    #[test]
    fn x_ray_bursts_are_walked_over_the_x_ray_binaries() {
        let row = TransientKind::XrayBurst.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::XRAY_BINARY));
        assert_eq!(row.pending(), Some("P11.T9"));
        let w = WattsPerSquareMetre::new;
        let limits = BandLimits::new(w(1e-13), w(1e-13)).unwrap();
        let xray = SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap();
        assert_eq!(class_reach(ClassId::XRAY_BINARY, &xray), LightYears::ZERO);
    }

    /// Every row and the band light pinned: each row's route, classes, bounds in every band,
    /// duration and curve, the spectra's shares and the templates' peaks. From P12.T5.b, which
    /// reads the rows, a change to any is a bless.
    #[test]
    fn registry_golden() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        use crate::GENERATOR_VERSION;

        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for kind in TransientKind::ALL {
            let row = kind.row();
            let classes: Vec<_> = row.classes().iter().map(ClassId::name).collect();
            w.line(&format!(
                "{kind:?}: {:?} over {classes:?}, pending {:?}, lasting {:?}, curve {:?}",
                row.route(),
                row.pending(),
                row.duration(),
                row.light_curve(),
            ));
            for band in AlertBand::ALL {
                match row.peak_bound(band) {
                    Some(PeakBound::Watts(watts)) => {
                        w.f64(&format!("{kind:?} in {band:?}"), watts.value());
                    }
                    Some(PeakBound::EddingtonShare(share)) => {
                        w.f64(&format!("{kind:?} in {band:?}, Eddington share"), share);
                    }
                    Some(PeakBound::PerHost) => {
                        w.line(&format!("{kind:?} in {band:?}: per host"));
                    }
                    None => {}
                }
            }
        }
        let kinds = [
            SupernovaKind::CoreCollapse(SupernovaType::IIP),
            SupernovaKind::CoreCollapse(SupernovaType::IIL),
            SupernovaKind::CoreCollapse(SupernovaType::IIb),
            SupernovaKind::CoreCollapse(SupernovaType::Ib),
            SupernovaKind::TypeIa(IaChannel::Merger),
            SupernovaKind::TypeIa(IaChannel::Iax),
        ];
        for kind in kinds {
            for band in AlertBand::ALL {
                w.f64(
                    &format!("{kind:?} peak in {band:?}"),
                    supernova_peak(kind, band).value(),
                );
                w.f64(
                    &format!("{kind:?} at 20 d in {band:?}"),
                    supernova_luminosity(kind, Years::new(20.0 / 365.25), band).value(),
                );
            }
        }
        golden!("alerts/registry", w.as_str());
    }

    /// The local kinds keep their sources' look-backs as their longest durations.
    #[test]
    fn the_local_rows_read_plan_06s_look_backs() {
        assert_eq!(
            TransientKind::Flare.row().duration(),
            Some(LongestDuration::AtMost(flares::MAX_DURATION))
        );
        assert_eq!(
            TransientKind::FuOrionisOutburst.row().duration(),
            Some(LongestDuration::AtMost(fu_orionis::MAX_DURATION))
        );
    }
}
