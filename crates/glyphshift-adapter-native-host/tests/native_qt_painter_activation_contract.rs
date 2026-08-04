#![cfg(windows)]

use glyphshift_adapter_native_abi::{
    NativeDecisionV1, NativeRuntimeHostV1, DECISION_TEXT_REPLACE, STATUS_ACTIVATION_FAILED,
    STATUS_OK,
};
use glyphshift_adapter_native_host::{LoadedNativeAdapter, NativeHostError};
use glyphshift_domain::Feature;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH,
};

const QPAINTER_DEVICE_CTOR_SYMBOL: &[u8] = b"??0QPainter@@QEAA@PEAVQPaintDevice@@@Z\0";
const QPAINTER_DTOR_SYMBOL: &[u8] = b"??1QPainter@@QEAA@XZ\0";
const QGUI_APPLICATION_CTOR_SYMBOL: &[u8] = b"??0QGuiApplication@@QEAA@AEAHPEAPEADH@Z\0";
const QGUI_APPLICATION_DTOR_SYMBOL: &[u8] = b"??1QGuiApplication@@UEAA@XZ\0";
const QIMAGE_CTOR_SYMBOL: &[u8] = b"??0QImage@@QEAA@HHW4Format@0@@Z\0";
const QIMAGE_DTOR_SYMBOL: &[u8] = b"??1QImage@@UEAA@XZ\0";
const QIMAGE_FILL_SYMBOL: &[u8] = b"?fill@QImage@@QEAAXI@Z\0";
const QIMAGE_BITS_SYMBOL: &[u8] = b"?constBits@QImage@@QEBAPEBEXZ\0";
const QIMAGE_SIZE_SYMBOL: &[u8] = b"?sizeInBytes@QImage@@QEBA_JXZ\0";
const DRAW_POINT_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQPointF@@AEBVQString@@HH@Z\0";
const DRAW_RECT_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQRect@@HAEBVQString@@PEAV2@@Z\0";
const DRAW_RECT_OPTION_SYMBOL: &[u8] =
    b"?drawText@QPainter@@QEAAXAEBVQRectF@@AEBVQString@@AEBVQTextOption@@@Z\0";
const DRAW_RECT_F_SYMBOL: &[u8] = b"?drawText@QPainter@@QEAAXAEBVQRectF@@HAEBVQString@@PEAV2@@Z\0";
const QTEXT_OPTION_CTOR_SYMBOL: &[u8] = b"??0QTextOption@@QEAA@XZ\0";
const QTEXT_OPTION_DTOR_SYMBOL: &[u8] = b"??1QTextOption@@QEAA@XZ\0";
const QSTRING_DTOR_SYMBOL: &[u8] = b"??1QString@@QEAA@XZ\0";
const QSTRING5_CTOR_SYMBOL: &[u8] = b"??0QString@@QEAA@PEBVQChar@@H@Z\0";
const QSTRING6_CTOR_SYMBOL: &[u8] = b"??0QString@@QEAA@PEBVQChar@@_J@Z\0";

type RawProc = unsafe extern "system" fn() -> isize;
type FnQPainterDeviceCtor = unsafe extern "system" fn(*mut c_void, *mut c_void) -> *mut c_void;
type FnQPainterDtor = unsafe extern "system" fn(*mut c_void);
type FnDrawPoint = unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, i32, i32);
type FnDrawRect =
    unsafe extern "system" fn(*mut c_void, *const c_void, i32, *const c_void, *mut c_void);
type FnDrawRectOption =
    unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, *const c_void);
type FnQTextOptionCtor = unsafe extern "system" fn(*mut c_void) -> *mut c_void;
type FnQTextOptionDtor = unsafe extern "system" fn(*mut c_void);
type FnQString5Ctor = unsafe extern "system" fn(*mut c_void, *const u16, i32) -> *mut c_void;
type FnQString6Ctor = unsafe extern "system" fn(*mut c_void, *const u16, i64) -> *mut c_void;
type FnQStringDtor = unsafe extern "system" fn(*mut c_void);
type FnQGuiApplicationCtor =
    unsafe extern "system" fn(*mut c_void, *mut i32, *mut *mut i8, i32) -> *mut c_void;
type FnQGuiApplicationDtor = unsafe extern "system" fn(*mut c_void);
type FnQImageCtor = unsafe extern "system" fn(*mut c_void, i32, i32, i32) -> *mut c_void;
type FnQImageDtor = unsafe extern "system" fn(*mut c_void);
type FnQImageFill = unsafe extern "system" fn(*mut c_void, u32);
type FnQImageBits = unsafe extern "system" fn(*const c_void) -> *const u8;
type FnQImageSize = unsafe extern "system" fn(*const c_void) -> i64;

static MODE: AtomicU8 = AtomicU8::new(1);
static OBSERVED: AtomicUsize = AtomicUsize::new(0);

fn native_package() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift_adapter_qt_painter_native.dll")
}

extern "C" fn decide(
    _context: *mut c_void,
    source: *const u16,
    source_len: u32,
    text_out: *mut u16,
    text_capacity: u32,
    _font_out: *mut u16,
    _font_capacity: u32,
) -> NativeDecisionV1 {
    let source = unsafe { std::slice::from_raw_parts(source, source_len as usize) };
    let source = String::from_utf16(source).expect("Adapter must provide valid UTF-16");
    if source == "Open" {
        OBSERVED.fetch_add(1, Ordering::AcqRel);
    }
    let replacement = match MODE.load(Ordering::Acquire) {
        1 if source == "Open" => "First translation",
        2 if source == "Open" => "Second translation",
        _ => "",
    }
    .encode_utf16()
    .collect::<Vec<_>>();
    assert!(replacement.len() <= text_capacity as usize);
    unsafe { std::ptr::copy_nonoverlapping(replacement.as_ptr(), text_out, replacement.len()) };
    NativeDecisionV1 {
        status: STATUS_OK,
        generation: u64::from(MODE.load(Ordering::Acquire)),
        decision_bits: if replacement.is_empty() {
            0
        } else {
            DECISION_TEXT_REPLACE
        },
        text_len: replacement.len() as u32,
        font_len: 0,
    }
}

extern "C" fn source_characters(_context: *mut c_void, _output: *mut u16, _capacity: u32) -> u32 {
    0
}

fn host() -> &'static NativeRuntimeHostV1 {
    Box::leak(Box::new(NativeRuntimeHostV1 {
        struct_size: std::mem::size_of::<NativeRuntimeHostV1>() as u32,
        context: std::ptr::null_mut(),
        decide_utf16: decide,
        source_characters_utf16: source_characters,
    }))
}

#[test]
fn qt_painter_activation_rejects_a_process_without_qt_modules() {
    let package = unsafe {
        LoadedNativeAdapter::load(
            &native_package(),
            &glyphshift_adapter_qt_painter::descriptor(),
        )
        .expect("load verified Qt Painter package")
    };

    assert_eq!(
        package.activate(
            host(),
            [Feature::TextObserve, Feature::TextReplace],
            [Feature::TextObserve, Feature::TextReplace],
        ),
        Err(NativeHostError::PackageFailure(STATUS_ACTIVATION_FAILED))
    );
}

#[repr(C, align(16))]
struct OpaqueObject([u8; 256]);

#[repr(C)]
struct PointF {
    x: f64,
    y: f64,
}

#[repr(C)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

#[repr(C)]
struct RectF {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
enum LocalDrawKind {
    Point,
    Rect,
    RectF,
    RectOption,
}

struct LocalQtRuntime {
    major: u8,
    core: HMODULE,
    gui: HMODULE,
    application: Box<OpaqueObject>,
    application_dtor: FnQGuiApplicationDtor,
    _argc: Box<i32>,
    _argument: Vec<u8>,
    _arguments: Vec<*mut i8>,
}

impl LocalQtRuntime {
    unsafe fn load(core: &Path, gui: &Path) -> Self {
        let major = match core.file_name().and_then(|name| name.to_str()) {
            Some(name) if name.eq_ignore_ascii_case("Qt5Core.dll") => 5,
            Some(name) if name.eq_ignore_ascii_case("Qt6Core.dll") => 6,
            _ => panic!("configured Qt Core module must be Qt5Core.dll or Qt6Core.dll"),
        };
        let expected_gui = if major == 5 {
            "Qt5Gui.dll"
        } else {
            "Qt6Gui.dll"
        };
        assert!(gui
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(expected_gui)));
        let core = load_module(core);
        let gui = load_module(gui);
        let application_ctor = std::mem::transmute::<RawProc, FnQGuiApplicationCtor>(resolve(
            gui,
            QGUI_APPLICATION_CTOR_SYMBOL,
        ));
        let application_dtor = std::mem::transmute::<RawProc, FnQGuiApplicationDtor>(resolve(
            gui,
            QGUI_APPLICATION_DTOR_SYMBOL,
        ));
        let mut application = Box::new(OpaqueObject([0; 256]));
        let mut argc = Box::new(1_i32);
        let mut argument = std::env::current_exe()
            .expect("current Qt contract executable")
            .to_string_lossy()
            .into_owned()
            .into_bytes();
        argument.push(0);
        let mut arguments = vec![argument.as_mut_ptr().cast::<i8>(), std::ptr::null_mut()];
        application_ctor(
            std::ptr::from_mut(application.as_mut()).cast(),
            std::ptr::from_mut(argc.as_mut()),
            arguments.as_mut_ptr(),
            0,
        );
        Self {
            major,
            core,
            gui,
            application,
            application_dtor,
            _argc: argc,
            _argument: argument,
            _arguments: arguments,
        }
    }

    unsafe fn render_signature(&self, text: &str, kind: LocalDrawKind) -> u64 {
        let image_ctor =
            std::mem::transmute::<RawProc, FnQImageCtor>(resolve(self.gui, QIMAGE_CTOR_SYMBOL));
        let image_dtor =
            std::mem::transmute::<RawProc, FnQImageDtor>(resolve(self.gui, QIMAGE_DTOR_SYMBOL));
        let image_fill =
            std::mem::transmute::<RawProc, FnQImageFill>(resolve(self.gui, QIMAGE_FILL_SYMBOL));
        let image_bits =
            std::mem::transmute::<RawProc, FnQImageBits>(resolve(self.gui, QIMAGE_BITS_SYMBOL));
        let image_size =
            std::mem::transmute::<RawProc, FnQImageSize>(resolve(self.gui, QIMAGE_SIZE_SYMBOL));
        let painter_ctor = std::mem::transmute::<RawProc, FnQPainterDeviceCtor>(resolve(
            self.gui,
            QPAINTER_DEVICE_CTOR_SYMBOL,
        ));
        let painter_dtor =
            std::mem::transmute::<RawProc, FnQPainterDtor>(resolve(self.gui, QPAINTER_DTOR_SYMBOL));
        let draw =
            std::mem::transmute::<RawProc, FnDrawPoint>(resolve(self.gui, DRAW_POINT_SYMBOL));
        let draw_rect =
            std::mem::transmute::<RawProc, FnDrawRect>(resolve(self.gui, DRAW_RECT_SYMBOL));
        let draw_rect_f =
            std::mem::transmute::<RawProc, FnDrawRect>(resolve(self.gui, DRAW_RECT_F_SYMBOL));
        let draw_rect_option = std::mem::transmute::<RawProc, FnDrawRectOption>(resolve(
            self.gui,
            DRAW_RECT_OPTION_SYMBOL,
        ));
        let string_dtor =
            std::mem::transmute::<RawProc, FnQStringDtor>(resolve(self.core, QSTRING_DTOR_SYMBOL));
        let units = text.encode_utf16().collect::<Vec<_>>();
        let mut image = MaybeUninit::<OpaqueObject>::uninit();
        let image = image.as_mut_ptr().cast::<c_void>();
        image_ctor(image, 640, 160, 5);
        image_fill(image, 0xffff_ffff);
        let mut painter = MaybeUninit::<OpaqueObject>::uninit();
        let painter = painter.as_mut_ptr().cast::<c_void>();
        painter_ctor(painter, image);
        let mut string = MaybeUninit::<OpaqueObject>::uninit();
        let string = string.as_mut_ptr().cast::<c_void>();
        if self.major == 5 {
            let ctor = std::mem::transmute::<RawProc, FnQString5Ctor>(resolve(
                self.core,
                QSTRING5_CTOR_SYMBOL,
            ));
            ctor(string, units.as_ptr(), units.len() as i32);
        } else {
            let ctor = std::mem::transmute::<RawProc, FnQString6Ctor>(resolve(
                self.core,
                QSTRING6_CTOR_SYMBOL,
            ));
            ctor(string, units.as_ptr(), units.len() as i64);
        }
        match kind {
            LocalDrawKind::Point => draw(
                painter,
                std::ptr::from_ref(&PointF { x: 24.0, y: 80.0 }).cast(),
                string,
                0,
                -1,
            ),
            LocalDrawKind::Rect => draw_rect(
                painter,
                std::ptr::from_ref(&Rect {
                    x1: 24,
                    y1: 24,
                    x2: 603,
                    y2: 123,
                })
                .cast(),
                0x81,
                string,
                std::ptr::null_mut(),
            ),
            LocalDrawKind::RectF => draw_rect_f(
                painter,
                std::ptr::from_ref(&RectF {
                    x: 24.0,
                    y: 24.0,
                    width: 580.0,
                    height: 100.0,
                })
                .cast(),
                0x81,
                string,
                std::ptr::null_mut(),
            ),
            LocalDrawKind::RectOption => {
                let option_ctor = std::mem::transmute::<RawProc, FnQTextOptionCtor>(resolve(
                    self.gui,
                    QTEXT_OPTION_CTOR_SYMBOL,
                ));
                let option_dtor = std::mem::transmute::<RawProc, FnQTextOptionDtor>(resolve(
                    self.gui,
                    QTEXT_OPTION_DTOR_SYMBOL,
                ));
                let mut option = MaybeUninit::<OpaqueObject>::uninit();
                let option = option.as_mut_ptr().cast::<c_void>();
                option_ctor(option);
                draw_rect_option(
                    painter,
                    std::ptr::from_ref(&RectF {
                        x: 24.0,
                        y: 24.0,
                        width: 580.0,
                        height: 100.0,
                    })
                    .cast(),
                    string,
                    option,
                );
                option_dtor(option);
            }
        }
        string_dtor(string);
        painter_dtor(painter);
        let byte_count = usize::try_from(image_size(image)).expect("positive QImage size");
        let bytes = std::slice::from_raw_parts(image_bits(image), byte_count);
        let signature = bytes
            .iter()
            .fold(14_695_981_039_346_656_037_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211)
            });
        image_dtor(image);
        signature
    }
}

impl Drop for LocalQtRuntime {
    fn drop(&mut self) {
        unsafe {
            (self.application_dtor)(std::ptr::from_mut(self.application.as_mut()).cast());
        }
    }
}

unsafe fn load_module(path: &Path) -> HMODULE {
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_WITH_ALTERED_SEARCH_PATH)
        .expect("load configured Qt module")
}

unsafe fn resolve(module: HMODULE, symbol: &'static [u8]) -> RawProc {
    GetProcAddress(module, PCSTR(symbol.as_ptr())).expect("resolve configured Qt export")
}

#[test]
#[ignore = "requires GLYPHSHIFT_QT_CORE_DLL and GLYPHSHIFT_QT_GUI_DLL for a local MSVC x64 Qt runtime"]
fn configured_qt_runtime_observes_hot_updates_and_deactivates_without_crashing() {
    let core = PathBuf::from(
        std::env::var_os("GLYPHSHIFT_QT_CORE_DLL").expect("configured Qt Core module"),
    );
    let gui =
        PathBuf::from(std::env::var_os("GLYPHSHIFT_QT_GUI_DLL").expect("configured Qt Gui module"));
    std::env::set_var(
        "QT_QPA_PLATFORM",
        std::env::var_os("GLYPHSHIFT_QT_PLATFORM").unwrap_or_else(|| "windows".into()),
    );
    if let Some(plugin_directory) = std::env::var_os("GLYPHSHIFT_QT_PLATFORM_PLUGIN_DIR") {
        let plugin_directory = PathBuf::from(plugin_directory);
        std::env::set_var("QT_QPA_PLATFORM_PLUGIN_PATH", &plugin_directory);
        if let Some(plugin_root) = plugin_directory.parent() {
            std::env::set_var("QT_PLUGIN_PATH", plugin_root);
        }
    }
    let qt = unsafe { LocalQtRuntime::load(&core, &gui) };
    let package = unsafe {
        LoadedNativeAdapter::load(
            &native_package(),
            &glyphshift_adapter_qt_painter::descriptor(),
        )
        .expect("load verified Qt Painter package")
    };
    OBSERVED.store(0, Ordering::Release);
    for kind in [
        LocalDrawKind::Point,
        LocalDrawKind::Rect,
        LocalDrawKind::RectF,
        LocalDrawKind::RectOption,
    ] {
        let baseline = unsafe { qt.render_signature("Open", kind) };
        let expected_first = unsafe { qt.render_signature("First translation", kind) };
        let expected_second = unsafe { qt.render_signature("Second translation", kind) };
        assert_ne!(baseline, expected_first);
        assert_ne!(expected_first, expected_second);

        package
            .activate(
                host(),
                [Feature::TextObserve, Feature::TextReplace],
                [Feature::TextObserve, Feature::TextReplace],
            )
            .expect("activate Qt Painter hooks");
        MODE.store(1, Ordering::Release);
        assert_eq!(unsafe { qt.render_signature("Open", kind) }, expected_first);
        MODE.store(2, Ordering::Release);
        assert_eq!(
            unsafe { qt.render_signature("Open", kind) },
            expected_second
        );

        package.deactivate().expect("deactivate Qt Painter hooks");
        assert_eq!(unsafe { qt.render_signature("Open", kind) }, baseline);
    }
    assert_eq!(OBSERVED.load(Ordering::Acquire), 8);

    package.deactivate().expect("leave hooks in pass-through");
}
