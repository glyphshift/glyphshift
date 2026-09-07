#![cfg(windows)]

use glyphshift_windows_host::{
    render_raw_draw_text, render_raw_gdi_glyph_indices, render_raw_gdi_symbol,
    render_raw_gdi_unicode, render_raw_gdiplus_symbol, render_raw_gdiplus_unicode,
    render_raw_text_out, run_ocr_capture_server, run_uia_standard_control_server,
    write_raw_console,
};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const CONSOLE_CHILD_ARGUMENT: &str = "--console-child-server";
const OCR_CAPTURE_TARGET_ARGUMENT: &str = "--ocr-capture-fixture";
const UIA_TARGET_ARGUMENT: &str = "--uia-standard-controls";
const PARENT_CONSOLE_TEXT: &str = "ParentConsoleText";
const CHILD_CONSOLE_TEXT: &str = "ChildConsoleText";

struct ConsoleChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ConsoleChild {
    fn start() -> io::Result<Self> {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new(std::env::current_exe()?)
            .arg(CONSOLE_CHILD_ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing synthetic child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .map(BufReader::new)
            .ok_or_else(|| io::Error::other("missing synthetic child stdout"))?;
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn write_console(&mut self) -> io::Result<()> {
        writeln!(self.stdin, "write-console")?;
        self.stdin.flush()?;
        let mut acknowledgement = String::new();
        self.stdout.read_line(&mut acknowledgement)?;
        if acknowledgement.trim() != "child-console-attempted" {
            return Err(io::Error::other("synthetic child did not acknowledge"));
        }
        Ok(())
    }

    fn stop(mut self) -> io::Result<()> {
        writeln!(self.stdin, "exit")?;
        self.stdin.flush()?;
        let status = self.child.wait()?;
        if !status.success() {
            return Err(io::Error::other("synthetic child exited unsuccessfully"));
        }
        Ok(())
    }

    fn start_renderer(executable: &std::path::Path) -> io::Result<Self> {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing child input"))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("missing child output"))?,
        );
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }
    fn render(&mut self, command: &str) -> io::Result<String> {
        writeln!(self.stdin, "{command}")?;
        self.stdin.flush()?;
        let mut value = String::new();
        self.stdout.read_line(&mut value)?;
        if value.is_empty() {
            return Err(io::Error::other("child render failed"));
        }
        Ok(value)
    }
}

impl Drop for ConsoleChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn run_console_child() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        match line?.trim() {
            "write-console" => {
                let _ = write_raw_console(CHILD_CONSOLE_TEXT);
                writeln!(stdout, "child-console-attempted")?;
                stdout.flush()?;
            }
            "exit" => break,
            _ => {
                writeln!(stdout, "error")?;
                stdout.flush()?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == UIA_TARGET_ARGUMENT)
    {
        let keepalive = arguments
            .iter()
            .find_map(|argument| argument.strip_prefix("--uia-keepalive-ms="))
            .and_then(|value| value.parse::<u64>().ok())
            .map(std::time::Duration::from_millis);
        return run_uia_standard_control_server(keepalive).map_err(Into::into);
    }
    if arguments
        .iter()
        .any(|argument| argument == OCR_CAPTURE_TARGET_ARGUMENT)
    {
        let protected = arguments.iter().any(|argument| argument == "--protected");
        let negative_origin = arguments
            .iter()
            .any(|argument| argument == "--negative-origin");
        return run_ocr_capture_server(protected, negative_origin).map_err(Into::into);
    }
    if arguments
        .iter()
        .any(|argument| argument == CONSOLE_CHILD_ARGUMENT)
    {
        return run_console_child().map_err(Into::into);
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut console_child = None;
    let mut render_child: Option<ConsoleChild> = None;
    for line in stdin.lock().lines() {
        match line?.trim() {
            command if command.starts_with("start-render-child ") => {
                if render_child.is_some() {
                    return Err(io::Error::other("child already started").into());
                }
                render_child = Some(ConsoleChild::start_renderer(std::path::Path::new(
                    &command[19..],
                ))?);
                writeln!(stdout, "child-started")?;
                stdout.flush()?;
            }
            command if command.starts_with("render-child ") => {
                let response = render_child
                    .as_mut()
                    .ok_or_else(|| io::Error::other("child missing"))?
                    .render(&command[13..])?;
                write!(stdout, "{response}")?;
                stdout.flush()?;
            }
            "render-text-out" => {
                let evidence = render_raw_text_out("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render-draw-text" => {
                let evidence = render_raw_draw_text("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            command if command.starts_with("render-text ") => {
                let text = &command[12..];
                if text.len() > 1024 {
                    return Err(io::Error::other("synthetic text limit").into());
                }
                let evidence = render_raw_gdi_unicode(text)?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render" => {
                let evidence = render_raw_gdi_unicode("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render-gdi-symbol" => {
                let evidence = render_raw_gdi_symbol("ABC")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render-gdi-glyph-indices" => {
                let evidence = render_raw_gdi_glyph_indices()?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render-gdiplus-symbol" => {
                let evidence = render_raw_gdiplus_symbol("ABC")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "render-gdiplus" => {
                let evidence = render_raw_gdiplus_unicode("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            "write-console" => {
                let _ = write_raw_console(PARENT_CONSOLE_TEXT);
                writeln!(stdout, "parent-console-attempted")?;
                stdout.flush()?;
            }
            "start-console-child" => {
                if console_child.is_none() {
                    console_child = Some(ConsoleChild::start()?);
                }
                writeln!(stdout, "child-started")?;
                stdout.flush()?;
            }
            "child-write-console" => {
                console_child
                    .as_mut()
                    .ok_or_else(|| io::Error::other("synthetic child is not running"))?
                    .write_console()?;
                writeln!(stdout, "child-console-attempted")?;
                stdout.flush()?;
            }
            "stop-console-child" => {
                if let Some(child) = console_child.take() {
                    child.stop()?;
                }
                writeln!(stdout, "child-stopped")?;
                stdout.flush()?;
            }
            "exit" => {
                if let Some(child) = console_child.take() {
                    child.stop()?;
                }
                if let Some(child) = render_child.take() {
                    child.stop()?;
                }
                break;
            }
            _ => {
                writeln!(stdout, "error")?;
                stdout.flush()?;
            }
        }
    }
    Ok(())
}
