//! P14.T26's wiring: the states `resource_section` gives a body's resource abundances where its
//! bulk or its layers are withheld, which the golden systems reach only in part.

use super::*;
use crate::planetary::chem::inventory::{InventoryInputs, element_inventory};
use crate::planetary::derive::atmosphere::VolatileDraws;
use crate::planetary::derive::composition;
use crate::planetary::index::{BodySlot, BodySub};
use crate::planetary::record::{DetailLevel, SectionState};
use crate::units::consts::METRES_PER_AU;
use crate::units::{Dex, EarthFluxes, EarthMasses, EarthRadii};

/// An Earth's bulk composition about a solar host.
fn earth_bulk() -> Section<BulkComposition> {
    let mass = EarthMasses::new(1.0);
    let side = SnowLineSide::Inside;
    let fractions = composition(mass, EarthRadii::new(1.0), side, EarthFluxes::new(1.0))
        .unwrap()
        .fractions();
    let elements = element_inventory(&InventoryInputs {
        mass,
        fractions,
        formed: side,
        formation_distance: Metres::new(METRES_PER_AU),
        snow_line: Metres::new(2.7 * METRES_PER_AU),
        draws: VolatileDraws::MEDIAN,
        age: Years::new(4.57e9),
    });
    Section::Ok(BulkComposition::new(
        fractions,
        elements,
        Dex::ZERO,
        Some(Dex::ZERO),
    ))
}

/// The record of a present planet whose surface and envelope sections are `surface` and
/// `envelope`, and every other section not modelled.
fn record(surface: Section<Surface>, envelope: Section<Envelope>) -> BodyRecord {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let index = BodyIndex::new(BodySlot::Planet(3), BodySub::Primary).unwrap();
    let identity = BodyIdentity::new(
        system,
        index,
        BodyKind::Planet,
        Some(OrbitHost::Star(0)),
        BodyState::Present,
    );
    BodyRecord::builder(identity)
        .surface(surface)
        .envelope(envelope)
        .build()
        .unwrap()
}

/// The state of the abundances of a body of bulk `bulk`, formed on `formed`'s side, whose record
/// is `record`.
fn state_of(
    bulk: &Section<BulkComposition>,
    formed: Option<SnowLineSide>,
    record: &BodyRecord,
) -> SectionState {
    let body = record.index().body_id(record.identity().system());
    resource_section(
        Seed::new(26),
        body,
        bulk,
        formed,
        record,
        Years::new(4.57e9),
    )
    .state()
}

/// The bulk's state comes first; with a bulk, a withheld layer gives its state, not resolved
/// before not modelled before not applicable, whichever of the two sections holds it.
#[test]
fn withheld_resources_take_the_state_of_what_withholds_them() {
    let bulk = earth_bulk();
    let inside = Some(SnowLineSide::Inside);
    let neither = record(Section::NotApplicable, Section::NotApplicable);
    for (bulk, formed, expected) in [
        (&Section::NotApplicable, inside, SectionState::NotApplicable),
        (&Section::NotModelled, inside, SectionState::NotModelled),
        (&bulk, None, SectionState::NotModelled),
    ] {
        assert_eq!(state_of(bulk, formed, &neither), expected);
    }
    for (surface, envelope, expected) in [
        (
            Section::NotApplicable,
            Section::NotApplicable,
            SectionState::NotApplicable,
        ),
        (
            Section::NotModelled,
            Section::NotApplicable,
            SectionState::NotModelled,
        ),
        (
            Section::NotApplicable,
            Section::NotModelled,
            SectionState::NotModelled,
        ),
        (
            Section::NotModelled,
            Section::NotModelled,
            SectionState::NotModelled,
        ),
    ] {
        let record = record(surface, envelope);
        assert_eq!(state_of(&bulk, inside, &record), expected, "{record:?}");
    }
    // Degraded below the bulk, both sections are not resolved, which comes before the rest.
    let withheld = neither.degrade(DetailLevel::MassAndOrbit);
    assert_eq!(withheld.surface(), &Section::NotResolved);
    assert_eq!(
        state_of(&bulk, inside, &withheld),
        SectionState::NotResolved
    );
}
