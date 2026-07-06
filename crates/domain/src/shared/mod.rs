use rust_decimal::Decimal;
use thiserror::Error;

/// A weight of filament in grams. Non-negative by construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Grams(f64);

/// Monetary amount (prices). Decimal to avoid float drift.
pub type Money = Decimal;

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
