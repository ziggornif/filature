use domain::printers::{
    AmsSyncState, AmsTray, LoadedSpool, MachineConnectivityUseCases, Printer, PrintersUseCases,
};
use domain::shared::{PrinterId, SpoolId};
use domain::spools::{ReconcilableSpool, SpoolsUseCases};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

/// Maximum CIEDE2000 distance accepted for an AMS attribute suggestion.
///
/// A value around 10 tolerates the common AMS/stock colour sampling variance
/// while still rejecting colours that are plainly different.
pub const AMS_COLOUR_MATCH_MAX_DELTA_E: f64 = 10.0;

fn parse_hex_colour(value: &str) -> Option<[f64; 3]> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    let expanded;
    let value = if value.len() == 3 {
        expanded = value.chars().flat_map(|c| [c, c]).collect::<String>();
        expanded.as_str()
    } else {
        value
    };
    if value.len() != 6 {
        return None;
    }
    let channel = |offset| u8::from_str_radix(&value[offset..offset + 2], 16).ok();
    Some([
        f64::from(channel(0)?) / 255.0,
        f64::from(channel(2)?) / 255.0,
        f64::from(channel(4)?) / 255.0,
    ])
}

fn hex_to_lab(value: &str) -> Option<[f64; 3]> {
    let [r, g, b] = parse_hex_colour(value)?;
    let linear = |channel: f64| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(r), linear(g), linear(b));
    let (x, y, z) = (
        (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47,
        0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b,
        (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83,
    );
    let lab = |component: f64| {
        const DELTA: f64 = 6.0 / 29.0;
        if component > DELTA * DELTA * DELTA {
            component.cbrt()
        } else {
            component / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (lab(x), lab(y), lab(z));
    Some([116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)])
}

/// Returns the perceptual CIEDE2000 colour distance between two CSS hex colours.
///
/// Both `#RRGGBB`/`RRGGBB` and short `#RGB`/`RGB` forms are accepted,
/// case-insensitively. Invalid colours return infinity so they can never match.
pub fn color_delta_e(hex_a: &str, hex_b: &str) -> f64 {
    let (Some([l1, a1, b1]), Some([l2, a2, b2])) = (hex_to_lab(hex_a), hex_to_lab(hex_b)) else {
        return f64::INFINITY;
    };
    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let c_bar = (c1 + c2) / 2.0;
    let c_bar_7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar_7 / (c_bar_7 + 25_f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |b: f64, a: f64| b.atan2(a).to_degrees().rem_euclid(360.0);
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let (dl, dc) = (l2 - l1, c2p - c1p);
    let dh_degrees = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p <= h1p {
        h2p - h1p + 360.0
    } else {
        h2p - h1p - 360.0
    };
    let dh = 2.0 * (c1p * c2p).sqrt() * (dh_degrees.to_radians() / 2.0).sin();
    let (l_bar, c_bar_p) = ((l1 + l2) / 2.0, (c1p + c2p) / 2.0);
    let h_bar = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (h_bar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * h_bar).to_radians().cos()
        + 0.32 * (3.0 * h_bar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * h_bar - 63.0).to_radians().cos();
    let sl = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * c_bar_p;
    let sh = 1.0 + 0.015 * c_bar_p * t;
    let delta_theta = 30.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp();
    let c_bar_p_7 = c_bar_p.powi(7);
    let rc = 2.0 * (c_bar_p_7 / (c_bar_p_7 + 25_f64.powi(7))).sqrt();
    let rt = -rc * (2.0 * delta_theta).to_radians().sin();
    let (dl, dc, dh) = (dl / sl, dc / sc, dh / sh);
    (dl * dl + dc * dc + dh * dh + rt * dc * dh).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationKind {
    Match,
    Removed,
    Conflict,
    Attributed,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmsReconciliationRow {
    pub tray: AmsTray,
    pub local_spool: Option<LoadedSpool>,
    pub suggested_spool_id: Option<SpoolId>,
    pub kind: ReconciliationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmsReconciliation {
    pub printer: Printer,
    pub rows: Vec<AmsReconciliationRow>,
    pub spools: Vec<ReconcilableSpool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmsConfirmationAction {
    Keep,
    Unload,
    Load {
        spool_id: SpoolId,
        tag_uid: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmsConfirmation {
    pub unit_index: u8,
    pub tray_index: u8,
    pub action: AmsConfirmationAction,
}

#[derive(Debug, Error)]
pub enum AmsReconciliationError {
    #[error("{0}")]
    Machine(#[from] domain::printers::MachineError),
    #[error("{0}")]
    Spools(#[from] domain::spools::RepositoryError),
    #[error("{0}")]
    Printers(#[from] domain::printers::RepositoryError),
}

fn tray_is_empty(tray: &AmsTray) -> bool {
    tray.tag_uid.is_none() && tray.filament_type.is_none() && tray.color_hex.is_none()
}

pub fn reconcile_trays(
    trays: Vec<AmsTray>,
    printer: &Printer,
    spools: &[ReconcilableSpool],
) -> Vec<AmsReconciliationRow> {
    let local: HashMap<_, _> = printer
        .slots
        .iter()
        .filter_map(|slot| {
            slot.loaded_spool
                .as_ref()
                .map(|spool| (slot.key.as_str(), spool))
        })
        .collect();
    let mut used = HashSet::new();
    trays
        .into_iter()
        .filter_map(|tray| {
            let slot_key = format!("ams{}-{}", tray.unit_index, tray.tray_index);
            let local_spool = local.get(slot_key.as_str()).copied();
            if tray_is_empty(&tray) && local_spool.is_none() {
                return None;
            }
            let by_rfid = tray.tag_uid.as_deref().and_then(|uid| {
                spools
                    .iter()
                    .find(|spool| spool.ams_tag_uid.as_deref() == Some(uid))
            });
            let (kind, suggested) = if tray_is_empty(&tray) {
                (ReconciliationKind::Removed, None)
            } else if tray.tag_uid.is_some()
                && local_spool
                    .is_some_and(|local| local.ams_tag_uid.as_deref() == tray.tag_uid.as_deref())
            {
                (ReconciliationKind::Match, local_spool.map(|s| &s.id))
            } else if tray.tag_uid.is_some() {
                (ReconciliationKind::Conflict, by_rfid.map(|s| &s.id))
            } else {
                let attributes = spools
                    .iter()
                    .filter(|spool| {
                        !used.contains(&spool.id)
                            && !spool.loaded
                            && tray.filament_type.as_deref().is_some_and(|kind| {
                                kind.trim().eq_ignore_ascii_case(spool.material_name.trim())
                            })
                    })
                    .filter_map(|spool| {
                        let delta =
                            color_delta_e(tray.color_hex.as_deref()?, spool.colour_hex.as_deref()?);
                        (delta <= AMS_COLOUR_MATCH_MAX_DELTA_E).then_some((spool, delta))
                    })
                    .min_by(|(_, left), (_, right)| left.total_cmp(right))
                    .map(|(spool, _)| spool);
                if let Some(spool) = attributes {
                    (ReconciliationKind::Attributed, Some(&spool.id))
                } else {
                    (ReconciliationKind::None, None)
                }
            };
            if let Some(id) = suggested {
                used.insert(id.clone());
            }
            Some(AmsReconciliationRow {
                tray,
                local_spool: local_spool.cloned(),
                suggested_spool_id: suggested.cloned(),
                kind,
            })
        })
        .collect()
}

pub struct AmsReconciliationService {
    machine: Arc<dyn MachineConnectivityUseCases>,
    spools: Arc<dyn SpoolsUseCases>,
    printers: Arc<dyn PrintersUseCases>,
}

impl AmsReconciliationService {
    pub fn new(
        machine: Arc<dyn MachineConnectivityUseCases>,
        spools: Arc<dyn SpoolsUseCases>,
        printers: Arc<dyn PrintersUseCases>,
    ) -> Self {
        Self {
            machine,
            spools,
            printers,
        }
    }

    pub async fn reconcile(
        &self,
        printer_id: PrinterId,
    ) -> Result<AmsReconciliation, AmsReconciliationError> {
        let printer = self
            .printers
            .list()
            .await?
            .into_iter()
            .find(|printer| printer.id == printer_id)
            .ok_or_else(|| domain::printers::RepositoryError::NotFound(printer_id.clone()))?;
        let trays = match self.machine.fetch_ams_trays(printer_id.clone()).await {
            Ok(trays) => trays,
            Err(error) => {
                self.printers
                    .set_ams_sync_state(printer_id, AmsSyncState::Offline)
                    .await?;
                return Err(error.into());
            }
        };
        let spools = self.spools.reconcilable().await?;
        let rows = reconcile_trays(trays, &printer, &spools);
        let state = if rows.iter().all(|row| row.kind == ReconciliationKind::Match) {
            AmsSyncState::UpToDate
        } else {
            AmsSyncState::Drift
        };
        self.printers.set_ams_sync_state(printer_id, state).await?;
        Ok(AmsReconciliation {
            printer,
            rows,
            spools,
        })
    }

    pub async fn confirm(
        &self,
        printer_id: PrinterId,
        rows: Vec<AmsConfirmation>,
    ) -> Result<(), AmsReconciliationError> {
        let printer = self
            .printers
            .list()
            .await?
            .into_iter()
            .find(|printer| printer.id == printer_id)
            .ok_or_else(|| domain::printers::RepositoryError::NotFound(printer_id.clone()))?;
        let keeps_drift = rows
            .iter()
            .any(|row| row.action == AmsConfirmationAction::Keep);
        for row in rows {
            let slot_key = format!("ams{}-{}", row.unit_index, row.tray_index);
            if !printer.slots.iter().any(|slot| slot.key == slot_key) {
                return Err(domain::printers::RepositoryError::SlotNotFound {
                    printer_id,
                    slot_key,
                }
                .into());
            }
            match row.action {
                AmsConfirmationAction::Keep => {}
                AmsConfirmationAction::Unload => {
                    self.printers
                        .unload_slot(printer_id.clone(), slot_key)
                        .await?;
                }
                AmsConfirmationAction::Load { spool_id, tag_uid } => {
                    if let Some(tag_uid) = tag_uid {
                        self.spools
                            .memorize_ams_tag(spool_id.clone(), tag_uid)
                            .await?;
                    }
                    self.printers
                        .load_slot(printer_id.clone(), slot_key, spool_id)
                        .await?;
                }
            }
        }
        self.printers
            .set_ams_sync_state(
                printer_id,
                if keeps_drift {
                    AmsSyncState::Drift
                } else {
                    AmsSyncState::UpToDate
                },
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::printers::{AmsSyncState, PrinterBrand, PrinterName, Slot};
    use domain::spools::SpoolStatus;

    fn spool(id: &str, uid: Option<&str>, loaded: bool) -> ReconcilableSpool {
        ReconcilableSpool {
            id: SpoolId::new(id),
            material_name: "PLA".into(),
            colour_hex: Some("#FF0000".into()),
            ams_tag_uid: uid.map(str::to_owned),
            status: SpoolStatus::Open,
            remaining_percent: 50,
            loaded,
        }
    }
    fn loaded(id: &str, uid: Option<&str>) -> LoadedSpool {
        LoadedSpool {
            id: SpoolId::new(id),
            manufacturer_name: None,
            colour_hex: Some("#FF0000".into()),
            colour_name: None,
            material_name: "PLA".into(),
            remaining_weight: 50.,
            net_weight: 100.,
            status: "Open".into(),
            ams_tag_uid: uid.map(str::to_owned),
        }
    }
    fn printer(local: Option<LoadedSpool>) -> Printer {
        Printer {
            id: PrinterId::new("p"),
            name: PrinterName::new("P1S").unwrap(),
            brand: PrinterBrand::BambuLab,
            model: "P1S".into(),
            heads: 1,
            module: domain::printers::Module::None,
            ams_units: 1,
            feed_modes: vec![],
            machine_link: None,
            ams_sync_state: AmsSyncState::Offline,
            slots: vec![Slot {
                key: "ams0-0".into(),
                group_label: "ams0".into(),
                position: 0,
                loaded_spool: local,
            }],
        }
    }
    fn tray(uid: Option<&str>, material: Option<&str>) -> AmsTray {
        AmsTray {
            unit_index: 0,
            tray_index: 0,
            filament_type: material.map(str::to_owned),
            color_hex: material.map(|_| "#FF0000".into()),
            sub_brand: None,
            remain_percent: Some(60),
            tag_uid: AmsTray::normalize_tag_uid(uid),
        }
    }

    #[test]
    fn classifies_all_five_states_and_omits_double_empty() {
        let known = spool("known", Some("UID2"), false);
        assert_eq!(
            reconcile_trays(
                vec![tray(Some("UID1"), Some("PLA"))],
                &printer(Some(loaded("local", Some("UID1")))),
                std::slice::from_ref(&known)
            )[0]
            .kind,
            ReconciliationKind::Match
        );
        assert_eq!(
            reconcile_trays(
                vec![tray(None, None)],
                &printer(Some(loaded("local", None))),
                std::slice::from_ref(&known)
            )[0]
            .kind,
            ReconciliationKind::Removed
        );
        assert_eq!(
            reconcile_trays(
                vec![tray(Some("UID2"), Some("PLA"))],
                &printer(Some(loaded("local", Some("UID1")))),
                std::slice::from_ref(&known)
            )[0]
            .kind,
            ReconciliationKind::Conflict
        );
        assert_eq!(
            reconcile_trays(vec![tray(None, Some("PLA"))], &printer(None), &[known])[0].kind,
            ReconciliationKind::Attributed
        );
        assert_eq!(
            reconcile_trays(vec![tray(None, Some("ABS"))], &printer(None), &[])[0].kind,
            ReconciliationKind::None
        );
        assert!(reconcile_trays(vec![tray(None, None)], &printer(None), &[]).is_empty());
    }

    #[test]
    fn ciede2000_accepts_supported_hex_forms_and_separates_colours() {
        assert_eq!(color_delta_e("#ff0000", "F00"), 0.0);
        assert!(color_delta_e("#ff0000", "#f31a18") < AMS_COLOUR_MATCH_MAX_DELTA_E);
        assert!(color_delta_e("#ff0000", "#0000ff") > AMS_COLOUR_MATCH_MAX_DELTA_E);
        assert!(color_delta_e("not-a-colour", "#000").is_infinite());
    }

    #[test]
    fn attribute_match_chooses_closest_colour_below_threshold() {
        let mut farther = spool("farther", None, false);
        farther.colour_hex = Some("#e62920".into());
        let mut closest = spool("closest", None, false);
        closest.colour_hex = Some("#fb0908".into());
        let rows = reconcile_trays(
            vec![tray(None, Some("PLA"))],
            &printer(None),
            &[farther, closest],
        );
        assert_eq!(rows[0].kind, ReconciliationKind::Attributed);
        assert_eq!(
            rows[0].suggested_spool_id.as_ref().map(SpoolId::as_str),
            Some("closest")
        );
    }

    #[test]
    fn attribute_match_rejects_distant_colour_and_different_material() {
        let mut blue = spool("blue", None, false);
        blue.colour_hex = Some("#0000ff".into());
        assert_eq!(
            reconcile_trays(vec![tray(None, Some("PLA"))], &printer(None), &[blue])[0].kind,
            ReconciliationKind::None
        );

        let mut abs = spool("abs", None, false);
        abs.material_name = "ABS".into();
        assert_eq!(
            reconcile_trays(vec![tray(None, Some("PLA"))], &printer(None), &[abs])[0].kind,
            ReconciliationKind::None
        );
    }
}
