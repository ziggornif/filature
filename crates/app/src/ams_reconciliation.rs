use domain::printers::{
    AmsSyncState, AmsTray, LoadedSpool, MachineConnectivityUseCases, Printer, PrintersUseCases,
};
use domain::shared::{PrinterId, SpoolId};
use domain::spools::{ReconcilableSpool, SpoolsUseCases};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

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
                let attributes = spools.iter().find(|spool| {
                    !used.contains(&spool.id)
                        && !spool.loaded
                        && tray.filament_type.as_deref().is_some_and(|kind| {
                            kind.trim().eq_ignore_ascii_case(spool.material_name.trim())
                        })
                        && tray.color_hex.as_deref().is_some_and(|colour| {
                            spool
                                .colour_hex
                                .as_deref()
                                .is_some_and(|known| colour.eq_ignore_ascii_case(known))
                        })
                });
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
}
