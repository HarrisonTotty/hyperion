//! The surface service's helpers, shared by the surface's integration tests (rendering plan R09,
//! R09.T17): the coarse pass's reference worlds as the service's inputs, and the generated systems
//! whose bodies the service computes fields for.
//!
//! Plan 14's surface section is not inhabited yet, so the production source answers every body
//! `NotModelled`; these tests give the service a reference world's inputs instead, through its
//! `InputsSource` seam, so that the service and the wire are tested before plan 14's asks land.

use std::sync::{Arc, OnceLock};

use hyperion_server::compute::GeneratedSystem;
use hyperion_server::surface::InputsSource;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
use hyperion_sim::id::{BodyId, Layer};
use hyperion_sim::planetary::record::{BodyRecord, Section};
use hyperion_sim::planetary::surface::{CoarseInputs, SurfaceInputsError, reference};
use hyperion_sim::planetary::{SystemContext, generate};
use hyperion_sim::time::UniverseTime;

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
        _: &SystemContext,
    ) -> Result<CoarseInputs, SurfaceInputsError> {
        match record.surface() {
            Section::NotApplicable => Err(SurfaceInputsError::NoSolidSurface),
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
