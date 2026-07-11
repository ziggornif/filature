use crate::shared::{DomainError, ManufacturerId};

/// A manufacturer's display name. Validated so it can never be
/// blank/whitespace (spools reference a manufacturer by id, but the name
/// is what the operator sees and searches).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ManufacturerName(String);

impl ManufacturerName {
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = s.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::BlankManufacturerName);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A filament manufacturer / brand (e.g. Prusament, Polymaker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manufacturer {
    pub id: ManufacturerId,
    pub name: ManufacturerName,
    /// ISO-3166 alpha-2 country of origin, optional (e.g. "CZ"). Sourced
    /// from the OpenPrintTag brand database for the seeded defaults.
    pub country: Option<String>,
}

/// Representation of a new manufacturer to be created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewManufacturer {
    pub name: ManufacturerName,
    pub country: Option<String>,
}

/// Normalize a country string: trim, uppercase, and map empty to None.
pub fn country_from(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manufacturer_name_trims_whitespace() {
        let name = ManufacturerName::new("  Prusament ".to_string()).unwrap();
        assert_eq!(name.as_str(), "Prusament");
    }

    #[test]
    fn blank_manufacturer_name_is_rejected() {
        assert_eq!(
            ManufacturerName::new("   ".to_string()),
            Err(DomainError::BlankManufacturerName)
        );
    }

    #[test]
    fn country_from_normalizes_and_maps_empty_to_none() {
        assert_eq!(country_from("  cz "), Some("CZ".to_string()));
        assert_eq!(country_from("   "), None);
    }
}
