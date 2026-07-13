//! Versioned JSON codec for instance backups.
//!
//! This adapter is independent from Axum and PostgreSQL so schema rejection and
//! round trips remain unit-testable without any I/O.

use domain::instance_transfer::{
    InstanceDocument, InstanceSnapshot, SnapshotConfiguration, SnapshotDiameter, SnapshotLocation,
    SnapshotManufacturer, SnapshotMaterial, SnapshotSensitivity, SnapshotSpool,
    SnapshotSpoolStatus, SnapshotSpoolType,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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
    configuration: WireConfiguration,
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
    created_at: String,
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
            configuration: WireConfiguration {
                low_stock_threshold: snapshot.configuration.low_stock_threshold,
            },
        })
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
            configuration: SnapshotConfiguration {
                low_stock_threshold: snapshot.configuration.low_stock_threshold,
            },
        })
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
        let decoded_again = decode(&encode(&document).unwrap()).unwrap();
        assert_eq!(decoded_again, document);
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
