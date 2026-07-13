use crate::shared::LowStockThreshold;

/// Global settings that apply to this Filature instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstanceConfiguration {
    pub low_stock_threshold: LowStockThreshold,
}
