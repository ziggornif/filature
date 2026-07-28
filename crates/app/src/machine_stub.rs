//! Probe déterministe réservé aux captures de l'instance de démo.

use async_trait::async_trait;
use domain::printers::{
    AmsTray, MachineError, MachineLink, MachineState, MachineStatus, MachineStatusProbe,
    MachineTelemetry, Temperature,
};
use serde::Deserialize;
use std::{collections::HashMap, fs};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    machines: HashMap<String, ScenarioMachine>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioMachine {
    state: ScenarioState,
    #[serde(default)]
    progress_percent: Option<u8>,
    #[serde(default)]
    remaining_seconds: Option<u64>,
    #[serde(default)]
    job_name: Option<String>,
    #[serde(default)]
    active_head: Option<usize>,
    #[serde(default)]
    nozzles: Vec<TemperatureDto>,
    #[serde(default)]
    bed: Option<TemperatureDto>,
    #[serde(default)]
    ams: Vec<AmsTrayDto>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ScenarioState {
    Printing,
    Idle,
    Paused,
    Error,
    Offline,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemperatureDto {
    actual: f32,
    #[serde(default)]
    target: Option<f32>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmsTrayDto {
    unit_index: u8,
    tray_index: u8,
    #[serde(default)]
    filament_type: Option<String>,
    #[serde(default)]
    color_hex: Option<String>,
    #[serde(default)]
    sub_brand: Option<String>,
    #[serde(default)]
    remain_percent: Option<u8>,
    #[serde(default)]
    tag_uid: Option<String>,
}

pub struct DemoMachineStatusProbe {
    machines: HashMap<String, ScenarioMachine>,
}

impl DemoMachineStatusProbe {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let bytes = fs::read(path)
            .map_err(|error| format!("impossible de lire le scénario machine '{path}': {error}"))?;
        let scenario: Scenario = serde_json::from_slice(&bytes).map_err(|error| {
            format!("scénario machine invalide dans le fichier '{path}': {error}")
        })?;
        Ok(Self {
            machines: scenario.machines,
        })
    }
}

/// Les hôtes REST identifient déjà leur endpoint, tandis que le numéro de série
/// Bambu reste stable même si le DHCP attribue une nouvelle adresse à la machine.
fn key_for(link: &MachineLink) -> &str {
    match link {
        MachineLink::PrusaLink { host, .. } => host,
        MachineLink::Moonraker { url } => url,
        MachineLink::BambuLan { serial, .. } => serial,
    }
}

impl From<ScenarioState> for MachineState {
    fn from(value: ScenarioState) -> Self {
        match value {
            ScenarioState::Printing => Self::Printing,
            ScenarioState::Idle => Self::Idle,
            ScenarioState::Paused => Self::Paused,
            ScenarioState::Error => Self::Error,
            ScenarioState::Offline => Self::Offline,
        }
    }
}

impl From<TemperatureDto> for Temperature {
    fn from(value: TemperatureDto) -> Self {
        Self {
            actual: value.actual,
            target: value.target,
        }
    }
}

impl From<AmsTrayDto> for AmsTray {
    fn from(value: AmsTrayDto) -> Self {
        Self {
            unit_index: value.unit_index,
            tray_index: value.tray_index,
            filament_type: value.filament_type,
            color_hex: value.color_hex,
            sub_brand: value.sub_brand,
            remain_percent: value.remain_percent,
            tag_uid: value.tag_uid,
        }
    }
}

impl ScenarioMachine {
    fn status(&self) -> MachineStatus {
        MachineStatus {
            state: self.state.into(),
            telemetry: MachineTelemetry {
                progress_percent: self.progress_percent,
                remaining_seconds: self.remaining_seconds,
                job_name: self.job_name.clone(),
                nozzle_temperatures: self.nozzles.iter().copied().map(Into::into).collect(),
                active_head: self.active_head,
                bed_temperature: self.bed.map(Into::into),
            },
        }
    }
}

#[async_trait]
impl MachineStatusProbe for DemoMachineStatusProbe {
    async fn fetch_status(&self, link: &MachineLink) -> Result<MachineStatus, MachineError> {
        Ok(self
            .machines
            .get(key_for(link))
            .map_or_else(MachineStatus::offline, ScenarioMachine::status))
    }

    async fn fetch_ams(&self, link: &MachineLink) -> Result<Vec<AmsTray>, MachineError> {
        if !matches!(link, MachineLink::BambuLan { .. }) {
            return Err(MachineError::AmsUnavailable);
        }
        Ok(self
            .machines
            .get(key_for(link))
            .map(|machine| machine.ams.iter().cloned().map(Into::into).collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn link(kind: &str, key: &str) -> MachineLink {
        match kind {
            "prusa" => MachineLink::PrusaLink {
                host: key.into(),
                api_key: "secret".into(),
            },
            "moonraker" => MachineLink::Moonraker { url: key.into() },
            "bambu" => MachineLink::BambuLan {
                host: "adresse-variable".into(),
                access_code: "secret".into(),
                serial: key.into(),
            },
            _ => unreachable!(),
        }
    }

    fn fixture(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "filature-machine-stub-{}-{}.json",
            std::process::id(),
            contents.len()
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    #[tokio::test]
    async fn charge_et_mappe_les_cinq_etats() {
        let path = fixture(
            r#"{"machines":{
                "printing":{"state":"printing","progress_percent":42},
                "idle":{"state":"idle"},"paused":{"state":"paused"},
                "error":{"state":"error"},"offline":{"state":"offline"}
            }}"#,
        );
        let probe = DemoMachineStatusProbe::from_file(path.to_str().unwrap()).unwrap();
        for (key, expected) in [
            ("printing", MachineState::Printing),
            ("idle", MachineState::Idle),
            ("paused", MachineState::Paused),
            ("error", MachineState::Error),
            ("offline", MachineState::Offline),
        ] {
            assert_eq!(
                probe.fetch_status(&link("prusa", key)).await.unwrap().state,
                expected
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn choisit_la_cle_stable_de_chaque_link() {
        assert_eq!(key_for(&link("prusa", "prusa.local")), "prusa.local");
        assert_eq!(
            key_for(&link("moonraker", "http://moonraker")),
            "http://moonraker"
        );
        assert_eq!(key_for(&link("bambu", "SERIAL-1")), "SERIAL-1");
    }

    #[test]
    fn charge_le_scenario_de_demo_livre() {
        let path = format!(
            "{}/../../tools/demo-machines.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let probe = DemoMachineStatusProbe::from_file(&path).unwrap();
        assert_eq!(probe.machines.len(), 8);
    }

    #[tokio::test]
    async fn une_cle_absente_est_offline() {
        let probe = DemoMachineStatusProbe {
            machines: HashMap::new(),
        };
        assert_eq!(
            probe.fetch_status(&link("bambu", "inconnu")).await.unwrap(),
            MachineStatus::offline()
        );
        assert!(
            probe
                .fetch_ams(&link("bambu", "inconnu"))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            probe.fetch_ams(&link("prusa", "inconnu")).await,
            Err(MachineError::AmsUnavailable)
        ));
    }

    #[test]
    fn refuse_un_etat_ou_un_champ_inconnu() {
        for invalid in [
            r#"{"machines":{"x":{"state":"warming"}}}"#,
            r#"{"machines":{"x":{"state":"idle","progres_percent":2}}}"#,
        ] {
            let path = fixture(invalid);
            assert!(DemoMachineStatusProbe::from_file(path.to_str().unwrap()).is_err());
            fs::remove_file(path).unwrap();
        }
    }
}
