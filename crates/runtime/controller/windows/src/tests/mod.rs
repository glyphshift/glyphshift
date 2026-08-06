use crate::controller::{ProcessInventory, ProcessRecord, WindowsController};
use glyphshift_controller_sdk::{
    ControllerPlugin, PluginError, WireAdapterRequirement, WireControllerConfiguration, WireFeature,
};
use std::collections::VecDeque;

struct SyntheticProcessInventory {
    snapshots: VecDeque<Vec<ProcessRecord>>,
}

impl SyntheticProcessInventory {
    fn new(snapshots: impl IntoIterator<Item = Vec<ProcessRecord>>) -> Self {
        Self {
            snapshots: snapshots.into_iter().collect(),
        }
    }
}

impl ProcessInventory for SyntheticProcessInventory {
    fn snapshot(&mut self) -> Result<Vec<ProcessRecord>, PluginError> {
        Ok(self.snapshots.pop_front().unwrap_or_default())
    }
}

fn process(
    process_id: u32,
    parent_process_id: u32,
    started_at: Option<u64>,
    executable_name: &str,
) -> ProcessRecord {
    ProcessRecord {
        process_id,
        parent_process_id,
        started_at,
        executable_name: executable_name.into(),
        executable_path: None,
        architecture: "x86_64".into(),
    }
}

fn process_family_controller(
    snapshots: impl IntoIterator<Item = Vec<ProcessRecord>>,
) -> WindowsController {
    let mut controller = WindowsController::with_process_inventory(Box::new(
        SyntheticProcessInventory::new(snapshots),
    ));
    controller
        .configure(
            "org.example.process-family",
            &WireControllerConfiguration {
                executable_names: vec!["Editor.exe".into()],
                executable_paths: Vec::new(),
                descendant_executable_names: vec!["Renderer.exe".into(), "Worker.exe".into()],
                adapter_requirements: vec![WireAdapterRequirement {
                    adapter_id: "example.text".into(),
                    version_major: 1,
                    version_minor: 0,
                    version_patch: 0,
                    features: vec![WireFeature::TextReplace],
                }],
            },
        )
        .expect("synthetic process family configuration");
    controller
}

mod controller;
mod elevation;
mod platform;
