use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AuthorizedTarget, DesktopRect, Granularity, InteractiveSelection, Provenance,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiaTextSelection {
    Word,
    TextRange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiaAcquisitionSnapshot {
    target: AuthorizedTarget,
    password: bool,
    text: Option<Box<str>>,
    text_anchors: Vec<DesktopRect>,
    text_selection: Option<UiaTextSelection>,
    name: Option<Box<str>>,
    control_anchor: Option<DesktopRect>,
}

impl UiaAcquisitionSnapshot {
    #[must_use]
    pub fn new(target: AuthorizedTarget) -> Self {
        Self {
            target,
            password: false,
            text: None,
            text_anchors: Vec::new(),
            text_selection: None,
            name: None,
            control_anchor: None,
        }
    }

    #[must_use]
    pub const fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    #[must_use]
    pub fn with_text(
        mut self,
        text: impl Into<Box<str>>,
        anchors: impl IntoIterator<Item = DesktopRect>,
        selection: UiaTextSelection,
    ) -> Self {
        self.text = Some(text.into());
        self.text_anchors = anchors.into_iter().collect();
        self.text_selection = Some(selection);
        self
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<Box<str>>, anchor: DesktopRect) -> Self {
        self.name = Some(name.into());
        self.control_anchor = Some(anchor);
        self
    }
}

/// Supplies one bounded UIA snapshot for the requested point or text range.
///
/// A Windows implementation owns `ElementFromPoint`, `RangeFromPoint`, RuntimeId, COM, and handle
/// validation. Fixtures implement the same seam without exposing those details to acquisition
/// callers.
pub trait UiaSelectionSource {
    fn snapshot(
        &mut self,
        target: &AuthorizedTarget,
        selection: InteractiveSelection,
    ) -> Result<UiaAcquisitionSnapshot, AcquisitionError>;
}

pub struct UiaAcquisitionAdapter<S> {
    source: S,
}

impl<S> UiaAcquisitionAdapter<S> {
    #[must_use]
    pub const fn new(source: S) -> Self {
        Self { source }
    }
}

impl<S> AcquisitionAdapter for UiaAcquisitionAdapter<S>
where
    S: UiaSelectionSource,
{
    fn provenance(&self) -> Provenance {
        Provenance::Structured
    }

    fn acquire(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        if matches!(request.selection(), InteractiveSelection::Region(_)) {
            return Err(AcquisitionError::ProviderUnavailable);
        }
        let snapshot = self
            .source
            .snapshot(request.target(), request.selection())?;
        if &snapshot.target != request.target() {
            return Err(AcquisitionError::TargetMismatch);
        }
        if snapshot.password {
            return Err(AcquisitionError::PermissionDenied);
        }
        if let (Some(text), Some(selection)) = (snapshot.text, snapshot.text_selection) {
            let selection_matches = matches!(
                (request.selection(), selection),
                (InteractiveSelection::Point(_), UiaTextSelection::Word)
                    | (
                        InteractiveSelection::TextRange { .. },
                        UiaTextSelection::TextRange
                    )
            );
            if selection_matches
                && anchors_match_selection(request.selection(), &snapshot.text_anchors)
            {
                let granularity = match selection {
                    UiaTextSelection::Word => Granularity::Word,
                    UiaTextSelection::TextRange => Granularity::Line,
                };
                return Ok(vec![AcquisitionCandidate::new(
                    text,
                    snapshot.text_anchors,
                    granularity,
                )]);
            }
        }
        if matches!(request.selection(), InteractiveSelection::Point(_)) {
            if let (Some(name), Some(anchor)) = (snapshot.name, snapshot.control_anchor) {
                if anchors_match_selection(request.selection(), &[anchor]) {
                    return Ok(vec![AcquisitionCandidate::new(
                        name,
                        [anchor],
                        Granularity::Control,
                    )]);
                }
            }
        }
        Err(AcquisitionError::NoText)
    }
}

fn anchors_match_selection(selection: InteractiveSelection, anchors: &[DesktopRect]) -> bool {
    match selection {
        InteractiveSelection::Point(point) => anchors.iter().any(|anchor| anchor.contains(point)),
        // UIA bounding rectangles may exclude selected trailing whitespace, so drag endpoints are
        // not required to lie inside the returned glyph bounds.
        InteractiveSelection::TextRange { .. } => !anchors.is_empty(),
        InteractiveSelection::Region(_) => false,
    }
}
