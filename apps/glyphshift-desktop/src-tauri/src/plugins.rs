//! Local package management shares the CLI store; mutations never hot-reload the App.
use super::{CommandError, DesktopRuntimeError, RuntimeBundle};
use glyphshift_plugin_package::{Manifest, Package, PackageError, PluginStore};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;

pub(super) struct DesktopPlugins {
    store: PluginStore,
    runtime_root: PathBuf,
    loaded: BTreeSet<String>,
    runtime_ready: bool,
    prepared: Option<(Package, Instant)>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginView {
    package_id: String,
    version: String,
    sha256: String,
    architectures: Vec<String>,
    adapters: Vec<String>,
    adapter_ids: Vec<String>,
    selected: bool,
    loaded: bool,
    problem: Option<CommandError>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginSnapshot {
    packages: Vec<PluginView>,
    restart_required: bool,
    runtime_ready: bool,
}

fn error(error: PackageError) -> CommandError {
    CommandError::new(match error {
        PackageError::Io => "plugin.storage",
        PackageError::Busy => "plugin.busy",
        PackageError::Conflict => "plugin.conflict",
        PackageError::NotInstalled => "plugin.not_installed",
        PackageError::ApprovalRequired => "plugin.approval",
        PackageError::HashMismatch => "plugin.integrity",
        PackageError::Incompatible => "plugin.incompatible",
        PackageError::SizeLimit => "plugin.too_large",
        PackageError::InvalidArchive | PackageError::InvalidManifest | PackageError::UnsafePath => {
            "plugin.invalid"
        }
    })
}

pub(super) fn selected_digests(store_root: &Path) -> Result<BTreeSet<String>, CommandError> {
    PluginStore::new(store_root)
        .selected()
        .map(|packages| {
            packages
                .into_iter()
                .map(|package| package.sha256().to_owned())
                .collect()
        })
        .map_err(error)
}

pub(super) fn loaded_adapter_digests(
    store_root: &Path,
    loaded: &BTreeSet<String>,
) -> Result<BTreeMap<Box<str>, Box<str>>, CommandError> {
    let store = PluginStore::new(store_root);
    let mut adapters = BTreeMap::new();
    for digest in loaded {
        let package = store.get(digest).map_err(error)?;
        for adapter_id in package
            .manifest()
            .variants
            .iter()
            .flat_map(|variant| &variant.adapters)
            .map(|adapter| adapter.native_metadata.adapter_id.as_str())
            .collect::<BTreeSet<_>>()
        {
            if adapters
                .insert(adapter_id.into(), digest.as_str().into())
                .is_some()
            {
                return Err(error(PackageError::Conflict));
            }
        }
    }
    Ok(adapters)
}

fn view(manifest: &Manifest, sha256: &str) -> PluginView {
    PluginView {
        package_id: manifest.package_id.clone(),
        version: manifest
            .version
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join("."),
        sha256: sha256.into(),
        architectures: manifest
            .variants
            .iter()
            .map(|v| v.architecture.clone())
            .collect(),
        adapters: manifest
            .variants
            .iter()
            .flat_map(|v| &v.adapters)
            .map(|a| a.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        adapter_ids: manifest
            .variants
            .iter()
            .flat_map(|v| &v.adapters)
            .map(|a| a.native_metadata.adapter_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        selected: false,
        loaded: false,
        problem: None,
    }
}

pub(super) fn open_runtime(
    root: &Path,
    store_root: PathBuf,
) -> (Result<RuntimeBundle, DesktopRuntimeError>, BTreeSet<String>) {
    let packages = PluginStore::new(store_root).selected();
    match packages {
        Ok(packages) => {
            let result = RuntimeBundle::open_with_packages(root, &packages);
            let loaded = if result.is_ok() {
                packages.iter().map(|p| p.sha256().to_owned()).collect()
            } else {
                BTreeSet::new()
            };
            (result, loaded)
        }
        Err(_) => (
            Err(DesktopRuntimeError::AdapterRegistryRejected),
            BTreeSet::new(),
        ),
    }
}

impl DesktopPlugins {
    pub(super) fn new(
        data_root: &Path,
        runtime_root: PathBuf,
        loaded: BTreeSet<String>,
        runtime_ready: bool,
    ) -> Self {
        Self {
            store: PluginStore::new(data_root.join("plugins")),
            runtime_root,
            loaded,
            runtime_ready,
            prepared: None,
        }
    }

    fn snapshot(&self) -> Result<PluginSnapshot, CommandError> {
        let mut releases = self.store.list().map_err(error)?;
        releases.sort_by(|a, b| {
            a.package_id
                .cmp(&b.package_id)
                .then_with(|| b.version.cmp(&a.version))
        });
        let selected: BTreeSet<_> = releases
            .iter()
            .filter(|r| r.selected)
            .map(|r| r.sha256.clone())
            .collect();
        let mut packages = Vec::new();
        for release in releases {
            let mut item = match self.store.get(&release.sha256) {
                Ok(package) => view(package.manifest(), &release.sha256),
                Err(problem) => PluginView {
                    package_id: release.package_id,
                    version: release
                        .version
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join("."),
                    sha256: release.sha256.clone(),
                    architectures: Vec::new(),
                    adapters: Vec::new(),
                    adapter_ids: Vec::new(),
                    selected: false,
                    loaded: false,
                    problem: Some(error(problem)),
                },
            };
            item.selected = release.selected;
            item.loaded = self.loaded.contains(&item.sha256);
            packages.push(item);
        }
        Ok(PluginSnapshot {
            packages,
            restart_required: selected != self.loaded,
            runtime_ready: self.runtime_ready,
        })
    }

    fn prepare(&mut self, path: &Path) -> Result<PluginView, CommandError> {
        self.prepared = None;
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("gsp"))
        {
            return Err(error(PackageError::InvalidArchive));
        }
        let package = Package::open(path).map_err(error)?;
        let result = view(package.manifest(), package.sha256());
        self.prepared = Some((package, Instant::now()));
        Ok(result)
    }

    fn install(&mut self, approved_sha256: &str) -> Result<PluginSnapshot, CommandError> {
        let (package, prepared_at) = self
            .prepared
            .take()
            .ok_or_else(|| error(PackageError::ApprovalRequired))?;
        if prepared_at.elapsed() > Duration::from_secs(300) {
            return Err(error(PackageError::ApprovalRequired));
        }
        self.store
            .install(&package, approved_sha256)
            .map_err(error)?;
        self.snapshot()
    }

    fn select(&self, sha256: &str, enabled: bool) -> Result<PluginSnapshot, CommandError> {
        let preflight = |packages: &[glyphshift_plugin_package::InstalledPackage]| {
            RuntimeBundle::open_with_packages(&self.runtime_root, packages)
                .map(|_| ())
                .map_err(|_| PackageError::Incompatible)
        };
        if enabled {
            self.store.select(sha256, preflight).map_err(error)?;
        } else {
            // Only deselect the requested selected version, never a newer one
            // that was selected by another window/tool after this UI rendered.
            self.store
                .deselect_release(sha256, preflight)
                .map_err(error)?;
        }
        self.snapshot()
    }
}

async fn blocking<T: Send + 'static>(
    app: tauri::AppHandle,
    action: impl FnOnce(&mut DesktopPlugins) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Mutex<DesktopPlugins>>();
        let mut state = state.lock().map_err(|_| error(PackageError::Busy))?;
        action(&mut state)
    })
    .await
    .map_err(|_| error(PackageError::Io))?
}

#[tauri::command]
pub(super) async fn desktop_plugins(app: tauri::AppHandle) -> Result<PluginSnapshot, CommandError> {
    blocking(app, |state| state.snapshot()).await
}

#[tauri::command]
pub(super) async fn desktop_prepare_plugin(
    app: tauri::AppHandle,
    path: PathBuf,
) -> Result<PluginView, CommandError> {
    blocking(app, move |state| state.prepare(&path)).await
}

#[tauri::command]
pub(super) async fn desktop_install_plugin(
    app: tauri::AppHandle,
    approved_sha256: String,
) -> Result<PluginSnapshot, CommandError> {
    blocking(app, move |state| state.install(&approved_sha256)).await
}

#[tauri::command]
pub(super) async fn desktop_select_plugin(
    app: tauri::AppHandle,
    sha256: String,
    enabled: bool,
) -> Result<PluginSnapshot, CommandError> {
    blocking(app, move |state| state.select(&sha256, enabled)).await
}

#[tauri::command]
pub(super) async fn desktop_cancel_plugin_install(
    app: tauri::AppHandle,
) -> Result<(), CommandError> {
    blocking(app, |state| {
        state.prepared = None;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests;
