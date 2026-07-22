use async_trait::async_trait;
use domain::printers::{
    MachineError, MachineLink, MachineState, MachineStatus, MachineStatusProbe, MachineTelemetry,
    Temperature,
};
use reqwest::{Client, redirect::Policy};
use serde_json::Value;
use std::time::Duration;

pub struct RestMachineStatusProbe {
    client: Client,
}

impl RestMachineStatusProbe {
    pub fn new() -> Result<Self, reqwest::Error> {
        Client::builder()
            .timeout(Duration::from_millis(2500))
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map(|client| Self { client })
    }

    async fn json(&self, url: &str, key: Option<&str>) -> Result<Value, MachineError> {
        let mut request = self.client.get(url);
        if let Some(key) = key {
            request = request.header("X-Api-Key", key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| MachineError::Connection(e.to_string()))?;
        if !response.status().is_success() {
            return Err(MachineError::Connection(format!(
                "machine returned HTTP {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|e| MachineError::Connection(e.to_string()))
    }

    async fn prusa(&self, host: &str, key: &str) -> Result<MachineStatus, MachineError> {
        let base = normalize_base(host);
        let base = base.trim_end_matches('/');
        let status_url = format!("{base}/api/v1/status");
        let job_url = format!("{base}/api/v1/job");
        let (status, job) = tokio::join!(
            self.json(&status_url, Some(key)),
            self.json(&job_url, Some(key))
        );
        let status = status?;
        let job = job.unwrap_or(Value::Null);
        Ok(MachineStatus {
            state: parse_state(
                status
                    .pointer("/printer/state")
                    .and_then(Value::as_str)
                    .or_else(|| status.get("state").and_then(Value::as_str)),
            ),
            telemetry: MachineTelemetry {
                // PrusaLink v1 (`/api/v1/job`): progress is a bare 0-100 number,
                // remaining is `time_remaining`. Older OctoPrint-compatible
                // firmwares nest them under `progress.{completion,printTimeLeft}`.
                progress_percent: percent(
                    job.get("progress")
                        .and_then(Value::as_f64)
                        .or_else(|| job.pointer("/progress/completion").and_then(Value::as_f64)),
                ),
                remaining_seconds: job.get("time_remaining").and_then(Value::as_u64).or_else(
                    || {
                        job.pointer("/progress/printTimeLeft")
                            .and_then(Value::as_u64)
                    },
                ),
                job_name: job
                    .pointer("/file/display_name")
                    .and_then(Value::as_str)
                    .or_else(|| job.pointer("/file/name").and_then(Value::as_str))
                    .map(str::to_owned),
                nozzle_temperatures: vec![
                    temperature(status.pointer("/temperature/tool0"))
                        .or_else(|| scalar_temperature(status.pointer("/printer/temp_nozzle"))),
                ]
                .into_iter()
                .flatten()
                .collect(),
                active_head: None,
                bed_temperature: temperature(status.pointer("/temperature/bed"))
                    .or_else(|| scalar_temperature(status.pointer("/printer/temp_bed"))),
            },
        })
    }

    async fn moonraker(&self, url: &str) -> Result<MachineStatus, MachineError> {
        let value = self
            .json(
                &format!("{}/printer/objects/query?print_stats&virtual_sdcard&extruder&heater_bed&toolhead", normalize_base(url).trim_end_matches('/')),
                None,
            )
            .await?;
        let status = value.pointer("/result/status").unwrap_or(&value);
        Ok(MachineStatus {
            state: parse_state(status.pointer("/print_stats/state").and_then(Value::as_str)),
            telemetry: MachineTelemetry {
                progress_percent: percent(
                    status
                        .pointer("/virtual_sdcard/progress")
                        .and_then(Value::as_f64)
                        .map(|value| value * 100.0),
                ),
                remaining_seconds: remaining_seconds(status),
                job_name: status
                    .pointer("/print_stats/filename")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
                nozzle_temperatures: vec![temperature(status.get("extruder"))]
                    .into_iter()
                    .flatten()
                    .collect(),
                active_head: None,
                bed_temperature: temperature(status.get("heater_bed")),
            },
        })
    }
}

/// Users paste bare IPs/hostnames ("192.168.1.71") as often as full URLs;
/// reqwest rejects scheme-less URLs instantly, so default to http:// (the
/// PrusaLink/Moonraker LAN norm) when no scheme is given.
fn normalize_base(value: &str) -> String {
    let value = value.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

fn parse_state(value: Option<&str>) -> MachineState {
    // PrusaLink: IDLE BUSY PRINTING PAUSED FINISHED STOPPED ERROR ATTENTION
    // Moonraker print_stats: standby printing paused complete cancelled error
    match value.unwrap_or("").to_ascii_lowercase().as_str() {
        "printing" => MachineState::Printing,
        "paused" => MachineState::Paused,
        "idle" | "ready" | "operational" | "standby" | "complete" | "finished" | "stopped"
        | "busy" | "cancelled" => MachineState::Idle,
        "error" | "shutdown" | "attention" => MachineState::Error,
        _ => MachineState::Offline,
    }
}

fn percent(value: Option<f64>) -> Option<u8> {
    value.map(|value| value.round().clamp(0., 100.) as u8)
}

fn temperature(value: Option<&Value>) -> Option<Temperature> {
    let value = value?;
    Some(Temperature {
        actual: value
            .get("actual")
            .or_else(|| value.get("temperature"))?
            .as_f64()? as f32,
        target: value
            .get("target")
            .and_then(Value::as_f64)
            .map(|value| value as f32),
    })
}

fn scalar_temperature(value: Option<&Value>) -> Option<Temperature> {
    Some(Temperature {
        actual: value?.as_f64()? as f32,
        target: None,
    })
}

fn remaining_seconds(status: &Value) -> Option<u64> {
    let progress = status.pointer("/virtual_sdcard/progress")?.as_f64()?;
    let elapsed = status.pointer("/print_stats/print_duration")?.as_f64()?;
    if progress <= 0.0 {
        return None;
    }
    Some((elapsed * (1.0 - progress) / progress).max(0.0).round() as u64)
}

#[async_trait]
impl MachineStatusProbe for RestMachineStatusProbe {
    async fn fetch_status(&self, link: &MachineLink) -> Result<MachineStatus, MachineError> {
        match link {
            MachineLink::PrusaLink { host, api_key } => self.prusa(host, api_key).await,
            MachineLink::Moonraker { url } => self.moonraker(url).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::printers::MachineStatusProbe;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn bare_hosts_default_to_http_scheme() {
        assert_eq!(normalize_base("192.168.1.71"), "http://192.168.1.71");
        assert_eq!(normalize_base(" prusa.local "), "http://prusa.local");
        assert_eq!(
            normalize_base("http://192.168.1.71/"),
            "http://192.168.1.71/"
        );
        assert_eq!(normalize_base("https://voron.lan"), "https://voron.lan");
    }

    async fn fake(response: &'static str) -> Option<String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok()?;
        let address = listener.local_addr().ok()?;
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        Some(format!("http://{address}"))
    }

    #[test]
    fn maps_vendor_states() {
        assert_eq!(parse_state(Some("PRINTING")), MachineState::Printing);
        assert_eq!(parse_state(Some("shutdown")), MachineState::Error);
    }

    #[tokio::test]
    async fn moonraker_status_is_mapped_from_a_fake_http_server() {
        let body = r#"{"result":{"status":{"print_stats":{"state":"printing","filename":"part.gcode"},"virtual_sdcard":{"progress":0.42},"extruder":{"temperature":211.0,"target":215.0},"heater_bed":{"temperature":59.0,"target":60.0}}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = Box::leak(response.into_boxed_str());
        let Some(url) = fake(response).await else {
            return;
        };
        let status = RestMachineStatusProbe::new()
            .unwrap()
            .fetch_status(&MachineLink::Moonraker { url })
            .await
            .unwrap();
        assert_eq!(status.state, MachineState::Printing);
        assert_eq!(status.telemetry.progress_percent, Some(42));
        assert_eq!(status.telemetry.job_name.as_deref(), Some("part.gcode"));
        assert_eq!(status.active_nozzle().unwrap().actual, 211.0);
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let Some(url) = fake(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/secret\r\nContent-Length: 0\r\n\r\n",
        )
        .await
        else {
            return;
        };
        let result = RestMachineStatusProbe::new()
            .unwrap()
            .fetch_status(&MachineLink::Moonraker { url })
            .await;
        assert!(result.is_err());
    }
}
