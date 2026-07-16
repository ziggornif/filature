use rust_decimal::Decimal;
use std::collections::HashSet;
use thiserror::Error;
use time::Date;

pub const FORMAT: &str = "filature-instance";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct InstanceDocument {
    pub format: String,
    pub version: u32,
    pub content: InstanceSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstanceSnapshot {
    pub materials: Vec<SnapshotMaterial>,
    pub manufacturers: Vec<SnapshotManufacturer>,
    pub locations: Vec<SnapshotLocation>,
    pub spools: Vec<SnapshotSpool>,
    pub printers: Vec<SnapshotPrinter>,
    pub configuration: SnapshotConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPrinter {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub model: String,
    pub heads: u8,
    pub module_kind: String,
    pub module_count: Option<u16>,
    pub ams_units: u8,
    pub feed_modes: Vec<String>,
    pub slots: Vec<SnapshotPrinterSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPrinterSlot {
    pub slot_key: String,
    pub group_label: String,
    pub position: u16,
    pub spool_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotMaterial {
    pub id: String,
    pub name: String,
    pub density: f64,
    pub drying_temp_c: u16,
    pub drying_time_h: u16,
    pub sensitivity: SnapshotSensitivity,
    pub nozzle_c: u16,
    pub bed_c: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSensitivity {
    Low,
    Medium,
    High,
}

impl SnapshotSensitivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotManufacturer {
    pub id: String,
    pub name: String,
    pub country: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotLocation {
    pub id: String,
    pub name: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotSpool {
    pub id: String,
    pub material_id: String,
    pub spool_type: SnapshotSpoolType,
    pub colour_hex: Option<String>,
    pub colour_name: Option<String>,
    pub diameter: SnapshotDiameter,
    pub net_weight: f64,
    pub remaining_weight: f64,
    pub price_paid: Decimal,
    pub status: SnapshotSpoolStatus,
    pub location_id: Option<String>,
    pub manufacturer_id: Option<String>,
    pub notes: Option<String>,
    pub purchased_at: Option<Date>,
    pub opened_at: Option<Date>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSpoolType {
    Complete,
    Recharge,
}

impl SnapshotSpoolType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::Recharge => "Recharge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotDiameter {
    Mm1_75,
    Mm2_85,
}

impl SnapshotDiameter {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mm1_75 => "1.75",
            Self::Mm2_85 => "2.85",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSpoolStatus {
    Sealed,
    Open,
    Empty,
    Archived,
}

impl SnapshotSpoolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sealed => "Sealed",
            Self::Open => "Open",
            Self::Empty => "Empty",
            Self::Archived => "Archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotConfiguration {
    pub low_stock_threshold: u8,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransferError {
    #[error("unsupported document format: {0}")]
    UnsupportedFormat(String),
    #[error("unsupported document version: {0}")]
    UnsupportedVersion(u32),
    #[error("invalid instance document: {0}")]
    Invalid(String),
    #[error("persistence backend error: {0}")]
    Backend(String),
}

impl InstanceDocument {
    pub fn validate(&self) -> Result<(), TransferError> {
        if self.format != FORMAT {
            return Err(TransferError::UnsupportedFormat(self.format.clone()));
        }
        if self.version != VERSION {
            return Err(TransferError::UnsupportedVersion(self.version));
        }
        self.content.validate()
    }
}

impl InstanceSnapshot {
    fn validate(&self) -> Result<(), TransferError> {
        if self.configuration.low_stock_threshold > 100 {
            return invalid("low_stock_threshold must be between 0 and 100");
        }

        let material_ids = unique_ids(
            "material",
            self.materials.iter().map(|material| material.id.as_str()),
        )?;
        unique_names(
            "material",
            self.materials.iter().map(|material| material.name.as_str()),
        )?;
        for material in &self.materials {
            if material.name.trim().is_empty() {
                return invalid("material name must not be blank");
            }
            if !material.density.is_finite() || material.density <= 0.0 {
                return invalid("material density must be finite and greater than zero");
            }
        }

        let manufacturer_ids = unique_ids(
            "manufacturer",
            self.manufacturers.iter().map(|item| item.id.as_str()),
        )?;
        unique_names(
            "manufacturer",
            self.manufacturers.iter().map(|item| item.name.as_str()),
        )?;
        if self
            .manufacturers
            .iter()
            .any(|item| item.name.trim().is_empty())
        {
            return invalid("manufacturer name must not be blank");
        }

        let location_ids = unique_ids(
            "location",
            self.locations.iter().map(|item| item.id.as_str()),
        )?;
        if self
            .locations
            .iter()
            .any(|item| item.name.trim().is_empty())
        {
            return invalid("location name must not be blank");
        }

        let spool_ids = unique_ids("spool", self.spools.iter().map(|spool| spool.id.as_str()))?;
        for spool in &self.spools {
            if !material_ids.contains(spool.material_id.as_str()) {
                return invalid("spool references an unknown material");
            }
            if spool
                .location_id
                .as_deref()
                .is_some_and(|id| !location_ids.contains(id))
            {
                return invalid("spool references an unknown location");
            }
            if spool
                .manufacturer_id
                .as_deref()
                .is_some_and(|id| !manufacturer_ids.contains(id))
            {
                return invalid("spool references an unknown manufacturer");
            }
            if !spool.net_weight.is_finite() || spool.net_weight <= 0.0 {
                return invalid("spool net_weight must be finite and greater than zero");
            }
            if !spool.remaining_weight.is_finite()
                || spool.remaining_weight < 0.0
                || spool.remaining_weight > spool.net_weight
            {
                return invalid("spool remaining_weight must be between zero and net_weight");
            }
            if spool.price_paid < Decimal::ZERO {
                return invalid("spool price_paid must not be negative");
            }
            if spool
                .colour_hex
                .as_deref()
                .is_some_and(|colour| !valid_colour(colour))
            {
                return invalid("spool colour_hex is invalid");
            }
        }

        unique_ids(
            "printer",
            self.printers.iter().map(|printer| printer.id.as_str()),
        )?;
        let mut loaded_spool_ids = HashSet::new();
        for printer in &self.printers {
            if printer.name.trim().is_empty() {
                return invalid("printer name must not be blank");
            }
            let mut slot_keys = HashSet::new();
            for slot in &printer.slots {
                if slot.slot_key.trim().is_empty() {
                    return invalid("printer slot key must not be blank");
                }
                if !slot_keys.insert(slot.slot_key.as_str()) {
                    return invalid("duplicate printer slot key");
                }
                if let Some(spool_id) = slot.spool_id.as_deref() {
                    if !spool_ids.contains(spool_id) {
                        return invalid("printer slot references an unknown spool");
                    }
                    if !loaded_spool_ids.insert(spool_id) {
                        return invalid("spool is loaded in more than one printer slot");
                    }
                }
            }
        }
        Ok(())
    }
}

fn invalid<T>(message: &str) -> Result<T, TransferError> {
    Err(TransferError::Invalid(message.to_string()))
}

fn unique_ids<'a>(
    kind: &str,
    values: impl Iterator<Item = &'a str>,
) -> Result<HashSet<&'a str>, TransferError> {
    let mut ids = HashSet::new();
    for id in values {
        if id.trim().is_empty() {
            return invalid(&format!("{kind} id must not be blank"));
        }
        if !ids.insert(id) {
            return invalid(&format!("duplicate {kind} id"));
        }
    }
    Ok(ids)
}

fn unique_names<'a>(
    kind: &str,
    values: impl Iterator<Item = &'a str>,
) -> Result<(), TransferError> {
    let mut names = HashSet::new();
    for name in values {
        if !names.insert(name) {
            return invalid(&format!("duplicate {kind} name"));
        }
    }
    Ok(())
}

fn valid_colour(value: &str) -> bool {
    value == "transparent"
        || value
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_document() -> InstanceDocument {
        InstanceDocument {
            format: FORMAT.to_string(),
            version: VERSION,
            content: InstanceSnapshot {
                materials: vec![SnapshotMaterial {
                    id: "mat-1".into(),
                    name: "PLA".into(),
                    density: 1.24,
                    drying_temp_c: 45,
                    drying_time_h: 6,
                    sensitivity: SnapshotSensitivity::Low,
                    nozzle_c: 210,
                    bed_c: 60,
                }],
                manufacturers: vec![],
                locations: vec![],
                spools: vec![SnapshotSpool {
                    id: "spool-1".into(),
                    material_id: "mat-1".into(),
                    spool_type: SnapshotSpoolType::Complete,
                    colour_hex: Some("#AABBCC".into()),
                    colour_name: Some("#AABBCC".into()),
                    diameter: SnapshotDiameter::Mm1_75,
                    net_weight: 1000.0,
                    remaining_weight: 500.0,
                    price_paid: Decimal::new(2499, 2),
                    status: SnapshotSpoolStatus::Open,
                    location_id: None,
                    manufacturer_id: None,
                    notes: Some("Test spool".into()),
                    purchased_at: Some(Date::from_ordinal_date(2026, 1).unwrap()),
                    opened_at: None,
                    created_at: "1970-01-01T00:00:00Z".into(),
                }],
                printers: vec![],
                configuration: SnapshotConfiguration {
                    low_stock_threshold: 15,
                },
            },
        }
    }

    #[test]
    fn valid_complete_document_is_accepted() {
        assert_eq!(valid_document().validate(), Ok(()));
    }

    #[test]
    fn dangling_reference_is_rejected() {
        let mut document = valid_document();
        document.content.spools[0].material_id = "missing".into();
        assert!(matches!(
            document.validate(),
            Err(TransferError::Invalid(_))
        ));
    }

    #[test]
    fn invalid_spool_weights_are_rejected() {
        let mut document = valid_document();
        document.content.spools[0].remaining_weight = 1001.0;
        assert!(matches!(
            document.validate(),
            Err(TransferError::Invalid(_))
        ));
    }
}
