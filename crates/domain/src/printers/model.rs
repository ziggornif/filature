use crate::shared::{DomainError, PrinterId, SpoolId};

pub const BAMBU_MODELS: &[&str] = &[
    "A1 mini",
    "A1",
    "A2L",
    "P1P",
    "P1S",
    "P2S",
    "X1 Carbon",
    "X2D",
    "H2S",
    "H2D",
    "H2C",
];
pub const PRUSA_MODELS: &[&str] = &[
    "MINI+",
    "MK3 / MK3S / MK3S+",
    "MK4S",
    "CORE One+",
    "CORE One L",
    "XL",
];
pub const XL_HEAD_COUNTS: &[u8] = &[1, 2, 5];
pub const PRUSA_MULTI_SLOT_COUNTS: &[u8] = &[4, 8];
pub const OTHER_SLOT_COUNTS: &[u8] = &[2, 3, 4, 5, 6, 8];
pub const MAX_AMS_UNITS: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedMode {
    Direct,
    AmsFed,
}
impl FeedMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::AmsFed => "ams_fed",
        }
    }
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "direct" => Ok(Self::Direct),
            "ams_fed" => Ok(Self::AmsFed),
            _ => Err(DomainError::InvalidPrinterConfiguration(format!(
                "unknown feed mode {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterName(String);
impl PrinterName {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(DomainError::BlankPrinterName);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterBrand {
    BambuLab,
    Prusa,
    Other,
}
impl PrinterBrand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BambuLab => "bambu",
            Self::Prusa => "prusa",
            Self::Other => "other",
        }
    }
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "bambu" => Ok(Self::BambuLab),
            "prusa" => Ok(Self::Prusa),
            "other" => Ok(Self::Other),
            _ => Err(DomainError::InvalidPrinterConfiguration(format!(
                "unknown brand {s}"
            ))),
        }
    }
    pub fn liner(self) -> &'static str {
        match self {
            Self::BambuLab => "#3a9d5c",
            Self::Prusa => "#e8720c",
            Self::Other => "#6a63d1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Module {
    None,
    Mmu,
    MultiSlot { slots: u8 },
}
impl Module {
    pub fn validate(
        brand: PrinterBrand,
        model: &str,
        heads: u8,
        module: Self,
    ) -> Result<Self, DomainError> {
        let valid_heads = match (brand, model) {
            (PrinterBrand::BambuLab, "X2D" | "H2D") => heads == 2,
            (PrinterBrand::Prusa, "XL") => XL_HEAD_COUNTS.contains(&heads),
            _ => heads == 1,
        };
        let valid_module = if heads > 1 {
            matches!(module, Module::None)
        } else {
            match (&brand, model, &module) {
                (PrinterBrand::BambuLab, "X2D" | "H2D", _) => false,
                (PrinterBrand::BambuLab, "H2C", Module::None | Module::MultiSlot { slots: 7 }) => {
                    true
                }
                (PrinterBrand::BambuLab, _, Module::None) => true,
                (PrinterBrand::Prusa, "XL", Module::None) => true,
                (
                    PrinterBrand::Prusa,
                    "CORE One" | "CORE One+" | "CORE One L",
                    Module::MultiSlot { slots },
                ) => PRUSA_MULTI_SLOT_COUNTS.contains(slots),
                (
                    PrinterBrand::Prusa,
                    "CORE One" | "CORE One+" | "CORE One L",
                    Module::None | Module::Mmu,
                ) => true,
                (PrinterBrand::Prusa, _, Module::None | Module::Mmu) if model != "XL" => true,
                (PrinterBrand::Other, _, Module::None) => true,
                (PrinterBrand::Other, _, Module::MultiSlot { slots }) => {
                    OTHER_SLOT_COUNTS.contains(slots)
                }
                _ => false,
            }
        };
        if valid_heads && valid_module {
            Ok(module)
        } else {
            Err(DomainError::InvalidPrinterConfiguration(format!(
                "{} / {model} / {heads} heads / {module:?}",
                brand.as_str()
            )))
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mmu => "mmu",
            Self::MultiSlot { .. } => "multi_slot",
        }
    }
    pub fn count(&self) -> Option<u8> {
        match self {
            Self::MultiSlot { slots } => Some(*slots),
            _ => None,
        }
    }
    pub fn from_storage(kind: &str, count: Option<i32>) -> Result<Self, DomainError> {
        match kind {
            "none" => Ok(Self::None),
            "mmu" => Ok(Self::Mmu),
            "multi_slot" | "indx" | "multi_colour" => Ok(Self::MultiSlot {
                slots: count.unwrap_or_default() as u8,
            }),
            _ => Err(DomainError::InvalidPrinterConfiguration(format!(
                "unknown module {kind}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub key: String,
    pub group_label: String,
    pub position: u16,
    pub loaded_spool: Option<LoadedSpool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSpool {
    pub id: SpoolId,
    pub manufacturer_name: Option<String>,
    pub colour_hex: Option<String>,
    pub colour_name: Option<String>,
    pub material_name: String,
    pub remaining_weight: f64,
    pub net_weight: f64,
    pub status: String,
}

impl LoadedSpool {
    pub fn remaining_pct(&self) -> u8 {
        ((self.remaining_weight / self.net_weight) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadableSpool {
    pub id: SpoolId,
    pub manufacturer_name: Option<String>,
    pub colour_hex: Option<String>,
    pub colour_name: Option<String>,
    pub material_name: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Printer {
    pub id: PrinterId,
    pub name: PrinterName,
    pub brand: PrinterBrand,
    pub model: String,
    pub heads: u8,
    pub module: Module,
    pub ams_units: u8,
    pub feed_modes: Vec<FeedMode>,
    pub machine_link: Option<MachineLink>,
    pub slots: Vec<Slot>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPrinter {
    pub name: PrinterName,
    pub brand: PrinterBrand,
    pub model: String,
    pub heads: u8,
    pub module: Module,
    pub ams_units: u8,
    pub feed_modes: Vec<FeedMode>,
    pub machine_link: Option<MachineLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineLink {
    PrusaLink { host: String, api_key: String },
    Moonraker { url: String },
}

impl MachineLink {
    pub fn validate_for_brand(self, brand: PrinterBrand) -> Result<Self, DomainError> {
        let valid = match (&self, brand) {
            (Self::PrusaLink { host, api_key }, PrinterBrand::Prusa) => {
                !host.trim().is_empty() && !api_key.trim().is_empty()
            }
            (Self::Moonraker { url }, PrinterBrand::Other) => !url.trim().is_empty(),
            _ => false,
        };
        valid.then_some(self).ok_or_else(|| {
            DomainError::InvalidPrinterConfiguration(
                "machine link does not match printer brand".into(),
            )
        })
    }
}

fn slots(label: &str, prefix: &str, count: u8) -> Vec<Slot> {
    (0..count)
        .map(|i| Slot {
            key: if count == 1 {
                prefix.to_string()
            } else {
                format!("{prefix}-{i}")
            },
            group_label: label.to_string(),
            position: u16::from(i),
            loaded_spool: None,
        })
        .collect()
}

pub fn derive_slots(
    brand: PrinterBrand,
    model: &str,
    heads: u8,
    module: &Module,
    ams_units: u8,
    feed_modes: &[FeedMode],
) -> Result<Vec<Slot>, DomainError> {
    Module::validate(brand, model, heads, module.clone())?;
    if feed_modes.len() != usize::from(heads)
        || ams_units > MAX_AMS_UNITS
        || (!matches!(module, Module::None) && ams_units != 0)
        || (brand != PrinterBrand::BambuLab
            && (ams_units != 0 || feed_modes.iter().any(|m| *m != FeedMode::Direct)))
        || (brand == PrinterBrand::BambuLab
            && feed_modes.contains(&FeedMode::AmsFed)
            && ams_units == 0)
    {
        return Err(DomainError::InvalidPrinterConfiguration(
            "invalid AMS topology".into(),
        ));
    }
    if brand == PrinterBrand::BambuLab && matches!(module, Module::None) {
        let mut out = Vec::new();
        for (head, mode) in feed_modes.iter().enumerate() {
            if *mode == FeedMode::Direct {
                out.push(Slot {
                    key: format!("head-{head}"),
                    group_label: "heads".into(),
                    position: u16::try_from(out.len()).unwrap_or(u16::MAX),
                    loaded_spool: None,
                });
            }
        }
        for unit in 0..ams_units {
            for slot in 0..4_u8 {
                out.push(Slot {
                    key: format!("ams{unit}-{slot}"),
                    group_label: format!("ams_unit_{}", unit + 1),
                    position: u16::try_from(out.len()).unwrap_or(u16::MAX),
                    loaded_spool: None,
                });
            }
        }
        if out.is_empty() {
            return Err(DomainError::InvalidPrinterConfiguration(
                "printer topology must have at least one slot".into(),
            ));
        }
        return Ok(out);
    }
    if heads > 1 {
        return Ok(slots("heads", "head", heads));
    }
    Ok(match module {
        Module::Mmu => slots("mmu", "mmu", 5),
        Module::MultiSlot { slots: count } => match brand {
            PrinterBrand::Prusa => slots("indx", "indx", *count),
            PrinterBrand::BambuLab => slots("buses", "multi", *count),
            PrinterBrand::Other => slots("multi_slot", "multi", *count),
        },
        Module::None => slots("spool", "main", 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keys(v: &[Slot]) -> Vec<&str> {
        v.iter().map(|s| s.key.as_str()).collect()
    }
    #[test]
    fn name_validation() {
        assert_eq!(
            PrinterName::new("  Shop P1S ").unwrap().as_str(),
            "Shop P1S"
        );
        assert_eq!(PrinterName::new(" \n "), Err(DomainError::BlankPrinterName));
    }
    #[test]
    fn bambu_single_and_dual_head_models() {
        assert_eq!(
            keys(
                &derive_slots(
                    PrinterBrand::BambuLab,
                    "H2S",
                    1,
                    &Module::None,
                    0,
                    &[FeedMode::Direct]
                )
                .unwrap()
            ),
            vec!["head-0"]
        );
        assert_eq!(
            keys(
                &derive_slots(
                    PrinterBrand::BambuLab,
                    "H2S",
                    1,
                    &Module::None,
                    2,
                    &[FeedMode::AmsFed]
                )
                .unwrap()
            ),
            vec![
                "ams0-0", "ams0-1", "ams0-2", "ams0-3", "ams1-0", "ams1-1", "ams1-2", "ams1-3"
            ]
        );
        assert_eq!(
            keys(
                &derive_slots(
                    PrinterBrand::BambuLab,
                    "H2D",
                    2,
                    &Module::None,
                    1,
                    &[FeedMode::AmsFed, FeedMode::Direct]
                )
                .unwrap()
            ),
            vec!["head-1", "ams0-0", "ams0-1", "ams0-2", "ams0-3"]
        );
        assert!(
            derive_slots(
                PrinterBrand::BambuLab,
                "H2S",
                1,
                &Module::None,
                0,
                &[FeedMode::AmsFed]
            )
            .is_err()
        );
        assert!(derive_slots(PrinterBrand::BambuLab, "P1S", 0, &Module::None, 0, &[]).is_err());
        assert!(
            derive_slots(
                PrinterBrand::BambuLab,
                "P1S",
                1,
                &Module::None,
                MAX_AMS_UNITS + 1,
                &[FeedMode::Direct]
            )
            .is_err()
        );
        assert_eq!(
            derive_slots(
                PrinterBrand::BambuLab,
                "H2C",
                1,
                &Module::MultiSlot { slots: 7 },
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            7
        );
        assert!(
            derive_slots(
                PrinterBrand::BambuLab,
                "H2C",
                1,
                &Module::MultiSlot { slots: 6 },
                0,
                &[FeedMode::Direct]
            )
            .is_err()
        );
    }
    #[test]
    fn prusa_single_mmu_multi_slot() {
        assert_eq!(
            derive_slots(
                PrinterBrand::Prusa,
                "MK4S",
                1,
                &Module::None,
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            1
        );
        assert_eq!(
            derive_slots(
                PrinterBrand::Prusa,
                "MK4S",
                1,
                &Module::Mmu,
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            5
        );
        assert_eq!(
            derive_slots(
                PrinterBrand::Prusa,
                "CORE One+",
                1,
                &Module::MultiSlot { slots: 4 },
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            4
        );
        assert_eq!(
            derive_slots(
                PrinterBrand::Prusa,
                "CORE One L",
                1,
                &Module::MultiSlot { slots: 8 },
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            8
        );
        for n in [0, 5, 7, 9] {
            assert!(
                derive_slots(
                    PrinterBrand::Prusa,
                    "CORE One+",
                    1,
                    &Module::MultiSlot { slots: n },
                    0,
                    &[FeedMode::Direct]
                )
                .is_err()
            );
        }
        assert!(
            derive_slots(
                PrinterBrand::Prusa,
                "MK4S",
                1,
                &Module::MultiSlot { slots: 4 },
                0,
                &[FeedMode::Direct]
            )
            .is_err()
        );
    }
    #[test]
    fn prusa_xl_all_head_counts() {
        for n in [1, 2, 5] {
            assert_eq!(
                derive_slots(
                    PrinterBrand::Prusa,
                    "XL",
                    n,
                    &Module::None,
                    0,
                    &vec![FeedMode::Direct; n as usize]
                )
                .unwrap()
                .len(),
                n as usize
            );
        }
        for n in [3, 4, 0, 6] {
            assert!(
                derive_slots(
                    PrinterBrand::Prusa,
                    "XL",
                    n,
                    &Module::None,
                    0,
                    &vec![FeedMode::Direct; n as usize]
                )
                .is_err()
            );
        }
    }
    #[test]
    fn other_all_counts() {
        assert_eq!(
            derive_slots(
                PrinterBrand::Other,
                "Ender",
                1,
                &Module::None,
                0,
                &[FeedMode::Direct]
            )
            .unwrap()
            .len(),
            1
        );
        for n in OTHER_SLOT_COUNTS {
            assert_eq!(
                derive_slots(
                    PrinterBrand::Other,
                    "Ender",
                    1,
                    &Module::MultiSlot { slots: *n },
                    0,
                    &[FeedMode::Direct]
                )
                .unwrap()
                .len(),
                *n as usize
            );
        }
        for n in [0, 1, 7, 9] {
            assert!(
                derive_slots(
                    PrinterBrand::Other,
                    "Ender",
                    1,
                    &Module::MultiSlot { slots: n },
                    0,
                    &[FeedMode::Direct]
                )
                .is_err()
            );
        }
    }
    #[test]
    fn invalid_cross_brand_modules() {
        assert!(Module::validate(PrinterBrand::BambuLab, "P1S", 1, Module::Mmu).is_err());
        assert!(Module::validate(PrinterBrand::Prusa, "XL", 2, Module::Mmu).is_err());
    }
    #[test]
    fn multi_slot_storage_round_trip() {
        let module = Module::MultiSlot { slots: 4 };
        assert_eq!(module.kind(), "multi_slot");
        assert_eq!(module.count(), Some(4));
        assert_eq!(Module::from_storage("multi_slot", Some(4)).unwrap(), module);
    }
}
