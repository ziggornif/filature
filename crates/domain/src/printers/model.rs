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
    Ams,
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
                (PrinterBrand::BambuLab, _, Module::None | Module::Ams) => true,
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
            Self::Ams => "ams",
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
            "ams" => Ok(Self::Ams),
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
    pub slots: Vec<Slot>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPrinter {
    pub name: PrinterName,
    pub brand: PrinterBrand,
    pub model: String,
    pub heads: u8,
    pub module: Module,
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
) -> Result<Vec<Slot>, DomainError> {
    Module::validate(brand, model, heads, module.clone())?;
    if heads > 1 {
        return Ok(slots("heads", "head", heads));
    }
    Ok(match module {
        Module::Ams => {
            let mut out = slots("external", "ext", 1);
            out.extend(slots("ams", "ams", 4));
            out
        }
        Module::None if brand == PrinterBrand::BambuLab => slots("external", "ext", 1),
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
            keys(&derive_slots(PrinterBrand::BambuLab, "H2S", 1, &Module::None).unwrap()),
            vec!["ext"]
        );
        assert_eq!(
            keys(&derive_slots(PrinterBrand::BambuLab, "H2S", 1, &Module::Ams).unwrap()),
            vec!["ext", "ams-0", "ams-1", "ams-2", "ams-3"]
        );
        for model in ["X2D", "H2D"] {
            assert_eq!(
                keys(&derive_slots(PrinterBrand::BambuLab, model, 2, &Module::None).unwrap()),
                vec!["head-0", "head-1"]
            );
            assert!(derive_slots(PrinterBrand::BambuLab, model, 2, &Module::Ams).is_err());
        }
        assert_eq!(
            derive_slots(
                PrinterBrand::BambuLab,
                "H2C",
                1,
                &Module::MultiSlot { slots: 7 }
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
                &Module::MultiSlot { slots: 6 }
            )
            .is_err()
        );
    }
    #[test]
    fn prusa_single_mmu_multi_slot() {
        assert_eq!(
            derive_slots(PrinterBrand::Prusa, "MK4S", 1, &Module::None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            derive_slots(PrinterBrand::Prusa, "MK4S", 1, &Module::Mmu)
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            derive_slots(
                PrinterBrand::Prusa,
                "CORE One+",
                1,
                &Module::MultiSlot { slots: 4 }
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
                &Module::MultiSlot { slots: 8 }
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
                    &Module::MultiSlot { slots: n }
                )
                .is_err()
            );
        }
        assert!(
            derive_slots(
                PrinterBrand::Prusa,
                "MK4S",
                1,
                &Module::MultiSlot { slots: 4 }
            )
            .is_err()
        );
    }
    #[test]
    fn prusa_xl_all_head_counts() {
        for n in [1, 2, 5] {
            assert_eq!(
                derive_slots(PrinterBrand::Prusa, "XL", n, &Module::None)
                    .unwrap()
                    .len(),
                n as usize
            );
        }
        for n in [3, 4, 0, 6] {
            assert!(derive_slots(PrinterBrand::Prusa, "XL", n, &Module::None).is_err());
        }
    }
    #[test]
    fn other_all_counts() {
        assert_eq!(
            derive_slots(PrinterBrand::Other, "Ender", 1, &Module::None)
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
                    &Module::MultiSlot { slots: *n }
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
                    &Module::MultiSlot { slots: n }
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
