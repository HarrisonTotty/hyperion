//! The registry of transient kinds: one row per [`TransientKind`] with its classes, how its events
//! are found, the bound on its peak luminosity in each band, its longest duration and its light
//! curve (plan 12, P12.T5.a, P12.T5.a.2, P12.T5.b and P12.T5.c).
//!
//! The rows read their sources' own constants where the sources state them. Plan 09's supernova
//! templates, plan 06's giant eruptions and magnetar outbursts and plan 11's light curves state no
//! bound, so their bounds are restated here and held to their sources by tests. Every row is built:
//!
//! | Kind                | Route     | Photometric     | X-ray           | D        | Curve      |
//! | ------------------- | --------- | --------------- | --------------- | -------- | ---------- |
//! | `CoreCollapse`      | one-shot  | per band        | 8.0931 × 10³⁶ W | unending | supernova  |
//! | `TypeIa`            | one-shot  | per band        | none            | unending | supernova  |
//! | `LuminousRedNova`   | one-shot  | per band        | none            | 280 d    | binary     |
//! | `Kilonova`          | one-shot  | per band        | none            | 35 d     | binary     |
//! | `Nova`              | recurrent | per band        | none            | 169.3 d  | binary     |
//! | `DwarfNova`         | local     | per band        | none            | 5,517 yr | binary     |
//! | `XrayTransient`     | recurrent | per band        | 1.0282 × 10³² W | 143.2 d  | binary     |
//! | `XrayBurst`         | recurrent | per band        | 3.9023 × 10³¹ W | 144 s    | binary     |
//! | `BeXOutburst`       | recurrent | none            | 2.9472 × 10³⁰ W | 102.1 d  | binary     |
//! | `GiantEruption`     | recurrent | per band        | none            | 20 yr    | eruption   |
//! | `TidalDisruption`   | centre    | `L_Edd` of M•   | `L_Edd` ÷ 15.8  | unending | tail       |
//! | `CentralFlare`      | centre    | none            | 3.5965 × 10²⁸ W | 8,175 s  | flare      |
//! | `Flare`             | local     | per host        | none            | 1 day    | star flare |
//! | `Glitch`            | local     | none            | none            | instant  | none       |
//! | `MagnetarBurst`     | recurrent | none            | 1.8 × 10³⁸ W    | 3 s      | top hat    |
//! | `FuOrionisOutburst` | local     | per host        | none            | 500 yr   | outburst   |
//!
//! The supernovae's bound in V is 2.1922 × 10³⁴ W for a core collapse and 1.4081 × 10³⁵ W for a
//! Type Ia ([`CORE_COLLAPSE_BAND_PEAKS`], [`TYPE_IA_BAND_PEAKS`]); the mergers' is 7.6311 × 10³³ W
//! for a luminous red nova and 4.6816 × 10³³ W for a kilonova ([`LUMINOUS_RED_NOVA_BAND_PEAKS`],
//! [`KILONOVA_BAND_PEAKS`]; P12.T5.b), and a nova's 3.2612 × 10³¹ W ([`NOVA_BAND_PEAKS`]; P12.T5.c).
//! X-ray bursts are seen across the Galaxy, so they are walked over the X-ray binaries, as the
//! magnetars are over the core collapses (main's ruling of 2026-10-30; P12.T5.b).
//!
//! A kind's hosts are a class's ([`TransientRow::classes`]): the supernovae are the two supernova
//! classes', plan 11's kinds its classes' (novae and dwarf novae both accreting white dwarf
//! classes'), the giant eruptions the luminous blue variables', the magnetar outbursts the core
//! collapses', whose neutron stars the magnetars are, and the disruptions their victims'. The
//! centre's flares and the local kinds of ordinary stars have none.
//!
//! # Rows whose source is not built
//!
//! A row whose source is not built keeps its kind, classes and route, and every other part is
//! `None`, with [`TransientRow::pending`] naming the task whose source fills it (the galaxy plans'
//! slice rule). Such a kind has no bound, so it adds nothing to its classes' reach, which is true
//! while no event of it can be generated; the task that fills the row raises the reach. No row is
//! pending now; the rule stands for a kind registered later.
//!
//! # Plan 11's binary events
//!
//! Plan 11's seven kinds take their light from its curves ([`light_curves`], P11.T9.a), as its
//! events list them ([`binary::events`], P11.T9.b): the mergers' luminous red novae and kilonovae
//! (P12.T5.b), a red nova's curve of the pair's total mass and a kilonova's of its ejecta, and the
//! novae, dwarf novae, X-ray transients, X-ray bursts and Be/X outbursts of the accreting white
//! dwarfs and the X-ray binaries (P12.T5.c). Every curve is a closed form of its event's masses,
//! and each kind's light at its peak grows with them, so its bound in each band is its curve's peak
//! at the top of its mass range, through the curve's colour ([`binary_peak`](super::binary_peak)),
//! its X-ray bound the curve's own 2–10 keV light ([`BinaryLightCurve::xray_luminosity`]); tests
//! hold each to the curves over the ranges and to the events of sampled hosts. Every fade is
//! asymptotic, so an event's duration is plan 11's, from its onset until its light stays under
//! [`ACTIVE_SHARE`] (a hundredth) of its peak ([`BinaryEvent::duration`]), after which the alerts
//! count none of it ([`binary_luminosity`](super::binary_luminosity)), and a row's is the longest of
//! its kind's ([`BinaryLightCurve::fades_below`] at that share):
//!
//! - **Luminous red novae:** the heaviest merger the stellar mergers draw, two of the tracks'
//!   150 M☉ stars.
//! - **Kilonovae:** the most a neutron-star merger ejects, two neutron stars' 0.05 M☉.
//! - **Novae:** at their peak for a 1.36 M☉ white dwarf, the top of the scratch sampler's white
//!   dwarf masses, and as long as a 0.55 M☉ dwarf's, its bottom, whose decline is the slowest.
//! - **Dwarf novae:** at Warner's maximum for the 10-hour period at which the curve holds it, and
//!   no longer than its rise, the light of the most mass a disc gathers at the faintest maximum,
//!   and the slowest decay, each at its own extreme: a disc gathers at most
//!   [`MAX_NOVA_TRANSFER_RATE`] over the longest cycle, 33 years, onto a 1.36 M☉ dwarf. That bound
//!   is 5,517 years, against outbursts of days to months in sampled hosts: a recorded finding.
//! - **X-ray transients:** at the Eddington luminosity of a 15 M☉ black hole, the top of the
//!   scratch sampler's, for the donors' hydrogen, where the curve holds its peak; the shape is
//!   fixed.
//! - **X-ray bursts:** a helium burst on a 2.2 M☉ neutron star, at its helium Eddington
//!   luminosity, brighter than a mixed burst's half of the hydrogen-rich one, which lasts longer.
//! - **Be/X outbursts:** a fifth of a 2.2 M☉ neutron star's Eddington luminosity, in X-rays alone.
//!
//! Dwarf novae are local (P12.T5.e): kept out of distant scans by their route, whatever their
//! bound.
//!
//! [`light_curves`]: crate::stellar::binary::light_curves
//! [`binary::events`]: crate::stellar::binary::events
//! [`ACTIVE_SHARE`]: crate::stellar::binary::events::ACTIVE_SHARE
//! [`BinaryEvent::duration`]: crate::stellar::binary::events::BinaryEvent::duration
//! [`BinaryLightCurve::xray_luminosity`]: crate::stellar::binary::light_curves::BinaryLightCurve::xray_luminosity
//! [`BinaryLightCurve::fades_below`]: crate::stellar::binary::light_curves::BinaryLightCurve::fades_below
//! [`MAX_NOVA_TRANSFER_RATE`]: crate::stellar::binary::scan::MAX_NOVA_TRANSFER_RATE
//!
//! The giant eruptions' row is built from its source, plan 06's (P06.T28.e), and their class's
//! process is P09.T43's, which the recurrent walk reads (P12.T5.c). Their bound is the top of plan
//! 06's peak law band by band, at their 5,000 K colour ([`GIANT_ERUPTION_BAND_PEAKS`]).
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
//! stripped stars' gives the core collapses their X-rays. Plan 11's kinds take their curves' own
//! light band by band (above), and the giant eruptions their colour's share of their peak's. Every
//! other row bounds the photometric bands alike. Where its source is bolometric (the tails'
//! bolometric light, the star flares and the FU Orionis outbursts) that is the bolometric bound,
//! an upper bound on every band, so a reach from it errs long and the flux decides (Design note 5).
//! The X-ray bound is also filled where the source models the 2–10 keV band itself, the centre's
//! flares and its tails, and for the magnetars, whose energies are the event's whole output, so
//! that their bound bounds the band too.
//!
//! # The centre's tails
//!
//! P09.T30.b's tails radiate `min(ε Ṁ c², L_Edd)`, with an efficiency ε that falls with the
//! fallback rate from generator version 25 (P09.T30.c), where it was η = 0.1 at every rate. The
//! bound here is the Eddington cap, and the light curve is the tail's own. The black hole's
//! quiescence is no transient and has no row.

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::features::centre::events::flares as centre_flares;
use crate::galaxy::features::centre::events::luminosity::{XRAY_SHARE, eddington_luminosity_watts};
use crate::stellar::events::{flares, fu_orionis, lbv, magnetar};
use crate::time::Span;
use crate::units::Watts;

#[cfg(doc)]
use crate::stellar::binary::light_curves::{
    BeXOutburstCurve, DwarfNovaCurve, NovaCurve, XrayBurstCurve, XrayTransientCurve,
};

use super::{AlertBand, BAND_COUNT, ClassSet};

#[cfg(doc)]
use crate::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
#[cfg(doc)]
use crate::galaxy::features::centre::events::{CentreFlare, DisruptionTail};
#[cfg(doc)]
use crate::stellar::binary::light_curves::{BinaryLightCurve, LuminousRedNovaCurve};
#[cfg(doc)]
use crate::stellar::binary::scan::NsbhOutflow;
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
    /// // Plan 11's novae give no X-rays.
    /// assert_eq!(TransientKind::Nova.row().peak_bound(AlertBand::XRay), None);
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

/// The longest a kind's light takes from its onset to its peak (plan 12, P12.T8.b.1; research's
/// lean of 2026-10-30, adopted): a scan that starts afresh walks back by it, since a riser whose
/// onset arrived before the scan's window can cross a limit in it, under the limit at the window's
/// start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LongestRise {
    /// No event's light peaks later after its onset.
    AtMost(Span),
    /// Each event's own, with no bound across events: a tidal disruption's tail peaks at its
    /// fallback time, which grows with its victim's radius (P09.T30.b). The centre's walk lists
    /// the tails over their own look-back, and walks back by nothing more.
    PerEvent,
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
    /// Plan 11's curve of a binary event (P11.T9.a), which the event carries
    /// ([`BinaryEvent::curve`](crate::stellar::binary::events::BinaryEvent::curve)):
    /// [`BinaryLightCurve::in_band`] and [`BinaryLightCurve::xray_luminosity`] since its onset,
    /// while the event is active ([`binary_luminosity`](super::binary_luminosity)). A merger's
    /// luminous red nova or kilonova (P12.T5.b), and a nova, a dwarf nova, an X-ray transient's
    /// outburst, a Type I X-ray burst or a Be/X giant outburst (P12.T5.c).
    BinaryCurve,
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
#[expect(
    clippy::large_enum_variant,
    reason = "the rows are one static table of 16, read in place and never moved"
)]
enum RowSource {
    /// The source is built.
    Built {
        /// The bound in each band, in [`AlertBand::ALL`]'s order.
        bounds: [Option<PeakBound>; BAND_COUNT],
        /// The longest an event lasts.
        duration: LongestDuration,
        /// The longest its light takes to peak.
        rise: LongestRise,
        /// The light curve, if the kind gives light.
        light_curve: Option<LightCurveSource>,
    },
    /// The source is not built: the task that builds it.
    #[expect(
        dead_code,
        reason = "the slice rule's row of a kind registered before its source; every row is built now"
    )]
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

    /// The longest an event of the kind takes from its onset to its peak; `None` while the source
    /// is not built.
    #[must_use]
    pub const fn longest_rise(&self) -> Option<LongestRise> {
        match self.source {
            RowSource::Built { rise, .. } => Some(rise),
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
    /// built: `P11.T9`, for plan 11's mergers.
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
    #[expect(
        dead_code,
        reason = "the slice rule's row of a kind registered before its source; every row is built now"
    )]
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
                rise: LongestRise::AtMost(Span::ZERO),
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
/// | U     | 1.9674 × 10³⁵   |
/// | B     | 2.6941 × 10³⁵   |
/// | V     | 1.4081 × 10³⁵   |
/// | R     | 1.5305 × 10³⁵   |
/// | I     | 5.9617 × 10³⁴   |
/// | J     | 4.3014 × 10³⁴   |
/// | H     | 1.8505 × 10³⁴   |
/// | K     | 9.3988 × 10³³   |
/// | N     | 3.6453 × 10³²   |
/// | radio | none            |
/// | X     | none            |
///
/// The normal template sets every band at its peak, 15.15 days in, with SN 2011fe's shares in U to
/// I (0.131 in V); the Type Iax's peaks some ten times fainter. A test holds each at or above a fine search
/// about its templates' peaks, and within 10⁻⁴ of it.
pub const TYPE_IA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(1.9674e35),
    Watts::new(2.6941e35),
    Watts::new(1.4081e35),
    Watts::new(1.5305e35),
    Watts::new(5.9617e34),
    Watts::new(4.3014e34),
    Watts::new(1.8505e34),
    Watts::new(9.3988e33),
    Watts::new(3.6453e32),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a luminous red nova's light in each band, W, in [`AlertBand::ALL`]'s order: plan
/// 11's curve (P11.T9.a) at its peak through its 4,500 K colour ([`binary_peak`](super::binary_peak))
/// for the heaviest merger the stellar mergers draw, two of the tracks' 150 M☉ stars on their main
/// sequences, which is the top of the curve's range ([`LuminousRedNovaCurve::of`]), rounded up at
/// the fifth figure (P12.T5.b). The curve's peak grows as the merger's mass to the 2.5 up to the
/// heaviest progenitors seen, 30 M☉, and to the ⅔ above (P11.T9.a's fix of 2026-11-01), so the
/// bound is 2.49 × 10⁸ L☉ bolometric, 9.5353 × 10³⁴ W, 21 times the merger's Eddington luminosity.
///
/// | Band  | Bound (W)       |
/// | ----- | --------------- |
/// | U     | 2.2998 × 10³³   |
/// | B     | 5.7464 × 10³³   |
/// | V     | 7.6311 × 10³³   |
/// | R     | 1.5192 × 10³⁴   |
/// | I     | 1.3608 × 10³⁴   |
/// | J     | 1.1078 × 10³⁴   |
/// | H     | 6.1912 × 10³³   |
/// | K     | 3.7884 × 10³³   |
/// | N     | 2.1459 × 10³²   |
/// | radio | none            |
/// | X     | none            |
pub const LUMINOUS_RED_NOVA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(2.2998e33),
    Watts::new(5.7464e33),
    Watts::new(7.6311e33),
    Watts::new(1.5192e34),
    Watts::new(1.3608e34),
    Watts::new(1.1078e34),
    Watts::new(6.1912e33),
    Watts::new(3.7884e33),
    Watts::new(2.1459e32),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a kilonova's light in each band, W, in [`AlertBand::ALL`]'s order: plan 11's curve
/// (P11.T9.a) at its peak through its 5,000 K colour ([`binary_peak`](super::binary_peak)) for the
/// most a neutron-star merger ejects, two neutron stars' GW170817 0.05 M☉ (P11.T9.b; a black hole
/// ejects at most 0.026 M☉ of its neutron star, [`NsbhOutflow`]), rounded up at the fifth figure
/// (P12.T5.b). The curve's peak grows as the ejecta's mass to the 0.35.
///
/// | Band  | Bound (W)       |
/// | ----- | --------------- |
/// | U     | 1.9013 × 10³³   |
/// | B     | 4.0589 × 10³³   |
/// | V     | 4.6816 × 10³³   |
/// | R     | 8.6275 × 10³³   |
/// | I     | 7.0573 × 10³³   |
/// | J     | 5.0297 × 10³³   |
/// | H     | 2.6705 × 10³³   |
/// | K     | 1.5757 × 10³³   |
/// | N     | 8.3017 × 10³¹   |
/// | radio | none            |
/// | X     | none            |
pub const KILONOVA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(1.9013e33),
    Watts::new(4.0589e33),
    Watts::new(4.6816e33),
    Watts::new(8.6275e33),
    Watts::new(7.0573e33),
    Watts::new(5.0297e33),
    Watts::new(2.6705e33),
    Watts::new(1.5757e33),
    Watts::new(8.3017e31),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a giant eruption's own light, W: 1.2106 × 10³⁴, 10^7.5 L☉, the top of plan 06's
/// law for its peak ([`lbv::LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES`]) with the IAU's nominal solar
/// luminosity, 3.828 × 10²⁶ W, rounded up at the fifth figure. The eruption's own light is the
/// drawn peak less the star's quiescent luminosity, so it stays under the bound. A constant, not a
/// factor of the host's, since the peak law is absolute. Bolometric: it bounds every band's bound
/// ([`GIANT_ERUPTION_BAND_PEAKS`]), which the row takes.
pub const GIANT_ERUPTION_PEAK: Watts = Watts::new(1.2106e34);

/// The bound on a giant eruption's own light in each band, W, in [`AlertBand::ALL`]'s order:
/// 10^7.5 L☉ times a 5,000 K blackbody's share in the band, the eruption's colour
/// ([`GIANT_ERUPTION_SHARES`](super::recurrent::GIANT_ERUPTION_SHARES)), rounded up at the fifth
/// figure (P12.T5.c, as P12.T5.a.2's record foresaw): 1.1401 × 10³³ W in V, which reaches 1,007
/// ly at a V limit of 10⁻⁶ W m⁻². None in radio or X-rays.
pub const GIANT_ERUPTION_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(4.6298e32),
    Watts::new(9.8840e32),
    Watts::new(1.1401e33),
    Watts::new(2.1010e33),
    Watts::new(1.7186e33),
    Watts::new(1.2248e33),
    Watts::new(6.5031e32),
    Watts::new(3.8370e32),
    Watts::new(2.0216e31),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a magnetar outburst's whole output, W: 5 × 10⁴⁰ (5 × 10⁴⁷ erg/s), a giant flare's
/// spike at the top of its energy law, 10⁴⁷ erg over [`magnetar::GIANT_FLARE_SPIKE`]'s 0.2 s. A
/// short burst reaches at most 2.512 × 10³⁶ W, 10⁴¹ erg over the 3.981 ms floor of its duration
/// law. Plan 06's energies are the whole event's (main's ruling of 2026-10-30), so it bounds the
/// 2–10 keV band too; the row takes the band's own, [`MAGNETAR_BURST_XRAY_PEAK`] (P12.T5.c). The
/// row has no photometric bound, since plan 06 models no optical light for them.
pub const MAGNETAR_BURST_PEAK: Watts = Watts::new(5e40);

/// The bound on a magnetar outburst's luminosity in the 2–10 keV band, its in-band power, W: 1.8 ×
/// 10³⁸, [`MAGNETAR_BURST_PEAK`] × [`MAGNETAR_GIANT_FLARE_XRAY_SHARE`] (0.0036), a giant flare's
/// spike at the top of its energy law (P12.T5.c). A short burst reaches at most 2.512 × 10³⁶ W ×
/// [`MAGNETAR_BURST_XRAY_SHARE`], 2.7 × 10³⁵ W, in the band. The row's X-ray bound.
///
/// [`MAGNETAR_BURST_XRAY_SHARE`]: super::MAGNETAR_BURST_XRAY_SHARE
/// [`MAGNETAR_GIANT_FLARE_XRAY_SHARE`]: super::MAGNETAR_GIANT_FLARE_XRAY_SHARE
pub const MAGNETAR_BURST_XRAY_PEAK: Watts =
    Watts::new(MAGNETAR_BURST_PEAK.value() * super::recurrent::MAGNETAR_GIANT_FLARE_XRAY_SHARE);

/// The bound on a classical or recurrent nova's light in each band, W, in [`AlertBand::ALL`]'s
/// order: plan 11's curve ([`NovaCurve`], P11.T9.a) at its peak for a 1.36 M☉ white dwarf, the
/// heaviest an accreting white dwarf host had (the scratch sampler's limit), rounded up at the
/// fifth figure (P12.T5.c). A heavier dwarf peaks brighter (Livio's maximum magnitude), at 3.0662 ×
/// 10³² W in all for 1.36 M☉, an 8,000 K blackbody's light through each band; no radio or X-rays.
pub const NOVA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(3.5937e31),
    Watts::new(4.5319e31),
    Watts::new(3.2612e31),
    Watts::new(4.6978e31),
    Watts::new(2.8575e31),
    Watts::new(1.3369e31),
    Watts::new(6.0405e30),
    Watts::new(3.1779e30),
    Watts::new(1.3266e29),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on a dwarf nova's light in each band, W, in [`AlertBand::ALL`]'s order: plan 11's
/// curve ([`DwarfNovaCurve`], P11.T9.a) at its peak for an orbit of 10 hours, the longest period
/// at which Warner's maximum is read, beyond which it is held, rounded up at the fifth figure
/// (P12.T5.c). The maximum brightens with the period and depends on nothing else: 1.2269 × 10²⁸ W
/// in all, a 25,000 K disc's light through each band; no radio or X-rays.
pub const DWARF_NOVA_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(5.7290e26),
    Watts::new(4.2345e26),
    Watts::new(1.8985e26),
    Watts::new(2.1739e26),
    Watts::new(9.9260e25),
    Watts::new(3.1174e25),
    Watts::new(1.2055e25),
    Watts::new(5.6658e24),
    Watts::new(1.8580e23),
    Watts::ZERO,
    Watts::ZERO,
];

/// The bound on an X-ray transient's outburst's light in each band, W, in [`AlertBand::ALL`]'s
/// order: plan 11's curve ([`XrayTransientCurve`], P11.T9.a) at its peak, held at the Eddington
/// luminosity of a 15 M☉ black hole, the heaviest the scratch sampler gave, for the donors'
/// hydrogen mass fraction of 0.7, 2.2184 × 10³² W in all, rounded up at the fifth figure
/// (P12.T5.c). Its 2–10 keV light is 98% of it at a 0.55 keV blackbody's share, 1.0282 × 10³² W,
/// and its optical light the 2% its disc reprocesses at 25,000 K; none in radio.
pub const XRAY_TRANSIENT_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(2.0718e29),
    Watts::new(1.5313e29),
    Watts::new(6.8655e28),
    Watts::new(7.8615e28),
    Watts::new(3.5896e28),
    Watts::new(1.1274e28),
    Watts::new(4.3595e27),
    Watts::new(2.0490e27),
    Watts::new(6.7189e25),
    Watts::ZERO,
    Watts::new(1.0282e32),
];

/// The bound on a Type I X-ray burst's light in each band, W, in [`AlertBand::ALL`]'s order: plan
/// 11's curve ([`XrayBurstCurve`], P11.T9.a) at the peak of a helium burst on a 2.2 M☉ neutron
/// star, the heaviest the scratch sampler gave, at its helium Eddington luminosity, 5.5311 × 10³¹ W
/// in all, rounded up at the fifth figure (P12.T5.c). A mixed burst peaks at half the hydrogen-rich
/// Eddington luminosity, under a third of it. Its 2–10 keV light is 98% of it at a 2 keV
/// blackbody's share, 3.9023 × 10³¹ W, its optical light the disc's 2% at 25,000 K; none in radio.
pub const XRAY_BURST_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::new(5.1656e28),
    Watts::new(3.8181e28),
    Watts::new(1.7118e28),
    Watts::new(1.9602e28),
    Watts::new(8.9499e27),
    Watts::new(2.8108e27),
    Watts::new(1.0870e27),
    Watts::new(5.1087e26),
    Watts::new(1.6753e25),
    Watts::ZERO,
    Watts::new(3.9023e31),
];

/// The bound on a Be/X-ray binary's giant outburst's light in each band, W, in [`AlertBand::ALL`]'s
/// order: plan 11's curve ([`BeXOutburstCurve`], P11.T9.a) at its peak, a fifth of the Eddington
/// luminosity of a 2.2 M☉ neutron star, the heaviest the scratch sampler gave, for the Be star's
/// hydrogen mass fraction of 0.7, 6.5072 × 10³⁰ W in all; its 2–10 keV light at a 3 keV blackbody's
/// share, rounded up at the fifth figure (P12.T5.c). Its optical light is the Be star's disc's, the
/// system's, so none is the outburst's own.
pub const BEX_OUTBURST_BAND_PEAKS: [Watts; BAND_COUNT] = [
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::ZERO,
    Watts::new(2.9472e30),
];

/// The bounds of a row that bounds every photometric band alike by `photometric`, and X-rays by
/// `xray`.
#[must_use]
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
#[must_use]
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

/// The latest a core collapse's light in any band peaks after its explosion: 78 days, the IIP
/// template's plateau, which peaks 77.4 days in on the grid its peaks are taken on
/// ([`light`](super::light)), rounded up to the day past the grid's twentieth of a day. The IIb's
/// peaks at 17.7 days, the Ib's and Ic's at 14.55 and the IIL's at 9.3.
const CORE_COLLAPSE_RISE: Span = Span::from_seconds(78 * 86_400);

/// The latest a Type Ia's light in any band peaks after its explosion: 16 days, the normal
/// template's, which peaks 15.15 days in on the grid ([`light`](super::light)), rounded up as
/// [`CORE_COLLAPSE_RISE`] is. The Type Iax's peaks at 10.45 days.
const TYPE_IA_RISE: Span = Span::from_seconds(16 * 86_400);

/// The longest a luminous red nova lasts: 280 days, plan 11's 10-day rise and 200-day plateau, the
/// longest, which mergers of 4 M☉ and up hold, and its 15-day e-folding fade to 1% of its peak
/// (plan 11's `ACTIVE_SHARE`), 69.08 days: 279.08 days, rounded up to the day.
const LUMINOUS_RED_NOVA_DURATION: Span = Span::from_seconds(280 * 86_400);

/// The longest a kilonova lasts: 35 days, plan 11's power-law fade to 1% of its peak (plan 11's
/// `ACTIVE_SHARE`) from its peak a day after the merger for the most a merger ejects, 0.05 M☉,
/// 34.55 days, rounded up to the day. A kilonova of less ejecta peaks and fades sooner.
const KILONOVA_DURATION: Span = Span::from_seconds(35 * 86_400);

/// The longest a luminous red nova's light rises: plan 11's 10 days, at every mass.
const LUMINOUS_RED_NOVA_RISE: Span = Span::from_seconds(10 * 86_400);

/// The longest a kilonova's light rises: a day, plan 11's diffusion time for GW170817's ejecta,
/// the most a merger ejects; less ejecta peak sooner, as the mass's square root.
const KILONOVA_RISE: Span = Span::from_seconds(86_400);

/// The longest a giant eruption's light rises: a quarter of its duration ([`lbv::RISE_SHARE`]),
/// which is at most [`lbv::MAX_DURATION`], 20 Julian years, so five years.
const GIANT_ERUPTION_RISE: Span = Span::from_seconds(lbv::MAX_DURATION.seconds() / 4);

/// The longest the central black hole's flare rises: to the middle of its Gaussian (P09.T30.a,
/// after Neilsen et al. 2013, section 3), half its duration, which is at most
/// [`centre_flares::MAX_DURATION`], 8,175 s, so 4,087.5 s, rounded up to 4,088 s.
const CENTRE_FLARE_RISE: Span = Span::from_seconds(4_088);

/// The longest a star's flare rises: Davenport et al.'s (2014, equation 1) template peaks one
/// half width after its onset, and a flare lasts 21 half widths within [`flares::MAX_DURATION`],
/// one day (P06.T28.a), so at most 86,400 ÷ 21 = 4,114.3 s, rounded up to 4,115 s.
const STAR_FLARE_RISE: Span = Span::from_seconds(4_115);

/// The longest an FU Orionis outburst rises: 25 Julian years, the top of the FU Ori class's rises
/// ([`fu_orionis::FU_ORI_RISE_YEARS`]), above the V1647 Ori class's 10
/// ([`fu_orionis::V1647_ORI_RISE_YEARS`]).
const FU_ORIONIS_RISE: Span = Span::from_seconds(25 * 31_557_600);

/// The longest a nova lasts (module documentation): a 0.55 M☉ white dwarf's, the lightest an
/// accreting white dwarf host has, whose decline is the slowest, until its light stays under a
/// hundredth of its peak, 14,624,034.9 s (169.26 days, t₃ = 95.8 days), rounded up to the second.
const NOVA_DURATION: Span = Span::from_seconds(14_624_035);

/// The longest a nova's light rises: the same dwarf's tenth of t₃, 827,775.6 s (9.58 days),
/// rounded up to the second; a dwarf of 1.2 M☉ or more rises in the curve's least, a day.
const NOVA_RISE: Span = Span::from_seconds(827_776);

/// The longest a dwarf nova lasts (module documentation): its rise, a day, plus 2.6808 × 10³⁸ J,
/// the fall of 3.3 × 10⁻⁶ M☉ onto a 1.36 M☉ white dwarf, at its faintest maximum, 1.5398 × 10²⁷
/// W for a 1.3-hour orbit, plus the slowest decay's 3.2772 days an e-fold (a 10-hour orbit's) to a
/// hundredth: 5,516.7 years, rounded up to the Julian year. No one outburst has all three extremes,
/// so it bounds every one, loosely.
const DWARF_NOVA_DURATION: Span = Span::from_seconds(5_517 * 31_557_600);

/// The longest a dwarf nova's light rises: the curve's day.
const DWARF_NOVA_RISE: Span = Span::from_seconds(86_400);

/// The longest an X-ray transient's outburst lasts: the curve's fixed five-day rise and 30-day
/// e-folding decay to a hundredth of its peak, 12,368,601.1 s (143.16 days), rounded up to the
/// second.
const XRAY_TRANSIENT_DURATION: Span = Span::from_seconds(12_368_602);

/// The longest an X-ray transient's outburst rises: the curve's five days.
const XRAY_TRANSIENT_RISE: Span = Span::from_seconds(5 * 86_400);

/// The longest a Type I X-ray burst lasts: a mixed burst's 5 s rise and 30 s e-folding decay to a
/// hundredth of its peak, 143.16 s, rounded up to the second; a helium burst's lasts 24.03 s.
const XRAY_BURST_DURATION: Span = Span::from_seconds(144);

/// The longest a Type I X-ray burst rises: a mixed burst's 5 s.
const XRAY_BURST_RISE: Span = Span::from_seconds(5);

/// The longest a Be/X giant outburst lasts: the curve's fixed 10-day rise and 20-day e-folding
/// decay to a hundredth of its peak, 8,821,734.1 s (102.10 days), rounded up to the second.
const BEX_OUTBURST_DURATION: Span = Span::from_seconds(8_821_735);

/// The longest a Be/X giant outburst rises: the curve's 10 days.
const BEX_OUTBURST_RISE: Span = Span::from_seconds(10 * 86_400);

/// The longest a class's built kinds take from their onsets to their peaks, by which a scan of the
/// class that starts afresh walks back (P12.T8.b.1); `None` while none of its kinds is built.
///
/// A class whose kinds include one with no bound across events ([`LongestRise::PerEvent`]) has
/// none.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::{LongestRise, class_longest_rise};
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::time::Span;
///
/// // A luminous blue variable's giant eruption rises for up to five years.
/// let five_years = Span::from_julian_years(5).ok_or("in range")?;
/// assert_eq!(
///     class_longest_rise(ClassId::LUMINOUS_BLUE_VARIABLE),
///     Some(LongestRise::AtMost(five_years))
/// );
/// // The X-ray binaries' slowest riser is a Be/X giant outburst, over ten days.
/// let ten_days = Span::from_seconds(10 * 86_400);
/// assert_eq!(
///     class_longest_rise(ClassId::XRAY_BINARY),
///     Some(LongestRise::AtMost(ten_days))
/// );
/// // A kilonova peaks within a day.
/// let day = Span::from_seconds(86_400);
/// assert_eq!(
///     class_longest_rise(ClassId::NEUTRON_STAR_MERGER),
///     Some(LongestRise::AtMost(day))
/// );
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn class_longest_rise(class: ClassId) -> Option<LongestRise> {
    ROWS.iter()
        .filter(|row| row.classes.contains(class))
        .filter_map(TransientRow::longest_rise)
        .reduce(|a, b| match (a, b) {
            (LongestRise::AtMost(a), LongestRise::AtMost(b)) => LongestRise::AtMost(a.max(b)),
            (LongestRise::PerEvent, _) | (_, LongestRise::PerEvent) => LongestRise::PerEvent,
        })
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
            rise: LongestRise::AtMost(CORE_COLLAPSE_RISE),
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
            rise: LongestRise::AtMost(TYPE_IA_RISE),
            light_curve: Some(LightCurveSource::SupernovaTemplate),
        },
    },
    TransientRow {
        kind: TransientKind::LuminousRedNova,
        classes: ClassSet::of(ClassId::STELLAR_MERGER),
        route: Route::OneShot,
        source: RowSource::Built {
            bounds: band_by_band(LUMINOUS_RED_NOVA_BAND_PEAKS),
            duration: LongestDuration::AtMost(LUMINOUS_RED_NOVA_DURATION),
            rise: LongestRise::AtMost(LUMINOUS_RED_NOVA_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::Kilonova,
        classes: ClassSet::of(ClassId::NEUTRON_STAR_MERGER),
        route: Route::OneShot,
        source: RowSource::Built {
            bounds: band_by_band(KILONOVA_BAND_PEAKS),
            duration: LongestDuration::AtMost(KILONOVA_DURATION),
            rise: LongestRise::AtMost(KILONOVA_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::Nova,
        classes: ACCRETING_WHITE_DWARFS,
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: band_by_band(NOVA_BAND_PEAKS),
            duration: LongestDuration::AtMost(NOVA_DURATION),
            rise: LongestRise::AtMost(NOVA_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::DwarfNova,
        classes: ACCRETING_WHITE_DWARFS,
        route: Route::Local,
        source: RowSource::Built {
            bounds: band_by_band(DWARF_NOVA_BAND_PEAKS),
            duration: LongestDuration::AtMost(DWARF_NOVA_DURATION),
            rise: LongestRise::AtMost(DWARF_NOVA_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::XrayTransient,
        classes: ClassSet::of(ClassId::XRAY_BINARY),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: band_by_band(XRAY_TRANSIENT_BAND_PEAKS),
            duration: LongestDuration::AtMost(XRAY_TRANSIENT_DURATION),
            rise: LongestRise::AtMost(XRAY_TRANSIENT_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::XrayBurst,
        classes: ClassSet::of(ClassId::XRAY_BINARY),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: band_by_band(XRAY_BURST_BAND_PEAKS),
            duration: LongestDuration::AtMost(XRAY_BURST_DURATION),
            rise: LongestRise::AtMost(XRAY_BURST_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::BeXOutburst,
        classes: ClassSet::of(ClassId::XRAY_BINARY),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: band_by_band(BEX_OUTBURST_BAND_PEAKS),
            duration: LongestDuration::AtMost(BEX_OUTBURST_DURATION),
            rise: LongestRise::AtMost(BEX_OUTBURST_RISE),
            light_curve: Some(LightCurveSource::BinaryCurve),
        },
    },
    TransientRow {
        kind: TransientKind::GiantEruption,
        classes: ClassSet::of(ClassId::LUMINOUS_BLUE_VARIABLE),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: band_by_band(GIANT_ERUPTION_BAND_PEAKS),
            duration: LongestDuration::AtMost(lbv::MAX_DURATION),
            rise: LongestRise::AtMost(GIANT_ERUPTION_RISE),
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
            rise: LongestRise::PerEvent,
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
            rise: LongestRise::AtMost(CENTRE_FLARE_RISE),
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
            rise: LongestRise::AtMost(STAR_FLARE_RISE),
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
            rise: LongestRise::AtMost(Span::ZERO),
            light_curve: None,
        },
    },
    TransientRow {
        kind: TransientKind::MagnetarBurst,
        classes: ClassSet::of(ClassId::CORE_COLLAPSE),
        route: Route::Recurrent,
        source: RowSource::Built {
            bounds: bands_alike(None, Some(PeakBound::Watts(MAGNETAR_BURST_XRAY_PEAK))),
            duration: LongestDuration::AtMost(magnetar::MAX_BURST_DURATION),
            rise: LongestRise::AtMost(Span::ZERO),
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
            rise: LongestRise::AtMost(FU_ORIONIS_RISE),
            light_curve: Some(LightCurveSource::FuOrionisOutburst),
        },
    },
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::events::TimeWindow;
    use crate::galaxy::catalogue_classes::binary::{
        AccretingWdFast, AwdProcess, AwdSide, XrayBinaries,
    };
    use crate::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
    use crate::galaxy::catalogue_classes::type_ia::IaChannel;
    use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
    use crate::galaxy::features::centre::events::CentreVictims;
    use crate::galaxy::features::centre::events::DisruptionTail;
    use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
    use crate::galaxy::gas::ccm::Band;
    use crate::math;
    use crate::stellar::binary::events::{
        ACTIVE_SHARE, BinaryEvent, BinaryEventKind, BinarySystem, DONOR_HYDROGEN,
        DWARF_NOVA_CYCLES_DAYS, events_in,
    };
    use crate::stellar::binary::light_curves::{
        BeXOutburstCurve, BinaryLightCurve, BurstFuel, DwarfNovaCurve, GW170817_EJECTA_MASS,
        KilonovaCurve, LuminousRedNovaCurve, NovaCurve, XrayAccretor, XrayBurstCurve,
        XrayTransientCurve,
    };
    use crate::stellar::binary::scan::{MAX_NOVA_TRANSFER_RATE, scan_cell};
    use crate::stellar::remnant::SupernovaType;
    use crate::tables::binary::{
        AWD_FAST_SAMPLER, AWD_SLOW_SAMPLER, AwdMark, XRB_SAMPLER, XrbHostKind, XrbMark,
    };
    use crate::time::{SourceHorizon, UniverseTime};
    use crate::units::consts::SOLAR_LUMINOSITY_W;
    use crate::units::{Days, LightYears, SolarMasses, SolarRadii, WattsPerSquareMetre, Years};

    use super::super::light;
    use super::super::reach::{ROOT_CUBE_DIAGONAL, class_reach, kind_reach, reach};
    use super::super::{
        BandLimits, SensorHorizon, binary_luminosity, binary_peak, supernova_luminosity,
        supernova_peak,
    };
    use crate::stellar::binary::scan::NsbhOutflow;

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
            0
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

    /// The galaxy's own tails stay under the row's bounds through the source horizon, their
    /// efficiency P09.T30.c's from generator version 25. Each tail is sampled from its fallback
    /// time at quarter-octave steps to the horizon's end.
    #[test]
    fn the_galaxys_own_tails_stay_under_the_rows_bounds() {
        let row = TransientKind::TidalDisruption.row();
        let galaxy = milky_way_galaxy();
        let bound = |band| {
            row.peak_bound(band)
                .unwrap()
                .in_galaxy(galaxy)
                .unwrap()
                .value()
        };
        let (xray, light) = (
            bound(AlertBand::XRay),
            bound(AlertBand::Photometric(Band::V)),
        );
        let victims = CentreVictims::new(galaxy, milky_way_centre()).unwrap();
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

    /// The magnetar row bounds every short burst and giant flare, whole and in 2–10 keV. A burst's
    /// mean luminosity is at most the energy law's top over the duration law's floor, 2.512 × 10³⁶
    /// W; a giant flare's spike at the top of its law is the whole-output bound, which its mean
    /// meets within rounding; their 2–10 keV shares put both under the row's in-band bound, the
    /// giant flare's at it; and D covers both.
    #[test]
    fn the_magnetar_row_bounds_every_burst_and_giant_flare() {
        let row = TransientKind::MagnetarBurst.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::CORE_COLLAPSE));
        assert_eq!(
            row.peak_bound(AlertBand::XRay),
            Some(PeakBound::Watts(MAGNETAR_BURST_XRAY_PEAK))
        );
        assert_eq!(row.peak_bound(AlertBand::Photometric(Band::V)), None);
        let (bound, in_band) = (
            MAGNETAR_BURST_PEAK.value(),
            MAGNETAR_BURST_XRAY_PEAK.value(),
        );

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

        let (burst_band, giant_band) = (
            burst * super::super::MAGNETAR_BURST_XRAY_SHARE,
            giant * super::super::MAGNETAR_GIANT_FLARE_XRAY_SHARE,
        );
        assert!(
            burst_band <= in_band && giant_band <= in_band,
            "{giant_band:e} W"
        );
        assert!(giant_band / in_band > 0.999_999, "{giant_band:e} W");
        assert!(
            (burst_band / 2.67e35 - 1.0).abs() < 1e-2,
            "{burst_band:e} W"
        );

        assert_eq!(
            row.duration(),
            Some(LongestDuration::AtMost(magnetar::MAX_BURST_DURATION))
        );
        assert!(magnetar::GIANT_FLARE_SPIKE <= magnetar::MAX_BURST_DURATION);
        assert_eq!(row.light_curve(), Some(LightCurveSource::MagnetarBurst));
    }

    /// The giant-eruption row bounds the eruption's own light in every photometric band: the top of
    /// plan 06's peak law, 10^7.5 L☉, is under the bolometric bound and within its fifth figure,
    /// and its 5,000 K share of it under each band's and within its fifth figure; and D is the
    /// eruptions' longest.
    #[test]
    fn the_giant_eruption_row_bounds_its_own_light() {
        let row = TransientKind::GiantEruption.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::LUMINOUS_BLUE_VARIABLE));
        let top = math::exp10(lbv::LOG_PEAK_LUMINOSITY_SOLAR_LUMINOSITIES.1) * SOLAR_LUMINOSITY_W;
        let bound = GIANT_ERUPTION_PEAK.value();
        assert!(top <= bound && top / bound > 0.999_9, "{top:e} W");
        let shares = super::super::recurrent::GIANT_ERUPTION_SHARES;
        for (band, share) in AlertBand::ALL.into_iter().zip(shares) {
            let light = top * share;
            match row.peak_bound(band) {
                Some(PeakBound::Watts(watts)) => {
                    let watts = watts.value();
                    assert!(light <= watts && watts / light - 1.0 < 1e-4, "{band:?}");
                    assert!(watts < bound, "{band:?}");
                }
                other => {
                    assert_eq!(other, None, "{band:?}");
                    assert_same_bits(share, 0.0);
                }
            }
        }
        assert_eq!(row.peak_bound(AlertBand::XRay), None);
        assert_eq!(row.peak_bound(AlertBand::Photometric(Band::Radio)), None);
        assert_eq!(
            row.duration(),
            Some(LongestDuration::AtMost(lbv::MAX_DURATION))
        );
        assert_eq!(row.light_curve(), Some(LightCurveSource::GiantEruption));
    }

    /// A binary curve's light in `band` `dt` after its onset.
    fn binary_light(curve: &BinaryLightCurve, dt: Span, band: AlertBand) -> f64 {
        match band {
            AlertBand::Photometric(band) => curve.in_band(dt, band).value(),
            AlertBand::XRay => curve.xray_luminosity(dt).value(),
        }
    }

    /// The row of `kind` bounds every one of `curves` in every band at its peak, and its rise
    /// and its duration to [`ACTIVE_SHARE`]; `top`, the curve at the top of the kind's range,
    /// meets each band's bound within 10⁻⁴, and `longest` the duration within a second, and a
    /// band `top` gives no light has no bound.
    fn assert_row_bounds(
        kind: TransientKind,
        curves: &[BinaryLightCurve],
        top: &BinaryLightCurve,
        longest: Option<&BinaryLightCurve>,
    ) {
        let row = kind.row();
        let Some(LongestDuration::AtMost(duration)) = row.duration() else {
            panic!("{kind:?} lasts {:?}", row.duration());
        };
        let Some(LongestRise::AtMost(rise)) = row.longest_rise() else {
            panic!("{kind:?} rises {:?}", row.longest_rise());
        };
        for band in AlertBand::ALL {
            let at_top = binary_light(top, top.peak(), band);
            let Some(PeakBound::Watts(bound)) = row.peak_bound(band) else {
                assert_same_bits(at_top, 0.0);
                continue;
            };
            let bound = bound.value();
            assert!(
                at_top <= bound && bound / at_top - 1.0 < 1e-4,
                "{kind:?} in {band:?}"
            );
            for curve in curves {
                let light = binary_light(curve, curve.peak(), band);
                assert!(
                    light <= bound,
                    "{kind:?} in {band:?}: {light:e} over {bound:e}"
                );
            }
        }
        for curve in curves {
            let fades = curve.fades_below(ACTIVE_SHARE).unwrap();
            assert!(fades <= duration, "{kind:?}: {curve:?} lasts {fades}");
            assert!(
                curve.peak() <= rise,
                "{kind:?}: {curve:?} rises {}",
                curve.peak()
            );
        }
        if let Some(longest) = longest {
            let fades = longest.fades_below(ACTIVE_SHARE).unwrap().as_seconds_f64();
            assert!(
                duration.as_seconds_f64() - fades < 1.0,
                "{kind:?}: {fades} s"
            );
            let rises = longest.peak().as_seconds_f64();
            assert!(rise.as_seconds_f64() - rises < 1.0, "{kind:?}: {rises} s");
        }
        assert_eq!(row.light_curve(), Some(LightCurveSource::BinaryCurve));
    }

    /// Plan 11's rows bound their kinds' curves over the masses plan 11's samplers give their
    /// hosts (P11.T8.a–b), at the top of each range: the heaviest white dwarf for a nova, a 10-hour
    /// orbit for a dwarf nova, the heaviest black hole for a transient, a helium burst on the
    /// heaviest neutron star and its Be/X outburst; a nova lasts longest on the lightest white
    /// dwarf, a burst burning hydrogen too, and a dwarf nova under its sum of extremes.
    #[test]
    fn plan_11s_rows_bound_their_curves_over_the_samplers_ranges() {
        let [fast, slow] = [&AWD_FAST_SAMPLER, &AWD_SLOW_SAMPLER]
            .map(|sampler| sampler.mark(AwdMark::WhiteDwarfMass).limits());
        let white_dwarfs = (fast.0.min(slow.0), fast.1.max(slow.1));
        let neutron_stars = XRB_SAMPLER.mark(XrbMark::NeutronStarMass).limits();
        let black_holes = XRB_SAMPLER.mark(XrbMark::BlackHoleMass).limits();
        assert_eq!(
            (white_dwarfs, neutron_stars, black_holes),
            ((0.55, 1.36), (1.2, 2.2), (5.0, 15.0)),
            "the rows' documented ranges"
        );
        let over = |(lo, hi): (f64, f64), steps: u32| {
            (0..=steps)
                .map(move |i| SolarMasses::new(lo + (hi - lo) * f64::from(i) / f64::from(steps)))
        };
        let solar = |m: f64| SolarMasses::new(m);

        let nova = |m| BinaryLightCurve::from(NovaCurve::of(m).unwrap());
        let novae: Vec<_> = over(white_dwarfs, 81).map(nova).collect();
        let (lightest, heaviest) = (nova(solar(0.55)), nova(solar(1.36)));
        assert_row_bounds(TransientKind::Nova, &novae, &heaviest, Some(&lightest));

        let hours = |h: f64| Days::new(h / 24.0);
        let gathered_most = MAX_NOVA_TRANSFER_RATE.value() * DWARF_NOVA_CYCLES_DAYS.1 / 365.25;
        assert!((gathered_most - 3.3e-6).abs() < 1e-18, "{gathered_most:e}");
        let mut dwarf_novae = Vec::new();
        for m in over(white_dwarfs, 9) {
            for h in [0.5, 1.3, 2.0, 5.0, 10.0, 24.0, 8_766.0] {
                for gathered in [1e-12, 1e-10, 1e-8, 1e-6, gathered_most] {
                    let curve = DwarfNovaCurve::of(m, hours(h), solar(gathered)).unwrap();
                    dwarf_novae.push(BinaryLightCurve::from(curve));
                }
            }
        }
        let dwarf = |h: f64, gathered: f64| {
            BinaryLightCurve::from(
                DwarfNovaCurve::of(solar(1.36), hours(h), solar(gathered)).unwrap(),
            )
        };
        assert_row_bounds(
            TransientKind::DwarfNova,
            &dwarf_novae,
            &dwarf(10.0, 1e-10),
            None,
        );
        // The sum of extremes: the rise, the most gathered light at the faintest maximum (the curve
        // radiates the gathered fall exactly when it has a plateau), and the slowest e-fold.
        let faint = dwarf(1.3, gathered_most);
        let e_fold = |curve: &BinaryLightCurve| {
            curve
                .fades_below(1.0 / core::f64::consts::E)
                .unwrap()
                .as_seconds_f64()
                - curve.fades_below(1.0).unwrap().as_seconds_f64()
        };
        let slowest = e_fold(&dwarf(10.0, 1e-10));
        let sum = 86_400.0
            + faint.radiated_joules() / faint.peak_luminosity().value()
            + slowest * math::ln(1.0 / ACTIVE_SHARE);
        let duration = DWARF_NOVA_DURATION.as_seconds_f64();
        assert!(
            sum <= duration && duration - sum < 31_557_600.0,
            "{} yr",
            sum / 31_557_600.0
        );

        let transient = |accretor, m, gathered| {
            BinaryLightCurve::from(
                XrayTransientCurve::of(accretor, m, solar(gathered), DONOR_HYDROGEN).unwrap(),
            )
        };
        let mut transients = Vec::new();
        for (accretor, range) in [
            (XrayAccretor::NeutronStar, neutron_stars),
            (XrayAccretor::BlackHole, black_holes),
        ] {
            for m in over(range, 10) {
                for gathered in [1e-12, 1e-10, 1e-8, 1e-6, 1e-3, 1.0] {
                    transients.push(transient(accretor, m, gathered));
                }
            }
        }
        let held = transient(XrayAccretor::BlackHole, solar(15.0), 1.0);
        assert_row_bounds(
            TransientKind::XrayTransient,
            &transients,
            &held,
            Some(&held),
        );

        let burst = |m, fuel| BinaryLightCurve::from(XrayBurstCurve::of(m, fuel).unwrap());
        let bursts: Vec<_> = over(neutron_stars, 10)
            .flat_map(|m| [BurstFuel::Helium, BurstFuel::HydrogenHelium].map(|fuel| burst(m, fuel)))
            .collect();
        let (helium, mixed) = (
            burst(solar(2.2), BurstFuel::Helium),
            burst(solar(2.2), BurstFuel::HydrogenHelium),
        );
        assert_row_bounds(TransientKind::XrayBurst, &bursts, &helium, Some(&mixed));

        let outburst = |m| BinaryLightCurve::from(BeXOutburstCurve::of(m, DONOR_HYDROGEN).unwrap());
        let outbursts: Vec<_> = over(neutron_stars, 10).map(outburst).collect();
        let top = outburst(solar(2.2));
        assert_row_bounds(TransientKind::BeXOutburst, &outbursts, &top, Some(&top));
    }

    /// Each of `events` is under its row's bounds in every band at its peak, its rise and its
    /// duration, counted by kind into `counts`.
    fn assert_under_their_rows(
        galaxy: &Galaxy,
        events: &[BinaryEvent],
        counts: &mut std::collections::BTreeMap<TransientKind, usize>,
    ) {
        for event in events {
            let kind = match event.kind() {
                BinaryEventKind::Nova => TransientKind::Nova,
                BinaryEventKind::DwarfNova => TransientKind::DwarfNova,
                BinaryEventKind::XrayTransient => TransientKind::XrayTransient,
                BinaryEventKind::XrayBurst => TransientKind::XrayBurst,
                BinaryEventKind::BeXGiantOutburst => TransientKind::BeXOutburst,
                BinaryEventKind::LuminousRedNova | BinaryEventKind::Kilonova => {
                    panic!("no merger is listed here: {event:?}")
                }
            };
            *counts.entry(kind).or_insert(0) += 1;
            let row = kind.row();
            let curve = event.curve();
            for band in AlertBand::ALL {
                let light = binary_light(curve, curve.peak(), band);
                let bound = row
                    .peak_bound(band)
                    .and_then(|bound| bound.in_galaxy(galaxy))
                    .map_or(0.0, Watts::value);
                assert!(light <= bound, "{kind:?} in {band:?}: {event:?}");
            }
            assert!(
                Some(LongestDuration::AtMost(event.duration())) <= row.duration(),
                "{event:?}"
            );
            assert!(
                Some(LongestRise::AtMost(curve.peak())) <= row.longest_rise(),
                "{event:?}"
            );
        }
    }

    /// Plan 11's rows bound the events plan 11 lists for sampled hosts of the galaxy of seed 9
    /// (P11.T9.b): the novae of both accreting white dwarf classes along the solar circle over the
    /// source horizon, the dwarf novae of their entries over forty years, and the X-ray binaries'
    /// events about the centre, each kind over a span that holds some of its events. Each event's
    /// light at its peak in every band, its rise and its duration are under its row's.
    #[test]
    fn plan_11s_rows_bound_the_events_of_sampled_hosts() {
        let galaxy = Galaxy::new(crate::Seed::new(9));
        let window = |from: UniverseTime, years: i64| {
            let to = Span::from_julian_years(years)
                .and_then(|span| from.checked_add(span))
                .unwrap();
            TimeWindow::new(from, to).unwrap()
        };
        let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
        let mut counts = std::collections::BTreeMap::new();
        let mut events = Vec::new();
        for side in [AwdSide::Fast, AwdSide::Slow] {
            let process = AwdProcess::new(&galaxy, side);
            for x in -3..3 {
                let key = CatalogueCellKey::new(side.class(), [x, 50, 0]).unwrap();
                let mut hosts = Vec::new();
                scan_cell(&galaxy, &process, key, &mut hosts);
                for host in &hosts {
                    events.clear();
                    let system = BinarySystem::accreting_white_dwarf_scan(galaxy.seed(), host);
                    events_in(&system, horizon, &mut events);
                    assert_under_their_rows(&galaxy, &events, &mut counts);
                }
            }
        }
        let process = AccretingWdFast::new(&galaxy);
        for x in -3..3 {
            let key = CatalogueCellKey::new(ClassId::ACCRETING_WHITE_DWARF, [x, 50, 0]).unwrap();
            for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
                events.clear();
                let system = BinarySystem::accreting_white_dwarf(galaxy.seed(), entry.value());
                events_in(&system, window(UniverseTime::EPOCH, 40), &mut events);
                events.retain(|event| event.kind() == BinaryEventKind::DwarfNova);
                assert_under_their_rows(&galaxy, &events, &mut counts);
            }
        }
        let process = XrayBinaries::new(&galaxy);
        let day =
            TimeWindow::new(UniverseTime::EPOCH, UniverseTime::new(86_400, 0).unwrap()).unwrap();
        for x in -8..8 {
            for y in [-1, 0, 11, 12] {
                let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, 0]).unwrap();
                let mut hosts = Vec::new();
                scan_cell(&galaxy, &process, key, &mut hosts);
                for host in &hosts {
                    let span = match host.kind() {
                        XrbHostKind::NeutronStarPersistent => day,
                        XrbHostKind::BeX => window(UniverseTime::EPOCH, 1_000),
                        XrbHostKind::NeutronStarTransient
                        | XrbHostKind::BlackHoleTransient
                        | XrbHostKind::BlackHolePersistent
                        | XrbHostKind::Supergiant
                        | XrbHostKind::Ultracompact
                        | XrbHostKind::Symbiotic
                        | XrbHostKind::BlackHoleHighMass => window(UniverseTime::EPOCH, 20_000),
                    };
                    events.clear();
                    events_in(
                        &BinarySystem::xray_binary(galaxy.seed(), host),
                        span,
                        &mut events,
                    );
                    assert_under_their_rows(&galaxy, &events, &mut counts);
                }
            }
        }
        for kind in [
            TransientKind::Nova,
            TransientKind::DwarfNova,
            TransientKind::XrayTransient,
            TransientKind::XrayBurst,
            TransientKind::BeXOutburst,
        ] {
            assert!(
                counts.get(&kind).is_some_and(|&n| n > 0),
                "{kind:?}: {counts:?}"
            );
        }
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
                TransientKind::LuminousRedNova,
                TransientKind::Kilonova,
                TransientKind::Nova,
                TransientKind::XrayTransient,
                TransientKind::XrayBurst,
                TransientKind::BeXOutburst,
                TransientKind::GiantEruption,
                TransientKind::MagnetarBurst,
            ]
        );
    }

    /// X-ray bursts are walked over the X-ray binaries' class, as the magnetars over the core
    /// collapses', not found locally, and the class reaches as far as its transients, its
    /// brightest kind in X-rays; dwarf novae are local, so the accreting white dwarfs reach as far
    /// as their novae, however faint the limit.
    #[test]
    fn x_ray_bursts_are_walked_over_the_x_ray_binaries() {
        let row = TransientKind::XrayBurst.row();
        assert_eq!(row.route(), Route::Recurrent);
        assert_eq!(row.classes(), ClassSet::of(ClassId::XRAY_BINARY));
        assert_eq!(row.pending(), None);
        let w = WattsPerSquareMetre::new;
        let limits = BandLimits::new(w(1e-6), w(1e-6)).unwrap();
        let xray = SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap();
        let reached = class_reach(ClassId::XRAY_BINARY, &xray);
        assert_eq!(reached, kind_reach(TransientKind::XrayTransient, &xray));
        assert_eq!(reached, reach(XRAY_TRANSIENT_BAND_PEAKS[10], limits));
        assert!(kind_reach(TransientKind::XrayBurst, &xray) < reached);
        assert!(kind_reach(TransientKind::BeXOutburst, &xray) < reached);
        // 1.0282 × 10³² W at a microwatt per square metre: some 300 ly.
        assert!((290.0..310.0).contains(&reached.value()), "{reached:?}");
        let visual = SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap();
        assert_eq!(TransientKind::DwarfNova.row().route(), Route::Local);
        for class in [
            ClassId::ACCRETING_WHITE_DWARF,
            ClassId::ACCRETING_WHITE_DWARF_SLOW,
        ] {
            assert_eq!(
                class_reach(class, &visual),
                kind_reach(TransientKind::Nova, &visual)
            );
        }
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
        for (kind, curve) in [
            (TransientKind::LuminousRedNova, red_nova(2.0)),
            (TransientKind::LuminousRedNova, red_nova(300.0)),
            (
                TransientKind::Kilonova,
                kilonova(GW170817_EJECTA_MASS.value()),
            ),
            (TransientKind::Kilonova, kilonova(0.01)),
        ] {
            let mass = match curve {
                BinaryLightCurve::LuminousRedNova(red) => red.merger_mass(),
                BinaryLightCurve::Kilonova(kilonova) => kilonova.ejecta_mass(),
                BinaryLightCurve::Nova(_)
                | BinaryLightCurve::DwarfNova(_)
                | BinaryLightCurve::XrayTransient(_)
                | BinaryLightCurve::XrayBurst(_)
                | BinaryLightCurve::BeXOutburst(_) => unreachable!("a merger's curve"),
            };
            let day = Span::from_seconds(86_400);
            for band in AlertBand::ALL {
                w.f64(
                    &format!("{kind:?} of {} M☉ peak in {band:?}", mass.value()),
                    binary_peak(&curve, band).value(),
                );
                w.f64(
                    &format!("{kind:?} of {} M☉ at 1 d in {band:?}", mass.value()),
                    binary_luminosity(&curve, day, band).value(),
                );
            }
        }
        golden!("alerts/registry", w.as_str());
    }

    /// A luminous red nova's curve of a merger of `mass` M☉.
    fn red_nova(mass: f64) -> BinaryLightCurve {
        LuminousRedNovaCurve::of(SolarMasses::new(mass))
            .unwrap()
            .into()
    }

    /// A kilonova's curve of `mass` M☉ of ejecta.
    fn kilonova(mass: f64) -> BinaryLightCurve {
        KilonovaCurve::of(SolarMasses::new(mass)).unwrap().into()
    }

    /// The mergers' curves across what the classes draw: red novae of 0.1 to 300 M☉, the class's
    /// heaviest, and kilonovae of two neutron stars' ejecta and of black holes' across the class's
    /// pairs, every one that ejects anything.
    fn merger_curves() -> Vec<(TransientKind, BinaryLightCurve)> {
        let mut out = Vec::new();
        for k in 0..=64 {
            let mass = 0.1 * math::powf(3_000.0, f64::from(k) / 64.0);
            out.push((TransientKind::LuminousRedNova, red_nova(mass)));
        }
        out.push((
            TransientKind::Kilonova,
            kilonova(GW170817_EJECTA_MASS.value()),
        ));
        for i in 0..=40 {
            let hole = 3.0 * math::powf(20.0 / 3.0, f64::from(i) / 40.0);
            for j in 0..=40 {
                let star = 1.13 + 0.87 * f64::from(j) / 40.0;
                let outflow = NsbhOutflow::of(SolarMasses::new(hole), SolarMasses::new(star));
                if outflow.ejecta().value() > 0.0 {
                    out.push((TransientKind::Kilonova, kilonova(outflow.ejecta().value())));
                }
            }
        }
        out
    }

    /// The mergers' rows bound their curves (P12.T5.b): every red nova and kilonova the classes
    /// draw peaks at or under its row's bound in every band and is over within its row's D, and
    /// at the top of each kind's mass range the bound and D are its curve's peak, within 10⁻⁴,
    /// and its duration, within a day. The red novae last longest from 4 M☉ on, on their longest
    /// plateau.
    #[test]
    fn the_merger_rows_bound_their_curves() {
        let at_most = |kind: TransientKind| match kind.row().duration() {
            Some(LongestDuration::AtMost(span)) => span,
            other => panic!("{kind:?} lasts {other:?}"),
        };
        for (kind, curve) in merger_curves() {
            let row = kind.row();
            assert_eq!(row.light_curve(), Some(LightCurveSource::BinaryCurve));
            for band in AlertBand::ALL {
                let peak = binary_peak(&curve, band);
                match row.peak_bound(band) {
                    Some(PeakBound::Watts(bound)) => {
                        assert!(peak <= bound, "{kind:?} {band:?}: {curve:?}");
                    }
                    None => assert_same_bits(peak.value(), 0.0),
                    other => panic!("{kind:?} {band:?}: {other:?}"),
                }
            }
            assert!(light::binary_duration(&curve) <= at_most(kind), "{curve:?}");
        }
        let day = 86_400.0;
        for (kind, top) in [
            (TransientKind::LuminousRedNova, red_nova(300.0)),
            (
                TransientKind::Kilonova,
                kilonova(GW170817_EJECTA_MASS.value()),
            ),
        ] {
            for band in AlertBand::ALL {
                let peak = binary_peak(&top, band).value();
                if let Some(PeakBound::Watts(bound)) = kind.row().peak_bound(band) {
                    assert!(bound.value() / peak - 1.0 < 1e-4, "{kind:?} {band:?}");
                }
            }
            let lasts = light::binary_duration(&top).as_seconds_f64();
            assert!(at_most(kind).as_seconds_f64() - lasts < day, "{kind:?}");
        }
        assert_eq!(
            light::binary_duration(&red_nova(4.0)),
            light::binary_duration(&red_nova(300.0))
        );
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

    /// The day of the grid ([`light::peak_grid`]) on which each of `kinds`' templates peaks latest
    /// in a band it lights, over every band.
    fn latest_peak_day(kinds: &[SupernovaKind]) -> f64 {
        kinds
            .iter()
            .flat_map(|&kind| AlertBand::ALL.map(move |band| (kind, band)))
            .filter_map(|(kind, band)| {
                let (age, peak) = light::peak_grid()
                    .map(|age| (age, light::supernova_luminosity(kind, age, band).value()))
                    .fold((Years::ZERO, 0.0), |best, here| {
                        if here.1 > best.1 { here } else { best }
                    });
                (peak > 0.0).then_some(age.value() * 365.25)
            })
            .fold(0.0, f64::max)
    }

    /// No built kind's light peaks later after its onset than its row's rise bound (P12.T8.b.1),
    /// and each class's bound is its kinds' longest.
    #[test]
    fn no_kinds_light_peaks_later_after_its_onset_than_its_rise_bound() {
        let at_most = |kind: TransientKind| match kind.row().longest_rise() {
            Some(LongestRise::AtMost(span)) => span.as_seconds_f64(),
            other => panic!("{kind:?} rises {other:?}"),
        };
        let day = 86_400.0;
        let core_collapses = [
            SupernovaType::IIP,
            SupernovaType::IIL,
            SupernovaType::IIb,
            SupernovaType::Ib,
            SupernovaType::Ic,
        ]
        .map(SupernovaKind::CoreCollapse);
        let type_ia = IaChannel::ALL.map(SupernovaKind::TypeIa);
        // The grid's peak, and a twentieth of a day on for the true peak beside it.
        let latest = [latest_peak_day(&core_collapses), latest_peak_day(&type_ia)];
        for (kind, latest) in [TransientKind::CoreCollapse, TransientKind::TypeIa]
            .into_iter()
            .zip(latest)
        {
            assert!(
                (latest + 0.05) * day <= at_most(kind),
                "{kind:?}: {latest} d"
            );
        }
        // A flare rises one of its 21 half widths, all within a day.
        assert!(flares::MAX_DURATION.as_seconds_f64() / 21.0 <= at_most(TransientKind::Flare));
        // The centre's flare peaks half-way through.
        assert!(centre_flares::MAX_DURATION_SECONDS / 2.0 <= at_most(TransientKind::CentralFlare));
        let year = 31_557_600.0;
        let rises = [
            fu_orionis::FU_ORI_RISE_YEARS.1,
            fu_orionis::V1647_ORI_RISE_YEARS.1,
        ];
        for rise in rises {
            assert!(
                rise * year <= at_most(TransientKind::FuOrionisOutburst),
                "{rise} yr"
            );
        }
        let eruption = lbv::MAX_DURATION.as_seconds_f64() * lbv::RISE_SHARE;
        assert!(eruption <= at_most(TransientKind::GiantEruption));
        // A merger's curve peaks at its rise: the red novae's ten days, and a kilonova's day at the
        // most a merger ejects.
        for (kind, curve) in merger_curves() {
            assert!(
                curve.peak().as_seconds_f64() <= at_most(kind),
                "{kind:?} {curve:?}"
            );
        }
        // A top hat and a glitch are at their light at once.
        assert_eq!(at_most(TransientKind::MagnetarBurst), 0.0);
        assert_eq!(at_most(TransientKind::Glitch), 0.0);
        assert_eq!(
            TransientKind::TidalDisruption.row().longest_rise(),
            Some(LongestRise::PerEvent)
        );
        for kind in TransientKind::ALL {
            assert_eq!(
                kind.row().longest_rise().is_none(),
                kind.row().pending().is_some(),
                "{kind:?}"
            );
        }
        let rise = |kind: TransientKind| kind.row().longest_rise();
        assert_eq!(
            class_longest_rise(ClassId::CORE_COLLAPSE),
            rise(TransientKind::CoreCollapse)
        );
        assert_eq!(
            class_longest_rise(ClassId::TYPE_IA),
            rise(TransientKind::TypeIa)
        );
        assert_eq!(
            class_longest_rise(ClassId::TIDAL_DISRUPTION_VICTIM),
            Some(LongestRise::PerEvent)
        );
        assert_eq!(
            class_longest_rise(ClassId::XRAY_BINARY),
            rise(TransientKind::BeXOutburst)
        );
        assert_eq!(
            class_longest_rise(ClassId::ACCRETING_WHITE_DWARF),
            rise(TransientKind::Nova)
        );
        for (class, kind) in [
            (ClassId::STELLAR_MERGER, TransientKind::LuminousRedNova),
            (ClassId::NEUTRON_STAR_MERGER, TransientKind::Kilonova),
        ] {
            assert_eq!(class_longest_rise(class), rise(kind), "{class}");
        }
    }
}
