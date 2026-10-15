//! The surface service's helpers, shared by the surface's integration tests (rendering plan R09,
//! R09.T17): the coarse pass's reference worlds as the service's inputs, and the generated systems
//! whose bodies the service computes fields for; and, for the Knowledge-bound test (R09.T19.c), a
//! field asked for over the socket ([`field_via_wire`]) and decoded as a client decodes it
//! ([`decode_all`], [`partial_field`], [`insert_all`]); and the checks that a frame carries no
//! surface seed ([`Secret`], [`check_no_seed_key`], [`has_key`]; R09.T1.b); and the field the
//! service serves from a reference world, with its albedo scale ([`served_field`], R10.T10.a).
//!
//! Plan 14's surface section is not inhabited yet, so the production source answers every body
//! `NotModelled`; these tests give the service a reference world's inputs instead, through its
//! `InputsSource` seam, so that the service and the wire are tested before plan 14's asks land.

use std::sync::{Arc, OnceLock};

use hyperion_protocol::{
    CoverDto, RequestBody, ResponseBody, ServerMessage, SurfaceFieldDto, SurfaceFieldRequest,
    SurfaceHeaderDto,
};
use hyperion_server::compute::GeneratedSystem;
use hyperion_server::surface::{InputsSource, RECORD_TIME, field_albedo_scale};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
use hyperion_sim::id::{BodyId, Layer};
use hyperion_sim::planetary::hooks::{SurfaceSeed, detail_seed, surface_seed};
use hyperion_sim::planetary::placement::OrbitHost;
use hyperion_sim::planetary::record::{BodyKind, BodyRecord, Section};
use hyperion_sim::planetary::surface::{
    CoarseInputs, ReadCoarseInputsError, SurfaceInputsError, coarse_pass, reference,
};
use hyperion_sim::planetary::{BodyIndex, SystemContext, generate};
use hyperion_sim::time::UniverseTime;
use hyperion_surface::field::{CoarseField, PartialField};
use hyperion_surface::wire::{DecodedBlock, decode_payload};
use serde_json::Value;

use super::{BinaryHeader, TestClient};

/// The seed of the universe the surface tests generate systems in, the other tests' own.
pub const SEED: u64 = 0x4d2;

/// One of the coarse pass's reference worlds (`hyperion_sim::planetary::surface::reference`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceWorld {
    /// The Earth-like world, a level-8 field.
    Earth,
    /// The Mars-like world, a level-7 field.
    Mars,
    /// The Moon-like world, a level-6 field.
    Moon,
    /// The Ceres-like world, a level-5 field and the quickest.
    Ceres,
}

impl ReferenceWorld {
    /// The world's inputs.
    pub fn inputs(self) -> CoarseInputs {
        match self {
            Self::Earth => reference::earth_like(),
            Self::Mars => reference::mars_like(),
            Self::Moon => reference::moon_like(),
            Self::Ceres => reference::ceres_like(),
        }
    }
}

/// An inputs source that gives every body with a solid surface one reference world's inputs, and
/// refuses one whose record has no surface section to give (a giant's) as the record's own reading
/// would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceInputs {
    world: ReferenceWorld,
}

impl InputsSource for ReferenceInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        _: Option<&BodyRecord>,
        _: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        match record.surface() {
            Section::NotApplicable => Err(SurfaceInputsError::NoSolidSurface.into()),
            Section::Ok(_) | Section::NotModelled | Section::NotResolved => Ok(self.world.inputs()),
        }
    }
}

/// The source that gives every body with a solid surface `world`'s inputs.
pub fn reference_inputs(world: ReferenceWorld) -> ReferenceInputs {
    ReferenceInputs { world }
}

/// The galaxy of [`SEED`], built once per test binary.
pub fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// A generated system with a body of each kind of surface section, as the server generates it.
#[derive(Debug, Clone)]
pub struct SurfaceSystem {
    /// The system: its context and its bodies as they were born.
    pub system: Arc<GeneratedSystem>,
    /// A body whose record has a surface section, not yet modelled: a rocky planet or a moon.
    pub solid: BodyId,
    /// A body whose record's surface section is not applicable: a giant.
    pub gaseous: BodyId,
}

/// The first system of the layer-C cell at the solar circle that has a body with a solid surface
/// and a body with none, generated once per test binary.
///
/// # Panics
///
/// If no system of the cell has both.
pub fn surface_system() -> &'static SurfaceSystem {
    static SYSTEM: OnceLock<SurfaceSystem> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
            &mut cell,
        );
        cell.iter()
            .find_map(|record| {
                let ctx = SystemContext::for_system(galaxy(), record.id()).ok()?;
                let planets = generate(Seed::new(SEED), &ctx);
                // The first body with a surface seed (a planet or a moon) whose surface section
                // applies, or does not.
                let with = |applicable: bool| {
                    planets.bodies().iter().find_map(|body| {
                        let index = body.index();
                        if !matches!(planets.surface_seed(index), Ok(Some(_))) {
                            return None;
                        }
                        let record = planets.body_at(&ctx, index, UniverseTime::EPOCH).ok()?;
                        (matches!(record.surface(), Section::NotApplicable) != applicable)
                            .then(|| index.body_id(planets.system()))
                    })
                };
                let (solid, gaseous) = (with(true)?, with(false)?);
                Some(SurfaceSystem {
                    system: Arc::new((ctx, planets)),
                    solid,
                    gaseous,
                })
            })
            .expect("a system of the cell has a solid body and a giant")
    })
}

/// A generated system with a moon whose surface section applies, as the server generates it
/// (R09.T10.f).
#[derive(Debug, Clone)]
pub struct MoonSystem {
    /// The system: its context and its bodies as they were born.
    pub system: Arc<GeneratedSystem>,
    /// A moon whose record has a surface section, not yet modelled.
    pub moon: BodyId,
    /// The planet it orbits, by its record's parent.
    pub planet: BodyId,
}

/// The first system of the layer-C cell at the solar circle with a moon (by its record's kind and
/// parent, not by `CoarseInputs::planet_of`) whose surface section applies, generated once per test
/// binary.
///
/// # Panics
///
/// If no system of the cell has one.
pub fn moon_system() -> &'static MoonSystem {
    static SYSTEM: OnceLock<MoonSystem> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
            &mut cell,
        );
        cell.iter()
            .find_map(|record| {
                let ctx = SystemContext::for_system(galaxy(), record.id()).ok()?;
                let planets = generate(Seed::new(SEED), &ctx);
                let (moon, planet) = planets.bodies().iter().find_map(|body| {
                    let record = planets.body_at(&ctx, body.index(), RECORD_TIME).ok()?;
                    let identity = record.identity();
                    match (identity.kind(), identity.parent(), record.surface()) {
                        (BodyKind::Moon(_), Some(OrbitHost::Body(planet)), Section::Ok(_)) => {
                            Some((body.index(), planet))
                        }
                        _ => None,
                    }
                })?;
                let id = planets.system();
                Some(MoonSystem {
                    system: Arc::new((ctx, planets)),
                    moon: moon.body_id(id),
                    planet: planet.body_id(id),
                })
            })
            .expect("a system of the cell has a moon with a surface")
    })
}

/// The field the surface service serves for `body` of [`surface_system`] from `world`'s inputs:
/// the sim's coarse pass of the body's surface seed in [`SEED`]'s universe, with the albedo scale
/// the service sets from the body's record at [`RECORD_TIME`] and its detail seed (R10.T10.a).
///
/// # Panics
///
/// If `body` is not a body of [`surface_system`], or its scale is not one the header holds.
pub fn served_field(world: ReferenceWorld, body: BodyId) -> CoarseField {
    let (ctx, planets) = surface_system().system.as_ref();
    let field = coarse_pass(surface_seed(Seed::new(SEED), body), &world.inputs());
    let index = BodyIndex::try_from(body.body_index()).expect("a body of the system has an index");
    let record = planets
        .body_at(ctx, index, RECORD_TIME)
        .expect("a body of the system resolves");
    let scale = field_albedo_scale(
        &field,
        record.photometry(),
        detail_seed(Seed::new(SEED), body),
    );
    field
        .with_albedo_scale(scale)
        .expect("a solved scale is finite and positive")
}

/// A body's coarse field as a `surface_field` answered it over the socket (R09.T19.b): what the
/// `ready` answer says, the answer as it arrived, and the payload its bulk frames carried.
#[derive(Debug)]
pub struct FieldViaWire {
    /// The field's header, as the answer states it.
    pub header: SurfaceHeaderDto,
    /// The body's coverage revision the payload reflects.
    pub revision: u32,
    /// The cells the payload holds, with their codes, margin cells at 0.
    pub coverage: CoverDto,
    /// The answer's text frame, as it arrived, for checks on the JSON itself.
    pub text: String,
    /// The bulk frames' headers, in the order they arrived.
    pub frames: Vec<BinaryHeader>,
    /// The payload: the bulk frames' payloads rejoined in chunk order.
    pub payload: Vec<u8>,
}

/// Asks for a body's coarse field over `client` and reads the answer as R03's transport sends it:
/// the payload's bulk frames, then the `ready` response.
///
/// The client must have no subscription open, so that nothing else arrives meanwhile: the frames
/// are read with [`TestClient::next_binary`], which refuses a text frame, and the answer with
/// [`TestClient::next_text`]. The frames are checked as a client checks them: each answers the
/// request, in sequence from 0, all of one count, and the answer's manifest states that count and
/// the payload's length.
///
/// # Panics
///
/// On any other answer: a refusal, `not_modelled` (which has no bulk frame), frames out of
/// sequence, or a manifest that does not match them.
pub async fn field_via_wire(client: &mut TestClient, request: SurfaceFieldRequest) -> FieldViaWire {
    let id = client
        .send_request(RequestBody::SurfaceField(request))
        .await;
    let mut frames: Vec<BinaryHeader> = Vec::new();
    let mut payload = Vec::new();
    // A `ready` payload always holds block 0, so it has a first frame, which states the count.
    loop {
        let (frame, bytes) = client.next_binary().await;
        let count = frames.first().map_or(frame.count, |first| first.count);
        assert_ne!(
            count, 0,
            "a `ready` payload holds block 0, so its frames are some"
        );
        let index = u32::try_from(frames.len()).expect("fewer than 2³² frames");
        assert_eq!(
            (frame.request, frame.index, frame.count),
            (id.0, index, count),
            "bulk frame {index} answers the request, in sequence, of one count"
        );
        frames.push(frame);
        payload.extend_from_slice(&bytes);
        if index + 1 == count {
            break;
        }
    }
    let text = client.next_text().await;
    let message: ServerMessage =
        serde_json::from_str(&text).expect("the server sends valid messages");
    let ServerMessage::Response {
        id: answered,
        body:
            ResponseBody::SurfaceField(SurfaceFieldDto::Ready {
                header,
                revision,
                coverage,
                bulk,
            }),
    } = message
    else {
        panic!("expected the field of request {}, got {message:?}", id.0);
    };
    assert_eq!(answered, id, "the answer is the request's");
    assert_eq!(
        (usize::try_from(bulk.chunks), usize::try_from(bulk.bytes)),
        (Ok(frames.len()), Ok(payload.len())),
        "the manifest states the frames and the payload's length"
    );
    FieldViaWire {
        header: *header,
        revision,
        coverage,
        text,
        frames,
        payload,
    }
}

/// A payload decoded as a client decodes it ([`decode_payload`]): every block checked, in
/// sequence from block 0, all of one body and level.
///
/// # Panics
///
/// If the payload does not decode.
pub fn decode_all(payload: &[u8]) -> Vec<DecodedBlock> {
    decode_payload(payload).unwrap_or_else(|error| panic!("the payload decodes: {error}"))
}

/// The field a client holds from a whole payload's blocks: [`PartialField::new`] from block 0's
/// header, then every block inserted, block 0 included.
///
/// # Panics
///
/// If there is no block 0, it carries no header, or a block is refused.
pub fn partial_field(blocks: &[DecodedBlock]) -> PartialField {
    let header = blocks
        .first()
        .and_then(DecodedBlock::header)
        .expect("block 0 carries the field's header");
    let mut field = PartialField::new(header.clone());
    insert_all(&mut field, blocks);
    field
}

/// Inserts every block of a payload, a delta's say, into a field the client holds already.
///
/// # Panics
///
/// If a block is refused.
pub fn insert_all(field: &mut PartialField, blocks: &[DecodedBlock]) {
    for block in blocks {
        field
            .insert(block)
            .unwrap_or_else(|error| panic!("block {} is inserted: {error}", block.index()));
    }
}

/// What must never reach a client: a body's surface seed, by its key or by its value (rendering
/// plan R09.T1.b, after R09.T19.c's own copy in `surface_knowledge.rs`).
#[derive(Debug, Clone)]
pub struct Secret {
    /// The seed as sixteen lowercase hexadecimal digits, as the crate prints 64-bit identifiers.
    hex: String,
    /// The seed in decimal.
    decimal: String,
}

impl Secret {
    /// The secret of the surface seed `seed`.
    pub fn of(seed: SurfaceSeed) -> Self {
        let value = seed.get();
        Self {
            hex: format!("{value:016x}"),
            decimal: value.to_string(),
        }
    }

    /// Checks a text frame as it arrived, and returns its message: no object in its JSON has a
    /// key named `surface_seed`, and its text does not hold the seed's value.
    ///
    /// # Panics
    ///
    /// If the frame fails either check, or is not a server message.
    pub fn check_text(&self, text: &str) -> ServerMessage {
        let message = check_no_seed_key(text);
        self.assert_absent(text);
        message
    }

    /// Checks that `text` does not hold the seed's value, in hexadecimal or in decimal.
    ///
    /// # Panics
    ///
    /// If it does.
    pub fn assert_absent(&self, text: &str) {
        assert!(
            !text.contains(&self.hex) && !text.contains(&self.decimal),
            "a frame holds the surface seed's value: {text}"
        );
    }
}

/// Checks a text frame as it arrived, and returns its message: no object in its JSON has a key
/// named `surface_seed`.
///
/// # Panics
///
/// If one has, or the frame is not a server message.
pub fn check_no_seed_key(text: &str) -> ServerMessage {
    let json: Value = serde_json::from_str(text).expect("the server sends JSON");
    assert!(
        !has_key(&json, "surface_seed"),
        "a frame carries a key named `surface_seed`: {text}"
    );
    serde_json::from_value(json).expect("the server sends valid messages")
}

/// Whether any object within `value` has a key named `key`.
pub fn has_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(members) => members
            .iter()
            .any(|(name, member)| name == key || has_key(member, key)),
        Value::Array(items) => items.iter().any(|item| has_key(item, key)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}
