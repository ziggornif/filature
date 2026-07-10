use crate::shared::{DomainError, LocationId};

/// A location's display name. Validated so it can never be blank/whitespace
/// (spools & the referential rely on a meaningful, trimmed name).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocationName(String);

impl LocationName {
    pub fn new(s: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = s.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::BlankLocationName);
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A location where spools are stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub id: LocationId,
    pub name: LocationName,
    pub note: Option<String>,
}

/// Representation of a new location to be created.
#[derive(Debug, Clone, PartialEq)]
pub struct NewLocation {
    pub name: LocationName,
    pub note: Option<String>,
}

/// Normalize a note string: trim it, and map empty/whitespace to None.
/// This helper allows the web edge to normalize raw user input before
/// constructing a domain object. Otherwise, normalization happens at the
/// port/service layer (Task 3+).
pub fn note_from(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_name_trims_whitespace() {
        let name = LocationName::new(" warehouse ".to_string()).unwrap();
        assert_eq!(name.as_str(), "warehouse");
    }

    #[test]
    fn location_name_rejects_blank_string() {
        assert_eq!(
            LocationName::new("".to_string()),
            Err(DomainError::BlankLocationName)
        );
    }

    #[test]
    fn location_name_rejects_whitespace_only() {
        assert_eq!(
            LocationName::new("   ".to_string()),
            Err(DomainError::BlankLocationName)
        );
    }

    #[test]
    fn location_name_rejects_tabs_and_newlines() {
        assert_eq!(
            LocationName::new("\t\n  \t".to_string()),
            Err(DomainError::BlankLocationName)
        );
    }

    #[test]
    fn location_name_accepts_meaningful_names() {
        assert_eq!(
            LocationName::new("Shelf A".to_string()).unwrap().as_str(),
            "Shelf A"
        );
    }

    #[test]
    fn note_from_maps_empty_string_to_none() {
        assert_eq!(note_from(""), None);
    }

    #[test]
    fn note_from_maps_whitespace_only_to_none() {
        assert_eq!(note_from("   "), None);
        assert_eq!(note_from("\t\n"), None);
    }

    #[test]
    fn note_from_trims_and_keeps_content() {
        assert_eq!(note_from(" shelf A "), Some("shelf A".to_string()));
    }

    #[test]
    fn note_from_preserves_internal_whitespace() {
        assert_eq!(note_from(" cool  place "), Some("cool  place".to_string()));
    }

    #[test]
    fn location_can_have_note() {
        let loc = Location {
            id: LocationId::new("loc-1"),
            name: LocationName::new("Main").unwrap(),
            note: Some("Primary storage".to_string()),
        };
        assert_eq!(loc.note, Some("Primary storage".to_string()));
    }

    #[test]
    fn location_can_have_no_note() {
        let loc = Location {
            id: LocationId::new("loc-1"),
            name: LocationName::new("Main").unwrap(),
            note: None,
        };
        assert_eq!(loc.note, None);
    }

    #[test]
    fn new_location_can_be_created() {
        let new_loc = NewLocation {
            name: LocationName::new("Shelf".to_string()).unwrap(),
            note: Some("Secondary".to_string()),
        };
        assert_eq!(new_loc.name.as_str(), "Shelf");
        assert_eq!(new_loc.note, Some("Secondary".to_string()));
    }
}
