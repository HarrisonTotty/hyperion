//! The seam between the surface service and plan 14: where a body's coarse-pass inputs come from
//! (rendering plan R09, Provides: `InputsSource`), with the production source, [`RecordInputs`],
//! which serves a generated body's inputs from generator version 25, when its serve gate opened
//! (P14.T54.a).

use std::fmt;

use hyperion_sim::planetary::SystemContext;
use hyperion_sim::planetary::record::BodyRecord;
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError};

/// Where the surface service takes a body's coarse-pass inputs from.
///
/// [`SurfaceService`](super::SurfaceService) calls it on a pool worker, once for each field it
/// computes, with the body's full record at [`RECORD_TIME`](super::RECORD_TIME), a moon's
/// planet's full record taken with it, and its system's context: the arguments of
/// [`CoarseInputs::for_body`] (R09.T10.f), so that every source reads records into inputs.
/// Production uses [`RecordInputs`]; most of the server's tests give the coarse pass's reference
/// worlds instead, worlds of known make from a level-5 field up, which were the only fields the
/// server computed until the serve gate opened at generator version 25 (P14.T54.a).
pub trait InputsSource: fmt::Debug + Send + Sync {
    /// The coarse-pass inputs of the body of `record`, in the system of `ctx`, with `planet` the
    /// record of the planet it orbits if it is a moon, `Some` exactly where
    /// [`CoarseInputs::planet_of`] is.
    ///
    /// # Errors
    ///
    /// [`ReadCoarseInputsError::Section`] for the body's own state: with
    /// [`NoSolidSurface`](hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface)
    /// for a body with no solid surface, and
    /// [`NotModelled`](hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled) or
    /// [`NotResolved`](hyperion_sim::planetary::surface::SurfaceInputsError::NotResolved) naming a
    /// section the pass reads that the record does not model or withholds. The service passes
    /// full records, so the last is a bug there, as is every other variant, a fault of the
    /// service's own reading (a planet's record not given or not the moon's, a record not taken
    /// at the record's time, inputs the builder refuses).
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError>;

    /// Whether the source may give any body's inputs: `false` only for one that answers every body
    /// not modelled, so that a system's prefetch reads no record for nothing (rendering plan R09,
    /// R09.T17). [`RecordInputs`] models them, the serve gate open from generator version 25
    /// (P14.T54.a).
    fn models_any_body(&self) -> bool {
        true
    }
}

/// The production source: the body's record, with a moon's planet's, read by
/// [`CoarseInputs::for_body`], whose answer it gives.
///
/// A generated body with a solid surface is served its field wherever `for_body` reads it, and is
/// otherwise answered `for_body`'s refusal: [`NoSolidSurface`] for a giant, and [`NotModelled`]
/// naming the section where the record leaves a gap the pass reads. That is the serve gate open,
/// from generator version 25 (plan 14's P14.T54.a, "The serve gate"; decision-bump-plan-after-22
/// §1.4). Until then this source answered every body `for_body` read, and every refusal generator
/// version 22 never gave, [`NotModelled`] naming the surface section, so that R09's and R10's
/// changes to the coarse pass, its quantisation, its synthesis and the payload re-blessed at the
/// version in force with no bump.
///
/// [`NoSolidSurface`]: hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface
/// [`NotModelled`]: hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordInputs;

impl InputsSource for RecordInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        CoarseInputs::for_body(record, planet, ctx)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    use hyperion_sim::galaxy::placement::CellKey;
    use hyperion_sim::id::{BodyId, Layer};
    use hyperion_sim::planetary::derive::PlanetClass;
    use hyperion_sim::planetary::generate;
    use hyperion_sim::planetary::hooks::surface_seed;
    use hyperion_sim::planetary::record::{BodyKind, BulkProperties, Section};
    use hyperion_sim::planetary::surface::{SurfaceInputsError, coarse_pass};
    use hyperion_sim::units::{Dex, SolarMasses, Years};
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_surface::field::{CoarseField, Cover, CoverRange, FieldView, ResolutionCode};
    use hyperion_surface::wire::encode_payload;
    use tokio::time::timeout;

    use super::*;
    use crate::compute::{CpuPool, GalaxyKey, GeneratedSystem};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
    use crate::surface::{FieldKey, RECORD_TIME, SurfaceJobError, SurfaceService};
    use crate::testing::WAIT;

    /// The seed of the universe these tests generate systems in.
    const SEED: u64 = 0x5eed_0009_0010;

    /// The whole field as the wire carries it, every cell surveyed: the encoder writes each value's
    /// bits, so two fields that encode alike are one field, bit for bit.
    fn payload(field: &CoarseField) -> Vec<u8> {
        let cells = field.header().level().cell_count();
        let all = Cover::from_ranges([CoverRange::new(0, cells, ResolutionCode::new(1)).unwrap()])
            .unwrap();
        encode_payload(field, &all, None)
    }

    /// A generated rocky planet whose record the sim reads `Ok` is served its surface: the field
    /// the service computes through [`RecordInputs`] is the sim's own coarse pass of the body's
    /// surface seed on `for_body`'s inputs, bit for bit, under the albedo scale the service sets;
    /// and a giant still answers that it has no solid surface, and runs no pass. One field is
    /// computed, since each pass is dear: the smallest rocky planet's of the systems up to the
    /// first that holds a giant.
    #[tokio::test]
    async fn a_generated_body_the_sim_reads_is_served_its_surface() {
        let cell = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell in range");
        let mut rocky: Option<(BodyId, CoarseInputs, Arc<GeneratedSystem>)> = None;
        let mut giant: Option<(BodyId, Arc<GeneratedSystem>)> = None;
        for index in 1..16 {
            let ctx = SystemContext::builder()
                .system(cell.candidate_id(index).expect("an index in range"))
                .star(SolarMasses::new(1.0))
                .fe_h(Dex::ZERO)
                .age_at_epoch(Years::new(4.57e9))
                .build()
                .expect("a Sun-like context");
            let planets = generate(Seed::new(SEED), &ctx);
            let system = Arc::new((ctx, planets));
            let (ctx, planets) = system.as_ref();
            for record in planets.snapshot_at(ctx, RECORD_TIME).bodies() {
                if !matches!(record.identity().kind(), BodyKind::Planet) {
                    continue;
                }
                let id = record.identity().id();
                match (
                    record.surface(),
                    record.bulk().ok().map(BulkProperties::class),
                ) {
                    (Section::Ok(_), Some(PlanetClass::Rocky)) => {
                        let Ok(inputs) = CoarseInputs::for_body(record, None, ctx) else {
                            continue;
                        };
                        if rocky.as_ref().is_none_or(|(_, smallest, _)| {
                            inputs.radius().value() < smallest.radius().value()
                        }) {
                            rocky = Some((id, inputs, Arc::clone(&system)));
                        }
                    }
                    (Section::NotApplicable, Some(PlanetClass::GasGiant)) => {
                        giant = giant.or_else(|| Some((id, Arc::clone(&system))));
                    }
                    _ => {}
                }
            }
            if rocky.is_some() && giant.is_some() {
                break;
            }
        }
        let (Some((planet, inputs, of)), Some((giant, giant_of))) = (rocky, giant) else {
            panic!("15 Sun-like systems hold no rocky planet the sim reads, or no giant");
        };

        let pool = Arc::new(
            CpuPool::new(
                NonZeroUsize::MIN,
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .unwrap(),
        );
        let service = SurfaceService::new(
            Arc::clone(&pool),
            Arc::new(RecordInputs),
            64 << 20,
            NonZeroUsize::MIN,
        );
        let key = |body| FieldKey::new(GalaxyKey::new(SEED, GENERATOR_VERSION), body);
        let served = timeout(WAIT, service.field(key(planet), &of))
            .await
            .expect("timed out computing a field")
            .expect("a generated rocky planet the sim reads is served its field");
        // The albedo scale is the service's own, not the pass's: its solve is checked by the
        // service's tests and `surface_service.rs`, each of which solves it again.
        let own = coarse_pass(surface_seed(Seed::new(SEED), planet), &inputs)
            .with_albedo_scale(served.header().albedo_scale())
            .unwrap();
        let (served, own) = (payload(&served), payload(&own));
        assert_eq!(served.len(), own.len(), "{planet}: the payloads' lengths");
        assert_eq!(
            served.iter().zip(&own).position(|(a, b)| a != b),
            None,
            "{planet}: the payloads' first differing byte"
        );

        let answer = timeout(WAIT, service.field(key(giant), &giant_of))
            .await
            .expect("timed out reading a giant's inputs");
        assert_eq!(
            answer.unwrap_err(),
            SurfaceJobError::Inputs(ReadCoarseInputsError::Section(
                SurfaceInputsError::NoSolidSurface
            )),
            "{giant}"
        );
        assert_eq!(service.counters().computed(), 1, "the planet's pass alone");
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }
}
