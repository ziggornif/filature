use async_trait::async_trait;
use domain::printers::{
    AmsTray, MachineError, MachineLink, MachineState, MachineStatus, MachineStatusProbe,
    MachineTelemetry, Temperature,
};
use rumqttc::tokio_rustls::rustls::{
    self, DigitallySignedStruct, Error as TlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::WebPkiSupportedAlgorithms,
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, TlsConfiguration, Transport};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

const MQTT_PORT: u16 = 8883;
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
static CLIENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct BambuCertificateVerifier {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for BambuCertificateVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // Bambu LAN printers use a self-signed certificate. This exception is
        // deliberately scoped to the MQTT config created in this module.
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // The printer's self-signed certificate is X.509 v1, which webpki's
        // signature verification rejects (UnsupportedCertVersion) even with the
        // chain check bypassed — validated against the real machine, 2026-07-22.
        // Handshake signature verification is therefore skipped too, matching
        // what every Bambu LAN integration does (mosquitto --insecure, Home
        // Assistant). Risk (LAN MITM on this link) recorded in
        // docs/security/accepted-risks.md; scoped strictly to this adapter.
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

fn tls12_config() -> Result<rustls::ClientConfig, MachineError> {
    // aws-lc-rs: the provider already in the tree via reqwest's rustls (22a) —
    // avoids enabling a second crypto backend just for this adapter.
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let algorithms = provider.signature_verification_algorithms;
    rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS12])
        .map_err(|e| MachineError::Connection(e.to_string()))
        .map(|builder| {
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(BambuCertificateVerifier { algorithms }))
                .with_no_client_auth()
        })
}

pub struct BambuMachineStatusProbe;
impl Default for BambuMachineStatusProbe {
    fn default() -> Self {
        Self::new()
    }
}
impl BambuMachineStatusProbe {
    pub fn new() -> Self {
        Self
    }
}

impl BambuMachineStatusProbe {
    async fn session(
        &self,
        host: &str,
        access_code: &str,
        serial: &str,
    ) -> Result<MachineStatus, MachineError> {
        let client_id = format!(
            "filature-{}-{}",
            std::process::id(),
            CLIENT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut options = MqttOptions::new(client_id, host, MQTT_PORT);
        options
            .set_credentials("bblp", access_code)
            .set_keep_alive(Duration::from_secs(5))
            .set_transport(Transport::tls_with_config(TlsConfiguration::Rustls(
                Arc::new(tls12_config()?),
            )));
        let report_topic = format!("device/{serial}/report");
        let request_topic = format!("device/{serial}/request");
        let (client, mut eventloop) = AsyncClient::new(options, 10);
        let mut subscribed = false;
        let mut requested = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|e| MachineError::Connection(e.to_string()))?
            {
                Event::Incoming(Incoming::ConnAck(_)) if !subscribed => {
                    client
                        .subscribe(report_topic.clone(), QoS::AtMostOnce)
                        .await
                        .map_err(|e| MachineError::Connection(e.to_string()))?;
                    subscribed = true;
                }
                Event::Incoming(Incoming::SubAck(_)) if !requested => {
                    client
                        .publish(
                            request_topic.clone(),
                            QoS::AtMostOnce,
                            false,
                            br#"{"pushing":{"sequence_id":"1","command":"pushall"}}"#,
                        )
                        .await
                        .map_err(|e| MachineError::Connection(e.to_string()))?;
                    requested = true;
                }
                Event::Incoming(Incoming::Publish(message)) if message.topic == report_topic => {
                    if let Some(status) = parse_complete_report(&message.payload)? {
                        let _ = client.disconnect().await;
                        return Ok(status);
                    }
                }
                _ => {}
            }
        }
    }

    async fn ams_session(
        &self,
        host: &str,
        access_code: &str,
        serial: &str,
    ) -> Result<Vec<AmsTray>, MachineError> {
        let client_id = format!(
            "filature-ams-{}-{}",
            std::process::id(),
            CLIENT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut options = MqttOptions::new(client_id, host, MQTT_PORT);
        options
            .set_credentials("bblp", access_code)
            .set_keep_alive(Duration::from_secs(5))
            .set_transport(Transport::tls_with_config(TlsConfiguration::Rustls(
                Arc::new(tls12_config()?),
            )));
        let report_topic = format!("device/{serial}/report");
        let request_topic = format!("device/{serial}/request");
        let (client, mut eventloop) = AsyncClient::new(options, 10);
        let mut subscribed = false;
        let mut requested = false;
        loop {
            match eventloop
                .poll()
                .await
                .map_err(|e| MachineError::Connection(e.to_string()))?
            {
                Event::Incoming(Incoming::ConnAck(_)) if !subscribed => {
                    client
                        .subscribe(report_topic.clone(), QoS::AtMostOnce)
                        .await
                        .map_err(|e| MachineError::Connection(e.to_string()))?;
                    subscribed = true;
                }
                Event::Incoming(Incoming::SubAck(_)) if !requested => {
                    client
                        .publish(
                            request_topic.clone(),
                            QoS::AtMostOnce,
                            false,
                            br#"{"pushing":{"sequence_id":"1","command":"pushall"}}"#,
                        )
                        .await
                        .map_err(|e| MachineError::Connection(e.to_string()))?;
                    requested = true;
                }
                Event::Incoming(Incoming::Publish(message)) if message.topic == report_topic => {
                    if let Some(trays) = parse_ams_report(&message.payload)? {
                        let _ = client.disconnect().await;
                        return Ok(trays);
                    }
                }
                _ => {}
            }
        }
    }
}

#[async_trait]
impl MachineStatusProbe for BambuMachineStatusProbe {
    async fn fetch_status(&self, link: &MachineLink) -> Result<MachineStatus, MachineError> {
        let MachineLink::BambuLan {
            host,
            access_code,
            serial,
        } = link
        else {
            return Err(MachineError::Connection(
                "Bambu probe received another machine link kind".into(),
            ));
        };
        tokio::time::timeout(PROBE_TIMEOUT, self.session(host, access_code, serial))
            .await
            .map_err(|_| MachineError::Connection("Bambu MQTT probe timed out".into()))?
    }

    async fn fetch_ams(&self, link: &MachineLink) -> Result<Vec<AmsTray>, MachineError> {
        let MachineLink::BambuLan {
            host,
            access_code,
            serial,
        } = link
        else {
            return Err(MachineError::AmsUnavailable);
        };
        tokio::time::timeout(PROBE_TIMEOUT, self.ams_session(host, access_code, serial))
            .await
            .map_err(|_| MachineError::Connection("Bambu MQTT AMS probe timed out".into()))?
    }
}

fn optional_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_ams_report(payload: &[u8]) -> Result<Option<Vec<AmsTray>>, MachineError> {
    let value: Value = serde_json::from_slice(payload)
        .map_err(|e| MachineError::Connection(format!("invalid Bambu report: {e}")))?;
    let Some(units) = value.pointer("/print/ams/ams").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut trays = Vec::new();
    for (unit_index, unit) in units.iter().enumerate() {
        let Some(unit_trays) = unit.get("tray").and_then(Value::as_array) else {
            continue;
        };
        for (tray_index, tray) in unit_trays.iter().enumerate() {
            trays.push(AmsTray {
                unit_index: unit_index as u8,
                tray_index: tray_index as u8,
                filament_type: optional_string(&tray["tray_type"]),
                color_hex: optional_string(&tray["tray_color"]).map(|value| {
                    let value = value.trim_start_matches('#');
                    format!("#{}", value.get(..6).unwrap_or(value).to_ascii_uppercase())
                }),
                sub_brand: optional_string(&tray["tray_sub_brands"]),
                remain_percent: tray["remain"].as_u64().map(|value| value.min(100) as u8),
                tag_uid: AmsTray::normalize_tag_uid(tray["tag_uid"].as_str()),
            });
        }
    }
    Ok(Some(trays))
}

fn number(value: Option<&Value>) -> Option<f32> {
    value.and_then(Value::as_f64).map(|n| n as f32)
}
fn temperature(actual: Option<&Value>, target: Option<&Value>) -> Option<Temperature> {
    number(actual).map(|actual| Temperature {
        actual,
        target: number(target),
    })
}

fn parse_complete_report(payload: &[u8]) -> Result<Option<MachineStatus>, MachineError> {
    let value: Value = serde_json::from_slice(payload)
        .map_err(|e| MachineError::Connection(format!("invalid Bambu report: {e}")))?;
    let print = &value["print"];
    let Some(state) = print["gcode_state"].as_str() else {
        return Ok(None);
    };
    let state = match state {
        "IDLE" | "FINISH" => MachineState::Idle,
        "RUNNING" => MachineState::Printing,
        "PAUSE" => MachineState::Paused,
        "FAILED" => MachineState::Error,
        _ => MachineState::Error,
    };
    let progress_percent = print["mc_percent"].as_i64().map(|n| n.clamp(0, 100) as u8);
    let remaining_seconds = print["mc_remaining_time"].as_u64().map(|n| n * 60);
    let job_name = print["subtask_name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .or_else(|| print["gcode_file"].as_str().filter(|name| !name.is_empty()))
        .map(str::to_string);
    let nozzle = temperature(
        print.get("nozzle_temper"),
        print.get("nozzle_target_temper"),
    );
    Ok(Some(MachineStatus {
        state,
        telemetry: MachineTelemetry {
            progress_percent,
            remaining_seconds,
            job_name,
            nozzle_temperatures: nozzle.into_iter().collect(),
            active_head: None,
            bed_temperature: temperature(print.get("bed_temper"), print.get("bed_target_temper")),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::{parse_ams_report, parse_complete_report};
    use domain::printers::{MachineState, Temperature};
    #[test]
    fn partial_push_is_ignored_until_gcode_state_arrives() {
        assert_eq!(
            parse_complete_report(br#"{"print":{"mc_percent":12}}"#).unwrap(),
            None
        );
    }
    #[test]
    fn running_report_maps_complete_telemetry_and_minutes() {
        let status = parse_complete_report(br#"{"print":{"gcode_state":"RUNNING","mc_percent":42,"mc_remaining_time":17,"subtask_name":"gearbox","gcode_file":"fallback.3mf","nozzle_temper":221.4,"nozzle_target_temper":220.0,"bed_temper":59.8,"bed_target_temper":60.0}}"#).unwrap().unwrap();
        assert_eq!(status.state, MachineState::Printing);
        assert_eq!(status.telemetry.progress_percent, Some(42));
        assert_eq!(status.telemetry.remaining_seconds, Some(1020));
        assert_eq!(status.telemetry.job_name.as_deref(), Some("gearbox"));
        assert_eq!(
            status.telemetry.nozzle_temperatures,
            vec![Temperature {
                actual: 221.4,
                target: Some(220.0)
            }]
        );
    }
    #[test]
    fn idle_report_uses_gcode_file_fallback() {
        let status = parse_complete_report(br#"{"print":{"gcode_state":"IDLE","mc_percent":100,"mc_remaining_time":0,"subtask_name":"","gcode_file":"finished.gcode","nozzle_temper":31.0,"nozzle_target_temper":0.0,"bed_temper":29.0,"bed_target_temper":0.0}}"#).unwrap().unwrap();
        assert_eq!(status.state, MachineState::Idle);
        assert_eq!(status.telemetry.job_name.as_deref(), Some("finished.gcode"));
    }
    #[test]
    fn unknown_state_maps_to_error() {
        let status = parse_complete_report(br#"{"print":{"gcode_state":"ALIEN"}}"#)
            .unwrap()
            .unwrap();
        assert_eq!(status.state, MachineState::Error);
    }

    #[test]
    fn parses_ams_trays_and_normalizes_zero_uid() {
        let trays = parse_ams_report(br##"{"print":{"ams":{"ams":[{"tray":[{"tray_type":"PLA","tray_color":"FF0000FF","tray_sub_brands":"PLA Basic","remain":73,"tag_uid":"ABC123"},{"tray_type":"PETG","tray_color":"00FF00","remain":5,"tag_uid":"0000000000000000"}]}]}}}"##)
            .unwrap()
            .unwrap();
        assert_eq!(trays.len(), 2);
        assert_eq!(trays[0].unit_index, 0);
        assert_eq!(trays[0].tray_index, 0);
        assert_eq!(trays[0].color_hex.as_deref(), Some("#FF0000"));
        assert_eq!(trays[0].tag_uid.as_deref(), Some("ABC123"));
        assert_eq!(trays[1].tag_uid, None);
    }

    #[test]
    fn report_without_ams_is_partial() {
        assert_eq!(
            parse_ams_report(br#"{"print":{"gcode_state":"IDLE"}}"#).unwrap(),
            None
        );
    }
}
