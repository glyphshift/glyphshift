use super::*;
use crate::probe::{
    probe_run_error, ProbeDictionaryBindingRequest, ProbeRunCreateRequest, ProbeRunView,
};
use crate::software::SoftwarePreflightState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

const QUICK_PROBE_SCHEMA: &str = "glyphshift.quick-probe-sessions/1";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum QuickProbePhase {
    Preparing,
    Active,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct QuickProbeRecord {
    run_id: Box<str>,
    software_id: Option<Box<str>>,
    dictionary_id: Box<str>,
    executable_path: Box<str>,
    owns_software: bool,
    owns_dictionary: bool,
    phase: QuickProbePhase,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuickProbeLedger {
    schema: Box<str>,
    sessions: Vec<QuickProbeRecord>,
}

pub(super) struct QuickProbeSessionStore {
    path: PathBuf,
    sessions: BTreeMap<Box<str>, QuickProbeRecord>,
}

impl QuickProbeSessionStore {
    pub(super) fn open(data_root: &Path) -> Result<Self, CommandError> {
        let path = data_root.join("quick-probe-sessions.json");
        let pending = path.with_extension("pending.json");
        let backup = path.with_extension("backup.json");
        if !path.exists() && !pending.exists() && !backup.exists() {
            return Ok(Self {
                path,
                sessions: BTreeMap::new(),
            });
        }
        let (ledger, recovered_from) = if path.exists() {
            (read_quick_probe_ledger(&path)?, None)
        } else if pending.exists() {
            match read_quick_probe_ledger(&pending) {
                Ok(ledger) => (ledger, Some(pending.clone())),
                Err(_) if backup.exists() => {
                    (read_quick_probe_ledger(&backup)?, Some(backup.clone()))
                }
                Err(error) => return Err(error),
            }
        } else {
            (read_quick_probe_ledger(&backup)?, Some(backup.clone()))
        };
        if let Some(recovered_from) = recovered_from {
            if pending.exists() && recovered_from != pending {
                let _ = fs::remove_file(&pending);
            }
            fs::rename(&recovered_from, &path)
                .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        }
        if backup.exists() {
            let _ = fs::remove_file(&backup);
        }
        if pending.exists() {
            let _ = fs::remove_file(&pending);
        }
        let sessions = ledger
            .sessions
            .into_iter()
            .map(|record| (record.run_id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        Ok(Self { path, sessions })
    }

    pub(super) fn contains(&self, run_id: &str) -> bool {
        self.sessions.contains_key(run_id)
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn records(&self) -> Vec<QuickProbeRecord> {
        self.sessions.values().cloned().collect()
    }

    fn record(&self, run_id: &str) -> Option<QuickProbeRecord> {
        self.sessions.get(run_id).cloned()
    }

    fn save(&mut self, record: QuickProbeRecord) -> Result<(), CommandError> {
        let run_id = record.run_id.clone();
        let previous = self.sessions.insert(run_id.clone(), record);
        if let Err(error) = self.persist() {
            if let Some(previous) = previous {
                self.sessions.insert(run_id, previous);
            } else {
                self.sessions.remove(run_id.as_ref());
            }
            return Err(error);
        }
        Ok(())
    }

    fn remove(&mut self, run_id: &str) -> Result<(), CommandError> {
        let Some(record) = self.sessions.remove(run_id) else {
            return Ok(());
        };
        if let Err(error) = self.persist() {
            self.sessions.insert(record.run_id.clone(), record);
            return Err(error);
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), CommandError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        }
        let ledger = QuickProbeLedger {
            schema: QUICK_PROBE_SCHEMA.into(),
            sessions: self.sessions.values().cloned().collect(),
        };
        let source = serde_json::to_string(&ledger)
            .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        let pending = self.path.with_extension("pending.json");
        let backup = self.path.with_extension("backup.json");
        let mut pending_file = fs::File::create(&pending)
            .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        pending_file
            .write_all(source.as_bytes())
            .and_then(|_| pending_file.sync_all())
            .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        drop(pending_file);
        if self.path.exists() {
            if backup.exists() {
                fs::remove_file(&backup)
                    .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
            }
            fs::rename(&self.path, &backup)
                .map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
        }
        if fs::rename(&pending, &self.path).is_err() {
            if backup.exists() {
                let _ = fs::rename(&backup, &self.path);
            }
            return Err(CommandError::new("quick_probe.storage_failed"));
        }
        if backup.exists() {
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }
}

fn read_quick_probe_ledger(path: &Path) -> Result<QuickProbeLedger, CommandError> {
    let source =
        fs::read_to_string(path).map_err(|_| CommandError::new("quick_probe.storage_failed"))?;
    let ledger: QuickProbeLedger = serde_json::from_str(&source)
        .map_err(|_| CommandError::new("quick_probe.invalid_ledger"))?;
    if ledger.schema.as_ref() != QUICK_PROBE_SCHEMA {
        return Err(CommandError::new("quick_probe.invalid_ledger"));
    }
    Ok(ledger)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuickProbeStartRequest {
    pub(super) executable_path: String,
    pub(super) target_locale: Box<str>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum QuickProbeAssetDisposition {
    Removed,
    Reused,
    Retained,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuickProbeCleanupView {
    pub(super) software: QuickProbeAssetDisposition,
    pub(super) dictionary: QuickProbeAssetDisposition,
}

impl DesktopApplication {
    pub(super) fn start_quick_probe(
        &mut self,
        request: QuickProbeStartRequest,
    ) -> Result<ProbeRunView, CommandError> {
        self.validate_quick_probe_request(&request)?;
        let preflight = self.software_preflight(&request.executable_path)?;
        self.validate_quick_probe_preflight(&preflight)?;
        self.start_quick_probe_preflighted(request)
    }

    #[cfg(test)]
    pub(super) fn start_quick_probe_for_test(
        &mut self,
        request: QuickProbeStartRequest,
    ) -> Result<ProbeRunView, CommandError> {
        self.validate_quick_probe_request(&request)?;
        let mut preflight = self.software_preflight(&request.executable_path)?;
        preflight.running = true;
        if preflight.state == SoftwarePreflightState::NotRunning {
            preflight.state = SoftwarePreflightState::Ready;
            preflight.can_add = true;
        }
        self.validate_quick_probe_preflight(&preflight)?;
        self.start_quick_probe_preflighted(request)
    }

    fn validate_quick_probe_request(
        &self,
        request: &QuickProbeStartRequest,
    ) -> Result<(), CommandError> {
        let target_locale = request.target_locale.trim();
        if target_locale.is_empty() || target_locale.chars().count() > 64 {
            return Err(CommandError::new("quick_probe.invalid_locale"));
        }
        Ok(())
    }

    fn validate_quick_probe_preflight(
        &self,
        preflight: &software::SoftwarePreflightView,
    ) -> Result<(), CommandError> {
        if preflight.state == SoftwarePreflightState::SelfTarget {
            return Err(software::software_preflight_error(preflight));
        }
        if !self.supports_probe_architecture(preflight.architecture.as_ref()) {
            return Err(CommandError::new("software.unsupported_architecture")
                .with_arg("architecture", preflight.architecture.to_string()));
        }
        if self.runtimes.is_none() {
            return Err(CommandError::new("software.runtime_unavailable"));
        }
        if !preflight.running {
            return Err(CommandError::new("software.not_running"));
        }
        if !matches!(
            preflight.state,
            SoftwarePreflightState::Ready | SoftwarePreflightState::AlreadyAdded
        ) {
            return Err(software::software_preflight_error(preflight));
        }
        Ok(())
    }

    fn start_quick_probe_preflighted(
        &mut self,
        request: QuickProbeStartRequest,
    ) -> Result<ProbeRunView, CommandError> {
        let target_locale = request.target_locale.trim();
        let snapshot = self.backend.snapshot();
        let existing_software_id = snapshot.software().iter().find_map(|software| {
            software
                .executable_path()
                .filter(|path| {
                    software::same_windows_path(
                        Path::new(path),
                        Path::new(&request.executable_path),
                    )
                })
                .map(|_| Box::<str>::from(software.id()))
        });
        let suffix = self.next_quick_probe_suffix();
        let run_id: Box<str> = format!("quick-probe-{suffix}").into();
        let dictionary_id: Box<str> = format!("quick-dictionary-{suffix}").into();
        let mut record = QuickProbeRecord {
            run_id: run_id.clone(),
            software_id: existing_software_id.clone(),
            dictionary_id: dictionary_id.clone(),
            executable_path: request.executable_path.clone().into(),
            owns_software: existing_software_id.is_none(),
            owns_dictionary: true,
            phase: QuickProbePhase::Preparing,
        };
        self.quick_probe_sessions.save(record.clone())?;

        let result = (|| {
            let software_id = if let Some(software_id) = existing_software_id {
                software_id
            } else {
                self.backend
                    .add_software(ExecutableSelection::new(&request.executable_path))
                    .map_err(|_| CommandError::new("software.invalid_executable"))?;
                let software_id = self
                    .backend
                    .snapshot()
                    .software()
                    .iter()
                    .find_map(|software| {
                        software
                            .executable_path()
                            .filter(|path| {
                                software::same_windows_path(
                                    Path::new(path),
                                    Path::new(&request.executable_path),
                                )
                            })
                            .map(|_| Box::<str>::from(software.id()))
                    })
                    .ok_or_else(|| CommandError::new("quick_probe.start_failed"))?;
                record.software_id = Some(software_id.clone());
                self.quick_probe_sessions.save(record.clone())?;
                software_id
            };
            let adapter_ids = self.compatible_probe_adapter_ids(&software_id)?;
            if adapter_ids.is_empty() {
                return Err(CommandError::new("capture.adapters_required"));
            }
            let software_name = self
                .backend
                .snapshot()
                .software()
                .iter()
                .find(|software| software.id() == software_id.as_ref())
                .map(|software| Box::<str>::from(software.name()))
                .ok_or_else(|| CommandError::new("capture.unknown_software"))?;
            self.backend
                .create_dictionary(DictionaryCreate::new(
                    dictionary_id.clone(),
                    format!("{software_name} 临时词典"),
                    "auto",
                    target_locale,
                ))
                .map_err(|_| CommandError::new("dictionary.invalid_create"))?;
            let view = self.create_probe_run(ProbeRunCreateRequest {
                id: run_id.clone(),
                name: format!("{software_name} 快速测试").into(),
                software_id,
                adapter_ids,
                live_preview_enabled: false,
                dictionary: ProbeDictionaryBindingRequest::Existing {
                    dictionary_id: dictionary_id.clone(),
                },
            })?;
            if view.summary.status() != ProbeRunStatus::Running {
                return Err(CommandError::new("quick_probe.target_stopped"));
            }
            record.phase = QuickProbePhase::Active;
            self.quick_probe_sessions.save(record.clone())?;
            Ok(view)
        })();

        if result.is_err() {
            let _ = self.cleanup_quick_probe(run_id.as_ref());
        }
        result
    }

    pub(super) fn retain_quick_probe(
        &mut self,
        run_id: &str,
    ) -> Result<ProbeRunView, CommandError> {
        if !self.quick_probe_sessions.contains(run_id) {
            return Err(CommandError::new("quick_probe.not_found"));
        }
        let view = self.probe_run_summary(run_id)?;
        self.quick_probe_sessions.remove(run_id)?;
        Ok(ProbeRunView {
            quick_probe: false,
            ..view
        })
    }

    pub(super) fn cleanup_quick_probe(
        &mut self,
        run_id: &str,
    ) -> Result<QuickProbeCleanupView, CommandError> {
        let mut record = self
            .quick_probe_sessions
            .record(run_id)
            .ok_or_else(|| CommandError::new("quick_probe.not_found"))?;
        if self.active_probe_run_id.as_deref() == Some(run_id) {
            self.disconnect_probe_run(run_id)?;
        }
        if self.probe_runs.summary(run_id).is_ok() {
            self.delete_probe_runs(&[run_id.into()])?;
        }

        let dictionary = if record.owns_dictionary
            && self
                .backend
                .dictionary(record.dictionary_id.as_ref())
                .is_ok()
        {
            let workflow_referenced = self.backend.snapshot().workflows().iter().any(|workflow| {
                workflow
                    .dictionary_ids()
                    .iter()
                    .any(|dictionary_id| dictionary_id.as_ref() == record.dictionary_id.as_ref())
            });
            let probe_referenced = self
                .probe_runs
                .list()
                .map_err(probe_run_error)?
                .iter()
                .any(|probe| probe.dictionary_id() == record.dictionary_id.as_ref());
            if workflow_referenced || probe_referenced {
                QuickProbeAssetDisposition::Retained
            } else {
                self.backend
                    .delete_dictionaries([record.dictionary_id.as_ref()])
                    .map_err(|_| CommandError::new("quick_probe.cleanup_failed"))?;
                QuickProbeAssetDisposition::Removed
            }
        } else {
            QuickProbeAssetDisposition::Reused
        };

        if record.software_id.is_none() && record.owns_software {
            record.software_id = self
                .backend
                .snapshot()
                .software()
                .iter()
                .find_map(|software| {
                    software
                        .executable_path()
                        .filter(|path| {
                            software::same_windows_path(
                                Path::new(path),
                                Path::new(record.executable_path.as_ref()),
                            )
                        })
                        .map(|_| Box::<str>::from(software.id()))
                });
        }
        let software = if record.owns_software {
            if let Some(software_id) = record.software_id.as_deref() {
                let snapshot = self.backend.snapshot();
                let workflow_referenced = snapshot.workflows().iter().any(|workflow| {
                    workflow
                        .targets()
                        .iter()
                        .any(|target| target.software_id() == software_id)
                });
                let probe_referenced = self
                    .probe_runs
                    .list()
                    .map_err(probe_run_error)?
                    .iter()
                    .any(|probe| probe.software_id() == software_id);
                if workflow_referenced || probe_referenced {
                    QuickProbeAssetDisposition::Retained
                } else {
                    if let Some(runtimes) = self.runtimes.as_mut() {
                        runtimes
                            .remove_software(software_id)
                            .map_err(|_| CommandError::new("quick_probe.cleanup_failed"))?;
                    }
                    self.backend
                        .remove_software(software_id)
                        .map_err(|_| CommandError::new("quick_probe.cleanup_failed"))?;
                    QuickProbeAssetDisposition::Removed
                }
            } else {
                QuickProbeAssetDisposition::Removed
            }
        } else {
            QuickProbeAssetDisposition::Reused
        };
        self.quick_probe_sessions.remove(run_id)?;
        Ok(QuickProbeCleanupView {
            software,
            dictionary,
        })
    }

    pub(super) fn recover_quick_probe_sessions(&mut self) -> Result<(), CommandError> {
        for record in self.quick_probe_sessions.records() {
            if self.probe_runs.summary(record.run_id.as_ref()).is_err() {
                self.cleanup_quick_probe(record.run_id.as_ref())?;
            }
        }
        Ok(())
    }

    fn next_quick_probe_suffix(&mut self) -> u64 {
        let mut candidate = glyphshift_capture::unix_time_millis();
        loop {
            let run_id = format!("quick-probe-{candidate}");
            if !self.quick_probe_sessions.contains(&run_id)
                && self.probe_runs.summary(&run_id).is_err()
            {
                return candidate;
            }
            candidate = candidate.saturating_add(1);
        }
    }
}

#[tauri::command]
pub(super) fn desktop_start_quick_probe(
    request: QuickProbeStartRequest,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .start_quick_probe(request)
}

#[tauri::command]
pub(super) fn desktop_retain_quick_probe(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<ProbeRunView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .retain_quick_probe(&run_id)
}

#[tauri::command]
pub(super) fn desktop_cleanup_quick_probe(
    run_id: String,
    application: State<'_, Mutex<DesktopApplication>>,
) -> Result<QuickProbeCleanupView, CommandError> {
    application
        .lock()
        .map_err(|_| workspace_unavailable())?
        .cleanup_quick_probe(&run_id)
}

#[cfg(test)]
mod store_tests {
    use super::*;
    use tempfile::tempdir;

    fn record() -> QuickProbeRecord {
        QuickProbeRecord {
            run_id: "quick-probe-test".into(),
            software_id: Some("software.test".into()),
            dictionary_id: "dictionary.test".into(),
            executable_path: "<authorized-executable>".into(),
            owns_software: false,
            owns_dictionary: true,
            phase: QuickProbePhase::Preparing,
        }
    }

    #[test]
    fn ledger_recovers_the_pending_commit_after_the_primary_was_backed_up() {
        let root = tempdir().expect("temporary ledger root");
        let mut store = QuickProbeSessionStore::open(root.path()).expect("open ledger");
        store.save(record()).expect("save ledger");
        let pending = store.path.with_extension("pending.json");
        let backup = store.path.with_extension("backup.json");
        fs::copy(&store.path, &pending).expect("copy pending ledger");
        fs::rename(&store.path, &backup).expect("move primary to backup");

        let recovered = QuickProbeSessionStore::open(root.path()).expect("recover pending ledger");

        assert!(recovered.contains("quick-probe-test"));
        assert!(recovered.path.exists());
        assert!(!pending.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn ledger_falls_back_to_the_backup_when_the_pending_commit_is_invalid() {
        let root = tempdir().expect("temporary ledger root");
        let mut store = QuickProbeSessionStore::open(root.path()).expect("open ledger");
        store.save(record()).expect("save ledger");
        let pending = store.path.with_extension("pending.json");
        let backup = store.path.with_extension("backup.json");
        fs::rename(&store.path, &backup).expect("move primary to backup");
        fs::write(&pending, b"incomplete").expect("write interrupted pending ledger");

        let recovered = QuickProbeSessionStore::open(root.path()).expect("recover backup ledger");

        assert!(recovered.contains("quick-probe-test"));
        assert!(recovered.path.exists());
        assert!(!pending.exists());
        assert!(!backup.exists());
    }
}
