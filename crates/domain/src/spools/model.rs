use crate::shared::{DomainError, Grams, LocationId, ManufacturerId, MaterialId, Money};
use std::f64::consts::PI;

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

/// Opaque identifier for a `Spool`, mirroring `MaterialId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpoolId(pub String);

impl SpoolId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data required to create a new spool. Remaining weight, status and id
/// are derived downstream (remaining starts equal to net; status defaults
/// to `Sealed`; id is assigned by the repository).
#[derive(Debug, Clone, PartialEq)]
pub struct NewSpool {
    pub material_id: MaterialId,
    pub colour: Colour,
    pub diameter: Diameter,
    pub net_weight: Grams,
    pub price_paid: Money,
    pub location_id: Option<LocationId>,
    pub manufacturer_id: Option<ManufacturerId>,
}

/// A physical spool of filament: net weight at purchase, current
/// remaining weight, and lifecycle status.
#[derive(Debug, Clone, PartialEq)]
pub struct Spool {
    pub id: SpoolId,
    pub material_id: MaterialId,
    pub colour: Colour,
    pub diameter: Diameter,
    pub net_weight: Grams,
    pub remaining_weight: Grams,
    pub price_paid: Money,
    pub status: SpoolStatus,
    pub location_id: Option<LocationId>,
    pub manufacturer_id: Option<ManufacturerId>,
}

impl Spool {
    /// Remaining weight as a fraction of net weight (0.0..=1.0+).
    pub fn remaining_ratio(&self) -> f64 {
        self.remaining_weight.ratio_of(self.net_weight)
    }

    /// Updates the net weight, clamping `remaining_weight` down to the
    /// new net weight if it currently exceeds it. If the new net weight
    /// is still >= the current remaining weight, remaining is left
    /// unchanged (it does not increase).
    pub fn set_net_clamping(&mut self, new_net: Grams) {
        if self.remaining_weight.value() > new_net.value() {
            self.remaining_weight = new_net;
        }
        self.net_weight = new_net;
    }

    /// Sets the remaining weight directly, deriving status from it.
    /// Rejects archived spools and remaining weights above net weight.
    pub fn set_remaining(&mut self, new: Grams) -> Result<(), DomainError> {
        if self.status == SpoolStatus::Archived {
            return Err(DomainError::SpoolArchived);
        }
        if new.value() > self.net_weight.value() {
            return Err(DomainError::RemainingAboveNet);
        }
        self.remaining_weight = new;
        self.status = status_for(new, self.net_weight);
        Ok(())
    }

    /// Consumes `amount` grams from the remaining weight, flooring at
    /// zero, and derives status from the result. Rejects archived spools.
    pub fn consume(&mut self, amount: Grams) -> Result<(), DomainError> {
        if self.status == SpoolStatus::Archived {
            return Err(DomainError::SpoolArchived);
        }
        let new = Grams::new((self.remaining_weight.value() - amount.value()).max(0.0)).unwrap();
        self.remaining_weight = new;
        self.status = status_for(new, self.net_weight);
        Ok(())
    }

    /// Archives the spool, leaving remaining weight untouched. Rejects
    /// spools that are already archived.
    pub fn archive(&mut self) -> Result<(), DomainError> {
        if self.status == SpoolStatus::Archived {
            return Err(DomainError::SpoolAlreadyArchived);
        }
        self.status = SpoolStatus::Archived;
        Ok(())
    }

    /// Restores an archived spool, deriving status from its remaining
    /// weight. Rejects spools that are not archived.
    pub fn restore(&mut self) -> Result<(), DomainError> {
        if self.status != SpoolStatus::Archived {
            return Err(DomainError::SpoolNotArchived);
        }
        self.status = status_for(self.remaining_weight, self.net_weight);
        Ok(())
    }

    /// Assigns (or clears, with `None`) the spool's storage location. Pure
    /// field assignment: no status/weight side effects, and no lifecycle
    /// restriction — a spool's physical location is independent of whether
    /// it's sealed, open, empty, or archived, so this is allowed regardless
    /// of `status` (unlike `set_remaining`/`consume`, which reject archived
    /// spools).
    pub fn assign_location(&mut self, location: Option<LocationId>) {
        self.location_id = location;
    }
}

/// Derives lifecycle status from remaining and net weight. `remaining` is
/// always within `0..=net` by construction. Never produces `Archived` —
/// that transition is explicit via `Spool::archive`.
pub fn status_for(remaining: Grams, net: Grams) -> SpoolStatus {
    if remaining.value() <= 0.0 {
        SpoolStatus::Empty
    } else if remaining.value() >= net.value() {
        SpoolStatus::Sealed
    } else {
        SpoolStatus::Open
    }
}

/// Estimated remaining filament length in metres, derived from the
/// remaining mass, the material density (g/cm³) and the filament
/// diameter. Mass (g) / (density (g/cm³) * cross-section area (cm²))
/// gives length in cm; dividing by 100 converts to metres.
pub fn remaining_length_m(remaining: Grams, density: f64, diameter: Diameter) -> f64 {
    let d_cm = diameter.mm() / 10.0;
    let radius_cm = d_cm / 2.0;
    remaining.value() / (density * PI * radius_cm.powi(2)) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spool(net: Grams, remaining: Grams) -> Spool {
        Spool {
            id: SpoolId::new("spool-1"),
            material_id: MaterialId::new("material-1"),
            colour: Colour::new("#1A9E4B".into(), None).unwrap(),
            diameter: Diameter::Mm1_75,
            net_weight: net,
            remaining_weight: remaining,
            price_paid: Money::new(2500, 2).unwrap(),
            status: SpoolStatus::Open,
            location_id: None,
            manufacturer_id: None,
        }
    }

    #[test]
    fn remaining_ratio_is_remaining_over_net() {
        let s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(250.0).unwrap());
        assert!((s.remaining_ratio() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn remaining_length_known_value_175() {
        // PLA density 1.24 g/cm3, diameter 1.75mm, remaining 1000g.
        // d_cm = 0.175, r_cm = 0.0875, area = pi * r_cm^2 cm^2.
        // length_cm = 1000 / (1.24 * area); length_m = length_cm / 100.
        // Hand-computed expected value: 335.2836194167644 m.
        let m = remaining_length_m(Grams::new(1000.0).unwrap(), 1.24, Diameter::Mm1_75);
        assert!((m - 335.2836194167644).abs() < 1e-6);
    }

    #[test]
    fn edit_clamps_remaining_when_net_lowered() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(800.0).unwrap());
        s.set_net_clamping(Grams::new(500.0).unwrap());
        assert_eq!(s.remaining_weight.value(), 500.0);
    }

    #[test]
    fn edit_does_not_raise_remaining_when_net_increased() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(800.0).unwrap());
        s.set_net_clamping(Grams::new(1500.0).unwrap());
        assert_eq!(s.remaining_weight.value(), 800.0);
        assert_eq!(s.net_weight.value(), 1500.0);
    }

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

    #[test]
    fn set_remaining_derives_status() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(1000.0).unwrap());
        s.set_remaining(Grams::new(400.0).unwrap()).unwrap();
        assert_eq!(s.remaining_weight.value(), 400.0);
        assert_eq!(s.status, SpoolStatus::Open);
        s.set_remaining(Grams::new(0.0).unwrap()).unwrap();
        assert_eq!(s.status, SpoolStatus::Empty);
        s.set_remaining(Grams::new(1000.0).unwrap()).unwrap();
        assert_eq!(s.status, SpoolStatus::Sealed);
        assert_eq!(
            s.set_remaining(Grams::new(1001.0).unwrap()),
            Err(DomainError::RemainingAboveNet)
        );
    }

    #[test]
    fn consume_partial_stays_open() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(1000.0).unwrap());
        s.consume(Grams::new(300.0).unwrap()).unwrap();
        assert_eq!(s.remaining_weight.value(), 700.0);
        assert_eq!(s.status, SpoolStatus::Open);
    }

    #[test]
    fn consume_more_than_remaining_floors_at_zero_and_empty() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(200.0).unwrap());
        s.consume(Grams::new(500.0).unwrap()).unwrap();
        assert_eq!(s.remaining_weight.value(), 0.0);
        assert_eq!(s.status, SpoolStatus::Empty);
    }

    #[test]
    fn set_remaining_and_consume_on_archived_spool_err() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(500.0).unwrap());
        s.status = SpoolStatus::Archived;
        assert_eq!(
            s.set_remaining(Grams::new(100.0).unwrap()),
            Err(DomainError::SpoolArchived)
        );
        // Value exceeds net weight (1000.0): proves the archived check runs
        // BEFORE the RemainingAboveNet check, not after.
        assert_eq!(
            s.set_remaining(Grams::new(1001.0).unwrap()),
            Err(DomainError::SpoolArchived)
        );
        assert_eq!(
            s.consume(Grams::new(100.0).unwrap()),
            Err(DomainError::SpoolArchived)
        );
    }

    #[test]
    fn archive_from_each_non_archived_status_succeeds() {
        for (net, remaining) in [
            (1000.0, 1000.0), // Sealed
            (1000.0, 400.0),  // Open
            (1000.0, 0.0),    // Empty
        ] {
            let mut s = sample_spool(Grams::new(net).unwrap(), Grams::new(remaining).unwrap());
            s.archive().unwrap();
            assert_eq!(s.status, SpoolStatus::Archived);
            assert_eq!(s.remaining_weight.value(), remaining);
        }
    }

    #[test]
    fn archive_when_already_archived_errs() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(500.0).unwrap());
        s.status = SpoolStatus::Archived;
        assert_eq!(s.archive(), Err(DomainError::SpoolAlreadyArchived));
    }

    #[test]
    fn restore_when_archived_derives_status() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(0.0).unwrap());
        s.status = SpoolStatus::Archived;
        s.restore().unwrap();
        assert_eq!(s.status, SpoolStatus::Empty);

        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(1000.0).unwrap());
        s.status = SpoolStatus::Archived;
        s.restore().unwrap();
        assert_eq!(s.status, SpoolStatus::Sealed);

        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(400.0).unwrap());
        s.status = SpoolStatus::Archived;
        s.restore().unwrap();
        assert_eq!(s.status, SpoolStatus::Open);
    }

    #[test]
    fn restore_when_not_archived_errs() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(400.0).unwrap());
        assert_eq!(s.restore(), Err(DomainError::SpoolNotArchived));
    }

    #[test]
    fn assign_location_sets_and_clears_field() {
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(400.0).unwrap());
        assert_eq!(s.location_id, None);
        s.assign_location(Some(crate::shared::LocationId::new("warehouse-1")));
        assert_eq!(
            s.location_id,
            Some(crate::shared::LocationId::new("warehouse-1"))
        );
        s.assign_location(None);
        assert_eq!(s.location_id, None);
    }

    #[test]
    fn assign_location_allowed_on_archived_spool() {
        // Location assignment is independent of lifecycle: an archived
        // spool's storage location may still be changed (e.g. moving
        // retired spools to a scrap shelf), unlike set_remaining/consume
        // which reject archived spools.
        let mut s = sample_spool(Grams::new(1000.0).unwrap(), Grams::new(400.0).unwrap());
        s.status = SpoolStatus::Archived;
        s.assign_location(Some(crate::shared::LocationId::new("shelf-9")));
        assert_eq!(
            s.location_id,
            Some(crate::shared::LocationId::new("shelf-9"))
        );
    }
}
