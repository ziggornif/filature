use crate::shared::{Grams, MaterialId, Money};
use crate::spools::model::{Colour, Diameter, SpoolId, SpoolStatus, remaining_length_m};

/// Cross-adapter read model for a spool-list row: the fields a UI list view
/// needs, joining a `Spool`'s own fields with the display-only material
/// name and density looked up by the persistence adapter. Lives in the
/// `spools` slice (not the shared kernel) — it carries the material fields
/// as plain primitives (`material_name`, `density`) rather than importing
/// the `materials` slice, so slice isolation holds. The adapter that
/// implements `SpoolRepository` is the one place that joins across the two
/// tables.
#[derive(Debug, Clone, PartialEq)]
pub struct SpoolListItem {
    pub id: SpoolId,
    pub material_name: String,
    pub colour: Colour,
    pub diameter: Diameter,
    pub remaining_weight: Grams,
    pub net_weight: Grams,
    pub status: SpoolStatus,
    pub density: f64,
    pub location_name: Option<String>,
}

impl SpoolListItem {
    /// Remaining weight as a fraction of net weight (0.0..=1.0+).
    pub fn remaining_ratio(&self) -> f64 {
        self.remaining_weight.ratio_of(self.net_weight)
    }

    /// Estimated remaining filament length in metres.
    pub fn remaining_length_m(&self) -> f64 {
        remaining_length_m(self.remaining_weight, self.density, self.diameter)
    }
}

/// Cross-adapter read model for a spool detail view: all of a `Spool`'s own
/// fields plus the display-only material name and density. See
/// `SpoolListItem` for why this lives in `spools` rather than `shared`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpoolDetail {
    pub id: SpoolId,
    pub material_id: MaterialId,
    pub material_name: String,
    pub colour: Colour,
    pub diameter: Diameter,
    pub net_weight: Grams,
    pub remaining_weight: Grams,
    pub price_paid: Money,
    pub status: SpoolStatus,
    pub density: f64,
    pub location_name: Option<String>,
    /// The assigned location's id (`None` when unassigned) — carried
    /// alongside the display-only `location_name` so a web edit form can
    /// preselect the current location on the rendered `<select>`.
    pub location_id: Option<String>,
}

impl SpoolDetail {
    /// Remaining weight as a fraction of net weight (0.0..=1.0+).
    pub fn remaining_ratio(&self) -> f64 {
        self.remaining_weight.ratio_of(self.net_weight)
    }

    /// Estimated remaining filament length in metres.
    pub fn remaining_length_m(&self) -> f64 {
        remaining_length_m(self.remaining_weight, self.density, self.diameter)
    }
}
