//! Copy the current, uniformly formatted layout without mutating the target's object.

use windows::core::{Interface, Result, PCWSTR};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFontFallback, IDWriteTextLayout, IDWriteTextLayout1, IDWriteTextLayout2,
    IDWriteTextLayout3, IDWriteTextLayout4, DWRITE_FONT_AXIS_VALUE, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_SPACING,
    DWRITE_LINE_SPACING_METHOD_DEFAULT, DWRITE_TEXT_RANGE, DWRITE_TRIMMING,
};

pub(super) unsafe fn extended_is_uniform(layout: &IDWriteTextLayout, length: u32) -> bool {
    if let Ok(layout) = layout.cast::<IDWriteTextLayout1>() {
        let mut range = DWRITE_TEXT_RANGE::default();
        let mut kerning = BOOL::default();
        if layout
            .GetPairKerning(0, &mut kerning, Some(&mut range))
            .is_err()
            || !super::range_covers_layout(range, length)
        {
            return false;
        }
        let (mut leading, mut trailing, mut minimum) = (0.0, 0.0, 0.0);
        range = DWRITE_TEXT_RANGE::default();
        if layout
            .GetCharacterSpacing(
                0,
                &mut leading,
                &mut trailing,
                &mut minimum,
                Some(&mut range),
            )
            .is_err()
            || !super::range_covers_layout(range, length)
        {
            return false;
        }
    }
    if let Ok(layout) = layout.cast::<IDWriteTextLayout4>() {
        let count = layout.GetFontAxisValueCount(0) as usize;
        if count > super::MAX_TEXT_UNITS {
            return false;
        }
        let mut axes = vec![DWRITE_FONT_AXIS_VALUE::default(); count];
        let mut range = DWRITE_TEXT_RANGE::default();
        if layout
            .GetFontAxisValues(0, &mut axes, Some(&mut range))
            .is_err()
            || !super::range_covers_layout(range, length)
        {
            return false;
        }
    }
    true
}

/// The caller has checked that all per-character attributes cover the source text.
pub(super) unsafe fn copy(
    source: &IDWriteTextLayout,
    target: &IDWriteTextLayout,
    length: u32,
) -> Result<()> {
    let range = DWRITE_TEXT_RANGE {
        startPosition: 0,
        length,
    };
    let mut family_length = 0;
    source.GetFontFamilyNameLength(0, &mut family_length, None)?;
    let mut locale_length = 0;
    source.GetLocaleNameLength(0, &mut locale_length, None)?;
    if family_length as usize > super::MAX_TEXT_UNITS
        || locale_length as usize > super::MAX_TEXT_UNITS
    {
        return Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_INVALIDARG,
        ));
    }
    let mut family = vec![0; family_length as usize + 1];
    let mut locale = vec![0; locale_length as usize + 1];
    source.GetFontFamilyName(0, &mut family, None)?;
    source.GetLocaleName(0, &mut locale, None)?;
    let mut collection = None;
    source.GetFontCollection(0, &mut collection, None)?;
    let mut weight = DWRITE_FONT_WEIGHT_NORMAL;
    let mut style = DWRITE_FONT_STYLE_NORMAL;
    let mut stretch = DWRITE_FONT_STRETCH_NORMAL;
    let mut size = 0.0;
    source.GetFontWeight(0, &mut weight, None)?;
    source.GetFontStyle(0, &mut style, None)?;
    source.GetFontStretch(0, &mut stretch, None)?;
    source.GetFontSize(0, &mut size, None)?;
    target.SetFontCollection(collection.as_ref(), range)?;
    target.SetFontFamilyName(PCWSTR(family.as_ptr()), range)?;
    target.SetLocaleName(PCWSTR(locale.as_ptr()), range)?;
    target.SetFontWeight(weight, range)?;
    target.SetFontStyle(style, range)?;
    target.SetFontStretch(stretch, range)?;
    target.SetFontSize(size, range)?;
    let (mut underline, mut strike) = (BOOL::default(), BOOL::default());
    source.GetUnderline(0, &mut underline, None)?;
    source.GetStrikethrough(0, &mut strike, None)?;
    target.SetUnderline(underline, range)?;
    target.SetStrikethrough(strike, range)?;
    let mut typography = None;
    source.GetTypography(0, &mut typography, None)?;
    target.SetTypography(typography.as_ref(), range)?;

    // Layout setters may have changed these after CreateTextLayout; the creation
    // format is not the authoritative formatting at the time of drawing.
    target.SetTextAlignment(source.GetTextAlignment())?;
    target.SetParagraphAlignment(source.GetParagraphAlignment())?;
    target.SetWordWrapping(source.GetWordWrapping())?;
    target.SetReadingDirection(source.GetReadingDirection())?;
    target.SetFlowDirection(source.GetFlowDirection())?;
    target.SetIncrementalTabStop(source.GetIncrementalTabStop())?;
    let mut trimming = DWRITE_TRIMMING::default();
    let mut sign = None;
    source.GetTrimming(&mut trimming, &mut sign)?;
    target.SetTrimming(&trimming, sign.as_ref())?;
    let (mut method, mut spacing, mut baseline) = (DWRITE_LINE_SPACING_METHOD_DEFAULT, 0.0, 0.0);
    source.GetLineSpacing(&mut method, &mut spacing, &mut baseline)?;
    target.SetLineSpacing(method, spacing, baseline)?;

    if let Ok(source) = source.cast::<IDWriteTextLayout1>() {
        let target = target.cast::<IDWriteTextLayout1>()?;
        let mut kerning = BOOL::default();
        source.GetPairKerning(0, &mut kerning, None)?;
        target.SetPairKerning(kerning, range)?;
        let (mut leading, mut trailing, mut minimum) = (0.0, 0.0, 0.0);
        source.GetCharacterSpacing(0, &mut leading, &mut trailing, &mut minimum, None)?;
        target.SetCharacterSpacing(leading, trailing, minimum, range)?;
    }
    if let Ok(source) = source.cast::<IDWriteTextLayout2>() {
        let target = target.cast::<IDWriteTextLayout2>()?;
        target.SetVerticalGlyphOrientation(source.GetVerticalGlyphOrientation())?;
        target.SetLastLineWrapping(source.GetLastLineWrapping())?;
        target.SetOpticalAlignment(source.GetOpticalAlignment())?;
        // A successful null fallback means system fallback. The generated getter
        // treats null as an error, so preserve HRESULT and optional ownership here.
        let mut fallback = std::ptr::null_mut();
        (Interface::vtable(&source).GetFontFallback)(source.as_raw(), &mut fallback).ok()?;
        let fallback = (!fallback.is_null()).then(|| IDWriteFontFallback::from_raw(fallback));
        target.SetFontFallback(fallback.as_ref())?;
    }
    if let Ok(source) = source.cast::<IDWriteTextLayout3>() {
        let target = target.cast::<IDWriteTextLayout3>()?;
        let mut spacing = DWRITE_LINE_SPACING::default();
        source.GetLineSpacing(&mut spacing)?;
        target.SetLineSpacing(&spacing)?;
    }
    if let Ok(source) = source.cast::<IDWriteTextLayout4>() {
        let target = target.cast::<IDWriteTextLayout4>()?;
        let count = source.GetFontAxisValueCount(0) as usize;
        if count > super::MAX_TEXT_UNITS {
            return Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_INVALIDARG,
            ));
        }
        let mut axes = vec![DWRITE_FONT_AXIS_VALUE::default(); count];
        source.GetFontAxisValues(0, &mut axes, None)?;
        target.SetFontAxisValues(&axes, range)?;
        target.SetAutomaticFontAxes(source.GetAutomaticFontAxes())?;
    }
    Ok(())
}
