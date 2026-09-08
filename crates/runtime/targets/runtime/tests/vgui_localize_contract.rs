#![cfg(all(windows, target_arch = "x86"))]
use glyphshift_adapter_native_abi::*;
use glyphshift_adapter_native_host::LoadedNativeAdapter;
use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime::{
    activate_deployment, deactivate_runtime, query_activation, query_observations,
    update_publication,
};
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use libloading::Library;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

struct Fixture {
    _library: Library,
    name: unsafe extern "C" fn(*const u8) -> *mut u16,
    other: unsafe extern "C" fn(*const u8) -> *mut u16,
    value: unsafe extern "C" fn(u32) -> *mut u16,
    source: unsafe extern "C" fn() -> *const u16,
    table: unsafe extern "C" fn() -> *const usize,
}
impl Fixture {
    unsafe fn load() -> Self {
        let root = PathBuf::from(
            std::env::var_os("GLYPHSHIFT_VGUI_LOCALIZE_FIXTURE_ROOT")
                .expect("synthetic fixture root"),
        );
        let library = Library::new(root.join("vgui2.dll")).unwrap();
        Self {
            name: *library.get(b"fixture_name\0").unwrap(),
            other: *library.get(b"fixture_other_name\0").unwrap(),
            value: *library.get(b"fixture_value\0").unwrap(),
            source: *library.get(b"fixture_source\0").unwrap(),
            table: *library.get(b"fixture_table\0").unwrap(),
            _library: library,
        }
    }
}
fn package() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("glyphshift_adapter_vgui_localize_native.dll")
}
unsafe fn text(pointer: *const u16) -> String {
    assert!(!pointer.is_null());
    let length = (0..2049).find(|index| *pointer.add(*index) == 0).unwrap();
    String::from_utf16(std::slice::from_raw_parts(pointer, length)).unwrap()
}
fn publication(generation: u64, translation: &str) -> RuntimePublication {
    RuntimePublication::new(
        RouteProgram::direct("text"),
        TranslationSnapshot::empty(Generation::new(generation))
            .with_entry("text", "Open", translation)
            .with_entry("text", "Count %s1", "计数 %s1"),
        FontPolicy::empty(),
    )
}
unsafe fn deployment(generation: u64, translation: &str) -> TargetRuntimeDeployment {
    let library = package();
    let descriptor = LoadedNativeAdapter::inspect(&library).unwrap();
    let binding = AdapterBinding {
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(
            Sha256::digest(std::fs::read(&library).unwrap()).into(),
        ),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("glyphshift_adapter_vgui_localize_native.dll"),
        },
        descriptor,
        features: vec![Feature::TextObserve, Feature::TextReplace],
    };
    TargetRuntimeDeployment::new(
        publication(generation, translation),
        [NativeAdapterDeployment::new(library, binding).unwrap()],
    )
    .with_observation_producer(
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("vgui-query-contract").unwrap(),
            1,
        )
        .unwrap(),
    )
}

#[test]
#[ignore = "requires the independently compiled synthetic VGUI query owner"]
fn native_localize_uses_runtime_publications_and_retains_old_pointers() {
    unsafe {
        let fixture = Fixture::load();
        let name = c"menu.open".as_ptr().cast();
        let original_table = (fixture.table)();
        let old_control = text((fixture.name)(name));
        assert_eq!(old_control, "Open");
        activate_deployment(deployment(1, "打开")).unwrap();
        assert!(
            query_activation()
                .unwrap()
                .active_adapter_ids()
                .any(|id| id == "windows.vgui.localize-query"),
            "native query ABI must pass before checking replacement"
        );
        assert_ne!((fixture.table)(), original_table);
        windows_sys::Win32::Foundation::SetLastError(123);
        let first = (fixture.name)(name);
        assert_eq!(windows_sys::Win32::Foundation::GetLastError(), 123);
        assert_eq!(text(first), "打开");
        assert_eq!(
            (fixture.value)(0),
            first,
            "both entry paths intern the same source/value pair"
        );
        assert_eq!(
            text((fixture.other)(name)),
            "Open",
            "another object sharing the old vtable is untouched"
        );
        assert_eq!(
            old_control, "Open",
            "a pre-existing control owns its previous copy"
        );
        assert!((fixture.name)(c"missing".as_ptr().cast()).is_null());
        assert!((fixture.value)(u32::MAX).is_null());
        assert_eq!(
            text((fixture.value)(2)),
            "Count %s1",
            "formatted values are observed but not rewritten"
        );
        let first_control = text(first);
        update_publication(publication(2, "开启")).unwrap();
        let second = (fixture.value)(0);
        assert_eq!(text(second), "开启");
        assert_ne!(second, first);
        assert_eq!(text(first), "打开", "the old pointer survives publication");
        assert_eq!(first_control, "打开", "existing copies require recreation");
        assert_eq!(
            text((fixture.source)()),
            "Open",
            "the localization table is never edited"
        );
        let records = query_observations().unwrap();
        assert!(records
            .records()
            .iter()
            .any(|record| record.source() == "Open"
                && record.adapter_id() == "windows.vgui.localize-query"));
        deactivate_runtime().unwrap();
        assert_eq!(text((fixture.name)(name)), "Open");
        assert_eq!(text((fixture.value)(0)), "Open");
        assert_eq!(text(first), "打开");
        assert_eq!(text(second), "开启");
        activate_deployment(deployment(3, "打开")).unwrap();
        assert_eq!(
            (fixture.name)(name),
            first,
            "repeated sessions reuse verified retained data"
        );
        *first = b'X' as u16;
        assert_eq!(
            text((fixture.name)(name)),
            "Open",
            "a modified returned buffer disables replacements"
        );
        update_publication(publication(4, "新文字")).unwrap();
        assert_eq!(
            text((fixture.name)(name)),
            "Open",
            "buffer rejection persists across generations"
        );
        deactivate_runtime().unwrap();
        assert_eq!(text((fixture.source)()), "Open");
    }
}

static GENERATION: AtomicU64 = AtomicU64::new(1);
static LONG_VALUE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static REENTER: AtomicUsize = AtomicUsize::new(0);
static MODE: AtomicUsize = AtomicUsize::new(0);
static BLOCK: Mutex<(bool, bool)> = Mutex::new((false, false));
static SIGNAL: Condvar = Condvar::new();
extern "C" fn budget_decision(
    _: *mut core::ffi::c_void,
    _: *const u16,
    _: u32,
    output: *mut u16,
    capacity: u32,
    _: *mut u16,
    _: u32,
) -> NativeDecisionV1 {
    CALLS.fetch_add(1, Ordering::AcqRel);
    let reenter = REENTER.load(Ordering::Acquire);
    if reenter != 0 {
        let value: unsafe extern "C" fn(u32) -> *mut u16 = unsafe { std::mem::transmute(reenter) };
        assert_eq!(
            unsafe { text(value(0)) },
            "Open",
            "nested queries keep the original result"
        );
    }
    let mode = MODE.load(Ordering::Acquire);
    if mode == 4 {
        let mut block = BLOCK.lock().unwrap();
        block.0 = true;
        SIGNAL.notify_all();
        drop(SIGNAL.wait_while(block, |state| !state.1).unwrap());
    }
    let generation = GENERATION.load(Ordering::Acquire);
    let value = if LONG_VALUE.load(Ordering::Acquire) {
        format!("{generation:0256}")
    } else {
        format!("Value {generation}")
    };
    let mut units = value.encode_utf16().collect::<Vec<_>>();
    if mode == 3 {
        units[0] = 0;
    }
    assert!(units.len() <= capacity as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(units.as_ptr(), output, units.len());
    }
    NativeDecisionV1 {
        status: if mode == 1 {
            STATUS_OUTPUT_TOO_SMALL
        } else {
            STATUS_OK
        },
        generation,
        decision_bits: if mode == 5 { 0 } else { DECISION_TEXT_REPLACE },
        text_len: if mode == 2 {
            capacity + 1
        } else {
            units.len() as u32
        },
        font_len: 0,
    }
}
extern "C" fn no_characters(_: *mut core::ffi::c_void, _: *mut u16, _: u32) -> u32 {
    0
}
fn host() -> &'static NativeRuntimeHostV1 {
    Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: budget_decision,
        source_characters_utf16: no_characters,
    }))
}

#[test]
#[ignore = "requires a synthetic VGUI owner with a deliberately wrong query method"]
fn native_localize_rejects_unproved_query_abi() {
    unsafe {
        let fixture = Fixture::load();
        let table = (fixture.table)();
        let descriptor = LoadedNativeAdapter::inspect(&package()).unwrap();
        let adapter = LoadedNativeAdapter::load(&package(), &descriptor).unwrap();
        assert!(adapter
            .activate(
                host(),
                [Feature::TextObserve, Feature::TextReplace],
                [Feature::TextObserve, Feature::TextReplace]
            )
            .is_err());
        assert_eq!(
            (fixture.table)(),
            table,
            "rejection precedes any vptr change"
        );
        assert_eq!(text((fixture.name)(c"menu.open".as_ptr().cast())), "Open");
    }
}

#[test]
#[ignore = "requires the independently compiled synthetic VGUI query owner"]
fn native_localize_bounds_storage_across_sessions() {
    unsafe {
        let fixture = Fixture::load();
        let descriptor = LoadedNativeAdapter::inspect(&package()).unwrap();
        let adapter = LoadedNativeAdapter::load(&package(), &descriptor).unwrap();
        let host = host();
        let features = [Feature::TextObserve, Feature::TextReplace];
        let original_table = (fixture.table)();
        adapter.activate(host, [], []).unwrap();
        assert_eq!(
            (fixture.table)(),
            original_table,
            "no requested capability installs no hook"
        );
        assert_eq!(text((fixture.value)(0)), "Open");
        assert_eq!(CALLS.load(Ordering::Acquire), 0);
        adapter.activate(host, features, features).unwrap();
        let first = (fixture.value)(0);
        assert_eq!(text(first), "Value 1");
        for generation in 2..=4096 {
            GENERATION.store(generation, Ordering::Release);
            assert_eq!(text((fixture.value)(0)), format!("Value {generation}"));
        }
        GENERATION.store(4097, Ordering::Release);
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "new values are refused at the process-wide budget"
        );
        assert_eq!(text(first), "Value 1");
        adapter.deactivate().unwrap();
        adapter.activate(host, features, features).unwrap();
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "new sessions do not reset the allocation budget"
        );
        GENERATION.store(1, Ordering::Release);
        adapter.deactivate().unwrap();
        adapter.activate(host, features, features).unwrap();
        assert_eq!(
            (fixture.value)(0),
            first,
            "already retained values remain usable at capacity"
        );
        adapter.deactivate().unwrap();
        drop(adapter);
        assert_eq!(
            text(first),
            "Value 1",
            "retained data outlives loader handles"
        );
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "disabled callbacks remain valid after loader handles close"
        );
    }
}

#[test]
#[ignore = "requires the independently compiled synthetic VGUI query owner"]
fn native_localize_bounds_committed_text_bytes() {
    unsafe {
        let fixture = Fixture::load();
        let descriptor = LoadedNativeAdapter::inspect(&package()).unwrap();
        let adapter = LoadedNativeAdapter::load(&package(), &descriptor).unwrap();
        LONG_VALUE.store(true, Ordering::Release);
        let features = [Feature::TextObserve, Feature::TextReplace];
        adapter.activate(host(), features, features).unwrap();
        let capacity = 1024 * 1024 / ((256 + 1) * 2);
        for generation in 1..=capacity {
            GENERATION.store(generation, Ordering::Release);
            assert_eq!(text((fixture.value)(0)), format!("{generation:0256}"));
        }
        GENERATION.store(capacity + 1, Ordering::Release);
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "byte budget is independent of the entry budget"
        );
        adapter.deactivate().unwrap();
    }
}

#[test]
#[ignore = "requires the independently compiled synthetic VGUI query owner"]
fn native_localize_gates_reentrancy_errors_generations_and_inflight_calls() {
    unsafe {
        let fixture = Fixture::load();
        let descriptor = LoadedNativeAdapter::inspect(&package()).unwrap();
        let adapter = LoadedNativeAdapter::load(&package(), &descriptor).unwrap();
        let host = host();
        let features = [Feature::TextObserve, Feature::TextReplace];
        assert!(adapter
            .activate(host, features, [Feature::TextObserve])
            .is_err());
        assert!(adapter
            .activate(host, [Feature::FontSubstitute], [Feature::FontSubstitute])
            .is_err());
        adapter
            .activate(host, [Feature::TextObserve], features)
            .unwrap();
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "observe-only never publishes a replacement pointer"
        );
        assert_eq!(CALLS.load(Ordering::Acquire), 1);
        adapter.deactivate().unwrap();
        adapter.activate(host, features, features).unwrap();
        REENTER.store(fixture.value as *const () as usize, Ordering::Release);
        assert_eq!(text((fixture.value)(0)), "Value 1");
        assert_eq!(
            CALLS.load(Ordering::Acquire),
            2,
            "one outer query causes one decision"
        );
        REENTER.store(0, Ordering::Release);
        GENERATION.store(2, Ordering::Release);
        assert_eq!(text((fixture.value)(0)), "Value 2");
        GENERATION.store(1, Ordering::Release);
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "an older decision cannot republish after a newer one"
        );
        GENERATION.store(3, Ordering::Release);
        for mode in 1..=3 {
            MODE.store(mode, Ordering::Release);
            assert_eq!(
                text((fixture.value)(0)),
                "Open",
                "invalid host output retains the original pointer"
            );
        }
        GENERATION.store(4, Ordering::Release);
        MODE.store(5, Ordering::Release);
        assert_eq!(text((fixture.value)(0)), "Open");
        GENERATION.store(3, Ordering::Release);
        MODE.store(0, Ordering::Release);
        assert_eq!(
            text((fixture.value)(0)),
            "Open",
            "a newer no-replacement decision also rejects older results"
        );
        GENERATION.store(5, Ordering::Release);
        MODE.store(4, Ordering::Release);
        let value = fixture.value;
        let query = std::thread::spawn(move || text(value(0)));
        let state = BLOCK.lock().unwrap();
        let (state, timeout) = SIGNAL
            .wait_timeout_while(state, std::time::Duration::from_secs(2), |state| !state.0)
            .unwrap();
        assert!(!timeout.timed_out(), "a query entered the host callback");
        drop(state);
        MODE.store(0, Ordering::Release);
        let closing_library = Library::new(package()).unwrap();
        let entry = closing_library
            .get::<extern "C" fn() -> NativeAdapterApiV1>(ENTRY_SYMBOL_V1)
            .unwrap();
        let stop = entry().deactivate;
        let stopping = std::thread::spawn(move || stop());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while text((fixture.value)(0)) != "Open" {
            assert!(
                std::time::Instant::now() < deadline,
                "deactivation closes the callback gate"
            );
            std::thread::yield_now();
        }
        assert!(
            !stopping.is_finished(),
            "deactivation must still wait for the suspended callback"
        );
        let calls = CALLS.load(Ordering::Acquire);
        assert_eq!(text((fixture.value)(0)), "Open");
        assert_eq!(
            CALLS.load(Ordering::Acquire),
            calls,
            "closed sessions cannot enter the host"
        );
        BLOCK.lock().unwrap().1 = true;
        SIGNAL.notify_all();
        assert_eq!(stopping.join().unwrap(), STATUS_OK);
        assert_eq!(
            query.join().unwrap(),
            "Open",
            "a decision completed during deactivation is discarded"
        );
        assert_eq!(text((fixture.value)(0)), "Open");
    }
}
