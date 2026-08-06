use glyphshift_acquisition::{AcquisitionError, Confidence};
use glyphshift_adapter_ocr::{OcrEngine, OcrInputFrame, OcrLocalRect, OcrTextBlock};
use std::ffi::{c_char, c_float, c_int, c_uchar, c_void, CStr, CString};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

const TESSERACT_ABI_MAJOR: &str = "5";
const PAGE_SEGMENTATION_SPARSE_TEXT: c_int = 11;
const PAGE_ITERATOR_WORD: c_int = 3;
const MAX_OCR_BLOCKS: usize = 256;
const MAX_OCR_TEXT_BYTES: usize = 16 * 1024;

/// Why a verified Tesseract artifact could not become an OCR engine.
///
/// Paths and native diagnostics are deliberately not carried in this value so callers can report
/// capability failures without leaking a local Bundle layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TesseractEngineError {
    InvalidLibrary,
    InvalidDataDirectory,
    InvalidLanguage,
    MissingLanguageData,
    LibraryLoadFailed,
    MissingSymbol,
    UnsupportedVersion,
    InitializationFailed,
}

/// One process-local Tesseract 5 engine loaded from a Runtime Bundle artifact.
///
/// The caller must verify the DLL, its colocated native dependencies and every language model
/// before constructing this type. Loading uses only the DLL directory and System32 for dependency
/// resolution; current-directory and user PATH lookup are excluded.
pub struct TesseractEngine {
    handle: *mut c_void,
    api: NativeApi,
    _library: NativeLibrary,
    version: Box<str>,
}

impl TesseractEngine {
    pub fn load(
        library_path: impl AsRef<Path>,
        data_directory: impl AsRef<Path>,
        languages: &str,
    ) -> Result<Self, TesseractEngineError> {
        let library_path =
            canonical_file(library_path.as_ref()).ok_or(TesseractEngineError::InvalidLibrary)?;
        let data_directory = canonical_directory(data_directory.as_ref())
            .ok_or(TesseractEngineError::InvalidDataDirectory)?;
        let languages = validate_languages(languages)?;
        validate_language_data(&data_directory, &languages)?;

        let library = NativeLibrary::load(&library_path)?;
        let api = NativeApi::load(library.module)?;
        let version = api.version()?;
        if version.split('.').next() != Some(TESSERACT_ABI_MAJOR) {
            return Err(TesseractEngineError::UnsupportedVersion);
        }

        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(TesseractEngineError::InitializationFailed);
        }
        let data_directory =
            path_c_string(&data_directory).ok_or(TesseractEngineError::InvalidDataDirectory)?;
        let languages =
            CString::new(languages.join("+")).map_err(|_| TesseractEngineError::InvalidLanguage)?;
        let initialized =
            unsafe { (api.init)(handle, data_directory.as_ptr(), languages.as_ptr()) };
        if initialized != 0 {
            unsafe { (api.delete)(handle) };
            return Err(TesseractEngineError::InitializationFailed);
        }
        unsafe { (api.set_page_segmentation_mode)(handle, PAGE_SEGMENTATION_SPARSE_TEXT) };

        Ok(Self {
            handle,
            api,
            _library: library,
            version: version.into(),
        })
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

impl OcrEngine for TesseractEngine {
    fn recognize(&mut self, frame: &OcrInputFrame) -> Result<Vec<OcrTextBlock>, AcquisitionError> {
        let width =
            c_int::try_from(frame.width()).map_err(|_| AcquisitionError::ProviderUnavailable)?;
        let height =
            c_int::try_from(frame.height()).map_err(|_| AcquisitionError::ProviderUnavailable)?;
        let stride = width
            .checked_mul(4)
            .ok_or(AcquisitionError::ProviderUnavailable)?;

        unsafe {
            (self.api.set_image)(
                self.handle,
                frame.rgba8().as_ptr(),
                width,
                height,
                4,
                stride,
            );
        }
        let _reset = RecognitionReset {
            handle: self.handle,
            clear: self.api.clear,
        };
        if unsafe { (self.api.recognize)(self.handle, std::ptr::null_mut()) } != 0 {
            return Err(AcquisitionError::ProviderUnavailable);
        }

        self.collect_blocks(frame.width(), frame.height())
            .map_err(|_| AcquisitionError::ProviderUnavailable)
    }
}

impl TesseractEngine {
    fn collect_blocks(
        &self,
        frame_width: usize,
        frame_height: usize,
    ) -> Result<Vec<OcrTextBlock>, TesseractEngineError> {
        let iterator = unsafe { (self.api.get_iterator)(self.handle) };
        if iterator.is_null() {
            return Ok(Vec::new());
        }
        let _iterator = IteratorReset {
            iterator,
            delete: self.api.delete_iterator,
        };
        let mut blocks = Vec::new();
        let mut text_bytes = 0usize;

        for _ in 0..MAX_OCR_BLOCKS {
            let text = unsafe { self.api.iterator_text(iterator, PAGE_ITERATOR_WORD)? };
            let mut native_bounds = [0; 4];
            let has_bounds = unsafe {
                (self.api.iterator_bounds)(
                    iterator,
                    PAGE_ITERATOR_WORD,
                    &mut native_bounds[0],
                    &mut native_bounds[1],
                    &mut native_bounds[2],
                    &mut native_bounds[3],
                )
            } != 0;
            let confidence =
                unsafe { (self.api.iterator_confidence)(iterator, PAGE_ITERATOR_WORD) };

            if let Some(text) = text {
                if let Some(block) =
                    block_from_native(&text, native_bounds, confidence, frame_width, frame_height)
                {
                    let source_bytes = text.trim().len();
                    if source_bytes <= MAX_OCR_TEXT_BYTES.saturating_sub(text_bytes) {
                        text_bytes += source_bytes;
                        if has_bounds {
                            blocks.push(block);
                        }
                    }
                }
            }
            if unsafe { (self.api.iterator_next)(iterator, PAGE_ITERATOR_WORD) } == 0 {
                break;
            }
        }
        Ok(blocks)
    }
}

impl Drop for TesseractEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.api.clear)(self.handle);
                (self.api.end)(self.handle);
                (self.api.delete)(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

fn canonical_file(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    path.is_absolute().then_some(())?;
    path.is_file().then_some(path)
}

fn canonical_directory(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    path.is_absolute().then_some(())?;
    path.is_dir().then_some(path)
}

fn path_c_string(path: &Path) -> Option<CString> {
    let path = path.to_str()?;
    let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(drive_path) = path.strip_prefix(r"\\?\") {
        drive_path.to_owned()
    } else {
        path.to_owned()
    };
    CString::new(path).ok()
}

fn validate_languages(languages: &str) -> Result<Vec<&str>, TesseractEngineError> {
    let languages = languages.split('+').collect::<Vec<_>>();
    if languages.is_empty()
        || languages.iter().any(|language| {
            language.is_empty()
                || language.len() > 32
                || !language
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    {
        return Err(TesseractEngineError::InvalidLanguage);
    }
    Ok(languages)
}

fn validate_language_data(
    data_directory: &Path,
    languages: &[&str],
) -> Result<(), TesseractEngineError> {
    let complete = languages.iter().all(|language| {
        data_directory
            .join(format!("{language}.traineddata"))
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    });
    if complete {
        Ok(())
    } else {
        Err(TesseractEngineError::MissingLanguageData)
    }
}

fn block_from_native(
    text: &str,
    bounds: [c_int; 4],
    confidence: c_float,
    frame_width: usize,
    frame_height: usize,
) -> Option<OcrTextBlock> {
    let source = text.trim();
    if source.is_empty() || source.len() > MAX_OCR_TEXT_BYTES {
        return None;
    }
    let [left, top, right, bottom] = bounds.map(u32::try_from);
    let bounds = OcrLocalRect::new(left.ok()?, top.ok()?, right.ok()?, bottom.ok()?)?;
    if usize::try_from(right.ok()?).ok()? > frame_width
        || usize::try_from(bottom.ok()?).ok()? > frame_height
    {
        return None;
    }
    let block = OcrTextBlock::new(source, bounds);
    Some(match confidence_basis_points(confidence) {
        Some(confidence) => block.with_confidence(confidence),
        None => block,
    })
}

fn confidence_basis_points(value: c_float) -> Option<Confidence> {
    value
        .is_finite()
        .then(|| (value.clamp(0.0, 100.0) * 100.0).round() as u16)
        .and_then(Confidence::new)
}

struct RecognitionReset {
    handle: *mut c_void,
    clear: TessBaseApiClear,
}

impl Drop for RecognitionReset {
    fn drop(&mut self) {
        unsafe { (self.clear)(self.handle) };
    }
}

struct IteratorReset {
    iterator: *mut c_void,
    delete: TessResultIteratorDelete,
}

impl Drop for IteratorReset {
    fn drop(&mut self) {
        unsafe { (self.delete)(self.iterator) };
    }
}

struct NativeLibrary {
    module: HMODULE,
}

impl NativeLibrary {
    fn load(path: &Path) -> Result<Self, TesseractEngineError> {
        let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
        wide.push(0);
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if module.is_null() {
            Err(TesseractEngineError::LibraryLoadFailed)
        } else {
            Ok(Self { module })
        }
    }
}

impl Drop for NativeLibrary {
    fn drop(&mut self) {
        if !self.module.is_null() {
            unsafe {
                let _ = FreeLibrary(self.module);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct NativeApi {
    version: TessVersion,
    create: TessBaseApiCreate,
    init: TessBaseApiInit,
    set_page_segmentation_mode: TessBaseApiSetPageSegMode,
    set_image: TessBaseApiSetImage,
    recognize: TessBaseApiRecognize,
    get_iterator: TessBaseApiGetIterator,
    iterator_text: TessResultIteratorGetUtf8Text,
    iterator_confidence: TessResultIteratorConfidence,
    iterator_bounds: TessPageIteratorBoundingBox,
    iterator_next: TessResultIteratorNext,
    delete_iterator: TessResultIteratorDelete,
    delete_text: TessDeleteText,
    clear: TessBaseApiClear,
    end: TessBaseApiEnd,
    delete: TessBaseApiDelete,
}

impl NativeApi {
    fn load(module: HMODULE) -> Result<Self, TesseractEngineError> {
        macro_rules! symbol {
            ($name:literal, $kind:ty) => {{
                let address = unsafe { GetProcAddress(module, concat!($name, "\0").as_ptr()) }
                    .ok_or(TesseractEngineError::MissingSymbol)?;
                unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $kind>(address)
                }
            }};
        }
        Ok(Self {
            version: symbol!("TessVersion", TessVersion),
            create: symbol!("TessBaseAPICreate", TessBaseApiCreate),
            init: symbol!("TessBaseAPIInit3", TessBaseApiInit),
            set_page_segmentation_mode: symbol!(
                "TessBaseAPISetPageSegMode",
                TessBaseApiSetPageSegMode
            ),
            set_image: symbol!("TessBaseAPISetImage", TessBaseApiSetImage),
            recognize: symbol!("TessBaseAPIRecognize", TessBaseApiRecognize),
            get_iterator: symbol!("TessBaseAPIGetIterator", TessBaseApiGetIterator),
            iterator_text: symbol!(
                "TessResultIteratorGetUTF8Text",
                TessResultIteratorGetUtf8Text
            ),
            iterator_confidence: symbol!(
                "TessResultIteratorConfidence",
                TessResultIteratorConfidence
            ),
            iterator_bounds: symbol!("TessPageIteratorBoundingBox", TessPageIteratorBoundingBox),
            iterator_next: symbol!("TessResultIteratorNext", TessResultIteratorNext),
            delete_iterator: symbol!("TessResultIteratorDelete", TessResultIteratorDelete),
            delete_text: symbol!("TessDeleteText", TessDeleteText),
            clear: symbol!("TessBaseAPIClear", TessBaseApiClear),
            end: symbol!("TessBaseAPIEnd", TessBaseApiEnd),
            delete: symbol!("TessBaseAPIDelete", TessBaseApiDelete),
        })
    }

    fn version(self) -> Result<String, TesseractEngineError> {
        let version = unsafe { (self.version)() };
        if version.is_null() {
            return Err(TesseractEngineError::UnsupportedVersion);
        }
        let version = unsafe { CStr::from_ptr(version) }
            .to_str()
            .map_err(|_| TesseractEngineError::UnsupportedVersion)?;
        Ok(version.to_owned())
    }

    unsafe fn iterator_text(
        self,
        iterator: *mut c_void,
        level: c_int,
    ) -> Result<Option<String>, TesseractEngineError> {
        let text = unsafe { (self.iterator_text)(iterator, level) };
        if text.is_null() {
            return Ok(None);
        }
        let bytes = unsafe { CStr::from_ptr(text) }.to_bytes().to_vec();
        unsafe { (self.delete_text)(text) };
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| TesseractEngineError::InitializationFailed)
    }
}

type TessVersion = unsafe extern "C" fn() -> *const c_char;
type TessBaseApiCreate = unsafe extern "C" fn() -> *mut c_void;
type TessBaseApiInit = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type TessBaseApiSetPageSegMode = unsafe extern "C" fn(*mut c_void, c_int);
type TessBaseApiSetImage =
    unsafe extern "C" fn(*mut c_void, *const c_uchar, c_int, c_int, c_int, c_int);
type TessBaseApiRecognize = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type TessBaseApiGetIterator = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type TessResultIteratorGetUtf8Text = unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_char;
type TessResultIteratorConfidence = unsafe extern "C" fn(*mut c_void, c_int) -> c_float;
type TessPageIteratorBoundingBox = unsafe extern "C" fn(
    *mut c_void,
    c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
) -> c_int;
type TessResultIteratorNext = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type TessResultIteratorDelete = unsafe extern "C" fn(*mut c_void);
type TessDeleteText = unsafe extern "C" fn(*mut c_char);
type TessBaseApiClear = unsafe extern "C" fn(*mut c_void);
type TessBaseApiEnd = unsafe extern "C" fn(*mut c_void);
type TessBaseApiDelete = unsafe extern "C" fn(*mut c_void);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_spec_is_bounded_and_cannot_escape_the_data_directory() {
        assert_eq!(
            validate_languages("chi_sim+eng"),
            Ok(vec!["chi_sim", "eng"])
        );
        assert_eq!(
            validate_languages("../eng"),
            Err(TesseractEngineError::InvalidLanguage)
        );
        assert_eq!(
            validate_languages("eng++chi_sim"),
            Err(TesseractEngineError::InvalidLanguage)
        );
    }

    #[test]
    fn native_words_are_trimmed_bounded_and_mapped_to_local_blocks() {
        let confidence = Confidence::new(9_876).expect("confidence");
        let expected = OcrTextBlock::new(
            "visible text",
            OcrLocalRect::new(2, 3, 20, 12).expect("bounds"),
        )
        .with_confidence(confidence);

        assert_eq!(
            block_from_native("  visible text  ", [2, 3, 20, 12], 98.76, 40, 20),
            Some(expected)
        );
        assert_eq!(
            block_from_native("outside", [2, 3, 41, 12], 50.0, 40, 20),
            None
        );
        assert_eq!(
            block_from_native("negative", [-1, 3, 20, 12], 50.0, 40, 20),
            None
        );
    }

    #[test]
    fn confidence_is_clamped_and_non_finite_values_are_omitted() {
        assert_eq!(confidence_basis_points(101.0), Confidence::new(10_000));
        assert_eq!(confidence_basis_points(-1.0), Confidence::new(0));
        assert_eq!(confidence_basis_points(f32::NAN), None);
    }

    #[test]
    fn extended_windows_paths_are_normalized_only_at_the_tesseract_abi() {
        let drive =
            path_c_string(Path::new(r"\\?\X:\SyntheticFixtures\tessdata")).expect("drive path");
        let unc = path_c_string(Path::new(
            r"\\?\UNC\SyntheticServer\SyntheticShare\tessdata",
        ))
        .expect("UNC path");

        assert_eq!(drive.to_str(), Ok(r"X:\SyntheticFixtures\tessdata"));
        assert_eq!(
            unc.to_str(),
            Ok(r"\\SyntheticServer\SyntheticShare\tessdata")
        );
    }
}
