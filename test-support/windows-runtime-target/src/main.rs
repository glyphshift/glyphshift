#![cfg(windows)]

use glyphshift_windows_host::{
    render_raw_gdi_glyph_indices, render_raw_gdi_symbol, render_raw_gdi_unicode,
    render_raw_gdiplus_symbol,
};
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
            "exit" => break,
            _ => {
                writeln!(stdout, "error")?;
                stdout.flush()?;
            }
        }
    }
    Ok(())
}
