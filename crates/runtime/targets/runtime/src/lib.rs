//! Injected target-process Runtime Host for verified native Adapter packages.

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_FONT_SUBSTITUTE, DECISION_TEXT_REPLACE,
    STATUS_OK, STATUS_OUTPUT_TOO_SMALL,
};
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_adapter_registry::AdapterBinding;
use glyphshift_capture::CaptureObservationBatch;
use glyphshift_domain::{FontDecision, SourceTextPolicy, TextDecision, TextObservation};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_runtime_kernel::RuntimeKernel;
use glyphshift_target_runtime_contract::{
    CaptureRuntimeControl, RuntimeActivationQueryV1, RuntimeActivationReport, RuntimeCommandV1,
    RuntimeDiagnosticsControl, RuntimeDiagnosticsQueryV1, RuntimeObservationQueryV1,
    RuntimeTraceBatch, RuntimeTraceRecord, TargetRuntimeDeployment,
    MAX_RUNTIME_ACTIVATION_REPORT_BYTES, MAX_RUNTIME_OBSERVATION_BYTES, MAX_RUNTIME_TRACE_BYTES,
    STATUS_TARGET_RUNTIME_ACTIVATION_FAILED, STATUS_TARGET_RUNTIME_ADAPTER_ACTIVATION_FAILED,
    STATUS_TARGET_RUNTIME_ADAPTER_CHANGED, STATUS_TARGET_RUNTIME_ADAPTER_LOAD_FAILED,
    STATUS_TARGET_RUNTIME_ALREADY_ACTIVE, STATUS_TARGET_RUNTIME_CAPTURE_FAILED,
    STATUS_TARGET_RUNTIME_INVALID_COMMAND, STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT,
    STATUS_TARGET_RUNTIME_KERNEL_ACTIVATION_FAILED, STATUS_TARGET_RUNTIME_OK,
    STATUS_TARGET_RUNTIME_OUTPUT_TOO_SMALL, STATUS_TARGET_RUNTIME_UNAVAILABLE,
    STATUS_TARGET_RUNTIME_UPDATE_FAILED, STATUS_TARGET_RUNTIME_UPDATE_REJECTED,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

mod capture;

use capture::{RuntimeCapture, RuntimeCaptureConfiguration};

const MAX_COMMAND_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRuntimeError {
    AlreadyActive,
    InvalidDeployment,
    AdapterLoad,
    AdapterArtifactChanged,
    KernelActivation,
    AdapterActivation,
    RuntimeUnavailable,
    UpdateRejected,
    Capture,
}

struct RuntimeState {
    kernel: RuntimeKernel,
    publication: RuntimePublication,
    bindings: Vec<AdapterBinding>,
    adapter_libraries: Vec<PathBuf>,
    adapters: Vec<Arc<LoadedNativeAdapter>>,
    native_hosts: Vec<usize>,
    active_adapters: Vec<bool>,
    capture: Option<RuntimeCapture>,
    active: bool,
    source_aliases: BTreeMap<SourceTextPolicy, BTreeMap<String, String>>,
}

fn runtime_state() -> &'static Mutex<Option<RuntimeState>> {
    static STATE: OnceLock<Mutex<Option<RuntimeState>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

struct NativeDecisionContext {
    adapter_id: Box<str>,
    source_policy: SourceTextPolicy,
}

fn native_host(adapter_id: &str, source_policy: SourceTextPolicy) -> usize {
    let context = Box::into_raw(Box::new(NativeDecisionContext {
        adapter_id: adapter_id.into(),
        source_policy,
    }));
    Box::into_raw(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: context.cast(),
        decide_utf16,
        source_characters_utf16,
    })) as usize
}

pub fn activate_deployment(deployment: TargetRuntimeDeployment) -> Result<(), TargetRuntimeError> {
    let capture_configuration = RuntimeCaptureConfiguration::from_deployment(&deployment);
    let bindings = deployment
        .adapters()
        .iter()
        .map(|adapter| adapter.binding().clone())
        .collect::<Vec<_>>();
    let adapter_libraries = deployment
        .adapters()
        .iter()
        .map(|adapter| adapter.library().to_path_buf())
        .collect::<Vec<_>>();
    for adapter in deployment.adapters() {
        let artifact_hash = file_sha256(adapter.library())
            .map_err(|_| TargetRuntimeError::AdapterArtifactChanged)?;
        if artifact_hash != adapter.binding().artifact_hash.as_bytes() {
            return Err(TargetRuntimeError::AdapterArtifactChanged);
        }
    }
    let publication = deployment.publication().clone();
    let kernel = RuntimeKernel::from_publication(bindings.clone(), publication.clone())
        .map_err(|_| TargetRuntimeError::KernelActivation)?;
    let (adapters, native_hosts) = {
        let mut state = runtime_state()
            .lock()
            .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
        if let Some(runtime) = state.as_mut() {
            if runtime.active {
                if !same_active_deployment(
                    runtime,
                    &bindings,
                    &adapter_libraries,
                    capture_configuration.as_ref(),
                ) {
                    return Err(TargetRuntimeError::InvalidDeployment);
                }
                let current_generation = runtime.publication.generation();
                let incoming_generation = publication.generation();
                if incoming_generation < current_generation {
                    return Err(TargetRuntimeError::UpdateRejected);
                }
                if incoming_generation == current_generation {
                    let current_identity = runtime
                        .publication
                        .identity()
                        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
                    let incoming_identity = publication
                        .identity()
                        .map_err(|_| TargetRuntimeError::UpdateRejected)?;
                    if current_identity != incoming_identity {
                        return Err(TargetRuntimeError::UpdateRejected);
                    }
                    return Ok(());
                }
                runtime
                    .kernel
                    .apply_publication(publication.clone())
                    .map_err(|_| TargetRuntimeError::UpdateRejected)?;
                runtime.publication = publication;
                runtime.source_aliases = source_aliases(&runtime.publication, &runtime.adapters);
                let refresh_adapters = active_native_adapters(runtime);
                drop(state);
                request_adapter_refreshes(&refresh_adapters);
                request_current_process_redraw();
                return Ok(());
            }
            if !same_adapter_set(&runtime.bindings, &bindings)
                || runtime.adapter_libraries != adapter_libraries
            {
                return Err(TargetRuntimeError::InvalidDeployment);
            }
            runtime.kernel = kernel;
            runtime.publication = publication;
            runtime.source_aliases = source_aliases(&runtime.publication, &runtime.adapters);
            runtime.bindings = bindings;
            runtime.capture = capture_configuration
                .clone()
                .map(RuntimeCapture::start)
                .transpose()?;
            (runtime.adapters.clone(), runtime.native_hosts.clone())
        } else {
            let capture = capture_configuration
                .clone()
                .map(RuntimeCapture::start)
                .transpose()?;
            let adapters = deployment
                .adapters()
                .iter()
                .map(|adapter| unsafe {
                    LoadedNativeAdapter::load(adapter.library(), &adapter.binding().descriptor)
                        .map(Arc::new)
                        .map_err(|_| TargetRuntimeError::AdapterLoad)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let native_hosts = bindings
                .iter()
                .zip(&adapters)
                .map(|(binding, adapter)| native_host(binding.adapter_id.as_str(), adapter.source_policy()))
                .collect::<Vec<_>>();
            let aliases = source_aliases(&publication, &adapters);
            *state = Some(RuntimeState {
                kernel,
                publication,
                bindings,
                adapter_libraries,
                adapters: adapters.clone(),
                native_hosts: native_hosts.clone(),
                active_adapters: vec![false; adapters.len()],
                capture,
                active: false,
                source_aliases: aliases,
            });
            (adapters, native_hosts)
        }
    };
    let mut active_adapters = vec![false; adapters.len()];
    for (index, ((adapter, deployment), native_host)) in adapters
        .iter()
        .zip(deployment.adapters())
        .zip(native_hosts)
        .enumerate()
    {
        let requested = deployment.binding().features.iter().copied();
        let expected = deployment.binding().features.clone();
        let host = unsafe { &*(native_host as *const NativeRuntimeHostV1) };
        let active = adapter.activate(host, requested, expected.iter().copied());
        if active.as_deref() == Ok(expected.as_slice()) {
            active_adapters[index] = true;
        } else {
            let _ = adapter.deactivate();
        }
    }
    if !adapters.is_empty() && !active_adapters.iter().any(|active| *active) {
        let adapters_deactivated = adapters
            .iter()
            .map(|adapter| adapter.deactivate().is_ok())
            .fold(true, |all_deactivated, deactivated| {
                all_deactivated & deactivated
            });
        let capture = runtime_state().lock().ok().and_then(|mut state| {
            state
                .as_mut()
                .filter(|runtime| same_loaded_adapters(runtime, &adapters))
                .and_then(|runtime| runtime.capture.take())
        });
        let capture_finished = capture.is_none_or(|capture| capture.finish().is_ok());
        if adapters_deactivated && capture_finished {
            let rolled_back = runtime_state().lock().is_ok_and(|mut state| {
                if state.as_ref().is_some_and(|runtime| {
                    !runtime.active && same_loaded_adapters(runtime, &adapters)
                }) {
                    *state = None;
                    true
                } else {
                    false
                }
            });
            if rolled_back {
                request_current_process_redraw();
            }
        }
        return Err(TargetRuntimeError::AdapterActivation);
    }
    let refresh_adapters = adapters
        .iter()
        .zip(&active_adapters)
        .filter(|(_, active)| **active)
        .map(|(adapter, _)| Arc::clone(adapter))
        .collect::<Vec<_>>();
    {
        let mut state = runtime_state()
            .lock()
            .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
        let runtime = state
            .as_mut()
            .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
        runtime.active_adapters = active_adapters;
        runtime.active = true;
    }
    request_adapter_refreshes(&refresh_adapters);
    request_current_process_redraw();
    Ok(())
}

fn same_loaded_adapters(runtime: &RuntimeState, adapters: &[Arc<LoadedNativeAdapter>]) -> bool {
    runtime.adapters.len() == adapters.len()
        && runtime
            .adapters
            .iter()
            .zip(adapters)
            .all(|(current, expected)| Arc::ptr_eq(current, expected))
}

fn same_active_deployment(
    runtime: &RuntimeState,
    bindings: &[AdapterBinding],
    adapter_libraries: &[PathBuf],
    capture_configuration: Option<&RuntimeCaptureConfiguration>,
) -> bool {
    same_adapter_set(&runtime.bindings, bindings)
        && runtime
            .bindings
            .iter()
            .zip(bindings)
            .all(|(previous, next)| previous.features == next.features)
        && runtime.adapter_libraries == adapter_libraries
        && runtime
            .capture
            .as_ref()
            .map(RuntimeCapture::configuration)
            .as_ref()
            == capture_configuration
}

fn same_adapter_set(previous: &[AdapterBinding], next: &[AdapterBinding]) -> bool {
    previous.len() == next.len()
        && previous.iter().zip(next).all(|(previous, next)| {
            previous.descriptor == next.descriptor
                && previous.adapter_id == next.adapter_id
                && previous.version == next.version
                && previous.apply_model == next.apply_model
                && previous.artifact_hash == next.artifact_hash
                && previous.host == next.host
        })
}

fn file_sha256(path: &Path) -> Result<[u8; 32], std::io::Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    Ok(digest.finalize().into())
}

pub fn update_publication(publication: RuntimePublication) -> Result<(), TargetRuntimeError> {
    let refresh_adapters = {
        let mut state = runtime_state()
            .lock()
            .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
        let runtime = state
            .as_mut()
            .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
        if !runtime.active {
            return Err(TargetRuntimeError::RuntimeUnavailable);
        }
        runtime
            .kernel
            .apply_publication(publication.clone())
            .map_err(|_| TargetRuntimeError::UpdateRejected)?;
        runtime.publication = publication;
        runtime.source_aliases = source_aliases(&runtime.publication, &runtime.adapters);
        active_native_adapters(runtime)
    };
    request_adapter_refreshes(&refresh_adapters);
    request_current_process_redraw();
    Ok(())
}

pub fn control_capture(control: CaptureRuntimeControl) -> Result<(), TargetRuntimeError> {
    let state = runtime_state()
        .lock()
        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
    let runtime = state
        .as_ref()
        .filter(|runtime| runtime.active)
        .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
    let capture = runtime
        .capture
        .as_ref()
        .ok_or(TargetRuntimeError::Capture)?;
    capture.set_paused(control.paused());
    Ok(())
}

pub fn query_observations() -> Result<CaptureObservationBatch, TargetRuntimeError> {
    let mut state = runtime_state()
        .lock()
        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
    let runtime = state
        .as_mut()
        .filter(|runtime| runtime.active)
        .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
    runtime
        .capture
        .as_mut()
        .ok_or(TargetRuntimeError::Capture)?
        .drain()
}

pub fn control_diagnostics(control: RuntimeDiagnosticsControl) -> Result<(), TargetRuntimeError> {
    let state = runtime_state()
        .lock()
        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
    let runtime = state
        .as_ref()
        .filter(|runtime| runtime.active)
        .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
    runtime.kernel.set_decision_tracing(control.enabled());
    Ok(())
}

pub fn query_diagnostics() -> Result<RuntimeTraceBatch, TargetRuntimeError> {
    let state = runtime_state()
        .lock()
        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
    let runtime = state
        .as_ref()
        .filter(|runtime| runtime.active)
        .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
    let batch = runtime.kernel.drain_decision_traces();
    Ok(RuntimeTraceBatch::new(
        batch.records().iter().map(|record| {
            RuntimeTraceRecord::from_decision(
                record.adapter_id(),
                record.source_text(),
                record.trace(),
                record.publication_identity(),
            )
        }),
        batch.dropped(),
    ))
}

pub fn query_activation() -> Result<RuntimeActivationReport, TargetRuntimeError> {
    let state = runtime_state()
        .lock()
        .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
    let runtime = state
        .as_ref()
        .filter(|runtime| runtime.active)
        .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
    Ok(RuntimeActivationReport::new(
        runtime
            .bindings
            .iter()
            .zip(&runtime.active_adapters)
            .filter(|(_, active)| **active)
            .map(|(binding, _)| binding.adapter_id.as_str()),
    ))
}

pub fn deactivate_runtime() -> Result<(), TargetRuntimeError> {
    let (capture, refresh_adapters) = {
        let mut state = runtime_state()
            .lock()
            .map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
        let runtime = state
            .as_mut()
            .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
        if !runtime.active {
            return Err(TargetRuntimeError::RuntimeUnavailable);
        }
        let refresh_adapters = active_native_adapters(runtime);
        let loaded = runtime.adapters.clone();
        // An adapter may drain callbacks which need this same Runtime mutex.
        // Never hold the decision lock while asking an adapter to deactivate.
        drop(state);
        let deactivated = refresh_adapters.iter().all(|adapter| adapter.deactivate().is_ok());
        let mut state = runtime_state().lock().map_err(|_| TargetRuntimeError::RuntimeUnavailable)?;
        let runtime = state.as_mut().filter(|runtime| same_loaded_adapters(runtime, &loaded))
            .ok_or(TargetRuntimeError::RuntimeUnavailable)?;
        if deactivated {
            runtime.active = false;
            runtime.active_adapters.fill(false);
        } else {
            return Err(TargetRuntimeError::AdapterActivation);
        }
        (runtime.capture.take(), refresh_adapters)
    };
    if let Some(capture) = capture {
        capture.finish()?;
    }
    request_adapter_refreshes(&refresh_adapters);
    request_current_process_redraw();
    Ok(())
}

fn active_native_adapters(runtime: &RuntimeState) -> Vec<Arc<LoadedNativeAdapter>> {
    runtime
        .adapters
        .iter()
        .zip(&runtime.active_adapters)
        .filter(|(_, active)| **active)
        .map(|(adapter, _)| Arc::clone(adapter))
        .collect()
}

fn request_adapter_refreshes(adapters: &[Arc<LoadedNativeAdapter>]) {
    for adapter in adapters {
        adapter.request_refresh();
    }
}

#[cfg(windows)]
fn request_current_process_redraw() {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        RedrawWindow, RDW_ALLCHILDREN, RDW_FRAME, RDW_INVALIDATE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId};

    struct RefreshState {
        process_id: u32,
    }

    unsafe extern "system" fn refresh_window(window: HWND, state: LPARAM) -> i32 {
        let state = &mut *(state as *mut RefreshState);
        let mut process_id = 0;
        GetWindowThreadProcessId(window, &mut process_id);
        if process_id == state.process_id {
            // Keep this asynchronous: a synchronous paint can re-enter the Decision callback.
            RedrawWindow(
                window,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_FRAME,
            );
        }
        1
    }

    let mut state = RefreshState {
        process_id: unsafe { GetCurrentProcessId() },
    };
    unsafe {
        EnumWindows(
            Some(refresh_window),
            (&mut state as *mut RefreshState) as LPARAM,
        );
    }
}

#[cfg(not(windows))]
fn request_current_process_redraw() {}

extern "C" fn decide_utf16(
    context: *mut core::ffi::c_void,
    source: *const u16,
    source_len: u32,
    text_out: *mut u16,
    text_capacity: u32,
    font_out: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1 {
    if context.is_null() || source.is_null() || source_len as usize > MAX_COMMAND_BYTES / 2 {
        return decision_error(STATUS_OUTPUT_TOO_SMALL);
    }
    let source = unsafe { std::slice::from_raw_parts(source, source_len as usize) };
    let source = String::from_utf16_lossy(source);
    let Ok(state) = runtime_state().lock() else {
        return decision_error(STATUS_OUTPUT_TOO_SMALL);
    };
    let Some(runtime) = state.as_ref() else {
        return decision_error(STATUS_OUTPUT_TOO_SMALL);
    };
    if !runtime.active {
        return decision_error(STATUS_OUTPUT_TOO_SMALL);
    }
    let context = unsafe { &*context.cast::<NativeDecisionContext>() };
    let canonical = context.source_policy.normalize(&source);
    if let Some(capture) = &runtime.capture {
        capture.try_observe(context.adapter_id.clone(), canonical.as_ref().into());
    }
    let lookup = |text: &str| runtime.kernel.decide(&TextObservation::new(
        context.adapter_id.clone(),
        text,
        "native.surface",
    ));
    let mut decision = lookup(&canonical);
    if matches!(decision.text, TextDecision::Keep) && canonical != source {
        decision = lookup(&source); // Preserve exact legacy wrapped entries.
    }
    if matches!(decision.text, TextDecision::Keep) && source.trim() == source {
        if let Some(alias) = runtime.source_aliases.get(&context.source_policy).and_then(|aliases| aliases.get(canonical.as_ref())) {
            if alias != &source { decision = lookup(alias); }
        }
    }
    let text = match decision.text {
        TextDecision::Keep => None,
        TextDecision::Replace(text) => Some(text.encode_utf16().collect::<Vec<_>>()),
    };
    let font = match decision.font {
        FontDecision::Keep => None,
        FontDecision::Substitute(font) => Some(font.encode_utf16().collect::<Vec<_>>()),
    };
    if text
        .as_ref()
        .is_some_and(|text| text.len() > text_capacity as usize)
        || font
            .as_ref()
            .is_some_and(|font| font.len() > font_capacity as usize)
    {
        return decision_error(STATUS_OUTPUT_TOO_SMALL);
    }
    if let Some(text) = &text {
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), text_out, text.len());
        }
    }
    if let Some(font) = &font {
        unsafe {
            std::ptr::copy_nonoverlapping(font.as_ptr(), font_out, font.len());
        }
    }
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: decision.generation.value(),
        decision_bits: if text.is_some() {
            DECISION_TEXT_REPLACE
        } else {
            0
        } | if font.is_some() {
            DECISION_FONT_SUBSTITUTE
        } else {
            0
        },
        text_len: text.as_ref().map_or(0, |text| text.len() as u32),
        font_len: font.as_ref().map_or(0, |font| font.len() as u32),
    }
}

extern "C" fn source_characters_utf16(
    _context: *mut core::ffi::c_void,
    output: *mut u16,
    capacity: u32,
) -> u32 {
    let Ok(state) = runtime_state().lock() else {
        return 0;
    };
    let Some(runtime) = state.as_ref() else {
        return 0;
    };
    let mut characters = BTreeSet::new();
    runtime
        .publication
        .snapshot()
        .visit_entries(|_, source, _| characters.extend(source.encode_utf16()));
    runtime
        .publication
        .snapshot()
        .visit_context_entries(|_, _, _, source, _| characters.extend(source.encode_utf16()));
    let units = characters.into_iter().collect::<Vec<_>>();
    if !output.is_null() && units.len() <= capacity as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(units.as_ptr(), output, units.len());
        }
    }
    units.len() as u32
}

fn decision_error(status: i32) -> NativeDecisionV1 {
    NativeDecisionV1 {
        status,
        generation: 0,
        decision_bits: 0,
        text_len: 0,
        font_len: 0,
    }
}

fn source_aliases(publication: &RuntimePublication, adapters: &[Arc<LoadedNativeAdapter>]) -> BTreeMap<SourceTextPolicy, BTreeMap<String, String>> {
    let policies = adapters.iter().map(|adapter| adapter.source_policy()).filter(|policy| *policy != SourceTextPolicy::Exact).collect::<BTreeSet<_>>();
    policies.into_iter().map(|policy| {
        let mut candidates = BTreeMap::<String, Option<(String, String)>>::new();
        let mut visit = |source: &str, translation: &str| {
            let canonical = policy.key(source);
            if canonical == source { return; }
            candidates.entry(canonical).and_modify(|candidate| {
                if candidate.as_ref().is_some_and(|(_, previous)| previous != translation) { *candidate = None; }
            }).or_insert_with(|| Some((source.to_owned(), translation.to_owned())));
        };
        publication.snapshot().visit_entries(|_, source, translation| visit(source, translation));
        publication.snapshot().visit_context_entries(|_, _, _, source, translation| visit(source, translation));
        (policy, candidates.into_iter().filter_map(|(canonical, candidate)| candidate.map(|(source, _)| (canonical, source))).collect())
    }).collect()
}

fn activation_status(error: TargetRuntimeError) -> u32 {
    match error {
        TargetRuntimeError::AlreadyActive => STATUS_TARGET_RUNTIME_ALREADY_ACTIVE,
        TargetRuntimeError::InvalidDeployment => STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT,
        TargetRuntimeError::AdapterLoad => STATUS_TARGET_RUNTIME_ADAPTER_LOAD_FAILED,
        TargetRuntimeError::AdapterArtifactChanged => STATUS_TARGET_RUNTIME_ADAPTER_CHANGED,
        TargetRuntimeError::KernelActivation => STATUS_TARGET_RUNTIME_KERNEL_ACTIVATION_FAILED,
        TargetRuntimeError::AdapterActivation => STATUS_TARGET_RUNTIME_ADAPTER_ACTIVATION_FAILED,
        TargetRuntimeError::RuntimeUnavailable => STATUS_TARGET_RUNTIME_UNAVAILABLE,
        TargetRuntimeError::UpdateRejected => STATUS_TARGET_RUNTIME_UPDATE_REJECTED,
        TargetRuntimeError::Capture => STATUS_TARGET_RUNTIME_CAPTURE_FAILED,
    }
}

unsafe fn command_json<'a>(command: *const RuntimeCommandV1) -> Option<&'a str> {
    if command.is_null()
        || (*command).struct_size != std::mem::size_of::<RuntimeCommandV1>() as u32
        || (*command).json.is_null()
        || (*command).json_len as usize > MAX_COMMAND_BYTES
    {
        return None;
    }
    std::str::from_utf8(std::slice::from_raw_parts(
        (*command).json,
        (*command).json_len as usize,
    ))
    .ok()
}

#[no_mangle]
/// Activates a complete target Runtime deployment from a bounded command buffer.
///
/// # Safety
///
/// `command` and its JSON buffer must remain readable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_activate_v1(
    command: *const RuntimeCommandV1,
) -> u32 {
    let Some(json) = command_json(command) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    let Ok(deployment) = TargetRuntimeDeployment::decode_json(json) else {
        return STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT;
    };
    match std::panic::catch_unwind(|| activate_deployment(deployment)) {
        Ok(Ok(())) => STATUS_TARGET_RUNTIME_OK,
        Ok(Err(error)) => activation_status(error),
        Err(_) => STATUS_TARGET_RUNTIME_ACTIVATION_FAILED,
    }
}

#[no_mangle]
/// Applies a newer Runtime Publication from a bounded command buffer.
///
/// # Safety
///
/// `command` and its JSON buffer must remain readable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_update_v1(
    command: *const RuntimeCommandV1,
) -> u32 {
    let Some(json) = command_json(command) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    let Ok(publication) = RuntimePublication::decode_json(json) else {
        return STATUS_TARGET_RUNTIME_INVALID_DEPLOYMENT;
    };
    match std::panic::catch_unwind(|| update_publication(publication)) {
        Ok(Ok(())) => STATUS_TARGET_RUNTIME_OK,
        Ok(Err(error)) => activation_status(error),
        Err(_) => STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    }
}

#[no_mangle]
/// Pauses or resumes capture without unloading adapters or the active preview publication.
///
/// # Safety
///
/// `command` and its JSON buffer must remain readable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_capture_control_v1(
    command: *const RuntimeCommandV1,
) -> u32 {
    let Some(json) = command_json(command) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    let Ok(control) = CaptureRuntimeControl::decode_json(json) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    match std::panic::catch_unwind(|| control_capture(control)) {
        Ok(Ok(())) => STATUS_TARGET_RUNTIME_OK,
        Ok(Err(error)) => activation_status(error),
        Err(_) => STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    }
}

#[no_mangle]
/// Drains one bounded observation batch into a caller-owned JSON buffer.
///
/// # Safety
///
/// `query` and its output buffer must remain writable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_observation_query_v1(
    query: *mut RuntimeObservationQueryV1,
) -> u32 {
    if query.is_null()
        || (*query).struct_size != std::mem::size_of::<RuntimeObservationQueryV1>() as u32
        || (*query).output.is_null()
        || (*query).output_capacity as usize > MAX_RUNTIME_OBSERVATION_BYTES
    {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    }
    let encoded = match std::panic::catch_unwind(|| {
        query_observations().and_then(|batch| {
            batch
                .encode_json()
                .map_err(|_| TargetRuntimeError::RuntimeUnavailable)
        })
    }) {
        Ok(Ok(encoded)) => encoded,
        Ok(Err(error)) => return activation_status(error),
        Err(_) => return STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    };
    (*query).output_len = encoded.len() as u32;
    if encoded.len() > (*query).output_capacity as usize {
        return STATUS_TARGET_RUNTIME_OUTPUT_TOO_SMALL;
    }
    std::ptr::copy_nonoverlapping(encoded.as_ptr(), (*query).output, encoded.len());
    STATUS_TARGET_RUNTIME_OK
}

#[no_mangle]
/// Reports the Adapter subset that is active in the current target Runtime deployment.
///
/// # Safety
///
/// `query` and its output buffer must remain writable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_activation_query_v1(
    query: *mut RuntimeActivationQueryV1,
) -> u32 {
    if query.is_null()
        || (*query).struct_size != std::mem::size_of::<RuntimeActivationQueryV1>() as u32
        || (*query).output.is_null()
        || (*query).output_capacity as usize > MAX_RUNTIME_ACTIVATION_REPORT_BYTES
    {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    }
    let encoded = match std::panic::catch_unwind(|| {
        query_activation().and_then(|report| {
            report
                .encode_json()
                .map_err(|_| TargetRuntimeError::RuntimeUnavailable)
        })
    }) {
        Ok(Ok(encoded)) => encoded,
        Ok(Err(error)) => return activation_status(error),
        Err(_) => return STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    };
    (*query).output_len = encoded.len() as u32;
    if encoded.len() > (*query).output_capacity as usize {
        return STATUS_TARGET_RUNTIME_OUTPUT_TOO_SMALL;
    }
    std::ptr::copy_nonoverlapping(encoded.as_ptr(), (*query).output, encoded.len());
    STATUS_TARGET_RUNTIME_OK
}

#[no_mangle]
/// Enables or disables bounded decision diagnostics.
///
/// # Safety
///
/// `command` and its JSON buffer must remain readable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_diagnostics_control_v1(
    command: *const RuntimeCommandV1,
) -> u32 {
    let Some(json) = command_json(command) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    let Ok(control) = RuntimeDiagnosticsControl::decode_json(json) else {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    };
    match std::panic::catch_unwind(|| control_diagnostics(control)) {
        Ok(Ok(())) => STATUS_TARGET_RUNTIME_OK,
        Ok(Err(error)) => activation_status(error),
        Err(_) => STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    }
}

#[no_mangle]
/// Drains the bounded decision diagnostics window into a caller-owned JSON buffer.
///
/// # Safety
///
/// `query` and its output buffer must remain writable for the duration of this call.
pub unsafe extern "system" fn glyphshift_runtime_diagnostics_query_v1(
    query: *mut RuntimeDiagnosticsQueryV1,
) -> u32 {
    if query.is_null()
        || (*query).struct_size != std::mem::size_of::<RuntimeDiagnosticsQueryV1>() as u32
        || (*query).output.is_null()
        || (*query).output_capacity as usize > MAX_RUNTIME_TRACE_BYTES
    {
        return STATUS_TARGET_RUNTIME_INVALID_COMMAND;
    }
    let encoded = match std::panic::catch_unwind(|| {
        query_diagnostics().and_then(|batch| {
            batch
                .encode_json()
                .map_err(|_| TargetRuntimeError::RuntimeUnavailable)
        })
    }) {
        Ok(Ok(encoded)) => encoded,
        Ok(Err(error)) => return activation_status(error),
        Err(_) => return STATUS_TARGET_RUNTIME_UPDATE_FAILED,
    };
    (*query).output_len = encoded.len() as u32;
    if encoded.len() > (*query).output_capacity as usize {
        return STATUS_TARGET_RUNTIME_OUTPUT_TOO_SMALL;
    }
    std::ptr::copy_nonoverlapping(encoded.as_ptr(), (*query).output, encoded.len());
    STATUS_TARGET_RUNTIME_OK
}

#[no_mangle]
pub extern "system" fn glyphshift_runtime_deactivate_v1(_command: *mut core::ffi::c_void) -> u32 {
    match std::panic::catch_unwind(deactivate_runtime) {
        Ok(Ok(())) => STATUS_TARGET_RUNTIME_OK,
        Ok(Err(error)) => activation_status(error),
        Err(_) => STATUS_TARGET_RUNTIME_ACTIVATION_FAILED,
    }
}

#[cfg(test)]
mod tests;
