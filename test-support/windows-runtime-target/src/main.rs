#![cfg(windows)]

use glyphshift_windows_host::{
    render_raw_directwrite_layout, render_raw_draw_text, render_raw_gdi_glyph_indices,
    render_raw_gdi_symbol, render_raw_gdi_unicode, render_raw_gdiplus_symbol,
    render_raw_gdiplus_unicode, render_raw_text_out,
};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct RenderChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl RenderChild {
    fn start(executable: &std::path::Path) -> io::Result<Self> {
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

    fn stop(mut self) -> io::Result<()> {
        writeln!(self.stdin, "exit")?;
        self.stdin.flush()?;
        let status = self.child.wait()?;
        if !status.success() {
            return Err(io::Error::other("synthetic child exited unsuccessfully"));
        }
        Ok(())
    }
}

impl Drop for RenderChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut render_child: Option<RenderChild> = None;

    for line in stdin.lock().lines() {
        let line = line?;
        let command = line.trim();
        match command {
            command if command.starts_with("start-render-child ") => {
                if render_child.is_some() {
                    return Err(io::Error::other("child already started").into());
                }
                let executable = command
                    .strip_prefix("start-render-child ")
                    .ok_or_else(|| io::Error::other("missing child executable"))?;
                render_child = Some(RenderChild::start(std::path::Path::new(executable))?);
                writeln!(stdout, "child-started")?;
                stdout.flush()?;
            }
            command if command.starts_with("render-child ") => {
                let child_command = command
                    .strip_prefix("render-child ")
                    .ok_or_else(|| io::Error::other("missing child command"))?;
                let response = render_child
                    .as_mut()
                    .ok_or_else(|| io::Error::other("child missing"))?
                    .render(child_command)?;
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
            "render-directwrite" => {
                let evidence = render_raw_directwrite_layout("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
                stdout.flush()?;
            }
            command if command.starts_with("render-text ") => {
                let text = command
                    .strip_prefix("render-text ")
                    .ok_or_else(|| io::Error::other("missing render text"))?;
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
            "exit" => {
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
