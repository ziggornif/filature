use crate::spools::model::{Colour, Diameter, SpoolId, SpoolStatus, remaining_length_m};
use rust_decimal::Decimal;
use thiserror::Error;

/// A weight of filament in grams. Non-negative by construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Grams(f64);

/// Monetary amount (prices). Decimal to avoid float drift.
pub type Money = Decimal;

/// Opaque identifier for a `Material`. Lives in the shared kernel (rather
/// than the `materials` slice) because other slices (e.g. `spools`)
/// reference a material by id — a cross-slice import of a sibling slice's
/// own module would violate slice isolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialId(pub String);

impl MaterialId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("weight must be non-negative, got {0}")]
    NegativeWeight(f64),
    #[error("density must be > 0 g/cm³, got {0}")]
    NonPositiveDensity(f64),
    #[error("unknown sensitivity: {0}")]
    UnknownSensitivity(String),
    #[error("material name must not be blank")]
    BlankMaterialName,
    #[error("invalid colour hex: {0}")]
    InvalidColour(String),
    #[error("unknown diameter: {0}")]
    UnknownDiameter(String),
    #[error("unknown spool status: {0}")]
    UnknownSpoolStatus(String),
}

impl Grams {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value < 0.0 {
            return Err(DomainError::NegativeWeight(value));
        }
        Ok(Self(value))
    }
    pub fn value(self) -> f64 {
        self.0
    }
    /// Remaining as a fraction of an initial (net) weight, 0.0..=1.0+.
    pub fn ratio_of(self, net: Grams) -> f64 {
        if net.0 <= 0.0 { 0.0 } else { self.0 / net.0 }
    }
}

/// Cross-slice read model for a spool-list row: the fields a UI list view
/// needs, joining a `Spool`'s own fields with the display-only material
/// name and density looked up by the persistence adapter. Lives in the
/// shared kernel (not the `spools` slice) so producing it never requires
/// `spools` to import from `materials` — a cross-slice import would
/// violate slice isolation. The adapter that implements `SpoolRepository`
/// is the one place that joins across the two tables.
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

/// Cross-slice read model for a spool detail view: all of a `Spool`'s own
/// fields plus the display-only material name and density. See
/// `SpoolListItem` for why this lives in `shared` rather than `spools`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grams_rejects_negative() {
        assert_eq!(Grams::new(-1.0), Err(DomainError::NegativeWeight(-1.0)));
    }

    #[test]
    fn grams_ratio_of_net() {
        let remaining = Grams::new(250.0).unwrap();
        let net = Grams::new(1000.0).unwrap();
        assert!((remaining.ratio_of(net) - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn grams_ratio_guards_zero_net() {
        assert_eq!(
            Grams::new(5.0).unwrap().ratio_of(Grams::new(0.0).unwrap()),
            0.0
        );
    }
}
