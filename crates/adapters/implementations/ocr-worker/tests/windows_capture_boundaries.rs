#![cfg(windows)]

use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionError, AcquisitionRequest, AuthorizedTarget, Confidence,
    DesktopRect, Granularity, InteractiveSelection, InteractiveTextAcquisition, Provenance,
    SourcePolicy,
};
use glyphshift_adapter_ocr::{
    FrameSource, OcrEngine, OcrInputFrame, OcrLocalRect, OcrTextBlock, VisualOcrAcquisitionAdapter,
};
use glyphshift_adapter_ocr_worker::{WindowsGraphicsFrameSource, ACQUISITION_ADAPTER_ID};
use glyphshift_worker_process_grant::{
    authorize_process_target, process_started_at, WINDOWS_PROCESS_GRANT_PLATFORM,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

static OCR_BOUNDARY_TARGET_LOCK: Mutex<()> = Mutex::new(());

struct BoundaryTarget {
    _serial: MutexGuard<'static, ()>,
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    bounds: DesktopRect,
}

impl BoundaryTarget {
    fn start(protected: bool, negative_origin: bool) -> Self {
        use std::os::windows::process::CommandExt;

        let serial = OCR_BOUNDARY_TARGET_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut command = Command::new(boundary_target_executable());
        command.arg("--ocr-capture-fixture");
        if protected {
            command.arg("--protected");
        }
        if negative_origin {
            command.arg("--negative-origin");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .expect("start OCR boundary target");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = child.stdout.take().expect("target stdout");
        let (sender, responses) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let ready = responses
            .recv_timeout(Duration::from_secs(3))
            .expect("target ready response");
        let values = ready
            .split_whitespace()
            .skip(1)
            .map(|value| value.parse::<i32>().expect("fixture coordinate"))
            .collect::<Vec<_>>();
        assert!(ready.starts_with("ocr-ready "));
        assert_eq!(values.len(), 4);
        let bounds =
            DesktopRect::new(values[0], values[1], values[2], values[3]).expect("fixture bounds");
        Self {
            _serial: serial,
            child,
            stdin,
            responses,
            reader: Some(reader),
            bounds,
        }
    }

    fn process_id(&self) -> u32 {
        self.child.id()
    }

    fn authorized_process(&self) -> glyphshift_worker_process_grant::AuthorizedProcess {
        let process_id = self.process_id();
        let started_at = process_started_at(process_id).expect("target start time");
        authorize_process_target(
            ACQUISITION_ADAPTER_ID,
            ACQUISITION_ADAPTER_ID,
            WINDOWS_PROCESS_GRANT_PLATFORM,
            &format!("{process_id}:{started_at}"),
        )
        .expect("authorized OCR boundary target")
    }

    fn stop(mut self) {
        writeln!(self.stdin, "exit").expect("stop target");
        self.stdin.flush().expect("flush target exit");
        assert_eq!(
            self.responses
                .recv_timeout(Duration::from_secs(3))
                .expect("target exit response"),
            "ocr-exiting"
        );
        for _ in 0..30 {
            if let Some(status) = self.child.try_wait().expect("query target exit") {
                assert!(status.success());
                self.join_reader();
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.join_reader();
        panic!("OCR boundary target did not exit in time");
    }

    fn join_reader(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for BoundaryTarget {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.join_reader();
    }
}

struct PixelProbe;

impl OcrEngine for PixelProbe {
    fn recognize(&mut self, frame: &OcrInputFrame) -> Result<Vec<OcrTextBlock>, AcquisitionError> {
        assert!(frame
            .rgba8()
            .chunks_exact(4)
            .any(|pixel| { pixel[3] != 0 && (pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0) }));
        let bounds = OcrLocalRect::new(
            0,
            0,
            u32::try_from(frame.width()).expect("frame width"),
            u32::try_from(frame.height()).expect("frame height"),
        )
        .expect("frame bounds");
        Ok(vec![OcrTextBlock::new("captured", bounds).with_confidence(
            Confidence::new(10_000).expect("confidence"),
        )])
    }
}

#[test]
#[ignore = "requires an interactive Windows desktop and deterministic local target"]
fn per_monitor_aware_capture_preserves_partial_negative_window_coordinates() {
    let target_process = BoundaryTarget::start(false, true);
    assert!(target_process.bounds.left() < 0);
    assert!(target_process.bounds.top() < 0);
    let target = AuthorizedTarget::new("negative-window").expect("authorized target");
    let frames = WindowsGraphicsFrameSource::for_region(
        target.clone(),
        target_process.authorized_process(),
        target_process.bounds,
    )
    .expect("negative-coordinate frame source");
    let adapter = VisualOcrAcquisitionAdapter::new(frames, PixelProbe);
    let mut acquisition =
        InteractiveTextAcquisition::new([Box::new(adapter) as Box<dyn AcquisitionAdapter>]);
    let request = AcquisitionRequest::new(
        target,
        InteractiveSelection::Region(target_process.bounds),
        SourcePolicy::VisualOnly,
    );

    let result = acquisition.acquire(&request).expect("visual acquisition");

    assert_eq!(result.blocks().len(), 1);
    assert_eq!(result.blocks()[0].granularity(), Granularity::Region);
    assert_eq!(result.blocks()[0].provenance(), Provenance::Visual);
    assert_eq!(result.blocks()[0].anchors(), [target_process.bounds]);
    target_process.stop();
}

#[test]
#[ignore = "requires an interactive Windows desktop and deterministic local target"]
fn capture_excluded_window_fails_closed_without_exposing_pixels() {
    let target_process = BoundaryTarget::start(true, false);
    let target = AuthorizedTarget::new("protected-window").expect("authorized target");
    let mut frames = WindowsGraphicsFrameSource::for_region(
        target.clone(),
        target_process.authorized_process(),
        target_process.bounds,
    )
    .expect("protected frame source");

    assert_eq!(
        frames.capture(&target).err(),
        Some(AcquisitionError::PermissionDenied)
    );
    target_process.stop();
}

fn boundary_target_executable() -> PathBuf {
    PathBuf::from(
        std::env::var_os("GLYPHSHIFT_OCR_BOUNDARY_TARGET_EXE")
            .expect("deterministic OCR boundary target executable"),
    )
}
