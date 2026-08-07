use glyphshift_adapter_gdi_native_support::allows_font_substitution;
use windows::Win32::Graphics::Gdi::{ANSI_CHARSET, DEFAULT_CHARSET, SYMBOL_CHARSET};

#[test]
fn gdi_font_substitution_protects_symbol_charsets_without_blocking_text_fonts() {
    assert!(!allows_font_substitution(SYMBOL_CHARSET));
    assert!(allows_font_substitution(DEFAULT_CHARSET));
    assert!(allows_font_substitution(ANSI_CHARSET));
}
