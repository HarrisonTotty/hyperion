//! The surface requests (rendering plan R09, R09.T19), added by plan 04's "Extending the
//! convention" (row R09): `surface_field`, a body's coarse field in the cells the ship has
//! surveyed, answered in bulk, and `survey_pass`, which records one survey pass of a body.
//!
//! # Cells, ranges and covers
//!
//! A body's coarse field is a cube-sphere grid at one level, 5 to 8 by the body's radius
//! ([`SurfaceHeaderDto::level`]), of 6 · 4ᴸ cells, 6,144 at level 5 to 393,216 at level 8. A cell
//! is named by its cell index, face-major and then in Morton order within the face, so that every
//! quadtree subtree is one run of indices. Sets of cells cross the wire as half-open runs of
//! indices, `start` to `end`, in order and not overlapping: a survey pass's
//! ([`CellRangeDto`]) and the cover a field's payload holds, each range with the resolution it was
//! surveyed at ([`CoverRangeDto`]).
//!
//! # The field
//!
//! A `surface_field` answer that is [`SurfaceFieldDto::Ready`] follows the payload's binary frames
//! (the crate docs lay out the frames). The payload is a sequence of self-contained blocks in
//! cell-index order, and block 0 of every payload carries the field's whole header, so a worker
//! can decode the payload as its blocks arrive. The layout is `hyperion-surface`'s `wire` module,
//! which the client runs as WebAssembly. The JSON answer repeats the header's scalars for the main
//! thread ([`SurfaceHeaderDto`]), says which cells the payload holds ([`CoverDto`]), and states the
//! revision of the body's coverage that it reflects. A body whose plan 14 sections the coarse pass
//! reads are not modelled yet answers [`SurfaceFieldDto::NotModelled`], with no bulk.
//!
//! # Revisions and deltas
//!
//! A body's coverage revision counts its survey passes: 0 before the first, and revision n is
//! always the fold of its first n passes, so a revision names one cover for good. A request with
//! no `have_revision` is answered with every surveyed cell and its margin; one naming the revision
//! a client holds is answered with the delta, the cells the client's revision did not hold or held
//! with another resolution code (a cell surveyed again more finely, or a margin cell since
//! surveyed). A delta's cells replace the client's copies, code and all, and the cells it leaves
//! out are unchanged. Asking with the revision answered last gives block 0 alone. The same request
//! of the same revision gives the same bytes.
//!
//! The scene topic names each body whose revision rises ([`SurfaceRevisionDto`], on the scene's
//! notifications as `surface_revisions`), so that a client keeping the body's field asks for the
//! delta. A subscription's state carries no revisions and a notification missed while
//! unsubscribed is not replayed, so a client that subscribes again asks `surface_field` with the
//! revision it holds for each body whose field it keeps.
//!
//! # The survey stand-in
//!
//! Until a sensors plan records passes from the ship's instruments, `survey_pass` lets an honest
//! client grant its ship coverage, as the server grants detail levels today. A pass carries at
//! most the server's `MAX_SURVEY_RANGES` (256) ranges, which keeps its frame under plan 04's
//! 16 KiB inbound limit; a longer pass is sent as several requests sharing its time span.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::bulk::BulkManifestDto;
use crate::primitives::{BodyIdHex, UniverseIdHex, UniverseTime};

/// Asks for one body's coarse field in the cells the ship has surveyed (`surface_field`), answered
/// in bulk with a [`SurfaceFieldDto`].
///
/// The server resolves the body as `body_detail` does: a system part that names no system of the
/// universe is `unknown_system`, an index outside plan 14's layout `bad_request`, and an index at
/// which the system holds no body `unknown_body`, each naming `body`. A body with no solid surface
/// (a giant, a ring, a belt or the halo) is `bad_request` naming `body`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurfaceFieldRequest {
    /// The universe the body is in.
    pub universe: UniverseIdHex,
    /// The body.
    pub body: BodyIdHex,
    /// The revision of the body's coverage the client holds, from an earlier answer: the answer
    /// then carries only the cells surveyed since. `null` (or 0) for the whole surveyed field. A
    /// revision above the body's own is `bad_request` naming `have_revision`.
    pub have_revision: Option<u32>,
}

/// A body's coarse field, the answer to `surface_field`, tagged by `state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum SurfaceFieldDto {
    /// The field's surveyed cells, with their margin, in the binary frames before this answer.
    Ready {
        // Boxed: it is most of the answer's size.
        /// The field's header, as block 0 of the payload carries it.
        header: Box<SurfaceHeaderDto>,
        /// The revision of the body's coverage the payload reflects: how many survey passes of the
        /// body are folded into it, 0 before the first. A later request names it as
        /// `have_revision`.
        revision: u32,
        /// The cells the payload holds, each range with the resolution it was surveyed at;
        /// [`CoverRangeDto::resolution`] 0 marks the margin about a survey, whose cells are sent
        /// whole and exact for the synthesis but were not surveyed. For a delta (a request with
        /// `have_revision`), the delta's cells alone, which replace the client's copies.
        coverage: CoverDto,
        /// The payload's manifest.
        bulk: BulkManifestDto,
    },
    /// The coarse pass reads a section of the body's record that this generator version does not
    /// model yet (the record's own `not_modelled`), so the body has no field yet. No binary frame
    /// precedes it.
    NotModelled {
        /// The section the pass needs.
        section: RecordSectionDto,
    },
}

/// A coarse field's header as the main thread reads it: the scalars of the payload's `FieldHeader`
/// (rendering plan R09, Design note 17), each in the unit its name states.
///
/// The band spectrum, the crater parameters and the palette's optical and mechanical values are
/// the synthesis's and travel in block 0 of the payload alone. Members may be added later; removing
/// one changes the contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurfaceHeaderDto {
    /// The body the field is of.
    pub body: BodyIdHex,
    /// The field's cube-sphere level, 5 to 8, which the body's radius gives: the shallowest whose
    /// mean cell is no wider than 40 km (Design note 4). The field has 6 · 4ᴸ cells.
    pub level: u8,
    /// The body's volumetric mean radius, m, positive: plan 14's, ∛(a² c) of its figure.
    pub radius_m: f64,
    /// The equatorial radius a of the body's rotational spheroid, m, positive: the datum every
    /// height of the field is measured from along its normal (R07's Design note 19).
    pub equatorial_radius_m: f64,
    /// The polar radius c of the same spheroid, m, positive and at most a.
    pub polar_radius_m: f64,
    /// The boundary diameter D<sub>b</sub>, m, positive: twice the level's largest cell edge, as
    /// 50.0 km on a Ceres at level 5 and 84.9 km on an Earth at level 8. Craters of
    /// D<sub>b</sub> and wider are the coarse field's, every narrower one the synthesis's
    /// (Design note 10).
    pub boundary_diameter_m: f64,
    /// The height of the sea surface above the datum, m, of either sign: the elevation below which
    /// lies plan 14's ocean fraction, and on a world with no sea the lowest cell's elevation.
    pub sea_level_m: f64,
    /// The rate at which the surface temperature falls with height, K/m, finite, positive for a
    /// fall: 0.0065 in Earth's standard atmosphere (ICAO 1993), 0 on an airless body.
    pub lapse_rate_k_per_m: f64,
    /// The realised hypsometric standard deviation σ<sub>h</sub> of the reconstructed field, m,
    /// non-negative: the area-weighted RMS elevation about its mean, which the pass holds to plan
    /// 14's.
    pub realised_sigma_h_m: f64,
    /// The realised greatest relief, m, non-negative: the highest elevation less the lowest,
    /// reported rather than matched, since a greatest relief grows with resolution.
    pub realised_relief_m: f64,
    /// The months of the body's year: 12, the twelve equal spans of the seasonal orbit's eccentric
    /// anomaly from periapsis, or 1 for a world with no seasonal forcing.
    pub months: u8,
    /// The eccentricity e of the seasonal orbit, the one that sets the sun's declination and
    /// distance: the body's own, or its planet's for a moon. At least 0 and below 1, and 0 in a
    /// one-month year.
    pub season_eccentricity: f64,
    /// The zero of the climate's sea-level temperatures, K, positive: plan 14's mean surface
    /// temperature, which the climate step imposes on the field.
    pub reference_temperature_k: f64,
    /// The body's surface age, Gyr (10⁹ years), non-negative and at most its system's age: plan
    /// 14's.
    pub surface_age_gyr: f64,
    /// The body's mean surface pressure, Pa, non-negative: plan 14's, 0 on an airless body.
    pub surface_pressure_pa: f64,
    /// Rendering plan R10's albedo scale, dimensionless and positive, or `null` until R10 fills it.
    pub albedo_scale: Option<f64>,
    /// The coarse model the climate step ran.
    pub climate_model: ClimateModelDto,
    /// Where the field's precipitation came from.
    pub precipitation: PrecipitationSourceDto,
    /// The substances the field names, in the payload's palette order, so that a cell's substance
    /// entry indexes this list: each a key of plan 14's substance registry with its role on the
    /// body. Empty for a body whose composition is not modelled.
    pub palette: Vec<PaletteEntryDto>,
}

/// The coarse model a body's climate step ran (rendering plan R09, Design note 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ClimateModelDto {
    /// A seasonal moist energy-balance model, for a world with a seasonal water cycle or one like
    /// it.
    EnergyBalance,
    /// The same model in tidally locked coordinates about the substellar axis, with no seasons.
    LockedEnergyBalance,
    /// Radiative equilibrium with thermal inertia, for an airless or thin-aired world.
    RadiativeEquilibrium,
    /// An isothermal surface under a thick atmosphere, as on Venus.
    Isothermal,
}

/// Where a field's precipitation came from, so that a console labels it (rendering plan R09,
/// Design note 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PrecipitationSourceDto {
    /// A documented monthly heuristic, after Siler, Roe and Armour (2018) and Hergarten and Robl
    /// (2022), labelled as a heuristic wherever it is shown: simple climate models cannot solve
    /// precipitation.
    Heuristic,
}

/// One substance of a body's palette.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PaletteEntryDto {
    /// The substance: a key of plan 14's substance registry, a formula (`H2O`, `Na2CO3`), the
    /// electron (`e-`) or a lowercase name (`basalt`, `mars_dust`).
    pub substance: String,
    /// What the substance is to the body.
    pub role: PaletteRoleDto,
}

/// What a palette entry is to the body (decision-composition §1.7), as `hyperion-surface`'s
/// `PaletteRole` names it, with its sources. The three crusts are Taylor's classes of planetary
/// crust (Taylor 1989, Tectonophysics 161, 147).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PaletteRoleDto {
    /// A primary crust, crystallised from the body's first global liquid, as the lunar highlands'
    /// feldspathic crust or a frozen ocean's shell.
    PrimaryCrust,
    /// A secondary crust, a solid mantle's partial melt, as basalt.
    SecondaryCrust,
    /// A tertiary crust, as the granite of Earth's continents.
    TertiaryCrust,
    /// The lithology of a stagnant lid's volcanic provinces where it is not the lid's own, as the
    /// lunar maria's basalt.
    Province,
    /// A condensate on the ground as ice or frost.
    Ice,
    /// A liquid on the ground: a sea's or a lake's.
    Liquid,
    /// A cover over the ground that is not its rock, as an organic cover.
    Cover,
    /// The source of a body's sand and dust where it is not the crust's, as a Mars's dust.
    Deposit,
    /// An undifferentiated body's own material where no crust covers it.
    Bulk,
}

/// A section of a body's record, as a `not_modelled` answer names it: the record's fields of the
/// same names (`mass` is the record's `mass_kg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RecordSectionDto {
    /// The label for people.
    Label,
    /// The mass.
    Mass,
    /// The orbit about the body's primary.
    Orbit,
    /// The body's moons.
    Moons,
    /// The body's rings.
    Rings,
    /// What a population body is and where it lies.
    Population,
    /// The bulk properties.
    Bulk,
    /// The surface.
    Surface,
    /// The hooks.
    Hooks,
    /// The photometry.
    Photometry,
    /// The figure.
    Figure,
    /// The rotation.
    Rotation,
    /// The envelope, a gas-envelope body's air (plan 14's P14.T48.e).
    ///
    /// No surface field names it: a body with an envelope has no solid surface, which the field
    /// refuses as a bad request.
    Envelope,
}

/// The cells a payload holds, each run with the resolution it was surveyed at. It travels from
/// the server to the client only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoverDto {
    /// Half-open runs of cell indices, in order and not overlapping; runs may meet where the
    /// resolution changes.
    pub ranges: Vec<CoverRangeDto>,
}

/// A half-open run of cell indices of a body's field with the resolution its cells were surveyed
/// at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoverRangeDto {
    /// The run's first cell index.
    pub start: u32,
    /// The index past its last, above `start`.
    pub end: u32,
    /// The run's resolution code (Design note 16): 0 for a margin cell, held but not surveyed, and
    /// c ≥ 1 for a survey no coarser than 1 cm × 2^((c − 1) ÷ 8), so that 1 is 1 cm or finer and
    /// the codes span 1 cm to about 36,000 km in steps of 9%. A smaller code above 0 is finer.
    pub resolution: u8,
}

/// A half-open run of cell indices of a body's field, as a survey pass covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CellRangeDto {
    /// The run's first cell index.
    pub start: u32,
    /// The index past its last, above `start` and at most the field's cell count.
    pub end: u32,
}

/// How a survey pass measured the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SurveySourceDto {
    /// From orbit.
    Orbital,
    /// From close range, below orbit.
    CloseRange,
    /// From a craft landed on the surface.
    Landed,
}

/// Records one survey pass of a body (`survey_pass`), answered with a [`SurveyPassDto`].
///
/// The body is resolved and refused as for a [`SurfaceFieldRequest`]. A field outside its range is
/// `bad_request` naming it: `from` or `to` outside the clock window, `to` before `from`, a
/// `resolution_m` that is not finite and positive or is coarser than about 36,000 km, and `cells`
/// that are empty, more than 256 ranges, out of order, overlapping, empty runs or past the field's
/// last cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurveyPassRequest {
    /// The universe the body is in.
    pub universe: UniverseIdHex,
    /// The body surveyed.
    pub body: BodyIdHex,
    /// When the pass began, within the clock window.
    pub from: UniverseTime,
    /// When it ended, within the clock window and no earlier than `from`.
    pub to: UniverseTime,
    /// How it measured.
    pub source: SurveySourceDto,
    /// How finely it measured, m: finite, positive, and no coarser than the coarsest resolution
    /// code's, about 36,000 km.
    pub resolution_m: f64,
    /// The cells it covered at the level of the body's field: at least one run, at most 256, in
    /// order and not overlapping (runs may meet).
    pub cells: Vec<CellRangeDto>,
}

/// A survey pass recorded: the body's new coverage revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurveyPassDto {
    /// The revision of the body's coverage with this pass folded in: how many passes of the body
    /// are recorded, from 1 after its first.
    pub revision: u32,
}

/// A body whose surveyed coverage grew, as the scene topic tells it (rendering plan R03's Design
/// note 4, R09.T19.b): a client that keeps the body's field asks `surface_field` again with the
/// revision it holds as `have_revision`, for the new cells alone.
///
/// A body's revision only rises, since revision n is the fold of the body's first n survey passes,
/// so a later notification never names a smaller revision for a body than an earlier one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SurfaceRevisionDto {
    /// The body surveyed.
    pub body: BodyIdHex,
    /// Its coverage's revision, from 1: how many survey passes of the body are recorded.
    pub revision: u32,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(42)
    }

    fn body() -> BodyIdHex {
        BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0103)
    }

    /// A Ceres-like body's header, at level 5.
    fn header() -> SurfaceHeaderDto {
        SurfaceHeaderDto {
            body: body(),
            level: 5,
            radius_m: 469_700.0,
            equatorial_radius_m: 482_000.0,
            polar_radius_m: 446_000.0,
            boundary_diameter_m: 50_000.0,
            sea_level_m: -2_500.5,
            lapse_rate_k_per_m: 0.0,
            realised_sigma_h_m: 2_300.25,
            realised_relief_m: 19_000.0,
            months: 12,
            season_eccentricity: 0.078,
            reference_temperature_k: 168.0,
            surface_age_gyr: 4.0,
            surface_pressure_pa: 0.0,
            albedo_scale: None,
            climate_model: ClimateModelDto::RadiativeEquilibrium,
            precipitation: PrecipitationSourceDto::Heuristic,
            palette: vec![
                PaletteEntryDto {
                    substance: "phyllosilicate".to_owned(),
                    role: PaletteRoleDto::PrimaryCrust,
                },
                PaletteEntryDto {
                    substance: "H2O".to_owned(),
                    role: PaletteRoleDto::Ice,
                },
            ],
        }
    }

    fn header_json() -> Value {
        json!({
            "body": "0200080020000000.0103",
            "level": 5,
            "radius_m": 469_700.0,
            "equatorial_radius_m": 482_000.0,
            "polar_radius_m": 446_000.0,
            "boundary_diameter_m": 50_000.0,
            "sea_level_m": -2_500.5,
            "lapse_rate_k_per_m": 0.0,
            "realised_sigma_h_m": 2_300.25,
            "realised_relief_m": 19_000.0,
            "months": 12,
            "season_eccentricity": 0.078,
            "reference_temperature_k": 168.0,
            "surface_age_gyr": 4.0,
            "surface_pressure_pa": 0.0,
            "albedo_scale": null,
            "climate_model": "radiative_equilibrium",
            "precipitation": "heuristic",
            "palette": [
                { "substance": "phyllosilicate", "role": "primary_crust" },
                { "substance": "H2O", "role": "ice" },
            ],
        })
    }

    #[test]
    fn surface_field_request_wire_form() {
        assert_wire_form(
            &SurfaceFieldRequest {
                universe: universe(),
                body: body(),
                have_revision: Some(3),
            },
            json!({
                "universe": "000000000000002a",
                "body": "0200080020000000.0103",
                "have_revision": 3,
            }),
        );
        assert_wire_form(
            &SurfaceFieldRequest {
                universe: universe(),
                body: body(),
                have_revision: None,
            },
            json!({
                "universe": "000000000000002a",
                "body": "0200080020000000.0103",
                "have_revision": null,
            }),
        );
    }

    #[test]
    fn surface_field_ready_wire_form() {
        let mut with_albedo = header();
        with_albedo.albedo_scale = Some(1.125);
        let mut with_albedo_json = header_json();
        with_albedo_json["albedo_scale"] = json!(1.125);
        assert_wire_form(&with_albedo, with_albedo_json);

        assert_wire_form(
            &SurfaceFieldDto::Ready {
                header: Box::new(header()),
                revision: 2,
                coverage: CoverDto {
                    ranges: vec![
                        CoverRangeDto {
                            start: 0,
                            end: 16,
                            resolution: 0,
                        },
                        CoverRangeDto {
                            start: 16,
                            end: 80,
                            resolution: 102,
                        },
                    ],
                },
                bulk: BulkManifestDto {
                    chunks: 1,
                    bytes: 4_096,
                },
            },
            json!({
                "state": "ready",
                "header": header_json(),
                "revision": 2,
                "coverage": {
                    "ranges": [
                        { "start": 0, "end": 16, "resolution": 0 },
                        { "start": 16, "end": 80, "resolution": 102 },
                    ],
                },
                "bulk": { "chunks": 1, "bytes": 4_096 },
            }),
        );
    }

    #[test]
    fn surface_field_not_modelled_wire_form() {
        assert_wire_form(
            &SurfaceFieldDto::NotModelled {
                section: RecordSectionDto::Surface,
            },
            json!({ "state": "not_modelled", "section": "surface" }),
        );
    }

    #[test]
    fn cover_and_cell_range_wire_forms() {
        assert_wire_form(&CoverDto { ranges: Vec::new() }, json!({ "ranges": [] }));
        assert_wire_form(
            &CellRangeDto {
                start: 393_088,
                end: 393_216,
            },
            json!({ "start": 393_088, "end": 393_216 }),
        );
    }

    #[test]
    fn survey_pass_request_wire_form() {
        assert_wire_form(
            &SurveyPassRequest {
                universe: universe(),
                body: body(),
                from: UniverseTime {
                    seconds: 3_600,
                    nanos: 0,
                },
                to: UniverseTime {
                    seconds: 9_000,
                    nanos: 250_000_000,
                },
                source: SurveySourceDto::CloseRange,
                resolution_m: 2.5,
                cells: vec![
                    CellRangeDto { start: 0, end: 96 },
                    CellRangeDto {
                        start: 1_000,
                        end: 1_001,
                    },
                ],
            },
            json!({
                "universe": "000000000000002a",
                "body": "0200080020000000.0103",
                "from": { "seconds": 3_600, "nanos": 0 },
                "to": { "seconds": 9_000, "nanos": 250_000_000 },
                "source": "close_range",
                "resolution_m": 2.5,
                "cells": [
                    { "start": 0, "end": 96 },
                    { "start": 1_000, "end": 1_001 },
                ],
            }),
        );
    }

    #[test]
    fn survey_pass_answer_wire_form() {
        assert_wire_form(&SurveyPassDto { revision: 7 }, json!({ "revision": 7 }));
    }

    #[test]
    fn surface_revision_wire_form() {
        assert_wire_form(
            &SurfaceRevisionDto {
                body: body(),
                revision: 4,
            },
            json!({ "body": "0200080020000000.0103", "revision": 4 }),
        );
    }

    #[test]
    fn surface_enum_strings() {
        assert_wire_strings(&[
            (SurveySourceDto::Orbital, "orbital"),
            (SurveySourceDto::CloseRange, "close_range"),
            (SurveySourceDto::Landed, "landed"),
        ]);
        assert_wire_strings(&[
            (ClimateModelDto::EnergyBalance, "energy_balance"),
            (
                ClimateModelDto::LockedEnergyBalance,
                "locked_energy_balance",
            ),
            (
                ClimateModelDto::RadiativeEquilibrium,
                "radiative_equilibrium",
            ),
            (ClimateModelDto::Isothermal, "isothermal"),
        ]);
        assert_wire_strings(&[(PrecipitationSourceDto::Heuristic, "heuristic")]);
        assert_wire_strings(&[
            (PaletteRoleDto::PrimaryCrust, "primary_crust"),
            (PaletteRoleDto::SecondaryCrust, "secondary_crust"),
            (PaletteRoleDto::TertiaryCrust, "tertiary_crust"),
            (PaletteRoleDto::Province, "province"),
            (PaletteRoleDto::Ice, "ice"),
            (PaletteRoleDto::Liquid, "liquid"),
            (PaletteRoleDto::Cover, "cover"),
            (PaletteRoleDto::Deposit, "deposit"),
            (PaletteRoleDto::Bulk, "bulk"),
        ]);
        assert_wire_strings(&[
            (RecordSectionDto::Label, "label"),
            (RecordSectionDto::Mass, "mass"),
            (RecordSectionDto::Orbit, "orbit"),
            (RecordSectionDto::Moons, "moons"),
            (RecordSectionDto::Rings, "rings"),
            (RecordSectionDto::Population, "population"),
            (RecordSectionDto::Bulk, "bulk"),
            (RecordSectionDto::Surface, "surface"),
            (RecordSectionDto::Hooks, "hooks"),
            (RecordSectionDto::Photometry, "photometry"),
            (RecordSectionDto::Figure, "figure"),
            (RecordSectionDto::Rotation, "rotation"),
            (RecordSectionDto::Envelope, "envelope"),
        ]);
    }

    #[test]
    fn an_unknown_surface_state_is_rejected() {
        let error =
            serde_json::from_value::<SurfaceFieldDto>(json!({ "state": "partial" })).unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `partial`"),
            "unexpected error: {error}"
        );
    }
}
