//! The recurrent walk's binary classes (P12.T5.c): the accreting white dwarfs' novae and the X-ray
//! binaries' transients, bursts and Be/X outbursts, through plan 11's scan marks alone (design note
//! 10 of plan 11), so that a scan of millions of hosts runs no binary engine.
//!
//! A cell's hosts are its scan marks
//! ([`scan::scan_cell`](crate::stellar::binary::scan::scan_cell)), the class's entries exactly,
//! each on its straight line through its site at the epoch. Their events are plan 11's
//! ([`events_in`], P11.T9.b) over each host's emission interval ([`lit`](super::lit)):
//!
//! - **Novae.** A fast host's (`ACCRETING_WHITE_DWARF`) novae are asked of the monotone phase its
//!   marks carry, unless the phase's noise bound puts no cycle's crossing in the interval. A slow
//!   host's (`ACCRETING_WHITE_DWARF_SLOW`) recur more slowly than L, so its marks carry its
//!   eruptions in the source horizon, at most two, and the host is passed over unless one falls
//!   in the interval; its phase is asked then, for the eruption's ID. Every onset whose light can
//!   arrive in a window inside the clock window is in the source horizon.
//! - **X-ray binaries.** A low-mass transient's outbursts, a persistent neutron star's Type I
//!   bursts (main's ruling of 2026-10-30: walked here, not locally) and a Be/X host's giant
//!   outbursts; a persistent black hole and a supergiant host have none.
//!
//! Each host is asked back by its own kind's longest event (the registry's duration), so that a
//! burster's seconds are not listed over a transient's months, and passed over unasked if it stays
//! beyond its own kind's reach, so that a scan for transients lists no burster beyond the bursts'.
//! Dwarf novae are local (P12.T5.e), as their row's route says, and are not walked here.

use crate::events::{PhaseClock, TimeWindow};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide, XrayBinaries};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueMotion, ClassId};
use crate::id::SystemId;
use crate::stellar::binary::events::{BinarySystem, events_in};
use crate::stellar::binary::scan::{self, AwdScanMarks};
use crate::tables::binary::XrbHostKind;

use super::super::reach::kind_reach;
use super::super::registry::TransientKind;
use super::super::{AlertContext, AlertQuery, Detection};
use super::lit::{LitEvent, LitHost, emission_interval, host_detections, longest};
use super::sort_detections;

/// Appends to `out` the detections of the novae of `process`'s cell `cell` that `query`'s sensor
/// sees in its window, sorted by arrival and then by event (module documentation). Nothing for a
/// cell of another class, or one the query does not ask about.
///
/// It reads the hosts' scan marks alone and never runs plan 11's binary engine.
///
/// # Panics
///
/// As plan 11's [`scan::scan_cell`] does.
pub fn nova_scan_cell(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &AwdProcess,
    cell: CatalogueCellKey,
    out: &mut Vec<Detection>,
) {
    let class = process.class();
    if cell.class() != class || !query.classes().contains(class) {
        return;
    }
    let observer = *query.observer_position();
    let look_back = longest(TransientKind::Nova);
    let reach = kind_reach(TransientKind::Nova, query.sensor());
    let mut hosts = Vec::new();
    scan::scan_cell(galaxy, process, cell, &mut hosts);
    let start = out.len();
    let (mut listed, mut events) = (Vec::new(), Vec::new());
    for marks in &hosts {
        let motion = CatalogueMotion::straight(*marks.position(), marks.velocity());
        let Some(interval) =
            emission_interval(&motion, &observer, query.window(), look_back, reach)
        else {
            continue;
        };
        let erupts = match marks.side() {
            AwdSide::Slow => marks.eruptions().iter().any(|&t| interval.contains(t)),
            AwdSide::Fast => may_erupt(marks, interval),
        };
        if !erupts {
            continue;
        }
        listed.clear();
        let system = BinarySystem::accreting_white_dwarf_scan(galaxy.seed(), marks);
        events_in(&system, interval, &mut listed);
        events.clear();
        events.extend(listed.iter().map(LitEvent::of_binary));
        let host = LitHost {
            host: SystemId::from(marks.id()),
            class,
            motion,
        };
        host_detections(galaxy, ctx, query, &host, &events, out);
    }
    sort_detections(&mut out[start..]);
}

/// The margin on the base phase beyond the noise's bound within which a fast host's eruption is
/// sought, cycles: 0.01, as plan 11's slow hosts' (`scan`'s `HORIZON_PHASE_MARGIN_CYCLES`), far
/// above the few units in the last place by which the phase as computed strays from monotone.
const PHASE_MARGIN_CYCLES: f64 = 0.01;

/// Whether a fast host's nova phase can erupt in `interval`: cycle n's eruption is where Φ = φ +
/// N(φ) reaches n, and |N| is at most the noise's bound, so one inside the interval has its base
/// phase φ there within the bound (and [`PHASE_MARGIN_CYCLES`]) of n. A host with no whole cycle
/// in that reach of the interval's base phases has no eruption in it, and its phase is not asked.
#[must_use]
fn may_erupt(marks: &AwdScanMarks, interval: TimeWindow) -> bool {
    let margin = marks.phase().noise_bound_cycles() + PHASE_MARGIN_CYCLES;
    let first = marks.clock().base_phase(interval.start()).offset(-margin);
    let last = marks.clock().base_phase(interval.end()).offset(margin);
    let lowest = if first.fraction() > 0.0 {
        first.cycles().saturating_add(1)
    } else {
        first.cycles()
    };
    lowest <= last.cycles()
}

/// Appends to `out` the detections of the transients' outbursts, the bursts and the Be/X outbursts
/// of the X-ray binaries of `process`'s cell `cell` that `query`'s sensor sees in its window,
/// sorted by arrival and then by event (module documentation). Nothing for a cell of another class,
/// or one the query does not ask about.
///
/// It reads the hosts' scan marks alone and never runs plan 11's binary engine.
///
/// # Panics
///
/// As plan 11's [`scan::scan_cell`] does.
pub fn xray_binary_scan_cell(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &XrayBinaries,
    cell: CatalogueCellKey,
    out: &mut Vec<Detection>,
) {
    let class = ClassId::XRAY_BINARY;
    if cell.class() != class || !query.classes().contains(class) {
        return;
    }
    let observer = *query.observer_position();
    let mut hosts = Vec::new();
    scan::scan_cell(galaxy, process, cell, &mut hosts);
    let start = out.len();
    let (mut listed, mut events) = (Vec::new(), Vec::new());
    for marks in &hosts {
        let kind = match marks.kind() {
            XrbHostKind::NeutronStarTransient | XrbHostKind::BlackHoleTransient => {
                TransientKind::XrayTransient
            }
            XrbHostKind::NeutronStarPersistent => TransientKind::XrayBurst,
            XrbHostKind::BeX => TransientKind::BeXOutburst,
            XrbHostKind::BlackHolePersistent | XrbHostKind::Supergiant => continue,
        };
        let motion = CatalogueMotion::straight(*marks.position(), marks.velocity());
        let reach = kind_reach(kind, query.sensor());
        let Some(interval) =
            emission_interval(&motion, &observer, query.window(), longest(kind), reach)
        else {
            continue;
        };
        listed.clear();
        events_in(
            &BinarySystem::xray_binary(galaxy.seed(), marks),
            interval,
            &mut listed,
        );
        events.clear();
        events.extend(listed.iter().map(LitEvent::of_binary));
        let host = LitHost {
            host: SystemId::from(marks.id()),
            class,
            motion,
        };
        host_detections(galaxy, ctx, query, &host, &events, out);
    }
    sort_detections(&mut out[start..]);
}
