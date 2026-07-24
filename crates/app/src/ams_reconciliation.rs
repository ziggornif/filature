use domain::printers::{AmsTray, MachineConnectivityUseCases, PrintersUseCases};
use domain::shared::{PrinterId, SpoolId};
use domain::spools::{ReconcilableSpool, SpoolsUseCases};
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Rfid,
    Attributes,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmsMatch {
    pub tray: AmsTray,
    pub suggested_spool_id: Option<SpoolId>,
    pub kind: Option<MatchKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmsConfirmation {
    pub unit_index: u8,
    pub tray_index: u8,
    pub spool_id: SpoolId,
    pub tag_uid: Option<String>,
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

pub fn match_trays(trays: Vec<AmsTray>, spools: &[ReconcilableSpool]) -> Vec<AmsMatch> {
    let mut used = HashSet::new();
    trays
        .into_iter()
        .map(|tray| {
            let rfid = tray.tag_uid.as_deref().and_then(|uid| {
                spools.iter().find(|spool| {
                    !used.contains(&spool.id)
                        && spool
                            .ams_tag_uid
                            .as_deref()
                            .is_some_and(|known| known == uid)
                })
            });
            let attributes = || {
                spools.iter().find(|spool| {
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
                })
            };
            let (spool, kind) = if let Some(spool) = rfid {
                (Some(spool), Some(MatchKind::Rfid))
            } else if let Some(spool) = attributes() {
                (Some(spool), Some(MatchKind::Attributes))
            } else {
                (None, None)
            };
            let suggested_spool_id = spool.map(|spool| {
                used.insert(spool.id.clone());
                spool.id.clone()
            });
            AmsMatch {
                tray,
                suggested_spool_id,
                kind,
            }
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
    ) -> Result<(Vec<AmsMatch>, Vec<ReconcilableSpool>), AmsReconciliationError> {
        let trays = self.machine.fetch_ams_trays(printer_id).await?;
        let spools = self.spools.reconcilable().await?;
        Ok((match_trays(trays, &spools), spools))
    }

    pub async fn confirm(
        &self,
        printer_id: PrinterId,
        rows: Vec<AmsConfirmation>,
    ) -> Result<(), AmsReconciliationError> {
        for row in rows {
            if let Some(tag_uid) = row.tag_uid {
                self.spools
                    .memorize_ams_tag(row.spool_id.clone(), tag_uid)
                    .await?;
            }
            self.printers
                .load_slot(
                    printer_id.clone(),
                    format!("ams{}-{}", row.unit_index, row.tray_index),
                    row.spool_id,
                )
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::spools::SpoolStatus;

    fn spool(id: &str, material: &str, colour: &str, uid: Option<&str>) -> ReconcilableSpool {
        ReconcilableSpool {
            id: SpoolId::new(id),
            material_name: material.into(),
            colour_hex: Some(colour.into()),
            ams_tag_uid: uid.map(str::to_owned),
            status: SpoolStatus::Open,
            remaining_percent: 50,
            loaded: false,
        }
    }

    fn tray(uid: Option<&str>, material: &str, colour: &str) -> AmsTray {
        AmsTray {
            unit_index: 0,
            tray_index: 0,
            filament_type: Some(material.into()),
            color_hex: Some(colour.into()),
            sub_brand: None,
            remain_percent: Some(60),
            tag_uid: AmsTray::normalize_tag_uid(uid),
        }
    }

    #[test]
    fn rfid_wins_over_attributes() {
        let spools = vec![
            spool("attributes", "PLA", "#FF0000", None),
            spool("rfid", "PETG", "#00FF00", Some("REAL")),
        ];
        let matched = match_trays(vec![tray(Some("REAL"), "PLA", "#FF0000")], &spools);
        assert_eq!(matched[0].suggested_spool_id, Some(SpoolId::new("rfid")));
        assert_eq!(matched[0].kind, Some(MatchKind::Rfid));
    }

    #[test]
    fn attributes_match_and_spool_is_only_suggested_once() {
        let spools = vec![spool("one", "PLA", "#FF0000", None)];
        let matched = match_trays(
            vec![
                tray(None, "pla", "#ff0000"),
                tray(Some("000000"), "PLA", "#FF0000"),
            ],
            &spools,
        );
        assert_eq!(matched[0].kind, Some(MatchKind::Attributes));
        assert_eq!(matched[1].suggested_spool_id, None);
    }

    #[test]
    fn no_candidate_means_no_suggestion() {
        let matched = match_trays(
            vec![tray(None, "ABS", "#FFFFFF")],
            &[spool("pla", "PLA", "#000000", None)],
        );
        assert_eq!(matched[0].suggested_spool_id, None);
        assert_eq!(matched[0].kind, None);
    }
}
