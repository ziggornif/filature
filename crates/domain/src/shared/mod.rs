use rust_decimal::Decimal;
use thiserror::Error;

/// A weight of filament in grams. Non-negative by construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Grams(f64);

/// Monetary amount (prices). Decimal to avoid float drift. Non-negative by
/// construction, mirroring `Grams`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money(Decimal);

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
    #[error("weight must be finite, got {0}")]
    NonFiniteWeight(f64),
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
    #[error("price must not be negative, got {0}")]
    NegativeMoney(Decimal),
    #[error("spool is archived and cannot be modified")]
    SpoolArchived,
    #[error("remaining weight must not exceed net weight")]
    RemainingAboveNet,
    #[error("spool is already archived")]
    SpoolAlreadyArchived,
    #[error("spool is not archived")]
    SpoolNotArchived,
}

impl Grams {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !value.is_finite() {
            return Err(DomainError::NonFiniteWeight(value));
        }
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

impl Money {
    /// Mirrors `Decimal::new(num, scale)`, rejecting a negative result.
    pub fn new(num: i64, scale: u32) -> Result<Self, DomainError> {
        Self::from_decimal(Decimal::new(num, scale))
    }

    pub fn from_decimal(d: Decimal) -> Result<Self, DomainError> {
        if d < Decimal::ZERO {
            return Err(DomainError::NegativeMoney(d));
        }
        Ok(Self(d))
    }

    pub fn value(self) -> Decimal {
        self.0
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
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
    fn grams_rejects_infinity() {
        assert!(matches!(
            Grams::new(f64::INFINITY),
            Err(DomainError::NonFiniteWeight(v)) if v == f64::INFINITY
        ));
    }

    #[test]
    fn grams_rejects_nan() {
        assert!(Grams::new(f64::NAN).is_err());
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

    #[test]
    fn money_rejects_negative() {
        assert_eq!(
            Money::new(-1, 0),
            Err(DomainError::NegativeMoney(Decimal::new(-1, 0)))
        );
        assert!(Money::from_decimal(Decimal::new(-5, 2)).is_err());
    }

    #[test]
    fn money_accepts_non_negative_and_displays() {
        let m = Money::from_decimal(Decimal::from_str_exact("24.99").unwrap()).unwrap();
        assert_eq!(m.to_string(), "24.99");
        assert_eq!(m.value(), Decimal::from_str_exact("24.99").unwrap());
    }
}
