use crate::shared::DomainError;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    Low,
    Medium,
    High,
}

impl Sensitivity {
    pub fn humidity_threshold_pct(self) -> u8 {
        match self {
            Sensitivity::Low => 40,
            Sensitivity::Medium => 30,
            Sensitivity::High => 15,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Sensitivity::Low => "Low",
            Sensitivity::Medium => "Medium",
            Sensitivity::High => "High",
        }
    }
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "Low" => Ok(Sensitivity::Low),
            "Medium" => Ok(Sensitivity::Medium),
            "High" => Ok(Sensitivity::High),
            other => Err(DomainError::UnknownSensitivity(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Density(f64);
impl Density {
    pub fn new(v: f64) -> Result<Self, DomainError> {
        if v > 0.0 {
            Ok(Self(v))
        } else {
            Err(DomainError::NonPositiveDensity(v))
        }
    }
    pub fn value(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temperature(u16);
impl Temperature {
    pub fn new(v: u16) -> Self {
        Self(v)
    }
    pub fn value(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DryingParams {
    pub temp: Temperature,
    pub time_h: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub id: MaterialId,
    pub name: String,
    pub density: Density,
    pub drying: DryingParams,
    pub sensitivity: Sensitivity,
    pub nozzle: Temperature,
    pub bed: Temperature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewMaterial {
    pub name: String,
    pub density: Density,
    pub drying: DryingParams,
    pub sensitivity: Sensitivity,
    pub nozzle: Temperature,
    pub bed: Temperature,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_by_sensitivity() {
        assert_eq!(Sensitivity::Low.humidity_threshold_pct(), 40);
        assert_eq!(Sensitivity::Medium.humidity_threshold_pct(), 30);
        assert_eq!(Sensitivity::High.humidity_threshold_pct(), 15);
    }

    #[test]
    fn sensitivity_roundtrips_through_str() {
        for s in [Sensitivity::Low, Sensitivity::Medium, Sensitivity::High] {
            assert_eq!(Sensitivity::parse(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn sensitivity_parse_rejects_unknown() {
        assert!(matches!(
            Sensitivity::parse("Wet"),
            Err(DomainError::UnknownSensitivity(_))
        ));
    }

    #[test]
    fn density_must_be_positive() {
        assert!(Density::new(1.24).is_ok());
        assert!(matches!(
            Density::new(0.0),
            Err(DomainError::NonPositiveDensity(_))
        ));
        assert!(matches!(
            Density::new(-1.0),
            Err(DomainError::NonPositiveDensity(_))
        ));
    }
}
