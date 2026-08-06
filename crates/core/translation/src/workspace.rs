use crate::TranslationSnapshot;
use glyphshift_domain::Generation;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRevision(u64);

impl SourceRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceLocation {
    id: Box<str>,
    label: Box<str>,
    context_kind: Option<Box<str>>,
}

impl WorkspaceLocation {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, label: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context_kind: None,
        }
    }

    #[must_use]
    pub fn with_context(
        id: impl Into<Box<str>>,
        label: impl Into<Box<str>>,
        context_kind: impl Into<Box<str>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context_kind: Some(context_kind.into()),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn context_kind(&self) -> Option<&str> {
        self.context_kind.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceContext {
    kind: Box<str>,
    key: Box<str>,
    label: Box<str>,
}

impl WorkspaceContext {
    #[must_use]
    pub fn new(
        kind: impl Into<Box<str>>,
        key: impl Into<Box<str>>,
        label: impl Into<Box<str>>,
    ) -> Self {
        Self {
            kind: kind.into(),
            key: key.into(),
            label: label.into(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceEntry {
    location: Box<str>,
    context: Option<WorkspaceContext>,
    source: Box<str>,
    translation: Box<str>,
    adapter_ids: BTreeSet<Box<str>>,
}

impl WorkspaceEntry {
    #[must_use]
    pub fn new(
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            translation: translation.into(),
            adapter_ids: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: WorkspaceContext) -> Self {
        self.context = Some(context);
        self
    }

    #[must_use]
    pub fn for_adapters(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.adapter_ids = adapter_ids.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    #[must_use]
    pub const fn context(&self) -> Option<&WorkspaceContext> {
        self.context.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }

    pub const fn adapter_ids(&self) -> &BTreeSet<Box<str>> {
        &self.adapter_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceChange {
    Upsert(WorkspaceEntry),
    Delete(WorkspaceEntryKey),
}

impl WorkspaceChange {
    #[must_use]
    pub const fn upsert(entry: WorkspaceEntry) -> Self {
        Self::Upsert(entry)
    }

    #[must_use]
    pub const fn delete(entry: WorkspaceEntryKey) -> Self {
        Self::Delete(entry)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceEntryKey {
    location: Box<str>,
    context: Option<WorkspaceContext>,
    source: Box<str>,
}

impl WorkspaceEntryKey {
    #[must_use]
    pub fn new(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: WorkspaceContext) -> Self {
        self.context = Some(context);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyResult {
    Applied {
        revision: SourceRevision,
        generation: Generation,
    },
    NoChange {
        revision: SourceRevision,
        generation: Generation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    Conflict { current: SourceRevision },
    UnknownLocation(Box<str>),
    Rejected(WorkspaceRejection),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(Box<str>);

impl SourceId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceLayer {
    Builtin,
    Package,
    User,
}

impl SourceLayer {
    const fn priority(self) -> u8 {
        match self {
            Self::Builtin => 0,
            Self::Package => 1,
            Self::User => 2,
        }
    }

    fn origin(self, source_id: SourceId) -> WorkspaceOrigin {
        match self {
            Self::Builtin => WorkspaceOrigin::Builtin(source_id),
            Self::Package => WorkspaceOrigin::Package(source_id),
            Self::User => WorkspaceOrigin::User,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceRejection {
    Malformed(Box<str>),
    UnknownLocation(Box<str>),
    InvalidContext { location: Box<str> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceOrigin {
    Builtin(SourceId),
    Package(SourceId),
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceViewEntry {
    location_id: Box<str>,
    location_label: Box<str>,
    context: Option<WorkspaceContext>,
    source: Box<str>,
    translation: Box<str>,
    adapter_ids: BTreeSet<Box<str>>,
    origin: WorkspaceOrigin,
}

impl WorkspaceViewEntry {
    #[must_use]
    pub fn location_id(&self) -> &str {
        &self.location_id
    }

    #[must_use]
    pub fn location_label(&self) -> &str {
        &self.location_label
    }

    #[must_use]
    pub const fn context(&self) -> Option<&WorkspaceContext> {
        self.context.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }

    pub const fn adapter_ids(&self) -> &BTreeSet<Box<str>> {
        &self.adapter_ids
    }

    #[must_use]
    pub const fn origin(&self) -> &WorkspaceOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceConflict {
    location: Box<str>,
    context: Option<WorkspaceContext>,
    source: Box<str>,
    translations: Vec<Box<str>>,
}

impl WorkspaceConflict {
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }

    #[must_use]
    pub const fn context(&self) -> Option<&WorkspaceContext> {
        self.context.as_ref()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translations(&self) -> &[Box<str>] {
        &self.translations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceView {
    extension_id: Box<str>,
    locale: Box<str>,
    entries: Vec<WorkspaceViewEntry>,
    conflicts: Vec<WorkspaceConflict>,
}

impl WorkspaceView {
    #[must_use]
    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn entries(&self) -> &[WorkspaceViewEntry] {
        &self.entries
    }

    #[must_use]
    pub fn conflicts(&self) -> &[WorkspaceConflict] {
        &self.conflicts
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(u64);

impl EventId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceEventKind {
    Published {
        revision: SourceRevision,
        generation: Generation,
    },
    Rejected(WorkspaceRejection),
    Conflict {
        current: SourceRevision,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceEvent {
    id: EventId,
    kind: WorkspaceEventKind,
}

impl WorkspaceEvent {
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    #[must_use]
    pub const fn kind(&self) -> &WorkspaceEventKind {
        &self.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceDocument {
    source_id: SourceId,
    layer: SourceLayer,
    entries: Vec<WorkspaceEntry>,
    rejection: Option<WorkspaceRejection>,
}

impl SourceDocument {
    #[must_use]
    pub fn new(
        source_id: SourceId,
        layer: SourceLayer,
        entries: impl IntoIterator<Item = WorkspaceEntry>,
    ) -> Self {
        Self {
            source_id,
            layer,
            entries: entries.into_iter().collect(),
            rejection: None,
        }
    }

    #[must_use]
    pub fn rejected(
        source_id: SourceId,
        layer: SourceLayer,
        rejection: WorkspaceRejection,
    ) -> Self {
        Self {
            source_id,
            layer,
            entries: Vec::new(),
            rejection: Some(rejection),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WorkspaceKey {
    location: Box<str>,
    context_kind: Option<Box<str>>,
    context_key: Option<Box<str>>,
    source: Box<str>,
}

impl WorkspaceKey {
    fn from_entry(entry: &WorkspaceEntry) -> Self {
        Self {
            location: entry.location.clone(),
            context_kind: entry.context.as_ref().map(|context| context.kind.clone()),
            context_key: entry.context.as_ref().map(|context| context.key.clone()),
            source: entry.source.clone(),
        }
    }

    fn from_entry_key(entry: &WorkspaceEntryKey) -> Self {
        Self {
            location: entry.location.clone(),
            context_kind: entry.context.as_ref().map(|context| context.kind.clone()),
            context_key: entry.context.as_ref().map(|context| context.key.clone()),
            source: entry.source.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct WorkspaceCandidate {
    source_id: Option<SourceId>,
    layer: SourceLayer,
    entry: WorkspaceEntry,
}

impl WorkspaceCandidate {
    fn origin(&self) -> WorkspaceOrigin {
        self.source_id
            .clone()
            .map_or(WorkspaceOrigin::User, |source_id| {
                self.layer.origin(source_id)
            })
    }
}

#[derive(Debug)]
pub struct TranslationWorkspace {
    extension_id: Box<str>,
    locale: Box<str>,
    locations: BTreeMap<Box<str>, WorkspaceLocation>,
    revision: u64,
    generation: u64,
    user_entries: BTreeMap<WorkspaceKey, WorkspaceEntry>,
    sources: BTreeMap<SourceId, SourceDocument>,
    snapshot: TranslationSnapshot,
    view_entries: Vec<WorkspaceViewEntry>,
    view_conflicts: Vec<WorkspaceConflict>,
    next_event_id: u64,
    events: Vec<WorkspaceEvent>,
}

impl TranslationWorkspace {
    #[must_use]
    pub fn new(
        extension_id: impl Into<Box<str>>,
        locale: impl Into<Box<str>>,
        locations: impl IntoIterator<Item = WorkspaceLocation>,
    ) -> Self {
        Self {
            extension_id: extension_id.into(),
            locale: locale.into(),
            locations: locations
                .into_iter()
                .map(|location| (location.id.clone(), location))
                .collect(),
            revision: 0,
            generation: 0,
            user_entries: BTreeMap::new(),
            sources: BTreeMap::new(),
            snapshot: TranslationSnapshot::empty(Generation::new(0)),
            view_entries: Vec::new(),
            view_conflicts: Vec::new(),
            next_event_id: 0,
            events: Vec::new(),
        }
    }

    pub fn apply(
        &mut self,
        change: WorkspaceChange,
        base_revision: SourceRevision,
    ) -> Result<ApplyResult, WorkspaceError> {
        if base_revision != SourceRevision::new(self.revision) {
            self.push_event(WorkspaceEventKind::Conflict {
                current: SourceRevision::new(self.revision),
            });
            return Err(WorkspaceError::Conflict {
                current: SourceRevision::new(self.revision),
            });
        }
        match change {
            WorkspaceChange::Upsert(entry) => {
                if let Err(rejection) = self.validate_entry(&entry) {
                    self.push_event(WorkspaceEventKind::Rejected(rejection.clone()));
                    return match rejection {
                        WorkspaceRejection::UnknownLocation(location) => {
                            Err(WorkspaceError::UnknownLocation(location))
                        }
                        rejection => Err(WorkspaceError::Rejected(rejection)),
                    };
                }
                let key = WorkspaceKey::from_entry(&entry);
                if self.user_entries.get(&key) == Some(&entry) {
                    return Ok(ApplyResult::NoChange {
                        revision: SourceRevision::new(self.revision),
                        generation: Generation::new(self.generation),
                    });
                }
                self.user_entries.insert(key, entry);
            }
            WorkspaceChange::Delete(entry) => {
                let validation = WorkspaceEntry {
                    location: entry.location.clone(),
                    context: entry.context.clone(),
                    source: entry.source.clone(),
                    translation: "delete-validation".into(),
                    adapter_ids: BTreeSet::new(),
                };
                if let Err(rejection) = self.validate_entry(&validation) {
                    self.push_event(WorkspaceEventKind::Rejected(rejection.clone()));
                    return match rejection {
                        WorkspaceRejection::UnknownLocation(location) => {
                            Err(WorkspaceError::UnknownLocation(location))
                        }
                        rejection => Err(WorkspaceError::Rejected(rejection)),
                    };
                }
                if self
                    .user_entries
                    .remove(&WorkspaceKey::from_entry_key(&entry))
                    .is_none()
                {
                    return Ok(ApplyResult::NoChange {
                        revision: SourceRevision::new(self.revision),
                        generation: Generation::new(self.generation),
                    });
                }
            }
        }
        self.revision += 1;
        self.generation += 1;
        self.rebuild_snapshot();
        self.push_published_event();
        Ok(ApplyResult::Applied {
            revision: SourceRevision::new(self.revision),
            generation: Generation::new(self.generation),
        })
    }

    pub fn rescan(&mut self, source: SourceDocument) -> Result<ApplyResult, WorkspaceError> {
        if let Some(rejection) = source.rejection.clone() {
            self.push_event(WorkspaceEventKind::Rejected(rejection.clone()));
            return Err(WorkspaceError::Rejected(rejection));
        }
        if let Some(rejection) = source
            .entries
            .iter()
            .find_map(|entry| self.validate_entry(entry).err())
        {
            self.push_event(WorkspaceEventKind::Rejected(rejection.clone()));
            return Err(WorkspaceError::Rejected(rejection));
        }
        if self.sources.get(&source.source_id) == Some(&source) {
            return Ok(ApplyResult::NoChange {
                revision: SourceRevision::new(self.revision),
                generation: Generation::new(self.generation),
            });
        }
        self.sources.insert(source.source_id.clone(), source);
        self.revision += 1;
        self.generation += 1;
        self.rebuild_snapshot();
        self.push_published_event();
        Ok(ApplyResult::Applied {
            revision: SourceRevision::new(self.revision),
            generation: Generation::new(self.generation),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> SourceRevision {
        SourceRevision::new(self.revision)
    }

    #[must_use]
    pub const fn snapshot(&self) -> &TranslationSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn subscribe(&self, after: EventId) -> Vec<WorkspaceEvent> {
        self.events
            .iter()
            .filter(|event| event.id > after)
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn view(&self) -> WorkspaceView {
        WorkspaceView {
            extension_id: self.extension_id.clone(),
            locale: self.locale.clone(),
            entries: self.view_entries.clone(),
            conflicts: self.view_conflicts.clone(),
        }
    }

    #[must_use]
    pub fn serialize_catalog(&self) -> String {
        let mut output = String::from("{\"schema\":\"glyphshift.translation/1\",\"extension\":\"");
        push_json_string_content(&mut output, &self.extension_id);
        output.push_str("\",\"locale\":\"");
        push_json_string_content(&mut output, &self.locale);
        output.push_str("\",\"entries\":[");
        for (index, entry) in self.view_entries.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str("{\"location\":\"");
            push_json_string_content(&mut output, &entry.location_id);
            output.push('"');
            if let Some(context) = &entry.context {
                output.push_str(",\"context\":{\"kind\":\"");
                push_json_string_content(&mut output, &context.kind);
                output.push_str("\",\"key\":\"");
                push_json_string_content(&mut output, &context.key);
                output.push_str("\",\"label\":\"");
                push_json_string_content(&mut output, &context.label);
                output.push_str("\"}");
            }
            if !entry.adapter_ids.is_empty() {
                output.push_str(",\"hooks\":[");
                for (adapter_index, adapter_id) in entry.adapter_ids.iter().enumerate() {
                    if adapter_index > 0 {
                        output.push(',');
                    }
                    output.push('"');
                    push_json_string_content(&mut output, adapter_id);
                    output.push('"');
                }
                output.push(']');
            }
            output.push_str(",\"source\":\"");
            push_json_string_content(&mut output, &entry.source);
            output.push_str("\",\"translation\":\"");
            push_json_string_content(&mut output, &entry.translation);
            output.push_str("\"}");
        }
        output.push_str("]}");
        output
    }

    fn rebuild_snapshot(&mut self) {
        let mut snapshot = TranslationSnapshot::empty(Generation::new(self.generation));
        let mut candidates: BTreeMap<WorkspaceKey, Vec<WorkspaceCandidate>> = BTreeMap::new();
        for source in self.sources.values() {
            for entry in &source.entries {
                candidates
                    .entry(WorkspaceKey::from_entry(entry))
                    .or_default()
                    .push(WorkspaceCandidate {
                        source_id: Some(source.source_id.clone()),
                        layer: source.layer,
                        entry: entry.clone(),
                    });
            }
        }
        for (key, entry) in &self.user_entries {
            candidates
                .entry(key.clone())
                .or_default()
                .push(WorkspaceCandidate {
                    source_id: None,
                    layer: SourceLayer::User,
                    entry: entry.clone(),
                });
        }
        let mut view_entries = Vec::with_capacity(candidates.len());
        let mut view_conflicts = Vec::new();
        for values in candidates.values_mut() {
            values.sort_by(|left, right| {
                right
                    .layer
                    .priority()
                    .cmp(&left.layer.priority())
                    .then_with(|| left.source_id.cmp(&right.source_id))
            });
            let winner = values
                .first()
                .expect("candidate groups are created with at least one entry");
            if let Some(context) = &winner.entry.context {
                snapshot = snapshot.with_context_entry_for_adapters(
                    winner.entry.location.clone(),
                    context.kind.clone(),
                    context.key.clone(),
                    winner.entry.source.clone(),
                    winner.entry.translation.clone(),
                    winner.entry.adapter_ids.iter().cloned(),
                );
            } else {
                snapshot = snapshot.with_entry_for_adapters(
                    winner.entry.location.clone(),
                    winner.entry.source.clone(),
                    winner.entry.translation.clone(),
                    winner.entry.adapter_ids.iter().cloned(),
                );
            }
            let location = self
                .locations
                .get(&winner.entry.location)
                .expect("validated entries reference a registered location");
            view_entries.push(WorkspaceViewEntry {
                location_id: winner.entry.location.clone(),
                location_label: location.label.clone(),
                context: winner.entry.context.clone(),
                source: winner.entry.source.clone(),
                translation: winner.entry.translation.clone(),
                adapter_ids: winner.entry.adapter_ids.clone(),
                origin: winner.origin(),
            });

            let winning_priority = winner.layer.priority();
            let mut translations: Vec<Box<str>> = values
                .iter()
                .take_while(|candidate| candidate.layer.priority() == winning_priority)
                .map(|candidate| candidate.entry.translation.clone())
                .collect();
            translations.sort();
            translations.dedup();
            if translations.len() > 1 {
                view_conflicts.push(WorkspaceConflict {
                    location: winner.entry.location.clone(),
                    context: winner.entry.context.clone(),
                    source: winner.entry.source.clone(),
                    translations,
                });
            }
        }
        self.snapshot = snapshot;
        self.view_entries = view_entries;
        self.view_conflicts = view_conflicts;
    }

    fn validate_entry(&self, entry: &WorkspaceEntry) -> Result<(), WorkspaceRejection> {
        let Some(location) = self.locations.get(&entry.location) else {
            return Err(WorkspaceRejection::UnknownLocation(entry.location.clone()));
        };
        match (&location.context_kind, &entry.context) {
            (None, None) => Ok(()),
            (Some(expected), Some(context))
                if expected == &context.kind
                    && !context.key.is_empty()
                    && !context.label.is_empty() =>
            {
                Ok(())
            }
            _ => Err(WorkspaceRejection::InvalidContext {
                location: entry.location.clone(),
            }),
        }
    }

    fn push_published_event(&mut self) {
        self.push_event(WorkspaceEventKind::Published {
            revision: SourceRevision::new(self.revision),
            generation: Generation::new(self.generation),
        });
    }

    fn push_event(&mut self, kind: WorkspaceEventKind) {
        self.next_event_id += 1;
        self.events.push(WorkspaceEvent {
            id: EventId::new(self.next_event_id),
            kind,
        });
    }
}

fn push_json_string_content(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to a string cannot fail");
            }
            character => output.push(character),
        }
    }
}
