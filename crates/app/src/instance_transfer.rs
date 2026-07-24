//! Versioned JSON codec for instance backups.
//!
//! This adapter is independent from Axum and PostgreSQL so schema rejection and
//! round trips remain unit-testable without any I/O.

use domain::instance_transfer::{
    InstanceDocument, InstanceSnapshot, SnapshotConfiguration, SnapshotDiameter, SnapshotLocation,
    SnapshotManufacturer, SnapshotMaterial, SnapshotPrinter, SnapshotPrinterSlot,
    SnapshotSensitivity, SnapshotSpool, SnapshotSpoolStatus, SnapshotSpoolType,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
use time::{Date, OffsetDateTime, format_description, format_description::well_known::Rfc3339};

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("invalid JSON document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid decimal price: {0}")]
    Decimal(#[from] rust_decimal::Error),
    #[error("invalid timestamp: {0}")]
    Timestamp(String),
}

pub fn encode(document: &InstanceDocument) -> Result<Vec<u8>, CodecError> {
    Ok(serde_json::to_vec_pretty(&WireDocument::try_from(
        document,
    )?)?)
}

pub fn decode(bytes: &[u8]) -> Result<InstanceDocument, CodecError> {
    InstanceDocument::try_from(serde_json::from_slice::<WireDocument>(bytes)?)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDocument {
    format: String,
    version: u32,
    content: WireSnapshot,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSnapshot {
    materials: Vec<WireMaterial>,
    manufacturers: Vec<WireManufacturer>,
    locations: Vec<WireLocation>,
    spools: Vec<WireSpool>,
    #[serde(default)]
    printers: Vec<WirePrinter>,
    configuration: WireConfiguration,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePrinter {
    id: String,
    name: String,
    brand: String,
    model: String,
    #[serde(default = "default_printer_heads")]
    heads: u8,
    module_kind: String,
    module_count: Option<u16>,
    #[serde(default)]
    ams_units: Option<u8>,
    #[serde(default)]
    feed_modes: Option<Vec<String>>,
    slots: Vec<WirePrinterSlot>,
}
fn default_printer_heads() -> u8 {
    1
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePrinterSlot {
    slot_key: String,
    group_label: String,
    position: u16,
    spool_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMaterial {
    id: String,
    name: String,
    density: f64,
    drying_temp_c: u16,
    drying_time_h: u16,
    sensitivity: WireSensitivity,
    nozzle_c: u16,
    bed_c: u16,
}

#[derive(Serialize, Deserialize)]
enum WireSensitivity {
    Low,
    Medium,
    High,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManufacturer {
    id: String,
    name: String,
    country: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLocation {
    id: String,
    name: String,
    note: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpool {
    id: String,
    material_id: String,
    spool_type: WireSpoolType,
    colour_hex: Option<String>,
    colour_name: Option<String>,
    diameter: WireDiameter,
    net_weight: f64,
    remaining_weight: f64,
    /// A JSON string preserves PostgreSQL NUMERIC precision exactly.
    price_paid: String,
    status: WireSpoolStatus,
    location_id: Option<String>,
    manufacturer_id: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    purchased_at: Option<String>,
    #[serde(default)]
    opened_at: Option<String>,
    #[serde(default)]
    ams_tag_uid: Option<String>,
    created_at: String,
}

fn format_date(date: Date) -> Result<String, CodecError> {
    let format = format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|error| CodecError::Timestamp(error.to_string()))?;
    date.format(&format)
        .map_err(|error| CodecError::Timestamp(error.to_string()))
}

fn parse_date(value: String) -> Result<Date, CodecError> {
    let format = format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|error| CodecError::Timestamp(error.to_string()))?;
    Date::parse(&value, &format).map_err(|error| CodecError::Timestamp(error.to_string()))
}

#[derive(Serialize, Deserialize)]
enum WireSpoolType {
    Complete,
    Recharge,
}

#[derive(Serialize, Deserialize)]
enum WireDiameter {
    #[serde(rename = "1.75")]
    Mm1_75,
    #[serde(rename = "2.85")]
    Mm2_85,
}

#[derive(Serialize, Deserialize)]
enum WireSpoolStatus {
    Sealed,
    Open,
    Empty,
    Archived,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConfiguration {
    low_stock_threshold: u8,
}

impl TryFrom<&InstanceDocument> for WireDocument {
    type Error = CodecError;

    fn try_from(document: &InstanceDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            format: document.format.clone(),
            version: document.version,
            content: WireSnapshot::try_from(&document.content)?,
        })
    }
}

impl TryFrom<&InstanceSnapshot> for WireSnapshot {
    type Error = CodecError;

    fn try_from(snapshot: &InstanceSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            materials: snapshot.materials.iter().map(WireMaterial::from).collect(),
            manufacturers: snapshot
                .manufacturers
                .iter()
                .map(WireManufacturer::from)
                .collect(),
            locations: snapshot.locations.iter().map(WireLocation::from).collect(),
            spools: snapshot
                .spools
                .iter()
                .map(WireSpool::try_from)
                .collect::<Result<_, _>>()?,
            printers: snapshot.printers.iter().map(WirePrinter::from).collect(),
            configuration: WireConfiguration {
                low_stock_threshold: snapshot.configuration.low_stock_threshold,
            },
        })
    }
}

impl From<&SnapshotPrinter> for WirePrinter {
    fn from(printer: &SnapshotPrinter) -> Self {
        Self {
            id: printer.id.clone(),
            name: printer.name.clone(),
            brand: printer.brand.clone(),
            model: printer.model.clone(),
            heads: printer.heads,
            module_kind: printer.module_kind.clone(),
            module_count: printer.module_count,
            ams_units: Some(printer.ams_units),
            feed_modes: Some(printer.feed_modes.clone()),
            slots: printer.slots.iter().map(WirePrinterSlot::from).collect(),
        }
    }
}

impl From<&SnapshotPrinterSlot> for WirePrinterSlot {
    fn from(slot: &SnapshotPrinterSlot) -> Self {
        Self {
            slot_key: slot.slot_key.clone(),
            group_label: slot.group_label.clone(),
            position: slot.position,
            spool_id: slot.spool_id.clone(),
        }
    }
}

impl From<&SnapshotMaterial> for WireMaterial {
    fn from(material: &SnapshotMaterial) -> Self {
        Self {
            id: material.id.clone(),
            name: material.name.clone(),
            density: material.density,
            drying_temp_c: material.drying_temp_c,
            drying_time_h: material.drying_time_h,
            sensitivity: match material.sensitivity {
                SnapshotSensitivity::Low => WireSensitivity::Low,
                SnapshotSensitivity::Medium => WireSensitivity::Medium,
                SnapshotSensitivity::High => WireSensitivity::High,
            },
            nozzle_c: material.nozzle_c,
            bed_c: material.bed_c,
        }
    }
}

impl From<&SnapshotManufacturer> for WireManufacturer {
    fn from(item: &SnapshotManufacturer) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            country: item.country.clone(),
        }
    }
}

impl From<&SnapshotLocation> for WireLocation {
    fn from(item: &SnapshotLocation) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            note: item.note.clone(),
        }
    }
}

impl TryFrom<&SnapshotSpool> for WireSpool {
    type Error = CodecError;

    fn try_from(spool: &SnapshotSpool) -> Result<Self, Self::Error> {
        Ok(Self {
            id: spool.id.clone(),
            material_id: spool.material_id.clone(),
            spool_type: match spool.spool_type {
                SnapshotSpoolType::Complete => WireSpoolType::Complete,
                SnapshotSpoolType::Recharge => WireSpoolType::Recharge,
            },
            colour_hex: spool.colour_hex.clone(),
            colour_name: spool.colour_name.clone(),
            diameter: match spool.diameter {
                SnapshotDiameter::Mm1_75 => WireDiameter::Mm1_75,
                SnapshotDiameter::Mm2_85 => WireDiameter::Mm2_85,
            },
            net_weight: spool.net_weight,
            remaining_weight: spool.remaining_weight,
            price_paid: spool.price_paid.to_string(),
            status: match spool.status {
                SnapshotSpoolStatus::Sealed => WireSpoolStatus::Sealed,
                SnapshotSpoolStatus::Open => WireSpoolStatus::Open,
                SnapshotSpoolStatus::Empty => WireSpoolStatus::Empty,
                SnapshotSpoolStatus::Archived => WireSpoolStatus::Archived,
            },
            location_id: spool.location_id.clone(),
            manufacturer_id: spool.manufacturer_id.clone(),
            notes: spool.notes.clone(),
            purchased_at: spool.purchased_at.map(format_date).transpose()?,
            opened_at: spool.opened_at.map(format_date).transpose()?,
            ams_tag_uid: spool.ams_tag_uid.clone(),
            created_at: {
                OffsetDateTime::parse(&spool.created_at, &Rfc3339)
                    .map_err(|error| CodecError::Timestamp(error.to_string()))?;
                spool.created_at.clone()
            },
        })
    }
}

impl TryFrom<WireDocument> for InstanceDocument {
    type Error = CodecError;

    fn try_from(document: WireDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            format: document.format,
            version: document.version,
            content: InstanceSnapshot::try_from(document.content)?,
        })
    }
}

impl TryFrom<WireSnapshot> for InstanceSnapshot {
    type Error = CodecError;

    fn try_from(snapshot: WireSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            materials: snapshot.materials.into_iter().map(Into::into).collect(),
            manufacturers: snapshot.manufacturers.into_iter().map(Into::into).collect(),
            locations: snapshot.locations.into_iter().map(Into::into).collect(),
            spools: snapshot
                .spools
                .into_iter()
                .map(SnapshotSpool::try_from)
                .collect::<Result<_, _>>()?,
            printers: snapshot.printers.into_iter().map(Into::into).collect(),
            configuration: SnapshotConfiguration {
                low_stock_threshold: snapshot.configuration.low_stock_threshold,
            },
        })
    }
}

impl From<WirePrinter> for SnapshotPrinter {
    fn from(printer: WirePrinter) -> Self {
        let old_bambu_ams = printer.brand == "bambu" && printer.module_kind == "ams";
        let (heads, module_kind, module_count) = match printer.module_kind.as_str() {
            "tool_changer" => (
                printer
                    .module_count
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or(1),
                "none".to_string(),
                None,
            ),
            "indx" | "multi_colour" => (
                printer.heads,
                "multi_slot".to_string(),
                printer.module_count,
            ),
            "ams" if printer.brand == "bambu" => (printer.heads, "none".to_string(), None),
            _ => (printer.heads, printer.module_kind, printer.module_count),
        };
        let ams_units = printer.ams_units.unwrap_or(u8::from(old_bambu_ams));
        let feed_modes = printer.feed_modes.unwrap_or_else(|| {
            vec![
                if old_bambu_ams {
                    "ams_fed".to_string()
                } else {
                    "direct".to_string()
                };
                usize::from(heads)
            ]
        });
        let legacy_bambu = printer.brand == "bambu" && printer.ams_units.is_none();
        let slots = printer
            .slots
            .into_iter()
            .filter_map(|mut slot| {
                if legacy_bambu && old_bambu_ams {
                    if slot.slot_key == "ext" {
                        return None;
                    }
                    if let Some(suffix) = slot.slot_key.strip_prefix("ams-") {
                        slot.slot_key = format!("ams0-{suffix}");
                        slot.group_label = "ams_unit_1".into();
                    }
                } else if legacy_bambu && slot.slot_key == "ext" {
                    slot.slot_key = "head-0".into();
                    slot.group_label = "heads".into();
                }
                Some(slot.into())
            })
            .collect();
        Self {
            id: printer.id,
            name: printer.name,
            brand: printer.brand,
            model: printer.model,
            heads,
            module_kind,
            module_count,
            ams_units,
            feed_modes,
            slots,
        }
    }
}

impl From<WirePrinterSlot> for SnapshotPrinterSlot {
    fn from(slot: WirePrinterSlot) -> Self {
        Self {
            slot_key: slot.slot_key,
            group_label: slot.group_label,
            position: slot.position,
            spool_id: slot.spool_id,
        }
    }
}

impl From<WireMaterial> for SnapshotMaterial {
    fn from(material: WireMaterial) -> Self {
        Self {
            id: material.id,
            name: material.name,
            density: material.density,
            drying_temp_c: material.drying_temp_c,
            drying_time_h: material.drying_time_h,
            sensitivity: match material.sensitivity {
                WireSensitivity::Low => SnapshotSensitivity::Low,
                WireSensitivity::Medium => SnapshotSensitivity::Medium,
                WireSensitivity::High => SnapshotSensitivity::High,
            },
            nozzle_c: material.nozzle_c,
            bed_c: material.bed_c,
        }
    }
}

impl From<WireManufacturer> for SnapshotManufacturer {
    fn from(item: WireManufacturer) -> Self {
        Self {
            id: item.id,
            name: item.name,
            country: item.country,
        }
    }
}

impl From<WireLocation> for SnapshotLocation {
    fn from(item: WireLocation) -> Self {
        Self {
            id: item.id,
            name: item.name,
            note: item.note,
        }
    }
}

impl TryFrom<WireSpool> for SnapshotSpool {
    type Error = CodecError;

    fn try_from(spool: WireSpool) -> Result<Self, Self::Error> {
        Ok(Self {
            id: spool.id,
            material_id: spool.material_id,
            spool_type: match spool.spool_type {
                WireSpoolType::Complete => SnapshotSpoolType::Complete,
                WireSpoolType::Recharge => SnapshotSpoolType::Recharge,
            },
            colour_hex: spool.colour_hex,
            colour_name: spool.colour_name,
            diameter: match spool.diameter {
                WireDiameter::Mm1_75 => SnapshotDiameter::Mm1_75,
                WireDiameter::Mm2_85 => SnapshotDiameter::Mm2_85,
            },
            net_weight: spool.net_weight,
            remaining_weight: spool.remaining_weight,
            price_paid: Decimal::from_str(&spool.price_paid)?,
            status: match spool.status {
                WireSpoolStatus::Sealed => SnapshotSpoolStatus::Sealed,
                WireSpoolStatus::Open => SnapshotSpoolStatus::Open,
                WireSpoolStatus::Empty => SnapshotSpoolStatus::Empty,
                WireSpoolStatus::Archived => SnapshotSpoolStatus::Archived,
            },
            location_id: spool.location_id,
            manufacturer_id: spool.manufacturer_id,
            notes: spool.notes,
            purchased_at: spool.purchased_at.map(parse_date).transpose()?,
            opened_at: spool.opened_at.map(parse_date).transpose()?,
            ams_tag_uid: spool.ams_tag_uid,
            created_at: {
                OffsetDateTime::parse(&spool.created_at, &Rfc3339)
                    .map_err(|error| CodecError::Timestamp(error.to_string()))?;
                spool.created_at
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r##"{
      "format":"filature-instance","version":1,
      "content":{
        "materials":[{"id":"m1","name":"PLA","density":1.24,"drying_temp_c":45,"drying_time_h":6,"sensitivity":"Low","nozzle_c":210,"bed_c":60}],
        "manufacturers":[],"locations":[],
        "spools":[{"id":"s1","material_id":"m1","spool_type":"Complete","colour_hex":"#AABBCC","colour_name":"#AABBCC","diameter":"1.75","net_weight":1000.0,"remaining_weight":500.0,"price_paid":"24.9900","status":"Open","location_id":null,"manufacturer_id":null,"created_at":"2026-07-13T12:00:00Z"}],
        "configuration":{"low_stock_threshold":15}
      }
    }"##;

    #[test]
    fn round_trip_preserves_the_document() {
        let document = decode(VALID.as_bytes()).unwrap();
        assert!(document.content.printers.is_empty());
        assert_eq!(document.content.spools[0].notes, None);
        assert_eq!(document.content.spools[0].purchased_at, None);
        assert_eq!(document.content.spools[0].opened_at, None);
        assert_eq!(document.content.spools[0].ams_tag_uid, None);
        let decoded_again = decode(&encode(&document).unwrap()).unwrap();
        assert_eq!(decoded_again, document);
    }

    #[test]
    fn round_trip_preserves_ams_tag_uid() {
        let json = VALID.replace(
            "\"created_at\":\"2026-07-13T12:00:00Z\"",
            "\"ams_tag_uid\":\"A1B2C3D4\",\"created_at\":\"2026-07-13T12:00:00Z\"",
        );
        let document = decode(json.as_bytes()).unwrap();
        assert_eq!(
            document.content.spools[0].ams_tag_uid.as_deref(),
            Some("A1B2C3D4")
        );
        assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
    }

    #[test]
    fn round_trip_preserves_optional_notes_and_dates() {
        let json = VALID.replace(
            "\"created_at\":\"2026-07-13T12:00:00Z\"",
            "\"notes\":\"Opened for a prototype\",\"purchased_at\":\"2026-07-01\",\"opened_at\":\"2026-07-12\",\"created_at\":\"2026-07-13T12:00:00Z\"",
        );
        let document = decode(json.as_bytes()).unwrap();
        let decoded_again = decode(&encode(&document).unwrap()).unwrap();
        assert_eq!(decoded_again, document);
    }

    #[test]
    fn old_printer_exports_default_heads_and_normalize_modules() {
        let printers = r#""printers":[
          {"id":"p1","name":"XL","brand":"prusa","model":"XL","module_kind":"tool_changer","module_count":5,"slots":[]},
          {"id":"p2","name":"Core","brand":"prusa","model":"CORE One","module_kind":"indx","module_count":8,"slots":[]},
          {"id":"p3","name":"Other","brand":"other","model":"Custom","module_kind":"multi_colour","module_count":4,"slots":[]}
          ,{"id":"p4","name":"P1S","brand":"bambu","model":"P1S","module_kind":"ams","module_count":null,"slots":[{"slot_key":"ext","group_label":"external","position":0,"spool_id":null},{"slot_key":"ams-0","group_label":"ams","position":1,"spool_id":"s1"}]}
          ,{"id":"p5","name":"A1","brand":"bambu","model":"A1","module_kind":"none","module_count":null,"slots":[{"slot_key":"ext","group_label":"external","position":0,"spool_id":null}]}
        ],"#;
        let json = VALID.replace("\"configuration\"", &format!("{printers}\"configuration\""));
        let document = decode(json.as_bytes()).unwrap();
        assert_eq!(document.content.printers[0].heads, 5);
        assert_eq!(document.content.printers[0].module_kind, "none");
        for printer in &document.content.printers[1..3] {
            assert_eq!(printer.heads, 1);
            assert_eq!(printer.module_kind, "multi_slot");
        }
        let ams = &document.content.printers[3];
        assert_eq!(ams.module_kind, "none");
        assert_eq!(ams.ams_units, 1);
        assert_eq!(ams.feed_modes, ["ams_fed"]);
        assert_eq!(ams.slots.len(), 1);
        assert_eq!(ams.slots[0].slot_key, "ams0-0");
        assert_eq!(ams.slots[0].spool_id.as_deref(), Some("s1"));
        let direct = &document.content.printers[4];
        assert_eq!(direct.ams_units, 0);
        assert_eq!(direct.feed_modes, ["direct"]);
        assert_eq!(direct.slots[0].slot_key, "head-0");
        assert_eq!(decode(&encode(&document).unwrap()).unwrap(), document);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = VALID.replace("\"version\":1", "\"version\":1,\"extra\":true");
        assert!(matches!(decode(json.as_bytes()), Err(CodecError::Json(_))));
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        let json = VALID.replace("\"diameter\":\"1.75\"", "\"diameter\":\"3.00\"");
        assert!(matches!(decode(json.as_bytes()), Err(CodecError::Json(_))));
    }

    #[test]
    fn numeric_price_is_rejected_to_avoid_precision_ambiguity() {
        let json = VALID.replace("\"price_paid\":\"24.9900\"", "\"price_paid\":24.99");
        assert!(matches!(decode(json.as_bytes()), Err(CodecError::Json(_))));
    }
}
