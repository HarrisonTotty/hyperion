//! The displaced classes as placement reads them: each class's density in light-years, its bound
//! over a cell, its systems in layers D and E, and the order the thinning sums them in (plan 08,
//! P08.T12.a and Design note 16).
//!
//! Plan 02 caps [`Fields`] at [`MAX_COMPONENTS`](crate::galaxy::fields::MAX_COMPONENTS) field
//! components in fixed arrays, so the hundred displaced classes are a list of their own,
//! [`DisplacedFields`], which a galaxy's fields carry once its build has installed them (the hold of
//! `DISPLACED_VERSION`) and which every sum over layers D and E reads
//! after the field components:
//!
//! - a class's density is its form ([`forms`](super::forms)) normalised to unit integral over the
//!   root cube, systems per cubic light-year at one system's weight;
//! - its weight in a band is its systems inside the cube there, its class weight times its source's
//!   band budget ([`ClassTable::class_count`], [`DisplacedClass::count`]), which the share matrix's
//!   displaced columns hold: a layer's density is then `Σ share × density` over the field
//!   components and the classes alike;
//! - its bound over a cell is its form's ([`bound`](super::bound), P08.T11);
//! - per band, the classes with weight there are summed in a fixed order, by descending weight
//!   across the galaxy and ties by class index (Design note 16): about a hundred in layer E and
//!   twenty in layer D. A class with weight in neither layer has no form and is left out.
//!
//! The class table is built from the field galaxy, the galaxy as it stands before its classes are
//! installed (`Galaxy::from_params`), once per galaxy and never as a static (decision-galaxy-25-train
//! §1.2, E2), and is kept here for the marks (P08.T12.c).

use super::bound::{form_bound, spheroid_bound};
use super::class_table::{CLASS_COUNT, ClassKey, ClassTable, FormTable};
use super::forms::{
    BuildFormError, ClassKinematics, CoredPowerLaw, DiscBornForm, FormPoint, young_disc,
};
use super::marks::kind_index;
use super::{DisplacedClassId, DisplacedKind};
use crate::galaxy::bounds::CellBox;
use crate::galaxy::fields::arms::ArmGeometry;
use crate::galaxy::fields::{ComponentId, Fields};
use crate::galaxy::imf::MassBand;
use crate::galaxy::map::MapSelection;
use crate::galaxy::{Galaxy, PointLy};

/// The two bands the classes have weight in, in their slots' order.
const BANDS: [MassBand; 2] = [MassBand::D, MassBand::E];

/// A displaced class's density form, in light-years, normalised to unit integral over the root
/// cube (plan 08, P08.T10).
#[derive(Debug, Clone, PartialEq)]
pub enum DisplacedForm {
    /// A thin-disc class: its ballistic layer in the two youngest age bins, its flared layer (with
    /// the blurred arm where it keeps one) and spheroid otherwise.
    Disc(Box<DiscBornForm>),
    /// An old source's class: its spheroid (the own-form part stays in the field, Design note 13).
    Spheroid(CoredPowerLaw),
}

impl DisplacedForm {
    /// The density at `p`, ly⁻³: unit integral over the root cube.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        match self {
            Self::Disc(form) => form.density(p),
            Self::Spheroid(spheroid) => spheroid.density(p),
        }
    }

    /// [`density`](Self::density), bit for bit, at the point `at` with what every form shares
    /// there (P08.T12's census).
    #[must_use]
    pub(crate) fn density_at(&self, at: &FormPoint<'_>) -> f64 {
        match self {
            Self::Disc(form) => form.density_at(at),
            Self::Spheroid(spheroid) => spheroid.density_at_radius(at.radius(), at.z()),
        }
    }

    /// The geometry of its arm, if it keeps one.
    #[must_use]
    pub(crate) fn arm_geometry(&self) -> Option<&ArmGeometry> {
        match self {
            Self::Disc(form) => form.arm_geometry(),
            Self::Spheroid(_) => None,
        }
    }

    /// An upper bound on [`density`](Self::density) over `cell`, ly⁻³ (P08.T11).
    #[must_use]
    pub fn bound(&self, cell: &CellBox) -> f64 {
        match self {
            Self::Disc(form) => form_bound(form, cell),
            Self::Spheroid(spheroid) => spheroid_bound(spheroid, cell),
        }
    }

    /// The column through the root cube's height at `(x, y)` (ly), ly⁻², as the map's face-on view
    /// reads it: the layers' columns in closed form (each times its arm factor at the point; a
    /// layer that flares to the cube's face by a 32-node rule), and the spheroid's by a 32-node
    /// rule.
    #[must_use]
    pub(crate) fn face_on_column(&self, x: f64, y: f64) -> f64 {
        let r = (x * x + y * y).sqrt();
        let arm_at =
            |arm: &crate::galaxy::fields::arms::SharpArm| arm.factor(&arm.geometry().point(x, y));
        let form = match self {
            Self::Spheroid(spheroid) => return spheroid.normalised_column(r),
            Self::Disc(form) => &**form,
        };
        match form {
            DiscBornForm::Ballistic(layer) => {
                layer.normalised_column(r) * layer.arm().map_or(1.0, arm_at)
            }
            DiscBornForm::Fitted {
                layer,
                arm,
                spheroid,
            } => {
                let flat = layer.as_ref().map_or(0.0, |(w, l)| {
                    w * l.normalised_column(r) * arm.as_ref().map_or(1.0, arm_at)
                });
                flat + spheroid
                    .as_ref()
                    .map_or(0.0, |(w, s)| w * s.normalised_column(r))
            }
        }
    }

    /// The smallest scale, ly, on which the density falls in z anywhere: the map's edge-on view
    /// panels a pixel's height by it.
    #[must_use]
    pub(crate) fn vertical_scale(&self) -> f64 {
        let form = match self {
            Self::Spheroid(spheroid) => return spheroid.vertical_scale(),
            Self::Disc(form) => &**form,
        };
        match form {
            DiscBornForm::Ballistic(layer) => layer.height(),
            DiscBornForm::Fitted {
                layer, spheroid, ..
            } => {
                let flat = layer
                    .as_ref()
                    .map_or(f64::INFINITY, |(_, l)| l.scale_height(0.0));
                let round = spheroid
                    .as_ref()
                    .map_or(f64::INFINITY, |(_, s)| s.vertical_scale());
                flat.min(round)
            }
        }
    }

    /// Whether the density depends on azimuth: a class that keeps the young disc's arm.
    #[must_use]
    pub(crate) fn has_arm(&self) -> bool {
        match self {
            Self::Disc(form) => form.arm().is_some(),
            Self::Spheroid(_) => false,
        }
    }
}

/// One displaced class as placement reads it: its key, its form and its systems inside the cube in
/// layers D and E.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplacedClass {
    key: ClassKey,
    form: Option<DisplacedForm>,
    /// `[D, E]`, systems inside the root cube.
    count: [f64; 2],
    /// Layer E's systems of the class that are remnants, inside the root cube.
    remnants_e: f64,
    /// Its members' velocities in units of its speed scale, plan 15's row's; `None` for the
    /// hypervelocity survivors, whose row gives none.
    kinematics: Option<ClassKinematics>,
}

impl DisplacedClass {
    /// Its members' mean rotation, dispersions and outbound share, in units of its source's speed
    /// scale (`v_c`, the nuclear disc's own for its classes): plan 15's row's (Design note 22,
    /// P08.T12.d); `None` for the hypervelocity survivors.
    #[must_use]
    pub fn kinematics(&self) -> Option<&ClassKinematics> {
        self.kinematics.as_ref()
    }

    /// What the class is.
    #[must_use]
    pub fn key(&self) -> ClassKey {
        self.key
    }

    /// Its form, or `None` for a class with weight in neither layer, which is left out.
    #[must_use]
    pub fn form(&self) -> Option<&DisplacedForm> {
        self.form.as_ref()
    }

    /// Its systems inside the root cube in `band`: its class weight times its source's band budget
    /// in layers D and E, and 0 in every other band.
    #[must_use]
    pub fn count(&self, band: MassBand) -> f64 {
        band_slot(band).map_or(0.0, |b| self.count[b])
    }
}

/// The displaced classes of one galaxy, as placement, the census and the map read them (module
/// documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplacedFields {
    /// Every class of the table, in [`ClassKey::id`]'s order, so that a [`DisplacedClassId`]
    /// indexes both.
    classes: Vec<DisplacedClass>,
    /// `[D, E]`: the classes with weight in the band, by descending count, ties by index.
    order: [Box<[DisplacedClassId]>; 2],
    /// Per field component, the share of its systems, every band together, that its field keeps:
    /// the map's weight of the component.
    kept: Box<[f64]>,
    table: Box<ClassTable>,
}

impl DisplacedFields {
    /// The classes of `galaxy` under plan 15's form table `forms`: the class table built from the
    /// galaxy, each class with weight in layer D or E given its form, and the two layers' orders.
    ///
    /// `galaxy` is the field galaxy, whose classes are not installed yet: its share matrix holds the
    /// populations' whole band shares, which are the budgets the class table splits.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] if a class with weight has a form whose parameters are out of range or
    /// whose integral over the cube is not positive and finite.
    ///
    /// # Panics
    ///
    /// Never: the table has 97 classes, whose indices fit a `u16`, and every old source has its
    /// rows.
    pub(crate) fn new(galaxy: &Galaxy, forms: &FormTable) -> Result<Self, BuildFormError> {
        let table = ClassTable::build(galaxy, forms);
        let fields = galaxy.fields();
        let scales = table.scales();
        let (young, young_arm) = young_disc(fields);
        let rows = forms.rows();
        let mut classes = Vec::with_capacity(CLASS_COUNT);
        for (index, entry) in table.classes().iter().enumerate() {
            let id = DisplacedClassId::new(u16::try_from(index).expect("under 97 classes"));
            let count = BANDS.map(|band| table.class_count(band, id));
            let weighted = count.iter().any(|&c| c > 0.0);
            let form = if weighted {
                Some(match entry.key() {
                    ClassKey::Thin { speed, age } => {
                        let row = &rows.disc_born[speed.index()][age.index()];
                        DisplacedForm::Disc(Box::new(DiscBornForm::new(
                            &row.layer,
                            &row.spheroid,
                            age,
                            entry.mean_ut(),
                            (young, &young_arm),
                            scales,
                        )?))
                    }
                    ClassKey::Old { source, speed } => {
                        let row = &rows
                            .old(source.source())
                            .expect("an old source has its rows")[speed.index()];
                        DisplacedForm::Spheroid(CoredPowerLaw::new(&row.spheroid, scales)?)
                    }
                    ClassKey::Hypervelocity => {
                        DisplacedForm::Spheroid(CoredPowerLaw::new(&rows.hypervelocity, scales)?)
                    }
                })
            } else {
                None
            };
            let remnants_e =
                count[1] * entry.marks(MassBand::E).kind_odds()[kind_index(DisplacedKind::Remnant)];
            let kinematics = match entry.key() {
                ClassKey::Thin { speed, age } => {
                    Some(rows.disc_born[speed.index()][age.index()].kinematics)
                }
                ClassKey::Old { source, speed } => rows
                    .old(source.source())
                    .map(|old| old[speed.index()].kinematics),
                ClassKey::Hypervelocity => None,
            };
            classes.push(DisplacedClass {
                key: entry.key(),
                form,
                count,
                remnants_e,
                kinematics,
            });
        }
        let order = [0, 1].map(|b| {
            let mut ids: Vec<DisplacedClassId> = (0..classes.len())
                .filter(|&i| classes[i].count[b] > 0.0)
                .map(|i| DisplacedClassId::new(u16::try_from(i).expect("under 97 classes")))
                .collect();
            ids.sort_by(|a, c| {
                classes[c.index()].count[b]
                    .total_cmp(&classes[a.index()].count[b])
                    .then(a.cmp(c))
            });
            ids.into_boxed_slice()
        });
        let shares = galaxy.shares();
        let kept = fields
            .component_ids()
            .map(|c| {
                let population = fields.component(c).population();
                BANDS.iter().fold(1.0, |kept, &band| {
                    kept - shares.share(band, population) * (1.0 - table.budget_stay_share(band, c))
                })
            })
            .collect();
        Ok(Self {
            classes,
            order,
            kept,
            table: Box::new(table),
        })
    }

    /// Every class, in the class table's order ([`ClassKey::id`]): a [`DisplacedClassId`] indexes
    /// it.
    #[must_use]
    pub fn classes(&self) -> &[DisplacedClass] {
        &self.classes
    }

    /// The class `id`.
    ///
    /// # Panics
    ///
    /// If `id` is not below the table's 97 classes.
    #[must_use]
    pub fn class(&self, id: DisplacedClassId) -> &DisplacedClass {
        &self.classes[id.index()]
    }

    /// The classes with weight in `band`, in the order every sum over them takes (Design note 16):
    /// by descending systems inside the cube, ties by class index. Empty outside layers D and E.
    #[must_use]
    pub fn order(&self, band: MassBand) -> &[DisplacedClassId] {
        band_slot(band).map_or(&[], |b| &self.order[b])
    }

    /// Class `id`'s density at `p`, systems per cubic light-year at one system's weight: its form,
    /// normalised over the root cube; 0 for a class with no form.
    ///
    /// # Panics
    ///
    /// If `id` is not below the table's 97 classes.
    #[must_use]
    pub fn density(&self, id: DisplacedClassId, p: &PointLy) -> f64 {
        self.classes[id.index()]
            .form
            .as_ref()
            .map_or(0.0, |form| form.density(p))
    }

    /// [`density`](Self::density), bit for bit, at the point `at` with what every form shares
    /// there: a caller reading several classes at one point finds its radius and arm point once.
    ///
    /// # Panics
    ///
    /// If `id` is not below the table's 97 classes.
    #[must_use]
    pub(crate) fn density_at(&self, id: DisplacedClassId, at: &FormPoint<'_>) -> f64 {
        self.classes[id.index()]
            .form
            .as_ref()
            .map_or(0.0, |form| form.density_at(at))
    }

    /// An upper bound on class `id`'s [`density`](Self::density) over `cell`, at one system's
    /// weight; 0 for a class with no form. The class's systems inside the cube multiply both
    /// through the share matrix's displaced column.
    ///
    /// # Panics
    ///
    /// If `id` is not below the table's 97 classes.
    #[must_use]
    pub fn bound(&self, id: DisplacedClassId, cell: &CellBox) -> f64 {
        self.classes[id.index()]
            .form
            .as_ref()
            .map_or(0.0, |form| form.bound(cell))
    }

    /// The class table the classes were built from (P08.T9), whose marks P08.T12.c draws.
    #[must_use]
    pub fn class_table(&self) -> &ClassTable {
        &self.table
    }

    /// The share of field component `c`'s systems, every band together, that its field keeps: 1
    /// less, for each of layers D and E, the band's share of the population times the share of the
    /// component's band budget that leaves the field. The map weighs the component by it.
    #[must_use]
    pub(crate) fn kept_share(&self, c: ComponentId) -> f64 {
        self.kept[c.index()]
    }

    /// Class `id`'s systems inside the cube that `selection` counts: in layers D and E together for
    /// every system, and layer E's remnants alone for [`MapSelection::LayerERemnants`]; `None` for
    /// the young disc alone, which counts no class.
    #[must_use]
    pub(crate) fn map_count(&self, id: DisplacedClassId, selection: MapSelection) -> Option<f64> {
        let class = &self.classes[id.index()];
        match selection {
            MapSelection::AllSystems => Some(class.count[0] + class.count[1]),
            MapSelection::LayerERemnants => Some(class.remnants_e),
            MapSelection::YoungOnly => None,
        }
    }

    /// The share matrix's part of the installation: each field component's stay share `[D, E]`, in
    /// component order, and each class's systems inside the cube `[D, E]`, in class order.
    #[must_use]
    pub(crate) fn share_columns(&self, fields: &Fields) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
        let stay = fields
            .component_ids()
            .map(|c| BANDS.map(|band| self.table.budget_stay_share(band, c)))
            .collect();
        let counts = self.classes.iter().map(|class| class.count).collect();
        (stay, counts)
    }

    /// The bytes the classes own on the heap: the list, the orders and the class table's
    /// conditional marks, approximately.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.classes.capacity() * size_of::<DisplacedClass>()
            + self
                .order
                .iter()
                .map(|o| o.len() * size_of::<DisplacedClassId>())
                .sum::<usize>()
            + self.kept.len() * size_of::<f64>()
            + size_of::<ClassTable>()
            + self.table.heap_bytes()
    }
}

/// The slot of band D (0) or E (1).
fn band_slot(band: MassBand) -> Option<usize> {
    match band {
        MassBand::D => Some(0),
        MassBand::E => Some(1),
        MassBand::A | MassBand::B | MassBand::C | MassBand::BrownDwarf | MassBand::RoguePlanet => {
            None
        }
    }
}
