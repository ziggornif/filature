use crate::shared::DomainError;

/// A spool colour: a validated `#RRGGBB` hex string plus an optional
/// human-friendly name (e.g. "vert sapin").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colour {
    hex: String,
    name: Option<String>,
}

impl Colour {
    pub fn new(hex: String, name: Option<String>) -> Result<Self, DomainError> {
        if !is_valid_hex(&hex) {
            return Err(DomainError::InvalidColour(hex));
        }
        Ok(Self { hex, name })
    }

    pub fn hex(&self) -> &str {
        &self.hex
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Manual hex validation (no regex dependency): `#` followed by exactly
/// 6 ASCII hex digits.
fn is_valid_hex(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// Filament diameter, one of the two standard sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diameter {
    Mm1_75,
    Mm2_85,
}

impl Diameter {
    pub fn mm(self) -> f64 {
        match self {
            Diameter::Mm1_75 => 1.75,
            Diameter::Mm2_85 => 2.85,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Diameter::Mm1_75 => "1.75",
            Diameter::Mm2_85 => "2.85",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "1.75" => Ok(Diameter::Mm1_75),
            "2.85" => Ok(Diameter::Mm2_85),
            other => Err(DomainError::UnknownDiameter(other.to_string())),
        }
    }
}

/// Lifecycle status of a physical spool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoolStatus {
    Sealed,
    Open,
    Empty,
    Archived,
}

impl SpoolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SpoolStatus::Sealed => "Sealed",
            SpoolStatus::Open => "Open",
            SpoolStatus::Empty => "Empty",
            SpoolStatus::Archived => "Archived",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "Sealed" => Ok(SpoolStatus::Sealed),
            "Open" => Ok(SpoolStatus::Open),
            "Empty" => Ok(SpoolStatus::Empty),
            "Archived" => Ok(SpoolStatus::Archived),
            other => Err(DomainError::UnknownSpoolStatus(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_accepts_hex_and_optional_name() {
        let c = Colour::new("#1A9E4B".into(), Some("vert sapin".into())).unwrap();
        assert_eq!(c.hex(), "#1A9E4B");
    }
    #[test]
    fn colour_rejects_bad_hex() {
        assert!(Colour::new("1A9E4B".into(), None).is_err()); // no #
        assert!(Colour::new("#12345".into(), None).is_err()); // 5 digits
        assert!(Colour::new("#zzzzzz".into(), None).is_err()); // non-hex
    }
    #[test]
    fn diameter_mm_values() {
        assert_eq!(Diameter::Mm1_75.mm(), 1.75);
        assert_eq!(Diameter::Mm2_85.mm(), 2.85);
        assert_eq!(Diameter::parse("1.75").unwrap(), Diameter::Mm1_75);
        assert!(Diameter::parse("3.0").is_err());
    }
    #[test]
    fn spool_status_roundtrip() {
        for s in [
            SpoolStatus::Sealed,
            SpoolStatus::Open,
            SpoolStatus::Empty,
            SpoolStatus::Archived,
        ] {
            assert_eq!(SpoolStatus::parse(s.as_str()).unwrap(), s);
        }
    }
}
