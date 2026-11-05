//! The intensity a merger class thins on (plan 11, P11.T8.c–d): its row of [`CLASS_SHARES`] over
//! the density components, as the accreting white dwarfs' is (P11.T8.a).
//!
//! The intensity at x is `Σ_c w_c n_c(x)`: component c's density of systems, times its hosts per
//! system, `w_c` = the row's hosts per solar mass formed in its population × the mass formed per
//! system × its born fraction, so that the hosts over the whole component are the row's rate × its
//! born systems' formed mass, as the share matrix counts them, and from generator version 27 times
//! the share the catalogue keeps, all but the features' (P11.T8.f). For the two merger classes a
//! host is a merger inside the source horizon. A cell's bound is `Σ_c w_c × bound_c`, each component's
//! density bound over the cell's closed box, and a candidate's site is thinned on the intensity
//! there ÷ the bound, the same mark picking its component.

use super::super::grid::site_stream;
use super::feature::catalogue_share;
use crate::coords::GalacticPosition;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::CatalogueSystemId;
use crate::rng::DomainTag;
use crate::stellar::binary::CarvedClass;
#[cfg(doc)]
use crate::tables::binary::CLASS_SHARES;
use crate::units::PerCubicLightYear;

/// One density component as a merger class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostComponent {
    id: ComponentId,
    hosts_per_system: f64,
}

impl HostComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its hosts per system of its density: the class's hosts per solar mass formed in its
    /// population × the mass formed per system × its born fraction.
    #[must_use]
    pub const fn hosts_per_system(&self) -> f64 {
        self.hosts_per_system
    }
}

/// A merger class's hosts over the density components: its intensity, the bound over a cell and
/// a candidate's thinned site (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassHosts {
    class: ClassId,
    components: Vec<HostComponent>,
}

impl ClassHosts {
    /// The hosts of `class` in `galaxy`, from the row of `carved` of the galaxy's share table
    /// ([`CLASS_SHARES`] below 29).
    #[must_use]
    pub(crate) fn new(galaxy: &Galaxy, class: ClassId, carved: CarvedClass) -> Self {
        let row = galaxy.rules().binary_tables.class_shares().row(carved);
        let formed = galaxy.mean_formed_mass().value();
        let fields = galaxy.fields();
        let components = fields
            .component_ids()
            .filter_map(|id| {
                let component = fields.component(id);
                let hosts = row.hosts_per_formed_mass(component.population())
                    * formed
                    * component.ages().born_fraction()
                    * catalogue_share(galaxy, row, component);
                (hosts > 0.0).then_some(HostComponent {
                    id,
                    hosts_per_system: hosts,
                })
            })
            .collect();
        Self { class, components }
    }

    /// The class in plan 09's registry.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.class
    }

    /// The components it thins on, in component order.
    #[must_use]
    pub fn components(&self) -> &[HostComponent] {
        &self.components
    }

    /// The class's expected hosts at `point`, per cubic light-year: the intensity `Σ_c w_c n_c`,
    /// before the class's test.
    #[must_use]
    pub fn hosts_density(&self, galaxy: &Galaxy, point: &PointLy) -> PerCubicLightYear {
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, point, &mut weights);
        PerCubicLightYear::new(weights[..n].iter().fold(0.0, |sum, w| sum + w))
    }

    /// The class's expected hosts in the whole galaxy, before its test: each component's hosts per
    /// system × the integral of its density.
    #[must_use]
    pub fn expected_hosts(&self, galaxy: &Galaxy) -> f64 {
        let fields = galaxy.fields();
        self.components.iter().fold(0.0, |sum, c| {
            sum + c.hosts_per_system * fields.component(c.id).count_with_unborn()
        })
    }

    /// Σ, over the components in order, its hosts per system × its density bound over the cell's
    /// closed box: each term bounds the matching weight of the site's thinning.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    #[must_use]
    pub fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let mut bounds = [0.0; MAX_COMPONENTS];
        galaxy.fields().component_bounds(&cube, &mut bounds);
        PerCubicLightYear::new(self.components.iter().fold(0.0, |sum, c| {
            sum + bounds[c.id.index()] * c.hosts_per_system
        }))
    }

    /// Candidate `id`'s site in `cell`, thinned on the intensity there ÷ `bound`, with the
    /// component the same mark picks, on `tag`: words 0–2 the site, word 3 the mark. `None` if
    /// the thinning rejects it.
    ///
    /// # Panics
    ///
    /// If `cell` is not a cell of the class.
    #[must_use]
    pub(crate) fn site(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
        tag: DomainTag,
    ) -> Option<(GalacticPosition, ComponentId)> {
        assert_eq!(
            cell.class(),
            self.class,
            "a merger candidate is of its own class's cell"
        );
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let (site, mut stream) = site_stream(galaxy, cell, id, tag);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &PointLy::from(&site), &mut weights);
        let picked = stream.mark().pick_weighted(&weights[..n], bound.value())?;
        Some((site, self.components[picked].id))
    }

    /// The intensity's terms at `point` into `out`, in the components' order, hosts per cubic
    /// light-year.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, c) in out.iter_mut().zip(&self.components) {
            *slot = c.hosts_per_system * fields.component(c.id).density(point);
        }
        self.components.len()
    }
}
