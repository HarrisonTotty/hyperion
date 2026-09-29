//! The fates of stars, tabulated over initial mass, metallicity and Reimers η: how and when each
//! dies and what it leaves (plan 06, P06.T38.c–d; rulings 89, 90 and 99).
//!
//! A dead star's state is a closed form of four numbers, its remnant's phase, mass, formation age
//! and (for a white dwarf) cooling origin ([`Track`]'s remnant segment). Those depend on the
//! track, which depends on the initial mass, the metallicity the formulae see (`z_fit`) and
//! Reimers η alone, apart from the companion-stripped mark in its electron-capture window and the
//! remnant draws that an iron core's collapse reads afterwards. So a table of the **fate** over
//! (m₀, Z, η) gives a dead star's state at any time by the track's own closed form, without the
//! track: the one artefact of ruling 89.3, ruling 77.3's fitted lifetime table with the fate's
//! columns. It is a table over mass, metallicity and a draw, never over age, so it keeps the
//! brainstorm's "no tables binned by age".
//!
//! - [`FateNode::of`] is one node: one [`Track::full`], at the median draws but η. `hyperion-fit`'s
//!   `stellar_fates` task evaluates it over the grid and validates it at the cells' centres, and
//!   writes `tables::stellar_fates_low`, `_mid` and `_high`, a file for each panel of mass.
//! - [`FittedFates`] reads the table: a tricubic Lagrange interpolant in log m₀, \[Fe/H\] of `z_fit`
//!   and η, and each cell's validated error bounds. It answers `None` wherever the table cannot:
//!   outside its domain, across a change of route, or in a cell whose validation failed.
//!
//! What the table gives is within its stated error of the track, not the track bit for bit. The
//! range brief ([`brief`](crate::stellar::brief)) reads it for stars dead through the whole clock
//! window, and guards the discrete outputs: it evaluates the brief at the corners of the error
//! box and goes to the exact track wherever they could differ (ruling 89.2 as amended by ruling
//! 90.4).
//!
//! [`TrackFates`] (P06.T30.b), the galaxy's mean-mass quadrature's lifetimes and remnants at one
//! metallicity, reads [`FittedFates`] at η's median where it answers.

use crate::galaxy::fates::{ProvisionalFates, StellarFates};
use crate::galaxy::imf::MASS_BAND_EDGES;
use crate::math;
use crate::rng::Mark;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::{RemnantDraws, core_collapse, electron_capture_remnant};
use crate::stellar::remnant::white_dwarf::WhiteDwarfCore;
use crate::stellar::sse::{
    MAX_INITIAL_MASS, RemnantModel, Track, bridged_origin, is_companion_stripped,
};
use crate::stellar::{Composition, Phase};
use crate::tables::{
    stellar_fates_high as high, stellar_fates_low as low, stellar_fates_mid as mid,
};
use crate::units::{Dex, HeliumExcess, Megayears, SolarMasses, Years};

/// How a star dies, as the fate table records it: the route its death takes, before the remnant
/// draws decide an iron core's remnant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FateRoute {
    /// The envelope is lost and a helium white dwarf is left.
    HeliumWhiteDwarf,
    /// The envelope is lost and a carbon–oxygen white dwarf is left.
    CarbonOxygenWhiteDwarf,
    /// The envelope is lost and an oxygen–neon white dwarf is left.
    OxygenNeonWhiteDwarf,
    /// An oxygen–neon core collapses by electron capture into Mandel and Müller's 1.26 M☉ neutron
    /// star.
    ElectronCapture,
    /// An iron core collapses; the remnant draws decide its remnant from the progenitor's cores.
    IronCore,
    /// Nothing is left (a thermonuclear disruption).
    NoRemnant,
    /// The envelope is lost on the AGB and the star crosses the post-AGB bridge to a
    /// carbon–oxygen white dwarf, which fades on Miller Bertolami's shape before its cooling law
    /// takes over (ruling 127.1).
    BridgedCarbonOxygenWhiteDwarf,
    /// As [`FateRoute::BridgedCarbonOxygenWhiteDwarf`], to an oxygen–neon white dwarf.
    BridgedOxygenNeonWhiteDwarf,
}

impl FateRoute {
    /// The route's code in the table: 0 to 7, in declaration order.
    #[must_use]
    pub const fn code(self) -> f64 {
        match self {
            Self::HeliumWhiteDwarf => 0.0,
            Self::CarbonOxygenWhiteDwarf => 1.0,
            Self::OxygenNeonWhiteDwarf => 2.0,
            Self::ElectronCapture => 3.0,
            Self::IronCore => 4.0,
            Self::NoRemnant => 5.0,
            Self::BridgedCarbonOxygenWhiteDwarf => 6.0,
            Self::BridgedOxygenNeonWhiteDwarf => 7.0,
        }
    }

    /// The route of `code`, or `None` for a value that is no route's code.
    #[must_use]
    pub fn from_code(code: f64) -> Option<Self> {
        [
            Self::HeliumWhiteDwarf,
            Self::CarbonOxygenWhiteDwarf,
            Self::OxygenNeonWhiteDwarf,
            Self::ElectronCapture,
            Self::IronCore,
            Self::NoRemnant,
            Self::BridgedCarbonOxygenWhiteDwarf,
            Self::BridgedOxygenNeonWhiteDwarf,
        ]
        .into_iter()
        .find(|route| route.code().total_cmp(&code).is_eq())
    }

    /// The white dwarf phase the route leaves, if it leaves one.
    #[must_use]
    pub const fn white_dwarf_phase(self) -> Option<Phase> {
        match self {
            Self::HeliumWhiteDwarf => Some(Phase::HeliumWhiteDwarf),
            Self::CarbonOxygenWhiteDwarf | Self::BridgedCarbonOxygenWhiteDwarf => {
                Some(Phase::CarbonOxygenWhiteDwarf)
            }
            Self::OxygenNeonWhiteDwarf | Self::BridgedOxygenNeonWhiteDwarf => {
                Some(Phase::OxygenNeonWhiteDwarf)
            }
            Self::ElectronCapture | Self::IronCore | Self::NoRemnant => None,
        }
    }
}

/// One node of the fate table: the fate of a star of one initial mass, metallicity and η, with
/// every other draw at its median and the companion-stripped mark unset.
///
/// The two value columns depend on the route:
///
/// | Route | `a` | `b` |
/// | --- | --- | --- |
/// | a white dwarf | its mass, M☉ | its cooling origin, Myr |
/// | a white dwarf off the post-AGB bridge | its mass, M☉ | log₁₀ L at its knee, L☉ |
/// | an iron core | the progenitor's carbon–oxygen core, M☉ | its helium core, M☉ |
/// | electron capture, no remnant | 0 | 0 |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FateNode {
    /// How the star dies.
    pub route: FateRoute,
    /// log₁₀ of its death age, Julian years since the onset of collapse.
    pub log_death_age: f64,
    /// The first value column.
    pub a: f64,
    /// The second value column.
    pub b: f64,
}

impl FateNode {
    /// The fate of a star of initial mass `m0`, composition `comp` and Reimers η draw `eta`: one
    /// [`Track::full`] at the median draws but η, read at its death.
    ///
    /// `m0` above 150 M☉, the tracks' top since P06.T14, is evolved as 150 M☉, as `StarModel` does.
    ///
    /// # Panics
    ///
    /// In debug builds, if `m0` is outside the track's 0.1–150 M☉ after that clamp.
    #[must_use]
    pub fn of(m0: SolarMasses, comp: &Composition, eta: StandardNormal) -> Self {
        let m0 = if m0 > MAX_INITIAL_MASS {
            MAX_INITIAL_MASS
        } else {
            m0
        };
        let track = Track::full(m0, comp, &node_draws(eta));
        let fate = track
            .fate_record()
            .expect("a full track reaches the star's death");
        let remnant = track
            .remnant_model()
            .expect("a full track ends with its remnant");
        let log_death_age = math::log10(fate.death.age().value());
        if fate.iron_core.is_some() {
            let progenitor = fate.death.progenitor();
            return Self {
                route: FateRoute::IronCore,
                log_death_age,
                a: progenitor.co_core_mass().value(),
                b: progenitor.helium_core_mass().value(),
            };
        }
        let (route, a, b) = match (remnant.phase, remnant.knee) {
            (Phase::CarbonOxygenWhiteDwarf, Some(knee)) => (
                FateRoute::BridgedCarbonOxygenWhiteDwarf,
                remnant.mass.value(),
                knee,
            ),
            (Phase::OxygenNeonWhiteDwarf, Some(knee)) => (
                FateRoute::BridgedOxygenNeonWhiteDwarf,
                remnant.mass.value(),
                knee,
            ),
            (Phase::HeliumWhiteDwarf, _) => (
                FateRoute::HeliumWhiteDwarf,
                remnant.mass.value(),
                remnant.origin.value(),
            ),
            (Phase::CarbonOxygenWhiteDwarf, None) => (
                FateRoute::CarbonOxygenWhiteDwarf,
                remnant.mass.value(),
                remnant.origin.value(),
            ),
            (Phase::OxygenNeonWhiteDwarf, None) => (
                FateRoute::OxygenNeonWhiteDwarf,
                remnant.mass.value(),
                remnant.origin.value(),
            ),
            (Phase::NeutronStar, _) => (FateRoute::ElectronCapture, 0.0, 0.0),
            _ => (FateRoute::NoRemnant, 0.0, 0.0),
        };
        Self {
            route,
            log_death_age,
            a,
            b,
        }
    }

    /// The node as the table stores it: (route code, log₁₀ death age, a, b).
    #[must_use]
    pub const fn to_row(self) -> (f64, f64, f64, f64) {
        (self.route.code(), self.log_death_age, self.a, self.b)
    }
}

/// The draws of a node: η, the companion-stripped mark
/// [`NEVER_STRIPPED`](crate::galaxy::displaced::binarity::NEVER_STRIPPED), since the table holds
/// unstripped stars, and every other draw at its median.
#[must_use]
pub fn node_draws(eta: StandardNormal) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta,
        stripped: crate::galaxy::displaced::binarity::NEVER_STRIPPED,
        ..StarDrawsParts::MEDIAN
    })
}

/// The masses, M☉, between which the companion-stripped mark can move a star's fate: the
/// stripped electron-capture window [`m_cc` − 1 M☉, `m_cc`) in the early AGB's mass (ruling 93)
/// over every metallicity, 6.7–8.5 M☉ of `m_cc`, widened for the main sequence's wind. A stripped
/// star inside is outside the table, which holds unstripped stars. A test checks that outside it
/// the mark moves no fate.
pub const STRIPPED_WINDOW: (f64, f64) = (5.5, 11.0);

/// The largest |η| a panel of one η node answers for: the η of its validation points, beyond
/// which the survey's independence of η is not checked (P06.T38.d's slow test found a 63 M☉ star
/// at η −2.8 past its bounds).
pub const SINGLE_NODE_ETA_LIMIT: f64 = 2.4;

/// The least bound a column takes, relative: the interpolation's own floor, below which a cell's
/// few validation points cannot vouch for it (P06.T38.d's slow test found iron cores' cores
/// three times past bounds of 10⁻⁵).
pub const BOUND_FLOOR: f64 = 1e-4;

/// The floors of the value columns' relative errors: a white dwarf's mass or a core's is compared
/// relative to at least 10⁻³ M☉, a cooling origin relative to at least 1 Myr.
pub const VALUE_FLOORS: [f64; 2] = [1e-3, 1.0];

/// One panel of the fate table: nodes evenly spaced in log₁₀ m₀ at every \[Fe/H\] node and at
/// each of its η nodes, and a bound per cell of mass and metallicity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FatePanel<'t> {
    /// log₁₀ of the lowest node's mass, M☉.
    pub log_mass_start: f64,
    /// The spacing of the nodes in log₁₀ m₀.
    pub log_mass_step: f64,
    /// The η nodes, increasing; one node for a panel whose fates do not depend on η.
    pub etas: &'t [f64],
    /// The nodes, `(route code, log₁₀ death age, a, b)`, mass fastest, then η, then \[Fe/H\].
    pub nodes: &'t [(f64, f64, f64, f64)],
    /// Each cell's validated relative bounds on the death age, `a` and `b`, mass cell fastest,
    /// then \[Fe/H\] cell; a negative bound marks a cell the table does not answer for.
    pub bounds: &'t [(f64, f64, f64)],
}

impl FatePanel<'_> {
    /// The number of mass nodes, given `metallicities` \[Fe/H\] nodes.
    #[must_use]
    pub fn masses(&self, metallicities: usize) -> usize {
        self.nodes
            .len()
            .checked_div(metallicities * self.etas.len())
            .unwrap_or(0)
    }
}

/// The fate a table gives a star, with its validated relative error bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittedFate {
    /// How the star dies.
    pub route: FateRoute,
    /// Its death age, Julian years since the onset of collapse.
    pub death_age: Years,
    /// The first value column (see [`FateNode`]).
    pub a: f64,
    /// The second value column.
    pub b: f64,
    /// The relative bounds on the death age, `a` and `b` (the value columns relative to at least
    /// [`VALUE_FLOORS`]).
    pub bounds: (f64, f64, f64),
}

impl FittedFate {
    /// The remnant the fate leaves for a star of remnant draws `draws`, with the columns moved by
    /// `shift` times their bounds, each −1 to 1 (the guard's corners). Nothing left is a remnant
    /// of no mass in the phase `NoRemnant`, as a full track's last segment holds it.
    #[must_use]
    pub(crate) fn remnant(&self, draws: RemnantDraws, shift: (f64, f64, f64)) -> RemnantModel {
        let birth = self.death_age.value() * (1.0 + shift.0 * self.bounds.0);
        let a = self.a + shift.1 * self.bounds.1 * self.a.abs().max(VALUE_FLOORS[0]);
        let b = self.b + shift.2 * self.bounds.2 * self.b.abs().max(VALUE_FLOORS[1]);
        if let Some(phase) = self.route.white_dwarf_phase() {
            let bridged = matches!(
                self.route,
                FateRoute::BridgedCarbonOxygenWhiteDwarf | FateRoute::BridgedOxygenNeonWhiteDwarf
            );
            let mass = SolarMasses::new(a);
            // A bridged dwarf's origin puts 10 L☉ at its fade's end (ruling 127.1); the Montreal
            // law does not read the metallicity, which is only passed for the call's shape.
            let (origin, knee) = if bridged {
                let core = WhiteDwarfCore::of(phase).expect("a white dwarf phase has a core");
                let origin = bridged_origin(RemnantRecipe::MandelMuller2020, core, mass, Z_SOLAR);
                (origin, Some(b))
            } else {
                (Megayears::new(b.max(0.0)), None)
            };
            return RemnantModel {
                phase,
                mass,
                birth,
                origin,
                knee,
            };
        }
        let remnant = match self.route {
            FateRoute::ElectronCapture => electron_capture_remnant(),
            FateRoute::IronCore => {
                let helium = b.max(0.0);
                core_collapse(
                    SolarMasses::new(a.clamp(0.0, helium)),
                    SolarMasses::new(helium),
                    draws,
                )
                .remnant()
            }
            FateRoute::NoRemnant
            | FateRoute::HeliumWhiteDwarf
            | FateRoute::CarbonOxygenWhiteDwarf
            | FateRoute::OxygenNeonWhiteDwarf
            | FateRoute::BridgedCarbonOxygenWhiteDwarf
            | FateRoute::BridgedOxygenNeonWhiteDwarf => {
                crate::stellar::remnant::CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO)
            }
        };
        let phase = match remnant.kind() {
            RemnantKind::BlackHole => Phase::BlackHole,
            RemnantKind::NeutronStar | RemnantKind::WhiteDwarf => Phase::NeutronStar,
            RemnantKind::None => Phase::NoRemnant,
        };
        RemnantModel {
            phase,
            mass: remnant.mass(),
            birth,
            origin: Megayears::ZERO,
            knee: None,
        }
    }
}

/// The fate table's reader: the fates of unstripped stars over initial mass, the metallicity the
/// formulae see and Reimers η (plan 06, P06.T38.d).
///
/// Three panels, one file each, share the \[Fe/H\] nodes: the two below the second split mass,
/// whose white dwarfs depend on η through the giant branches' winds, have η nodes; the one above,
/// whose fates the survey found independent of η (P06.T38.a), has one η node, validated at
/// other η. Each panel's nodes run a little past its splits, so that its stencils stay inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittedFates<'t> {
    /// The \[Fe/H\] nodes, log₁₀(`z_fit` ÷ 0.02), increasing.
    pub fe_h: &'t [f64],
    /// The panels, by increasing mass.
    pub panels: [FatePanel<'t>; 3],
    /// The masses, M☉, at which the second and third panels take over.
    pub splits: [f64; 2],
}

impl FittedFates<'static> {
    /// The generator's table: [`tables::stellar_fates_low`](crate::tables::stellar_fates_low),
    /// [`tables::stellar_fates_mid`](crate::tables::stellar_fates_mid) and
    /// [`tables::stellar_fates_high`](crate::tables::stellar_fates_high).
    #[must_use]
    pub const fn generator() -> Self {
        Self {
            fe_h: &low::FE_H_NODES,
            panels: [
                FatePanel {
                    log_mass_start: low::LOG_MASS_START,
                    log_mass_step: low::LOG_MASS_STEP,
                    etas: &low::ETA_NODES,
                    nodes: &low::NODES,
                    bounds: &low::BOUNDS,
                },
                FatePanel {
                    log_mass_start: mid::LOG_MASS_START,
                    log_mass_step: mid::LOG_MASS_STEP,
                    etas: &mid::ETA_NODES,
                    nodes: &mid::NODES,
                    bounds: &mid::BOUNDS,
                },
                FatePanel {
                    log_mass_start: high::LOG_MASS_START,
                    log_mass_step: high::LOG_MASS_STEP,
                    etas: &high::ETA_NODES,
                    nodes: &high::NODES,
                    bounds: &high::BOUNDS,
                },
            ],
            splits: [low::SPLIT_MASS, mid::SPLIT_MASS],
        }
    }

    /// Whether the three files share their \[Fe/H\] nodes, as the reader assumes.
    #[must_use]
    pub fn metallicities_agree() -> bool {
        let same = |a: &[f64], b: &[f64]| {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.total_cmp(y).is_eq())
        };
        same(&low::FE_H_NODES, &mid::FE_H_NODES) && same(&low::FE_H_NODES, &high::FE_H_NODES)
    }
}

/// Where a star falls in a panel: its cells of mass and metallicity, and the number of mass
/// nodes.
#[derive(Debug, Clone, Copy)]
struct Stencil {
    mass_cell: usize,
    fe_h_cell: usize,
    masses: usize,
}

/// One axis of a lookup: `x`, the cell holding it (nodes `cell` and `cell + 1`), and the nodes.
#[derive(Debug, Clone, Copy)]
struct Axis<F> {
    x: f64,
    n: usize,
    cell: usize,
    node: F,
}

impl<F: Fn(usize) -> f64> Axis<F> {
    /// `x` on the axis of `n` nodes whose k-th is `node(k)`, or `None` outside them; one node
    /// holds every `x`.
    fn locate(x: f64, n: usize, node: F) -> Option<Self> {
        if n == 0 || !x.is_finite() {
            return None;
        }
        if n > 1 && (x < node(0) || x > node(n - 1)) {
            return None;
        }
        let mut cell = 0;
        while cell + 2 < n && x >= node(cell + 1) {
            cell += 1;
        }
        Some(Self { x, n, cell, node })
    }

    /// The stencils of `order` points that hold the cell, as (first node, points), in order of
    /// preference: centred, then shifted down, then up; all the nodes if there are fewer.
    fn firsts(&self, order: usize) -> Vec<(usize, usize)> {
        let points = order.min(self.n);
        let last = self.n - points;
        let centre = self.cell.saturating_sub((points - 1) / 2).min(last);
        let mut out = Vec::with_capacity(3);
        for first in [centre, centre.saturating_sub(1), (centre + 1).min(last)] {
            let holds = first <= self.cell && (points == 1 || self.cell < first + points - 1);
            if holds && !out.iter().any(|&(f, _)| f == first) {
                out.push((first, points));
            }
        }
        out
    }

    /// The Lagrange weights at `x` of the stencil of `points` nodes from `first`.
    fn weights(&self, first: usize, points: usize) -> [f64; 4] {
        let mut weights = [0.0; 4];
        if points == 1 {
            weights[0] = 1.0;
            return weights;
        }
        for (i, w) in weights.iter_mut().enumerate().take(points) {
            let xi = (self.node)(first + i);
            let mut v = 1.0;
            for j in 0..points {
                if j != i {
                    let xj = (self.node)(first + j);
                    v *= (self.x - xj) / (xi - xj);
                }
            }
            *w = v;
        }
        weights
    }
}

/// The orders of the stencils tried, highest first: cubic, then quadratic, then linear, taken
/// where a higher order's stencil crosses a change of route.
const ORDERS: [usize; 3] = [4, 3, 2];

impl<'t> FittedFates<'t> {
    /// The fate of a star of initial mass `m0`, composition `comp` and Reimers η `eta`, unstripped,
    /// with its bounds, or `None` where the table does not answer: outside 0.741–150 M☉ (above
    /// 150 M☉ it reads 150, as the tracks do), outside its η nodes, for a composition with a helium
    /// excess, across a change of route, or in a cell whose validation failed.
    #[must_use]
    pub fn fate_fitted(
        &self,
        m0: SolarMasses,
        comp: &Composition,
        eta: StandardNormal,
    ) -> Option<FittedFate> {
        let (fate, stencil, panel) = self.interpolate(m0, comp, eta)?;
        let cell = stencil.fe_h_cell * (stencil.masses - 1) + stencil.mass_cell;
        let bounds = *panel.bounds.get(cell)?;
        if bounds.0 < 0.0 || bounds.1 < 0.0 || bounds.2 < 0.0 {
            return None;
        }
        Some(FittedFate { bounds, ..fate })
    }

    /// The death age the table gives, for plan 08's placement and P06.T30 (ruling 77.3), under
    /// [`FittedFates::fate_fitted`]'s conditions.
    #[must_use]
    pub fn lifetime_fitted(
        &self,
        m0: SolarMasses,
        comp: &Composition,
        eta: StandardNormal,
    ) -> Option<Years> {
        self.fate_fitted(m0, comp, eta).map(|fate| fate.death_age)
    }

    /// The certified bracket on the death age: the fitted age less and more its bound.
    #[must_use]
    pub fn lifetime_bracket(
        &self,
        m0: SolarMasses,
        comp: &Composition,
        eta: StandardNormal,
    ) -> Option<(Years, Years)> {
        self.fate_fitted(m0, comp, eta).map(|fate| {
            let t = fate.death_age.value();
            (
                Years::new(t * (1.0 - fate.bounds.0)),
                Years::new(t * (1.0 + fate.bounds.0)),
            )
        })
    }

    /// The interpolated fate, ignoring the cells' bounds but not a change of route in the stencil,
    /// with no bounds: what `hyperion-fit` validates the table by.
    #[must_use]
    pub fn fate_unbounded(
        &self,
        m0: SolarMasses,
        comp: &Composition,
        eta: StandardNormal,
    ) -> Option<FittedFate> {
        self.interpolate(m0, comp, eta).map(|(fate, _, _)| fate)
    }

    /// The panel `m0` falls in.
    #[must_use]
    fn panel(&self, m0: f64) -> &FatePanel<'t> {
        let k = self.splits.iter().filter(|&&split| m0 >= split).count();
        &self.panels[k]
    }

    /// The interpolated fate, its stencil and its panel.
    fn interpolate(
        &self,
        m0: SolarMasses,
        comp: &Composition,
        eta: StandardNormal,
    ) -> Option<(FittedFate, Stencil, &FatePanel<'t>)> {
        if comp.helium_excess().value() != 0.0 {
            return None;
        }
        let m = m0.value().min(MAX_INITIAL_MASS.value());
        if !m.is_finite() || m <= 0.0 {
            return None;
        }
        let panel = self.panel(m);
        let masses = panel.masses(self.fe_h.len());
        let mass = Axis::locate(math::log10(m), masses, |k| {
            panel.log_mass_start + panel.log_mass_step * index(k)
        })?;
        let x =
            math::log10(comp.z_fit().value() / 0.02).clamp(*self.fe_h.first()?, *self.fe_h.last()?);
        let fe_h = Axis::locate(x, self.fe_h.len(), |k| self.fe_h[k])?;
        // A panel of one η node answers only as far as its validation reached.
        if panel.etas.len() == 1 && eta.value().abs() > SINGLE_NODE_ETA_LIMIT {
            return None;
        }
        let eta_axis = Axis::locate(eta.value(), panel.etas.len(), |k| panel.etas[k])?;
        let n_eta = panel.etas.len();
        let node = |z: usize, e: usize, m: usize| panel.nodes[(z * n_eta + e) * masses + m];
        // The first stencil, highest order first, whose every node dies by one route.
        let same_route =
            |(zf, zp): (usize, usize), (ef, ep): (usize, usize), (mf, mp): (usize, usize)| {
                let route = node(zf, ef, mf).0;
                (0..zp).all(|z| {
                    (0..ep).all(|e| {
                        (0..mp).all(|k| node(zf + z, ef + e, mf + k).0.total_cmp(&route).is_eq())
                    })
                })
            };
        let (zs, es, ms) = ORDERS.iter().find_map(|&order| {
            fe_h.firsts(order).into_iter().find_map(|zs| {
                eta_axis.firsts(order).into_iter().find_map(|es| {
                    mass.firsts(order)
                        .into_iter()
                        .find(|&ms| same_route(zs, es, ms))
                        .map(|ms| (zs, es, ms))
                })
            })
        })?;
        let (wz, we, wm) = (
            fe_h.weights(zs.0, zs.1),
            eta_axis.weights(es.0, es.1),
            mass.weights(ms.0, ms.1),
        );
        let first = node(zs.0, es.0, ms.0);
        let (mut log_t, mut col_a, mut col_b) = (0.0, 0.0, 0.0);
        for (z, &w_z) in wz.iter().enumerate().take(zs.1) {
            for (e, &w_e) in we.iter().enumerate().take(es.1) {
                let w_ze = w_z * w_e;
                for (k, &w_m) in wm.iter().enumerate().take(ms.1) {
                    let n = node(zs.0 + z, es.0 + e, ms.0 + k);
                    let w = w_ze * w_m;
                    log_t += w * n.1;
                    col_a += w * n.2;
                    col_b += w * n.3;
                }
            }
        }
        let fate = FittedFate {
            route: FateRoute::from_code(first.0)?,
            death_age: Years::new(math::exp10(log_t)),
            a: col_a,
            b: col_b,
            bounds: (0.0, 0.0, 0.0),
        };
        let stencil = Stencil {
            mass_cell: mass.cell,
            fe_h_cell: fe_h.cell,
            masses,
        };
        Some((fate, stencil, panel))
    }
}

/// `k` as an `f64`, exact for any node index.
#[must_use]
fn index(k: usize) -> f64 {
    f64::from(u32::try_from(k).expect("a node index fits in 32 bits"))
}

/// Whether the companion-stripped mark of `draws` can move the fate of a star of initial mass
/// `m0` and composition `comp`: it is set, and `m0` lies in [`STRIPPED_WINDOW`]. Such a star is
/// not the table's.
#[must_use]
pub fn stripped_mark_matters(m0: SolarMasses, comp: &Composition, draws: &StarDraws) -> bool {
    is_companion_stripped(m0, comp, draws)
}

/// The number of masses of a [`TrackFates`] table: 96, log-spaced over the stellar range, 0.08–150
/// M☉, with the mass bands' edges among them (plan 06, P06.T30.b).
pub const TRACK_FATES_MASSES: usize = 96;

/// The intervals of the [`TrackFates`] grid in each mass band, A to E: the 95 intervals shared out
/// in proportion to each band's width in ln m (1.833, 0.405, 1.204, 1.163 and 2.931 of 7.536),
/// rounded, so that every band edge is a node and the spacing is near 0.079 in ln m everywhere.
const TRACK_FATES_INTERVALS: [usize; 5] = [23, 5, 15, 15, 37];

const _: () = assert!(
    TRACK_FATES_INTERVALS[0]
        + TRACK_FATES_INTERVALS[1]
        + TRACK_FATES_INTERVALS[2]
        + TRACK_FATES_INTERVALS[3]
        + TRACK_FATES_INTERVALS[4]
        + 1
        == TRACK_FATES_MASSES,
    "the bands' intervals make the grid"
);

/// The nodes of the remnant draws' quadrature in each of the type and fallback marks: 8, at the
/// midpoints of eight equal shares of the marks, (2k + 1) ÷ 16 of 2⁵³ (plan 06, P06.T30.b; design
/// note 2 of T2 builds them as marks).
pub const REMNANT_QUADRATURE_NODES: u32 = 8;

/// The masses of the [`TrackFates`] grid, M☉, ascending: [`TRACK_FATES_MASSES`] of them, log-spaced
/// within each mass band ([`MASS_BAND_EDGES`](crate::galaxy::imf::MASS_BAND_EDGES)), the edges
/// exact.
///
/// # Panics
///
/// Never: each band's interval count is a few dozen.
#[must_use]
pub fn track_fates_grid() -> Vec<f64> {
    let edges = MASS_BAND_EDGES;
    let mut grid = Vec::with_capacity(TRACK_FATES_MASSES);
    grid.push(edges[0]);
    for (band, &intervals) in TRACK_FATES_INTERVALS.iter().enumerate() {
        let (lo, hi) = (math::ln(edges[band]), math::ln(edges[band + 1]));
        let n = f64::from(u32::try_from(intervals).expect("a few dozen intervals"));
        for i in 1..intervals {
            let t = f64::from(u32::try_from(i).expect("a few dozen intervals")) / n;
            grid.push(math::exp(lo + (hi - lo) * t));
        }
        grid.push(edges[band + 1]);
    }
    grid
}

/// The mean remnant mass, M☉, of an iron core of carbon–oxygen core `co` and helium core `helium`,
/// over the remnant type and fallback marks by [`REMNANT_QUADRATURE_NODES`] midpoint nodes in each,
/// the mass's normal at its median (plan 06, P06.T30.b).
#[must_use]
fn mean_iron_core_remnant(co: f64, helium: f64) -> f64 {
    let helium = helium.max(0.0);
    let co = co.clamp(0.0, helium);
    let n = REMNANT_QUADRATURE_NODES;
    let mark = |k: u32| {
        // (2k + 1) ÷ 2n of 2⁵³ marks, as a word's top 53 bits.
        let share = (2 * u64::from(k) + 1) << (53 - 1 - n.trailing_zeros());
        Mark::from_word(share << 11)
    };
    let mut sum = 0.0;
    for t in 0..n {
        for f in 0..n {
            let draws = RemnantDraws::from_parts(mark(t), mark(f), StandardNormal::ZERO);
            sum += core_collapse(SolarMasses::new(co), SolarMasses::new(helium), draws)
                .remnant()
                .mass()
                .value();
        }
    }
    sum / f64::from(n * n)
}

/// The lifetime, years, the mean remnant mass, M☉, and the remnant's mass if it is a white dwarf
/// (0 for a neutron star, a black hole or nothing), M☉, of an unstripped star of initial mass `m0` and
/// composition `comp` at the median draws: the fate table's ([`FittedFates::fate_fitted`]) where
/// it answers, else one [`FateNode::of`], a full track. Below the tracks' lightest mass the star is
/// taken at [`MIN_INITIAL_MASS`](crate::stellar::sse::MIN_INITIAL_MASS), whose lifetime is far
/// longer than the universe's age.
#[must_use]
fn node_fate(m0: f64, comp: &Composition) -> (f64, f64, f64) {
    let m0 = SolarMasses::new(m0.max(crate::stellar::sse::MIN_INITIAL_MASS.value()));
    let eta = StandardNormal::ZERO;
    let (route, death_age, a, b) =
        if let Some(fate) = FittedFates::generator().fate_fitted(m0, comp, eta) {
            (fate.route, fate.death_age.value(), fate.a, fate.b)
        } else {
            let node = FateNode::of(m0, comp, eta);
            (node.route, math::exp10(node.log_death_age), node.a, node.b)
        };
    let (remnant, white_dwarf) = match route {
        FateRoute::HeliumWhiteDwarf
        | FateRoute::CarbonOxygenWhiteDwarf
        | FateRoute::OxygenNeonWhiteDwarf
        | FateRoute::BridgedCarbonOxygenWhiteDwarf
        | FateRoute::BridgedOxygenNeonWhiteDwarf => (a, a),
        FateRoute::ElectronCapture => (electron_capture_remnant().mass().value(), 0.0),
        FateRoute::IronCore => (mean_iron_core_remnant(a, b), 0.0),
        FateRoute::NoRemnant => (0.0, 0.0),
    };
    (death_age, remnant, white_dwarf)
}

/// Real lifetimes and remnant masses at one metallicity, for the galaxy's mean mass per system
/// (plan 06, P06.T30.b): plan 02's [`StellarFates`](crate::galaxy::fates::StellarFates) from the
/// generator's own tracks.
///
/// A star of initial mass m lives to its track's death at the median draws, and leaves the mean of
/// its remnant over the remnant type and fallback marks ([`REMNANT_QUADRATURE_NODES`] nodes each,
/// the other draws at their medians). Both are tabulated once at the [`TRACK_FATES_MASSES`]
/// masses of [`track_fates_grid`] and interpolated linearly in ln m, the lifetime's logarithm and
/// the remnant's mass. The lifetime is held non-increasing in mass (each node at most the one
/// below it), as the quadrature asks. The nodes are the fate table's at η's median where it
/// answers ([`FittedFates::fate_fitted`], within its validated error of the track) and a full track
/// ([`FateNode::of`]) where it does not, below 0.741 M☉ and in its unusable cells.
///
/// Every interior node is a kink of the interpolant, so [`breaks`](StellarFates::breaks) lists
/// them all, and plan 02's quadrature takes each as a panel edge. Companions are
/// [`ProvisionalFates`](crate::galaxy::fates::ProvisionalFates)'; P11.T1.d's
/// [`MultiplicityFates`](crate::stellar::multiplicity::MultiplicityFates) wraps this with plan 11's.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fates::StellarFates;
/// use hyperion_sim::stellar::fates::TrackFates;
/// use hyperion_sim::units::Dex;
///
/// let solar = TrackFates::at(Dex::new(0.0));
/// // The Sun lives about 10 Gyr on its tracks and leaves a white dwarf of about 0.5 M☉.
/// let t = solar.lifetime(1.0).value() / 1e9;
/// assert!((8.0..13.0).contains(&t), "{t} Gyr");
/// assert!((0.45..0.65).contains(&solar.remnant_mass(1.0)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TrackFates {
    fe_h: Dex,
    /// ln m of each node.
    ln_masses: Vec<f64>,
    /// log₁₀ of each node's lifetime, years, non-increasing.
    log_lifetimes: Vec<f64>,
    /// Each node's mean remnant mass, M☉.
    remnants: Vec<f64>,
    /// Each node's remnant mass if it is a white dwarf, else 0, M☉.
    white_dwarfs: Vec<f64>,
    /// The interior nodes' masses, M☉.
    breaks: Vec<f64>,
}

impl TrackFates {
    /// The table at \[Fe/H\] `fe_h`, with no helium excess: [`TRACK_FATES_MASSES`] fates, some
    /// tens of milliseconds where the fate table does not answer.
    #[must_use]
    pub fn at(fe_h: Dex) -> Self {
        let comp = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
        let grid = track_fates_grid();
        let mut log_lifetimes = Vec::with_capacity(grid.len());
        let mut remnants = Vec::with_capacity(grid.len());
        let mut white_dwarfs = Vec::with_capacity(grid.len());
        let mut longest = f64::INFINITY;
        for &m in &grid {
            let (lifetime, remnant, white_dwarf) = node_fate(m, &comp);
            longest = longest.min(math::log10(lifetime));
            log_lifetimes.push(longest);
            remnants.push(remnant);
            white_dwarfs.push(white_dwarf);
        }
        // The interpolant's kinks, and the provisional companions' bin edges.
        let mut breaks: Vec<f64> = grid[1..grid.len() - 1]
            .iter()
            .chain(ProvisionalFates.breaks())
            .copied()
            .collect();
        breaks.sort_by(f64::total_cmp);
        breaks.dedup();
        Self {
            fe_h,
            ln_masses: grid.iter().map(|&m| math::ln(m)).collect(),
            log_lifetimes,
            remnants,
            white_dwarfs,
            breaks,
        }
    }

    /// The table's \[Fe/H\].
    #[must_use]
    pub fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The mass, M☉, of the remnant a star of `m` M☉ leaves if it is a white dwarf, and 0 for a
    /// neutron star, a black hole or nothing: what a census of stars and white dwarfs holds of it
    /// once it dies (ruling 138.4), interpolated as [`remnant_mass`](StellarFates::remnant_mass) is.
    #[must_use]
    pub fn white_dwarf_mass(&self, m: f64) -> f64 {
        self.interpolate(&self.white_dwarfs, m)
    }

    /// The segment holding `m` M☉ (clamped to the grid) and the weight of its upper node.
    #[must_use]
    fn segment(&self, m: f64) -> (usize, f64) {
        let last = self.ln_masses.len() - 1;
        let x = math::ln(m).clamp(self.ln_masses[0], self.ln_masses[last]);
        let upper = self
            .ln_masses
            .partition_point(|&node| node <= x)
            .clamp(1, last);
        let (lo, hi) = (self.ln_masses[upper - 1], self.ln_masses[upper]);
        (upper - 1, (x - lo) / (hi - lo))
    }

    /// `values` interpolated at `m` M☉.
    #[must_use]
    fn interpolate(&self, values: &[f64], m: f64) -> f64 {
        let (i, t) = self.segment(m);
        values[i] + t * (values[i + 1] - values[i])
    }
}

impl StellarFates for TrackFates {
    fn lifetime(&self, m: f64) -> Years {
        Years::new(math::exp10(self.interpolate(&self.log_lifetimes, m)))
    }

    fn remnant_mass(&self, m: f64) -> f64 {
        self.interpolate(&self.remnants, m)
    }

    fn mean_companions(&self, m: f64) -> f64 {
        ProvisionalFates.mean_companions(m)
    }

    fn breaks(&self) -> &[f64] {
        &self.breaks
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::units::{Dex, HeliumExcess};

    fn random(rng: &mut Lcg) -> (SolarMasses, Composition, StandardNormal) {
        let m = 0.7 * math::exp10(rng.next_f64() * math::log10(100.0 / 0.7));
        let fe_h = -2.5 + 3.0 * rng.next_f64();
        let eta = -3.0 + 6.0 * rng.next_f64();
        (
            SolarMasses::new(m),
            Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
            StandardNormal::new(eta).expect("finite"),
        )
    }

    /// P06.T30.b: the grid has 96 masses, log-spaced within each band with the bands' edges
    /// among them.
    #[test]
    fn the_track_fates_grid_holds_the_band_edges() {
        let grid = track_fates_grid();
        assert_eq!(grid.len(), TRACK_FATES_MASSES);
        for edge in MASS_BAND_EDGES {
            assert!(grid.iter().any(|&m| m.total_cmp(&edge).is_eq()), "{edge}");
        }
        for pair in grid.windows(2) {
            let step = math::ln(pair[1] / pair[0]);
            assert!((0.07..0.09).contains(&step), "{step} at {pair:?}");
        }
    }

    /// P06.T30.b: at masses between the nodes the table is within a few per cent of full tracks,
    /// its lifetime never rises with mass, and its remnants are those the tracks leave.
    #[test]
    fn track_fates_follow_the_tracks() {
        use crate::stellar::sse::Track;
        for fe_h in [0.0, -1.2] {
            let fates = TrackFates::at(Dex::new(fe_h));
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for m in [0.9, 1.3, 2.2, 4.0, 6.5] {
                let track = Track::full(SolarMasses::new(m), &comp, &StarDraws::median());
                let t = track.lifetime().unwrap().value();
                let ours = fates.lifetime(m).value();
                assert!(
                    (ours / t - 1.0).abs() < 0.03,
                    "[Fe/H] {fe_h}, {m} M☉: {ours} against {t}"
                );
                let remnant = track.remnant().unwrap().mass().value();
                let mine = fates.remnant_mass(m);
                assert!(
                    (mine - remnant).abs() < 0.03,
                    "{m} M☉: {mine} against {remnant}"
                );
            }
            let mut previous = f64::INFINITY;
            for k in 0..=2_000 {
                let m = 0.08 * math::exp(f64::from(k) / 2_000.0 * math::ln(150.0 / 0.08));
                let t = fates.lifetime(m).value();
                assert!(t <= previous, "the lifetime rises at {m} M☉");
                previous = t;
                assert!((0.0..=60.0).contains(&fates.remnant_mass(m)), "{m}");
            }
            assert!(fates.breaks().windows(2).all(|p| p[0] < p[1]));
        }
    }

    /// The remnant quadrature's marks are the midpoints of eight shares, and a core that always
    /// leaves one kind of remnant gives that remnant's mass.
    #[test]
    fn the_remnant_quadrature_integrates_the_marks() {
        let low = mean_iron_core_remnant(1.0, 3.0);
        let expected = core_collapse(
            SolarMasses::new(1.0),
            SolarMasses::new(3.0),
            RemnantDraws::from_parts(Mark::from_word(0), Mark::from_word(0), StandardNormal::ZERO),
        )
        .remnant()
        .mass()
        .value();
        assert!((low - expected).abs() < 1e-12, "{low} against {expected}");
        let heavy = mean_iron_core_remnant(20.0, 30.0);
        assert!(heavy > 10.0, "{heavy}");
    }

    #[test]
    fn route_codes_round_trip() {
        for code in 0..8 {
            let c = f64::from(code);
            assert_eq!(FateRoute::from_code(c).map(FateRoute::code), Some(c));
        }
        assert_eq!(FateRoute::from_code(8.0), None);
        assert_eq!(FateRoute::from_code(0.5), None);
    }

    /// A node's draws leave the stripped mark unset, so a node is an unstripped star's fate.
    #[test]
    fn a_nodes_draws_are_unstripped() {
        let at = SolarMasses::new(8.0);
        assert!(!is_companion_stripped(
            at,
            &Composition::SOLAR,
            &node_draws(StandardNormal::ZERO)
        ));
    }

    /// The mark moves no fate outside [`STRIPPED_WINDOW`], so a stripped star there is the
    /// table's.
    #[test]
    fn the_stripped_mark_moves_no_fate_outside_its_window() {
        let mut rng = Lcg::new(0x7374_7269);
        let mut stripped_parts = StarDrawsParts::MEDIAN;
        // A mark of zero lies below every share.
        stripped_parts.stripped = crate::rng::Mark::from_word(0);
        // Just outside the window's edges at the fitted metallicities' ends and middle, where a
        // metallicity-shifted window would first show.
        for m in [5.2, 5.45, 11.1, 12.5] {
            for fe_h in [-2.3, -1.0, 0.0, 0.18] {
                let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                let stripped = StarDraws::from_parts(stripped_parts.clone());
                let (m, eta) = (SolarMasses::new(m), StandardNormal::ZERO);
                let a = Track::full(m, &comp, &stripped);
                let b = Track::full(m, &comp, &node_draws(eta));
                assert_eq!(a.fate_record(), b.fate_record(), "{m:?} {comp:?}");
            }
        }
        let mut checked = 0;
        while checked < 40 {
            let (m, comp, eta) = random(&mut rng);
            if (STRIPPED_WINDOW.0..=STRIPPED_WINDOW.1).contains(&m.value()) {
                continue;
            }
            let stripped = StarDraws::from_parts(StarDrawsParts {
                eta,
                ..stripped_parts.clone()
            });
            assert!(crate::galaxy::displaced::binarity::is_stripped(
                stripped.stripped(),
                m,
                &comp
            ));
            assert!(!is_companion_stripped(m, &comp, &stripped));
            let a = Track::full(m, &comp, &stripped);
            let b = Track::full(m, &comp, &node_draws(eta));
            assert_eq!(a.fate_record(), b.fate_record(), "{m:?} {comp:?}");
            checked += 1;
        }
    }

    /// `x` to the table's eight significant digits, as `hyperion-fit` stores it.
    fn stored(x: f64) -> f64 {
        if x == 0.0 {
            x
        } else {
            format!("{x:.7e}").parse().unwrap()
        }
    }

    /// The node check (plan 15, design note 7's stand-in): twelve nodes of the committed table,
    /// four of each panel, across the routes, rebuilt by [`FateNode::of`] and compared with
    /// the table to its stored digits. A change to the tracks that moves a node fails here, in
    /// ordinary CI, and not only in the table's slow reproduction.
    /// A white dwarf off the post-AGB bridge is tabulated by its knee (ruling 127.1): the node's
    /// `b` is its knee's log₁₀ L, and the remnant the table rebuilds from it fades as the track's
    /// does, to 10⁻⁴ dex through the fade and after the cooling law takes over.
    #[test]
    fn a_table_routed_bridged_white_dwarf_fades_as_its_track() {
        let (m0, comp, eta) = (
            SolarMasses::new(4.264),
            Composition::SOLAR,
            StandardNormal::new(0.34).expect("finite"),
        );
        let node = FateNode::of(m0, &comp, eta);
        assert_eq!(node.route, FateRoute::BridgedCarbonOxygenWhiteDwarf);
        assert!((3.0..5.0).contains(&node.b), "{node:?}");
        let fate = FittedFate {
            route: node.route,
            death_age: Years::new(math::exp10(node.log_death_age)),
            a: node.a,
            b: node.b,
            bounds: (0.0, 0.0, 0.0),
        };
        let remnant = fate.remnant(RemnantDraws::of(&node_draws(eta)), (0.0, 0.0, 0.0));
        let track = Track::full(m0, &comp, &node_draws(eta));
        let death = track.lifetime().expect("dies").value();
        for dt in [1.0, 100.0, 1e4, 1e5, 1e6, 1e8] {
            let age = Years::new(death + dt);
            let exact = track.state_at(age).luminosity().value();
            let table = track.remnant_state_at(remnant, age).luminosity().value();
            assert!(
                math::log10(table / exact).abs() < 1e-4,
                "+{dt}: {table} against {exact}"
            );
        }
    }

    #[test]
    fn the_committed_nodes_are_the_tracks() {
        // Per panel, four nodes as (\[Fe/H\] node, η node, mass node).
        type Picks = [(usize, usize, usize); 4];
        let fates = FittedFates::generator();
        let z = fates.fe_h.len();
        let mut checked = std::collections::BTreeSet::new();
        let picks: [(usize, Picks); 3] = [
            (0, [(0, 0, 10), (6, 1, 25), (14, 2, 40), (23, 3, 55)]),
            (1, [(3, 2, 0), (10, 0, 20), (17, 1, 40), (23, 3, 64)]),
            (2, [(0, 0, 0), (8, 0, 12), (16, 0, 35), (23, 0, 62)]),
        ];
        for (panel, picks) in picks.map(|(k, p)| (&fates.panels[k], p)) {
            let masses = panel.masses(z);
            for (iz, ie, im) in picks {
                let (iz, ie, im) = (
                    iz.min(z - 1),
                    ie.min(panel.etas.len() - 1),
                    im.min(masses - 1),
                );
                let row = panel.nodes[(iz * panel.etas.len() + ie) * masses + im];
                let m = math::exp10(panel.log_mass_start + panel.log_mass_step * index(im));
                let comp = Composition::from_fe_h(Dex::new(fates.fe_h[iz]), HeliumExcess::ZERO);
                let node = FateNode::of(
                    SolarMasses::new(m),
                    &comp,
                    StandardNormal::new(panel.etas[ie]).unwrap(),
                )
                .to_row();
                assert_eq!(
                    (node.0, stored(node.1), stored(node.2), stored(node.3)),
                    row,
                    "node ({iz}, {ie}, {im}) at {m} M☉"
                );
                checked.insert(FateRoute::from_code(row.0).unwrap());
            }
        }
        assert!(
            checked.len() >= 3,
            "the nodes span several routes: {checked:?}"
        );
        assert!(FittedFates::metallicities_agree());
    }

    /// The remnant build is the full track's remnant and fate bit for bit, for stars of random
    /// draws, the stripped mark and the remnant draws included.
    #[test]
    fn the_remnant_build_is_the_full_tracks() {
        let mut rng = Lcg::new(0x7265_6d6f);
        for i in 0..40_u64 {
            let (m, comp, _) = random(&mut rng);
            let draws = StarDraws::for_star(
                crate::Seed::new(0x5eed + i),
                crate::id::BodyId::new(
                    crate::id::SystemId::from_raw(0x0200_0800_2000_0000 + (i << 20)).unwrap(),
                    0,
                ),
            );
            let full = Track::full(m, &comp, &draws);
            let (remnant, fate) = crate::stellar::sse::remnant_of(
                m,
                &comp,
                &draws,
                crate::stellar::sse::TrackOptions::default(),
            );
            assert_eq!(Some(remnant), full.remnant_model(), "{m:?} {comp:?}");
            assert_eq!(Some(fate), full.fate_record(), "{m:?} {comp:?}");
        }
    }

    /// Over `n` random unstripped stars the table answers for: the route is the track's, the
    /// death age lies within the margin the range brief takes to call a star dead (three times
    /// its bound, at least `brief::DEAD_MARGIN_FLOOR`), the columns within three times their
    /// bounds, and the share past the bounds themselves is returned. The bounds are validated,
    /// not proved.
    fn check_fates(seed: u64, n: u32) -> (u32, u32, u32) {
        let fates = FittedFates::generator();
        let mut rng = Lcg::new(seed);
        let (mut answered, mut outside, mut wrong_route) = (0, 0, 0);
        for _ in 0..n {
            let (m, comp, eta) = random(&mut rng);
            let Some(fitted) = fates.fate_fitted(m, &comp, eta) else {
                continue;
            };
            answered += 1;
            let exact = FateNode::of(m, &comp, eta);
            if exact.route != fitted.route {
                wrong_route += 1;
                continue;
            }
            let t = math::exp10(exact.log_death_age);
            let (lo, hi) = fates
                .lifetime_bracket(m, &comp, eta)
                .expect("the table answers");
            let within = |x: f64, fit: f64, bound: f64, floor: f64| {
                (x - fit).abs() <= bound * x.abs().max(floor) * (1.0 + 1e-9)
            };
            let inside = (lo.value()..=hi.value()).contains(&t)
                || within(t, fitted.death_age.value(), fitted.bounds.0, 0.0);
            let k = crate::stellar::brief::STATED_FACTOR;
            let dead_margin = (k * fitted.bounds.0).max(crate::stellar::brief::DEAD_MARGIN_FLOOR);
            assert!(
                within(t, fitted.death_age.value(), dead_margin, 0.0)
                    && within(exact.a, fitted.a, k * fitted.bounds.1, VALUE_FLOORS[0])
                    && within(exact.b, fitted.b, k * fitted.bounds.2, VALUE_FLOORS[1]),
                "{m:?} {comp:?} {eta:?}: {exact:?} past three times {fitted:?}"
            );
            if !(inside
                && within(exact.a, fitted.a, fitted.bounds.1, VALUE_FLOORS[0])
                && within(exact.b, fitted.b, fitted.bounds.2, VALUE_FLOORS[1]))
            {
                outside += 1;
            }
        }
        (answered, outside, wrong_route)
    }

    /// P06.T38.d on 400 stars: the route is always the track's, every star lies within three
    /// times its bounds, and at most one in fifty past a bound itself (the slow test holds 10⁴
    /// to one in a hundred).
    #[test]
    fn the_table_is_within_its_bounds() {
        let (answered, outside, wrong_route) = check_fates(0x6661_7465, 400);
        assert!(answered > 150, "{answered} answered");
        assert_eq!(wrong_route, 0);
        assert!(
            outside * 50 <= answered,
            "{outside} of {answered} past a bound"
        );
    }

    #[test]
    #[ignore = "slow: 10⁴ random stars, each a full track"]
    fn the_table_is_within_its_bounds_over_ten_thousand_stars() {
        let (answered, outside, wrong_route) = check_fates(0x6661_7466, 10_000);
        println!("{answered} answered, {outside} past a bound, {wrong_route} by another route");
        assert_eq!(wrong_route, 0);
        assert!(
            outside * 100 <= answered,
            "{outside} of {answered} past a bound"
        );
    }

    /// A remnant-only track is the full track's state bit for bit after the death.
    #[test]
    fn a_remnant_only_track_is_the_full_tracks_remnant() {
        let mut rng = Lcg::new(0x7265_6d6e);
        for _ in 0..30 {
            let (m, comp, eta) = random(&mut rng);
            let draws = node_draws(eta);
            let full = Track::full(m, &comp, &draws);
            let remnant = full.remnant_model().expect("a remnant");
            let alone = Track::remnant_only(
                &comp,
                eta.value(),
                crate::stellar::sse::TrackOptions::default(),
                remnant,
            );
            for later in [1.0, 1e3, 1e7, 5e9] {
                let age = Years::new(remnant.birth + later);
                assert_eq!(alone.state_at(age), full.state_at(age), "{m:?} {comp:?}");
            }
        }
    }
}
