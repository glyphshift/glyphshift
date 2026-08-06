use super::windows_support::{
    authorize_process_target, process_started_at, ComApartment, ProcessInstance, WindowsTargetError,
};
use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AcquisitionResult, AuthorizedTarget,
    DesktopPoint, DesktopRect, InteractiveSelection, InteractiveTextAcquisition,
};
use glyphshift_acquisition_worker_sdk::{AcquisitionWorker, WorkerAcquisitionRequest};
use glyphshift_adapter_uia::{
    UiaAcquisitionAdapter, UiaAcquisitionSnapshot, UiaSelectionSource, UiaTextSelection,
    ACQUISITION_ADAPTER_ID,
};
use std::ffi::c_void;
use windows::core::BSTR;
use windows::Win32::Foundation::{E_ACCESSDENIED, POINT, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, SAFEARRAY};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, TextUnit_Word, UIA_TextPatternId,
};
use windows_sys::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};

const LOCAL_TARGET: &str = "worker-authorized-target";
const MAX_TEXT_UNITS: i32 = 16 * 1024;
const MAX_ANCHORS: usize = 256;

pub struct WindowsUiaAcquisitionWorker;

impl AcquisitionWorker for WindowsUiaAcquisitionWorker {
    fn acquire(
        &mut self,
        request: &WorkerAcquisitionRequest,
    ) -> Result<AcquisitionResult, AcquisitionError> {
        let target = authorize_process_target(
            request.adapter_id(),
            ACQUISITION_ADAPTER_ID,
            request.target_grant().platform(),
            request.target_grant().payload(),
        )
        .map_err(map_target_error)?;
        let apartment = ComApartment::enter().map_err(map_target_error)?;
        let automation = unsafe {
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        }
        .map_err(|error| map_read_error(&error))?;
        let local_target = AuthorizedTarget::new(LOCAL_TARGET)?;
        let source = WindowsUiaSelectionSource {
            automation,
            process: target,
            local_target: local_target.clone(),
            _apartment: apartment,
        };
        let adapter = UiaAcquisitionAdapter::new(source);
        let acquisition_request =
            AcquisitionRequest::new(local_target, request.selection(), request.source_policy());
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>])
            .acquire(&acquisition_request)
    }
}

struct WindowsUiaSelectionSource {
    automation: IUIAutomation,
    process: ProcessInstance,
    local_target: AuthorizedTarget,
    _apartment: ComApartment,
}

impl UiaSelectionSource for WindowsUiaSelectionSource {
    fn snapshot(
        &mut self,
        target: &AuthorizedTarget,
        selection: InteractiveSelection,
    ) -> Result<UiaAcquisitionSnapshot, AcquisitionError> {
        if target != &self.local_target
            || process_started_at(self.process.process_id) != Some(self.process.started_at)
        {
            return Err(AcquisitionError::TargetMismatch);
        }
        match selection {
            InteractiveSelection::Point(point) => self.point_snapshot(point),
            InteractiveSelection::TextRange { start, end } => self.text_range_snapshot(start, end),
            InteractiveSelection::Region(_) => Err(AcquisitionError::ProviderUnavailable),
        }
    }
}

impl WindowsUiaSelectionSource {
    fn point_snapshot(
        &self,
        point: DesktopPoint,
    ) -> Result<UiaAcquisitionSnapshot, AcquisitionError> {
        let element = self.element_from_point(point)?;
        self.validate_element(&element)?;
        let mut snapshot = UiaAcquisitionSnapshot::new(self.local_target.clone());
        if let (Some(name), Some(anchor)) = (element_name(&element)?, element_rect(&element)?) {
            snapshot = snapshot.with_name(name, anchor);
        }
        if let Some(pattern) = text_pattern(&element)? {
            let range = unsafe { pattern.RangeFromPoint(win32_point(point)) }
                .map_err(|error| map_read_error(&error))?;
            unsafe { range.ExpandToEnclosingUnit(TextUnit_Word) }
                .map_err(|error| map_read_error(&error))?;
            let text = range_text(&range)?;
            let anchors = range_rects(&range)?;
            if !text.trim().is_empty() && !anchors.is_empty() {
                snapshot = snapshot.with_text(text, anchors, UiaTextSelection::Word);
            }
        }
        Ok(snapshot)
    }

    fn text_range_snapshot(
        &self,
        start: DesktopPoint,
        end: DesktopPoint,
    ) -> Result<UiaAcquisitionSnapshot, AcquisitionError> {
        let start_element = self.element_from_point(start)?;
        let end_element = self.element_from_point(end)?;
        self.validate_element(&start_element)?;
        self.validate_element(&end_element)?;
        let start_pattern = text_pattern(&start_element)?.ok_or(AcquisitionError::NoText)?;
        let end_pattern = text_pattern(&end_element)?.ok_or(AcquisitionError::NoText)?;
        let start_range = unsafe { start_pattern.RangeFromPoint(win32_point(start)) }
            .map_err(|error| map_read_error(&error))?;
        let end_range = unsafe { end_pattern.RangeFromPoint(win32_point(end)) }
            .map_err(|error| map_read_error(&error))?;
        let start_document =
            unsafe { start_range.GetEnclosingElement() }.map_err(|error| map_read_error(&error))?;
        let end_document =
            unsafe { end_range.GetEnclosingElement() }.map_err(|error| map_read_error(&error))?;
        let same_document = unsafe {
            self.automation
                .CompareElements(&start_document, &end_document)
        }
        .map_err(|error| map_read_error(&error))?
        .as_bool();
        if !same_document {
            return Err(AcquisitionError::NoText);
        }
        unsafe { start_range.ExpandToEnclosingUnit(TextUnit_Character) }
            .map_err(|error| map_read_error(&error))?;
        unsafe { end_range.ExpandToEnclosingUnit(TextUnit_Character) }
            .map_err(|error| map_read_error(&error))?;
        let order = unsafe {
            start_range.CompareEndpoints(
                TextPatternRangeEndpoint_Start,
                &end_range,
                TextPatternRangeEndpoint_Start,
            )
        }
        .map_err(|error| map_read_error(&error))?;
        let (first, last) = if order <= 0 {
            (&start_range, &end_range)
        } else {
            (&end_range, &start_range)
        };
        let combined = unsafe { first.Clone() }.map_err(|error| map_read_error(&error))?;
        unsafe {
            combined.MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                last,
                TextPatternRangeEndpoint_End,
            )
        }
        .map_err(|error| map_read_error(&error))?;
        let text = range_text(&combined)?;
        let anchors = range_rects(&combined)?;
        if text.trim().is_empty() || anchors.is_empty() {
            return Err(AcquisitionError::NoText);
        }
        Ok(
            UiaAcquisitionSnapshot::new(self.local_target.clone()).with_text(
                text,
                anchors,
                UiaTextSelection::TextRange,
            ),
        )
    }

    fn element_from_point(
        &self,
        point: DesktopPoint,
    ) -> Result<IUIAutomationElement, AcquisitionError> {
        unsafe { self.automation.ElementFromPoint(win32_point(point)) }
            .map_err(|error| map_read_error(&error))
    }

    fn validate_element(&self, element: &IUIAutomationElement) -> Result<(), AcquisitionError> {
        let process_id =
            unsafe { element.CurrentProcessId() }.map_err(|error| map_read_error(&error))?;
        if process_id <= 0 || process_id as u32 != self.process.process_id {
            return Err(AcquisitionError::TargetMismatch);
        }
        let password = unsafe { element.CurrentIsPassword() }
            .map_err(|error| map_read_error(&error))?
            .as_bool();
        if password {
            return Err(AcquisitionError::PermissionDenied);
        }
        Ok(())
    }
}

fn win32_point(value: DesktopPoint) -> POINT {
    POINT {
        x: value.x(),
        y: value.y(),
    }
}

fn text_pattern(
    element: &IUIAutomationElement,
) -> Result<Option<IUIAutomationTextPattern>, AcquisitionError> {
    match unsafe { element.GetCurrentPatternAs(UIA_TextPatternId) } {
        Ok(pattern) => Ok(Some(pattern)),
        Err(error) if error.code() == E_ACCESSDENIED => Err(AcquisitionError::PermissionDenied),
        Err(_) => Ok(None),
    }
}

fn element_name(element: &IUIAutomationElement) -> Result<Option<String>, AcquisitionError> {
    match unsafe { element.CurrentName() } {
        Ok(name) => {
            let name = bstr_string(name);
            Ok((!name.trim().is_empty()).then_some(name))
        }
        Err(error) if error.code() == E_ACCESSDENIED => Err(AcquisitionError::PermissionDenied),
        Err(_) => Ok(None),
    }
}

fn element_rect(element: &IUIAutomationElement) -> Result<Option<DesktopRect>, AcquisitionError> {
    match unsafe { element.CurrentBoundingRectangle() } {
        Ok(rect) => Ok(rect_from_win32(rect)),
        Err(error) if error.code() == E_ACCESSDENIED => Err(AcquisitionError::PermissionDenied),
        Err(_) => Ok(None),
    }
}

fn rect_from_win32(rect: RECT) -> Option<DesktopRect> {
    DesktopRect::new(rect.left, rect.top, rect.right, rect.bottom).ok()
}

fn range_text(range: &IUIAutomationTextRange) -> Result<String, AcquisitionError> {
    unsafe { range.GetText(MAX_TEXT_UNITS + 1) }
        .map(bstr_string)
        .map_err(|error| map_read_error(&error))
}

fn range_rects(range: &IUIAutomationTextRange) -> Result<Vec<DesktopRect>, AcquisitionError> {
    let array = unsafe { range.GetBoundingRectangles() }.map_err(|error| map_read_error(&error))?;
    let array = SafeArrayGuard(array);
    if array.0.is_null() {
        return Ok(Vec::new());
    }
    let system_array = array.0.cast();
    if unsafe { SafeArrayGetDim(system_array) } != 1 {
        return Err(AcquisitionError::ProviderUnavailable);
    }
    let mut lower = 0;
    let mut upper = 0;
    if unsafe { SafeArrayGetLBound(system_array, 1, &mut lower) } < 0
        || unsafe { SafeArrayGetUBound(system_array, 1, &mut upper) } < 0
        || upper < lower
    {
        return Err(AcquisitionError::ProviderUnavailable);
    }
    let count = usize::try_from(i64::from(upper) - i64::from(lower) + 1)
        .map_err(|_| AcquisitionError::ProviderUnavailable)?;
    if count == 0 || count > MAX_ANCHORS * 4 || count % 4 != 0 {
        return Err(AcquisitionError::ProviderUnavailable);
    }
    let mut data = std::ptr::null_mut::<c_void>();
    if unsafe { SafeArrayAccessData(system_array, &mut data) } < 0 || data.is_null() {
        return Err(AcquisitionError::ProviderUnavailable);
    }
    let access = SafeArrayAccessGuard(system_array);
    let values = unsafe { std::slice::from_raw_parts(data.cast::<f64>(), count) }.to_vec();
    drop(access);
    let anchors = values
        .chunks_exact(4)
        .filter_map(|values| rect_from_doubles(values[0], values[1], values[2], values[3]))
        .collect();
    Ok(anchors)
}

fn rect_from_doubles(x: f64, y: f64, width: f64, height: f64) -> Option<DesktopRect> {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return None;
    }
    DesktopRect::new(
        floor_i32(x)?,
        floor_i32(y)?,
        ceil_i32(x + width)?,
        ceil_i32(y + height)?,
    )
    .ok()
}

fn floor_i32(value: f64) -> Option<i32> {
    let value = value.floor();
    (value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX)).then_some(value as i32)
}

fn ceil_i32(value: f64) -> Option<i32> {
    let value = value.ceil();
    (value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX)).then_some(value as i32)
}

fn bstr_string(value: BSTR) -> String {
    value.to_string()
}

fn map_target_error(error: WindowsTargetError) -> AcquisitionError {
    match error {
        WindowsTargetError::ActivationRejected
        | WindowsTargetError::InvalidGrant
        | WindowsTargetError::TargetChanged => AcquisitionError::TargetMismatch,
        WindowsTargetError::PermissionDenied => AcquisitionError::PermissionDenied,
        WindowsTargetError::ComUnavailable => AcquisitionError::ProviderUnavailable,
    }
}

fn map_read_error(error: &windows::core::Error) -> AcquisitionError {
    if error.code() == E_ACCESSDENIED {
        AcquisitionError::PermissionDenied
    } else {
        AcquisitionError::ProviderUnavailable
    }
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

struct SafeArrayAccessGuard(*const windows_sys::Win32::System::Com::SAFEARRAY);

impl Drop for SafeArrayAccessGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SafeArrayUnaccessData(self.0);
        }
    }
}
