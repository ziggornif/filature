use super::{MachineConnectivityUseCases, MachineLink, MachineLinkRepository, MachineStatusProbe};
use crate::shared::PrinterId;
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Offline,
    Idle,
    Printing,
    Paused,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Temperature {
    pub actual: f32,
    pub target: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MachineTelemetry {
    pub progress_percent: Option<u8>,
    pub remaining_seconds: Option<u64>,
    pub job_name: Option<String>,
    pub nozzle_temperatures: Vec<Temperature>,
    pub active_head: Option<usize>,
    pub bed_temperature: Option<Temperature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MachineStatus {
    pub state: MachineState,
    pub telemetry: MachineTelemetry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmsTray {
    pub unit_index: u8,
    pub tray_index: u8,
    pub filament_type: Option<String>,
    pub color_hex: Option<String>,
    pub sub_brand: Option<String>,
    pub remain_percent: Option<u8>,
    pub tag_uid: Option<String>,
}

impl AmsTray {
    pub fn normalize_tag_uid(value: Option<&str>) -> Option<String> {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|value| !value.chars().all(|character| character == '0'))
            .map(str::to_owned)
    }
}

impl MachineStatus {
    pub fn offline() -> Self {
        Self {
            state: MachineState::Offline,
            telemetry: MachineTelemetry::default(),
        }
    }

    pub fn active_nozzle(&self) -> Option<Temperature> {
        self.telemetry
            .active_head
            .and_then(|index| self.telemetry.nozzle_temperatures.get(index).copied())
            .or_else(|| self.telemetry.nozzle_temperatures.first().copied())
    }
}

#[derive(Debug, Error)]
pub enum MachineError {
    #[error("printer not found")]
    PrinterNotFound,
    #[error("printer has no machine link")]
    NoMachineLink,
    #[error("AMS tray discovery is unavailable for this machine")]
    AmsUnavailable,
    #[error("machine connection failed: {0}")]
    Connection(String),
    #[error("machine link repository failed: {0}")]
    Repository(String),
}

pub struct MachineConnectivityService {
    repository: Arc<dyn MachineLinkRepository>,
    probe: Arc<dyn MachineStatusProbe>,
}

impl MachineConnectivityService {
    pub fn new(
        repository: Arc<dyn MachineLinkRepository>,
        probe: Arc<dyn MachineStatusProbe>,
    ) -> Self {
        Self { repository, probe }
    }
}

#[async_trait]
impl MachineConnectivityUseCases for MachineConnectivityService {
    async fn fetch_ams_trays(&self, printer_id: PrinterId) -> Result<Vec<AmsTray>, MachineError> {
        let link = self
            .repository
            .find_link(&printer_id)
            .await?
            .ok_or(MachineError::NoMachineLink)?;
        if !matches!(link, MachineLink::BambuLan { .. }) {
            return Err(MachineError::AmsUnavailable);
        }
        self.probe.fetch_ams(&link).await
    }
    async fn get_printer_status(
        &self,
        printer_id: PrinterId,
    ) -> Result<MachineStatus, MachineError> {
        let link = self
            .repository
            .find_link(&printer_id)
            .await?
            .ok_or(MachineError::NoMachineLink)?;
        Ok(self
            .probe
            .fetch_status(&link)
            .await
            .unwrap_or_else(|_| MachineStatus::offline()))
    }

    async fn test_machine_link(&self, link: MachineLink) -> Result<MachineStatus, MachineError> {
        self.probe.fetch_status(&link).await
    }

    async fn test_printer_machine_link(
        &self,
        printer_id: PrinterId,
        endpoint: String,
    ) -> Result<MachineStatus, MachineError> {
        let stored = self
            .repository
            .find_link(&printer_id)
            .await?
            .ok_or(MachineError::NoMachineLink)?;
        let link = match stored {
            MachineLink::PrusaLink { api_key, .. } => MachineLink::PrusaLink {
                host: endpoint,
                api_key,
            },
            MachineLink::Moonraker { .. } => MachineLink::Moonraker { url: endpoint },
            MachineLink::BambuLan {
                access_code,
                serial,
                ..
            } => MachineLink::BambuLan {
                host: endpoint,
                access_code,
                serial,
            },
        };
        self.probe.fetch_status(&link).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn active_head_falls_back_to_first_head() {
        let status = MachineStatus {
            state: MachineState::Printing,
            telemetry: MachineTelemetry {
                nozzle_temperatures: vec![
                    Temperature {
                        actual: 210.0,
                        target: Some(215.0),
                    },
                    Temperature {
                        actual: 200.0,
                        target: None,
                    },
                ],
                active_head: Some(9),
                ..MachineTelemetry::default()
            },
        };
        assert_eq!(
            status.active_nozzle(),
            status.telemetry.nozzle_temperatures.first().copied()
        );
    }

    struct StoredPrusaLink;
    #[async_trait]
    impl MachineLinkRepository for StoredPrusaLink {
        async fn find_link(&self, _: &PrinterId) -> Result<Option<MachineLink>, MachineError> {
            Ok(Some(MachineLink::PrusaLink {
                host: "http://old-host".into(),
                api_key: "stored-secret".into(),
            }))
        }
    }

    struct RecordingProbe(Mutex<Option<MachineLink>>);
    #[async_trait]
    impl MachineStatusProbe for RecordingProbe {
        async fn fetch_status(&self, link: &MachineLink) -> Result<MachineStatus, MachineError> {
            *self.0.lock().unwrap() = Some(link.clone());
            Ok(MachineStatus::offline())
        }
        async fn fetch_ams(&self, _: &MachineLink) -> Result<Vec<AmsTray>, MachineError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn configured_prusa_test_reuses_stored_key_with_entered_host() {
        let probe = Arc::new(RecordingProbe(Mutex::new(None)));
        let service = MachineConnectivityService::new(Arc::new(StoredPrusaLink), probe.clone());
        service
            .test_printer_machine_link(
                PrinterId::new("printer-1"),
                "https://new-host.example".into(),
            )
            .await
            .unwrap();
        assert_eq!(
            *probe.0.lock().unwrap(),
            Some(MachineLink::PrusaLink {
                host: "https://new-host.example".into(),
                api_key: "stored-secret".into(),
            })
        );
    }
}
