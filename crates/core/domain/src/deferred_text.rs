//! A bounded, renderer-owned transaction: glyph calls have not been drawn yet.
//! Unlike observation reconstruction, an eligible transaction can be replaced
//! before committing any drawing. Unknown operations force original replay.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphStyle {
    pub font: u64,
    pub color: u32,
    pub draw_kind: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeferredGlyph {
    pub unit: u16,
    pub x: i32,
    pub y: i32,
    pub advance: i32,
    pub style: GlyphStyle,
}

const MAX_GLYPHS: usize = 4096;

#[derive(Default)]
pub struct DeferredTextDraw {
    glyphs: Vec<DeferredGlyph>,
}

impl DeferredTextDraw {
    /// Rejected calls still belong to the caller and must be drawn normally,
    /// after replaying the buffered prefix in its original order.
    pub fn push(&mut self, glyph: DeferredGlyph) -> Result<(), DeferredGlyph> {
        if self.glyphs.len() == MAX_GLYPHS || glyph.unit == 0 || glyph.advance <= 0 {
            return Err(glyph);
        }
        if let Some(previous) = self.glyphs.last() {
            if glyph.style != previous.style || glyph.y != previous.y
                || previous.x.checked_add(previous.advance) != Some(glyph.x)
            {
                return Err(glyph);
            }
        }
        self.glyphs.push(glyph);
        Ok(())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool { self.glyphs.is_empty() }

    /// Consumes a text-only transaction at its renderer-provided end boundary.
    pub fn finish(self) -> DeferredTextCommit {
        let units = self.glyphs.iter().map(|glyph| glyph.unit).collect::<Vec<_>>();
        let source = if units.is_empty() { None } else { String::from_utf16(&units).ok() };
        DeferredTextCommit { original: self.glyphs, source }
    }
}

pub struct DeferredTextCommit {
    original: Vec<DeferredGlyph>,
    source: Option<String>,
}

impl DeferredTextCommit {
    #[must_use]
    pub fn source(&self) -> Option<&str> { self.source.as_deref() }
    #[must_use]
    pub fn original(&self) -> &[DeferredGlyph] { &self.original }
    #[must_use]
    pub fn origin(&self) -> Option<(i32,i32,GlyphStyle)> {
        self.original.first().map(|glyph|(glyph.x,glyph.y,glyph.style))
    }
    /// Fit only inside the proven original extent; never guess missing clipping
    /// bounds or stretch character positions to match a translated string.
    #[must_use]
    pub fn fits_width(&self, width: i32) -> bool {
        let Some(first)=self.original.first() else { return false; };
        let Some(last)=self.original.last() else { return false; };
        width > 0 && last.x.checked_add(last.advance).and_then(|end|end.checked_sub(first.x))
            .is_some_and(|available|width<=available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn glyph(unit:u16,x:i32)->DeferredGlyph {
        DeferredGlyph{unit,x,y:20,advance:12,style:GlyphStyle{font:1,color:0xffffffff,draw_kind:0}}
    }
    #[test]
    fn explicit_adjacent_calls_form_one_replaceable_transaction() {
        let mut draw=DeferredTextDraw::default();
        for (index,unit) in "开始游戏".encode_utf16().enumerate() {draw.push(glyph(unit,index as i32*12)).unwrap();}
        let commit=draw.finish();assert_eq!(commit.source(),Some("开始游戏"));
        assert!(commit.fits_width(48));assert!(!commit.fits_width(49));
        assert_eq!(commit.original().len(),4);
    }
    #[test]
    fn uncertainty_returns_the_unconsumed_call_for_exact_replay() {
        for next in [glyph(66,24),DeferredGlyph{y:40,..glyph(66,12)},
            DeferredGlyph{style:GlyphStyle{font:2,color:0xffffffff,draw_kind:0},..glyph(66,12)}] {
            let mut draw=DeferredTextDraw::default();draw.push(glyph(65,0)).unwrap();
            assert_eq!(draw.push(next),Err(next));
            assert_eq!(draw.finish().original(),[glyph(65,0)]);
        }
    }
    #[test]
    fn single_labels_spaces_repeats_and_overflow_keep_their_meaning() {
        let mut draw=DeferredTextDraw::default();draw.push(glyph(0x662f,0)).unwrap();
        assert_eq!(draw.finish().source(),Some("是"));
        let mut draw=DeferredTextDraw::default();
        for (i,unit) in "A A".encode_utf16().enumerate(){draw.push(glyph(unit,i as i32*12)).unwrap();}
        assert_eq!(draw.finish().source(),Some("A A"));
        let mut draw=DeferredTextDraw::default();draw.push(glyph(65,i32::MAX)).unwrap();
        assert!(!draw.finish().fits_width(1));
    }
    #[test]
    fn capacity_failure_does_not_drop_or_translate_a_prefix() {
        let mut draw=DeferredTextDraw::default();
        for i in 0..MAX_GLYPHS {draw.push(glyph(65,i as i32*12)).unwrap();}
        let next=glyph(66,MAX_GLYPHS as i32*12);assert_eq!(draw.push(next),Err(next));
        assert_eq!(draw.finish().original().len(),MAX_GLYPHS);
    }
}
