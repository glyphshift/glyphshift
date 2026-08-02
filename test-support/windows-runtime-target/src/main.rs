#![cfg(windows)]

use glyphshift_windows_host::render_raw_gdi_unicode;
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        match line?.trim() {
            "render" => {
                let evidence = render_raw_gdi_unicode("Open")?;
                writeln!(stdout, "{}:{}", evidence.ink_pixels(), evidence.signature())?;
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
