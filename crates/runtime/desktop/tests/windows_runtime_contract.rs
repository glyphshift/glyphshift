#![cfg(windows)]

use glyphshift_capture::{CaptureCatalog, CaptureConfiguration, CaptureSessionId};
use glyphshift_desktop_backend::{
    DesktopBackend, DesktopEnvironment, DesktopRuntimeSpec, DictionaryCreate, DictionaryEdit,
    DictionaryEntryCreate, DictionaryView, ExecutableSelection, FontCoverage, WorkflowCreate,
    WorkflowFontPolicy, WorkflowTargetCreate,
};
use glyphshift_desktop_runtime::{DesktopRuntimePool, RuntimeBundle};
use glyphshift_domain::Feature;
use glyphshift_session::{RuntimeTextOutcome, RuntimeTraceStatus};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::tempdir;

const TEST_ADAPTER_ID: &str = "windows.gdi.ext-text-out";
const TEST_GDIPLUS_ADAPTER_ID: &str = "windows.gdiplus.draw-string";
#[path = "windows_runtime_contract/authorized_host.rs"]
mod authorized_host;
#[path = "windows_runtime_contract/pool.rs"]
mod pool;
#[path = "windows_runtime_contract/synthetic_runtime.rs"]
mod synthetic_runtime;

fn open_backend(data_root: &Path, runtime_root: &Path) -> DesktopBackend {
    let bundle = RuntimeBundle::open(runtime_root).expect("verified Runtime bundle");
    DesktopBackend::open_with_environment(
        data_root,
        DesktopEnvironment::new(
            bundle.adapter_requirements().iter().cloned(),
            ["Arial", "Courier New"],
        ),
    )
    .expect("desktop backend")
}

struct TargetProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl TargetProcess {
    fn spawn(executable: &Path) -> Self {
        Self::spawn_with_args(executable, std::iter::empty::<&str>())
    }

    fn spawn_with_args<'a>(
        executable: &Path,
        arguments: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .expect("isolated target should start");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = BufReader::new(child.stdout.take().expect("target stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn render(&mut self) -> String {
        self.render_command("render")
    }

    fn render_command(&mut self, command: &str) -> String {
        writeln!(self.stdin, "{command}").expect("render command");
        self.stdin.flush().expect("flush render command");
        self.read_response()
    }

    fn read_response(&mut self) -> String {
        let mut evidence = String::new();
        self.stdout
            .read_line(&mut evidence)
            .expect("read target evidence");
        assert!(!evidence.is_empty(), "target should return pixel evidence");
        evidence.trim().to_owned()
    }

    fn stop(&mut self) {
        let _ = writeln!(self.stdin, "exit");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

fn create_all_observations_font_workflow(
    backend: &mut DesktopBackend,
    software_id: &str,
    family: &str,
) -> DesktopRuntimeSpec {
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.font-safety", "Font safety").with_targets([
                WorkflowTargetCreate::new(
                    software_id,
                    [TEST_ADAPTER_ID, TEST_GDIPLUS_ADAPTER_ID],
                    [] as [&str; 0],
                )
                .with_font_policy(WorkflowFontPolicy::new(
                    [family],
                    FontCoverage::AllObservations,
                )),
            ]),
        )
        .expect("create all-observations font workflow");
    backend
        .workflow_runtime_spec("workflow.font-safety", software_id)
        .expect("compiled font safety Runtime spec")
}

impl Drop for TargetProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn create_text_workflow(
    backend: &mut DesktopBackend,
    software_id: &str,
    dictionary_id: &str,
    workflow_id: &str,
    source: &str,
    translation: &str,
) -> (DictionaryView, DesktopRuntimeSpec) {
    create_text_workflow_with_adapter(
        backend,
        software_id,
        TEST_ADAPTER_ID,
        dictionary_id,
        workflow_id,
        source,
        translation,
    )
}

fn create_text_workflow_with_adapter(
    backend: &mut DesktopBackend,
    software_id: &str,
    adapter_id: &str,
    dictionary_id: &str,
    workflow_id: &str,
    source: &str,
    translation: &str,
) -> (DictionaryView, DesktopRuntimeSpec) {
    let dictionary = backend
        .create_dictionary(
            DictionaryCreate::new(dictionary_id, dictionary_id, "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new(source, translation)]),
        )
        .expect("create Runtime dictionary");
    backend
        .create_workflow(WorkflowCreate::new(workflow_id, workflow_id).with_targets([
            WorkflowTargetCreate::new(software_id, [adapter_id], [dictionary_id]),
        ]))
        .expect("create Runtime workflow");
    let spec = backend
        .workflow_runtime_spec(workflow_id, software_id)
        .expect("compiled workflow Runtime spec");
    (dictionary, spec)
}
