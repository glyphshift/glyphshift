//! Windows UI Automation isolated worker.
//!
//! The worker keeps COM/UIA objects and callbacks outside the Desktop process. It accepts only a
//! Controller-authorized process-instance grant and publishes bounded observation batches through
//! the generic isolated-worker protocol.

#[cfg(windows)]
mod acquisition_worker;
#[cfg(windows)]
mod windows_support;

#[cfg(windows)]
pub use acquisition_worker::WindowsUiaAcquisitionWorker;

#[cfg(windows)]
mod windows_worker {
    use super::windows_support::{
        authorize_process_target, process_started_at, ComApartment, ProcessInstance,
        WindowsTargetError,
    };
    use glyphshift_adapter_uia::{
        UiaElementSnapshot, UiaObservationOutcome, UiaObserver, ADAPTER_ID,
    };
    use glyphshift_capture::{
        CaptureBatchIngress, CaptureBatchProducer, CaptureIngressStatus,
        CaptureProducerConfiguration, CaptureProducerId,
    };
    use glyphshift_isolated_worker_sdk::{
        IsolatedWorker, WireWorkerHealthReport, WorkerActivation, WorkerError,
    };
    use std::collections::BTreeSet;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use windows::core::{implement, BSTR};
    use windows::Win32::Foundation::{E_ACCESSDENIED, HWND as WindowsHwnd};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, SAFEARRAY};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
        IUIAutomationEventHandler, IUIAutomationEventHandler_Impl,
        IUIAutomationPropertyChangedEventHandler, IUIAutomationPropertyChangedEventHandler_Impl,
        IUIAutomationStructureChangedEventHandler, IUIAutomationStructureChangedEventHandler_Impl,
        IUIAutomationTextPattern, IUIAutomationValuePattern, StructureChangeType,
        TreeScope_Subtree, UIA_NamePropertyId, UIA_TextPatternId, UIA_Text_TextChangedEventId,
        UIA_ValuePatternId, UIA_ValueValuePropertyId, UIA_EVENT_ID, UIA_PROPERTY_ID,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::System::Ole::SafeArrayDestroy;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    const MAX_PENDING_EVENTS: u64 = 2_048;
    const MAX_INITIAL_ELEMENTS: i32 = 4_096;
    const MAX_TEXT_UNITS: i32 = 16 * 1024;
    const FULL_SCAN_INTERVAL: Duration = Duration::from_secs(1);

    #[derive(Default)]
    struct PendingEvents {
        count: AtomicU64,
        rescan: AtomicBool,
        dropped: AtomicU64,
    }

    impl PendingEvents {
        fn notify(&self) {
            if self
                .count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                    (count < MAX_PENDING_EVENTS).then_some(count + 1)
                })
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            self.rescan.store(true, Ordering::Release);
        }
    }

    #[implement(IUIAutomationEventHandler)]
    struct AutomationEventHandler {
        pending: Arc<PendingEvents>,
    }

    #[allow(non_snake_case)]
    impl IUIAutomationEventHandler_Impl for AutomationEventHandler_Impl {
        fn HandleAutomationEvent(
            &self,
            _sender: Option<&IUIAutomationElement>,
            _event_id: UIA_EVENT_ID,
        ) -> windows::core::Result<()> {
            self.pending.notify();
            Ok(())
        }
    }

    #[implement(IUIAutomationPropertyChangedEventHandler)]
    struct PropertyChangedEventHandler {
        pending: Arc<PendingEvents>,
    }

    #[allow(non_snake_case)]
    impl IUIAutomationPropertyChangedEventHandler_Impl for PropertyChangedEventHandler_Impl {
        fn HandlePropertyChangedEvent(
            &self,
            _sender: Option<&IUIAutomationElement>,
            _property_id: UIA_PROPERTY_ID,
            _new_value: &windows::core::VARIANT,
        ) -> windows::core::Result<()> {
            self.pending.notify();
            Ok(())
        }
    }

    #[implement(IUIAutomationStructureChangedEventHandler)]
    struct StructureChangedEventHandler {
        pending: Arc<PendingEvents>,
    }

    #[allow(non_snake_case)]
    impl IUIAutomationStructureChangedEventHandler_Impl for StructureChangedEventHandler_Impl {
        fn HandleStructureChangedEvent(
            &self,
            _sender: Option<&IUIAutomationElement>,
            _change_type: StructureChangeType,
            _runtime_id: *const SAFEARRAY,
        ) -> windows::core::Result<()> {
            self.pending.notify();
            Ok(())
        }
    }

    struct RegisteredRoot {
        handle: isize,
        element: IUIAutomationElement,
        automation_handler: IUIAutomationEventHandler,
        property_handler: IUIAutomationPropertyChangedEventHandler,
        structure_handler: IUIAutomationStructureChangedEventHandler,
    }

    struct UiaSession {
        automation: IUIAutomation,
        target: ProcessInstance,
        adapter_id: Box<str>,
        ingress: CaptureBatchIngress,
        pending: Arc<PendingEvents>,
        roots: Vec<RegisteredRoot>,
        observer: UiaObserver,
        known_element_keys: BTreeSet<Box<str>>,
        last_full_scan: Instant,
        permission_denied: bool,
    }

    impl UiaSession {
        fn start(
            target: ProcessInstance,
            adapter_id: impl Into<Box<str>>,
            ingress: CaptureBatchIngress,
        ) -> Result<Self, WorkerError> {
            let automation = unsafe {
                CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            }
            .map_err(|_| WorkerError::new("uia_client_unavailable"))?;
            let mut session = Self {
                automation,
                target,
                adapter_id: adapter_id.into(),
                ingress,
                pending: Arc::new(PendingEvents::default()),
                roots: Vec::new(),
                observer: UiaObserver::default(),
                known_element_keys: BTreeSet::new(),
                last_full_scan: Instant::now()
                    .checked_sub(FULL_SCAN_INTERVAL)
                    .unwrap_or_else(Instant::now),
                permission_denied: false,
            };
            session.refresh_roots()?;
            if session.roots.is_empty() {
                return Err(WorkerError::new("target_window_unavailable"));
            }
            session.pending.rescan.store(true, Ordering::Release);
            session.pump()?;
            Ok(session)
        }

        fn refresh_roots(&mut self) -> Result<(), WorkerError> {
            if process_started_at(self.target.process_id) != Some(self.target.started_at) {
                return Err(WorkerError::new("target_instance_changed"));
            }
            let handles = visible_top_level_windows(self.target.process_id);
            let active = handles.iter().copied().collect::<BTreeSet<_>>();
            let mut retained = Vec::with_capacity(self.roots.len());
            for root in self.roots.drain(..) {
                if active.contains(&root.handle) {
                    retained.push(root);
                } else {
                    unregister_root(&self.automation, &root);
                }
            }
            self.roots = retained;
            let known = self
                .roots
                .iter()
                .map(|root| root.handle)
                .collect::<BTreeSet<_>>();
            for handle in handles.into_iter().filter(|handle| !known.contains(handle)) {
                let root = self.register_root(handle)?;
                self.roots.push(root);
                self.pending.rescan.store(true, Ordering::Release);
            }
            Ok(())
        }

        fn register_root(&self, handle: isize) -> Result<RegisteredRoot, WorkerError> {
            let element = unsafe {
                self.automation
                    .ElementFromHandle(WindowsHwnd(handle as *mut c_void))
            }
            .map_err(|error| worker_error(error, "target_uia_root_unavailable"))?;
            let automation_handler: IUIAutomationEventHandler = AutomationEventHandler {
                pending: Arc::clone(&self.pending),
            }
            .into();
            let property_handler: IUIAutomationPropertyChangedEventHandler =
                PropertyChangedEventHandler {
                    pending: Arc::clone(&self.pending),
                }
                .into();
            let structure_handler: IUIAutomationStructureChangedEventHandler =
                StructureChangedEventHandler {
                    pending: Arc::clone(&self.pending),
                }
                .into();
            unsafe {
                self.automation
                    .AddAutomationEventHandler(
                        UIA_Text_TextChangedEventId,
                        &element,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &automation_handler,
                    )
                    .map_err(|error| worker_error(error, "uia_event_registration_failed"))?;
                self.automation
                    .AddPropertyChangedEventHandlerNativeArray(
                        &element,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &property_handler,
                        &[UIA_NamePropertyId, UIA_ValueValuePropertyId],
                    )
                    .map_err(|error| worker_error(error, "uia_event_registration_failed"))?;
                self.automation
                    .AddStructureChangedEventHandler(
                        &element,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &structure_handler,
                    )
                    .map_err(|error| worker_error(error, "uia_event_registration_failed"))?;
            }
            Ok(RegisteredRoot {
                handle,
                element,
                automation_handler,
                property_handler,
                structure_handler,
            })
        }

        fn pump(&mut self) -> Result<(), WorkerError> {
            if self.last_full_scan.elapsed() >= FULL_SCAN_INTERVAL {
                self.pending.rescan.store(true, Ordering::Release);
            }
            if self.pending.rescan.swap(false, Ordering::AcqRel) {
                self.pending.count.store(0, Ordering::Release);
                let mut elements = Vec::new();
                for root in &self.roots {
                    let condition = unsafe { self.automation.CreateTrueCondition() }
                        .map_err(|_| WorkerError::new("uia_condition_unavailable"))?;
                    let found = match unsafe { root.element.FindAll(TreeScope_Subtree, &condition) }
                    {
                        Ok(found) => found,
                        Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                            self.permission_denied = true;
                            continue;
                        }
                        Err(_) => return Err(WorkerError::new("uia_tree_unavailable")),
                    };
                    let length = match unsafe { found.Length() } {
                        Ok(length) => length.clamp(0, MAX_INITIAL_ELEMENTS),
                        Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                            self.permission_denied = true;
                            continue;
                        }
                        Err(_) => return Err(WorkerError::new("uia_tree_unavailable")),
                    };
                    for index in 0..length {
                        match unsafe { found.GetElement(index) } {
                            Ok(element) => elements.push(element),
                            Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                                self.permission_denied = true;
                            }
                            Err(_) => {}
                        }
                    }
                }
                let mut active_element_keys = BTreeSet::new();
                for element in elements {
                    if let Some(element_key) = self.observe_element(&element) {
                        active_element_keys.insert(element_key);
                    }
                }
                for stale_key in self
                    .known_element_keys
                    .difference(&active_element_keys)
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    self.observer.invalidate(&stale_key);
                }
                self.known_element_keys = active_element_keys;
                self.last_full_scan = Instant::now();
            }
            Ok(())
        }

        fn observe_element(&mut self, element: &IUIAutomationElement) -> Option<Box<str>> {
            let process_id = match unsafe { element.CurrentProcessId() } {
                Ok(process_id) => process_id,
                Err(error) => {
                    self.record_read_error(read_error(&error));
                    return None;
                }
            };
            if process_id < 0 || process_id as u32 != self.target.process_id {
                return None;
            }
            let element_key: Box<str> = match element_key(&self.automation, element) {
                Ok(element_key) => element_key.into(),
                Err(error) => {
                    self.record_read_error(error);
                    return None;
                }
            };
            let snapshot = match snapshot_element(element, element_key.clone()) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.record_read_error(error);
                    return Some(element_key);
                }
            };
            let UiaObservationOutcome::Observed(observation) = self.observer.observe(snapshot)
            else {
                return Some(element_key);
            };
            let _status: CaptureIngressStatus = self
                .ingress
                .try_observe(self.adapter_id.clone(), observation.text());
            Some(element_key)
        }

        fn record_read_error(&mut self, error: UiaReadError) {
            if error == UiaReadError::PermissionDenied {
                self.permission_denied = true;
            }
        }

        fn health(&self) -> WireWorkerHealthReport {
            if process_started_at(self.target.process_id) != Some(self.target.started_at) {
                return WireWorkerHealthReport::degraded("target_instance_changed");
            }
            if self.roots.is_empty() {
                return WireWorkerHealthReport::degraded("target_window_unavailable");
            }
            if self.permission_denied {
                return WireWorkerHealthReport::degraded("uia_permission_denied");
            }
            if self.pending.dropped.load(Ordering::Relaxed) > 0 {
                return WireWorkerHealthReport::degraded("uia_event_queue_overflow");
            }
            if self.ingress.dropped_total() > 0 {
                return WireWorkerHealthReport::degraded("observation_queue_overflow");
            }
            WireWorkerHealthReport::healthy()
        }

        fn shutdown(mut self) {
            for root in self.roots.drain(..) {
                unregister_root(&self.automation, &root);
            }
            let _ = self.pump();
        }
    }

    fn unregister_root(automation: &IUIAutomation, root: &RegisteredRoot) {
        unsafe {
            let _ = automation.RemoveAutomationEventHandler(
                UIA_Text_TextChangedEventId,
                &root.element,
                &root.automation_handler,
            );
            let _ =
                automation.RemovePropertyChangedEventHandler(&root.element, &root.property_handler);
            let _ = automation
                .RemoveStructureChangedEventHandler(&root.element, &root.structure_handler);
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum UiaReadError {
        PermissionDenied,
        Unavailable,
    }

    fn read_error(error: &windows::core::Error) -> UiaReadError {
        if error.code() == E_ACCESSDENIED {
            UiaReadError::PermissionDenied
        } else {
            UiaReadError::Unavailable
        }
    }

    fn worker_error(error: windows::core::Error, fallback: &'static str) -> WorkerError {
        if read_error(&error) == UiaReadError::PermissionDenied {
            WorkerError::new("uia_permission_denied")
        } else {
            WorkerError::new(fallback)
        }
    }

    fn optional_read<T>(result: windows::core::Result<T>) -> Result<Option<T>, UiaReadError> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                Err(UiaReadError::PermissionDenied)
            }
            Err(_) => Ok(None),
        }
    }

    fn snapshot_element(
        element: &IUIAutomationElement,
        element_key: Box<str>,
    ) -> Result<UiaElementSnapshot, UiaReadError> {
        let password = unsafe { element.CurrentIsPassword() }
            .map_err(|error| read_error(&error))?
            .as_bool();
        let mut snapshot = UiaElementSnapshot::new(element_key).password(password);
        if password {
            return Ok(snapshot);
        }
        if let Some(text) = text_pattern(element)? {
            snapshot = snapshot.with_text(text);
        }
        if let Some(value) = value_pattern(element)? {
            snapshot = snapshot.with_value(value);
        }
        if let Some(name) = optional_read(unsafe { element.CurrentName() })? {
            snapshot = snapshot.with_name(bstr_string(name));
        }
        Ok(snapshot)
    }

    fn text_pattern(element: &IUIAutomationElement) -> Result<Option<String>, UiaReadError> {
        let Some(pattern): Option<IUIAutomationTextPattern> =
            optional_read(unsafe { element.GetCurrentPatternAs(UIA_TextPatternId) })?
        else {
            return Ok(None);
        };
        let Some(range) = optional_read(unsafe { pattern.DocumentRange() })? else {
            return Ok(None);
        };
        Ok(optional_read(unsafe { range.GetText(MAX_TEXT_UNITS + 1) })?.map(bstr_string))
    }

    fn value_pattern(element: &IUIAutomationElement) -> Result<Option<String>, UiaReadError> {
        let Some(pattern): Option<IUIAutomationValuePattern> =
            optional_read(unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) })?
        else {
            return Ok(None);
        };
        Ok(optional_read(unsafe { pattern.CurrentValue() })?.map(bstr_string))
    }

    fn bstr_string(value: BSTR) -> String {
        value.to_string()
    }

    struct SafeArrayGuard(*mut SAFEARRAY);

    impl Drop for SafeArrayGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = SafeArrayDestroy(self.0.cast());
                }
            }
        }
    }

    struct NativeIntArrayGuard(*mut i32);

    impl Drop for NativeIntArrayGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CoTaskMemFree(self.0.cast()) };
            }
        }
    }

    fn element_key(
        automation: &IUIAutomation,
        element: &IUIAutomationElement,
    ) -> Result<String, UiaReadError> {
        match unsafe { element.GetRuntimeId() } {
            Ok(safe_array) => {
                let safe_array = SafeArrayGuard(safe_array);
                let mut values = std::ptr::null_mut();
                match unsafe { automation.IntSafeArrayToNativeArray(safe_array.0, &mut values) } {
                    Ok(length) => {
                        let values = NativeIntArrayGuard(values);
                        if length > 0 && !values.0.is_null() {
                            let values =
                                unsafe { std::slice::from_raw_parts(values.0, length as usize) };
                            return Ok(format!(
                                "rid:{}",
                                values
                                    .iter()
                                    .map(i32::to_string)
                                    .collect::<Vec<_>>()
                                    .join(".")
                            ));
                        }
                    }
                    Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                        return Err(UiaReadError::PermissionDenied);
                    }
                    Err(_) => {}
                }
            }
            Err(error) if read_error(&error) == UiaReadError::PermissionDenied => {
                return Err(UiaReadError::PermissionDenied);
            }
            Err(_) => {}
        }
        let handle = unsafe { element.CurrentNativeWindowHandle() }
            .map_err(|error| read_error(&error))?
            .0 as isize;
        let control = unsafe { element.CurrentControlType() }
            .map_err(|error| read_error(&error))?
            .0;
        let automation_id = optional_read(unsafe { element.CurrentAutomationId() })?
            .map(bstr_string)
            .unwrap_or_default();
        let class_name = optional_read(unsafe { element.CurrentClassName() })?
            .map(bstr_string)
            .unwrap_or_default();
        Ok(format!(
            "fallback:{handle}:{control}:{automation_id}:{class_name}"
        ))
    }

    struct WindowSearch {
        process_id: u32,
        handles: Vec<isize>,
    }

    unsafe extern "system" fn collect_window(handle: HWND, parameter: LPARAM) -> i32 {
        let search = &mut *(parameter as *mut WindowSearch);
        let mut process_id = 0;
        GetWindowThreadProcessId(handle, &mut process_id);
        if process_id == search.process_id && IsWindowVisible(handle) != 0 {
            search.handles.push(handle as isize);
        }
        1
    }

    fn visible_top_level_windows(process_id: u32) -> Vec<isize> {
        let mut search = WindowSearch {
            process_id,
            handles: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(collect_window),
                (&mut search as *mut WindowSearch) as LPARAM,
            );
        }
        search.handles
    }

    fn parse_target(activation: &WorkerActivation) -> Result<ProcessInstance, WorkerError> {
        authorize_process_target(
            &activation.adapter_id,
            ADAPTER_ID,
            &activation.target_grant.platform,
            &activation.target_grant.payload,
        )
        .map_err(|error| match error {
            WindowsTargetError::ActivationRejected => WorkerError::new("activation_rejected"),
            WindowsTargetError::InvalidGrant => WorkerError::new("invalid_target_grant"),
            WindowsTargetError::TargetChanged => WorkerError::new("target_instance_changed"),
            WindowsTargetError::PermissionDenied => WorkerError::new("uia_permission_denied"),
            WindowsTargetError::ComUnavailable => WorkerError::new("uia_com_initialization_failed"),
        })
    }

    #[derive(Default)]
    pub struct WindowsUiaWorker {
        producer_generation: u64,
        publication_generation: u64,
        producer: Option<CaptureBatchProducer>,
        session: Option<UiaSession>,
        apartment: Option<ComApartment>,
        paused: bool,
        deactivated: bool,
    }

    impl WindowsUiaWorker {
        fn require_active(&self) -> Result<(), WorkerError> {
            if self.session.is_some() && !self.deactivated {
                Ok(())
            } else {
                Err(WorkerError::new("worker_not_active"))
            }
        }

        fn stop_session(&mut self) {
            if let Some(session) = self.session.take() {
                session.shutdown();
            }
            self.apartment.take();
        }
    }

    impl IsolatedWorker for WindowsUiaWorker {
        fn activate(&mut self, activation: &WorkerActivation) -> Result<(), WorkerError> {
            if self.producer.is_some() || self.session.is_some() {
                return Err(WorkerError::new("worker_already_active"));
            }
            let target = parse_target(activation)?;
            let producer_id = CaptureProducerId::new(activation.producer_id.clone())
                .map_err(|_| WorkerError::new("invalid_producer"))?;
            let configuration =
                CaptureProducerConfiguration::new(producer_id, activation.producer_generation)
                    .map_err(|_| WorkerError::new("invalid_generation"))?;
            let (producer, ingress) = CaptureBatchProducer::start(configuration)
                .map_err(|_| WorkerError::new("producer_unavailable"))?;
            let apartment = ComApartment::enter()
                .map_err(|_| WorkerError::new("uia_com_initialization_failed"))?;
            let session = UiaSession::start(target, activation.adapter_id.clone(), ingress)?;
            self.producer_generation = activation.producer_generation;
            self.publication_generation = activation.publication_generation;
            self.producer = Some(producer);
            self.session = Some(session);
            self.apartment = Some(apartment);
            self.paused = false;
            self.deactivated = false;
            Ok(())
        }

        fn control_capture(&mut self, paused: bool) -> Result<(), WorkerError> {
            self.require_active()?;
            let producer = self
                .producer
                .as_ref()
                .ok_or_else(|| WorkerError::new("worker_not_active"))?;
            producer.set_paused(paused);
            self.paused = paused;
            if !paused {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WorkerError::new("worker_not_active"))?;
                session.refresh_roots()?;
                session.pump()?;
            }
            Ok(())
        }

        fn update_generation(&mut self, publication_generation: u64) -> Result<u64, WorkerError> {
            self.require_active()?;
            if publication_generation <= self.publication_generation {
                return Err(WorkerError::new("generation_not_monotonic"));
            }
            self.publication_generation = publication_generation;
            Ok(publication_generation)
        }

        fn query_observations(&mut self) -> Result<String, WorkerError> {
            if !self.deactivated && !self.paused {
                let session = self
                    .session
                    .as_mut()
                    .ok_or_else(|| WorkerError::new("worker_not_active"))?;
                session.refresh_roots()?;
                session.pump()?;
            }
            self.producer
                .as_mut()
                .ok_or_else(|| WorkerError::new("worker_not_active"))?
                .drain()
                .and_then(|batch| batch.encode_json())
                .map_err(|_| WorkerError::new("observation_unavailable"))
        }

        fn health(&mut self) -> Result<WireWorkerHealthReport, WorkerError> {
            self.require_active()?;
            Ok(self
                .session
                .as_ref()
                .ok_or_else(|| WorkerError::new("worker_not_active"))?
                .health())
        }

        fn deactivate(&mut self) -> Result<u64, WorkerError> {
            self.require_active()?;
            self.producer
                .as_ref()
                .ok_or_else(|| WorkerError::new("worker_not_active"))?
                .set_paused(false);
            self.stop_session();
            self.producer
                .as_ref()
                .ok_or_else(|| WorkerError::new("worker_not_active"))?
                .set_paused(true);
            self.deactivated = true;
            Ok(self.producer_generation)
        }
    }

    impl Drop for WindowsUiaWorker {
        fn drop(&mut self) {
            self.stop_session();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn access_denied_has_a_stable_permission_reason() {
            let denied = windows::core::Error::from_hresult(E_ACCESSDENIED);
            let unavailable =
                windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005_u32 as i32));

            assert_eq!(read_error(&denied), UiaReadError::PermissionDenied);
            assert_eq!(read_error(&unavailable), UiaReadError::Unavailable);
        }
    }
}

#[cfg(windows)]
pub use windows_worker::WindowsUiaWorker;

#[cfg(not(windows))]
#[derive(Default)]
pub struct WindowsUiaWorker;

#[cfg(not(windows))]
impl glyphshift_isolated_worker_sdk::IsolatedWorker for WindowsUiaWorker {
    fn activate(
        &mut self,
        _activation: &glyphshift_isolated_worker_sdk::WorkerActivation,
    ) -> Result<(), glyphshift_isolated_worker_sdk::WorkerError> {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }

    fn control_capture(
        &mut self,
        _paused: bool,
    ) -> Result<(), glyphshift_isolated_worker_sdk::WorkerError> {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }

    fn update_generation(
        &mut self,
        _publication_generation: u64,
    ) -> Result<u64, glyphshift_isolated_worker_sdk::WorkerError> {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }

    fn query_observations(
        &mut self,
    ) -> Result<String, glyphshift_isolated_worker_sdk::WorkerError> {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }

    fn health(
        &mut self,
    ) -> Result<
        glyphshift_isolated_worker_sdk::WireWorkerHealthReport,
        glyphshift_isolated_worker_sdk::WorkerError,
    > {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }

    fn deactivate(&mut self) -> Result<u64, glyphshift_isolated_worker_sdk::WorkerError> {
        Err(glyphshift_isolated_worker_sdk::WorkerError::new(
            "unsupported_operating_system",
        ))
    }
}

#[cfg(not(windows))]
pub struct WindowsUiaAcquisitionWorker;

#[cfg(not(windows))]
impl glyphshift_acquisition_worker_sdk::AcquisitionWorker for WindowsUiaAcquisitionWorker {
    fn acquire(
        &mut self,
        _request: &glyphshift_acquisition_worker_sdk::WorkerAcquisitionRequest,
    ) -> Result<glyphshift_acquisition::AcquisitionResult, glyphshift_acquisition::AcquisitionError>
    {
        Err(glyphshift_acquisition::AcquisitionError::ProviderUnavailable)
    }
}
