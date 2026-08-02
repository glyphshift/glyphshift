//! Immutable translation inputs consumed by the Decision Engine.

use glyphshift_domain::Generation;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

type ContextualTranslationKey = (Box<str>, Box<str>, Box<str>, Box<str>);
type ContextualTranslations = BTreeMap<ContextualTranslationKey, Arc<str>>;
type TranslationScopeKey = (Box<str>, Box<str>);
type TranslationScopes = BTreeMap<TranslationScopeKey, BTreeSet<Box<str>>>;
type ContextualTranslationScopes = BTreeMap<ContextualTranslationKey, BTreeSet<Box<str>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationSnapshot {
    generation: Generation,
    entries: BTreeMap<Box<str>, BTreeMap<Box<str>, Arc<str>>>,
    contextual_entries: ContextualTranslations,
    adapter_scopes: TranslationScopes,
    contextual_adapter_scopes: ContextualTranslationScopes,
    digest: SnapshotDigest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontPolicyDigest([u8; 32]);

impl SnapshotDigest {
    fn from_entries(
        entries: &BTreeMap<Box<str>, BTreeMap<Box<str>, Arc<str>>>,
        contextual_entries: &ContextualTranslations,
        adapter_scopes: &TranslationScopes,
        contextual_adapter_scopes: &ContextualTranslationScopes,
    ) -> Self {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x8422_2325_cbf2_9ce4_u64,
            0x9e37_79b9_7f4a_7c15_u64,
            0x517c_c1b7_2722_0a95_u64,
        ];
        for (location, translations) in entries {
            for (source, translation) in translations {
                Self::digest_fields(
                    &mut lanes,
                    [
                        location.as_bytes(),
                        source.as_bytes(),
                        translation.as_bytes(),
                    ],
                );
            }
        }
        for ((location, kind, key, source), translation) in contextual_entries {
            Self::digest_fields(
                &mut lanes,
                [
                    location.as_bytes(),
                    kind.as_bytes(),
                    key.as_bytes(),
                    source.as_bytes(),
                    translation.as_bytes(),
                ],
            );
        }
        for ((location, source), adapters) in adapter_scopes {
            for adapter in adapters {
                Self::digest_fields(
                    &mut lanes,
                    [location.as_bytes(), source.as_bytes(), adapter.as_bytes()],
                );
            }
        }
        for ((location, kind, key, source), adapters) in contextual_adapter_scopes {
            for adapter in adapters {
                Self::digest_fields(
                    &mut lanes,
                    [
                        location.as_bytes(),
                        kind.as_bytes(),
                        key.as_bytes(),
                        source.as_bytes(),
                        adapter.as_bytes(),
                    ],
                );
            }
        }
        let mut digest = [0_u8; 32];
        for (index, lane) in lanes.into_iter().enumerate() {
            digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_le_bytes());
        }
        Self(digest)
    }

    fn digest_fields<'a>(lanes: &mut [u64; 4], fields: impl IntoIterator<Item = &'a [u8]>) {
        for bytes in fields {
            for byte in bytes {
                for (index, lane) in lanes.iter_mut().enumerate() {
                    *lane ^= u64::from(*byte) + index as u64;
                    *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            for lane in lanes.iter_mut() {
                *lane ^= 0xff;
                *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
}

impl TranslationSnapshot {
    #[must_use]
    pub fn empty(generation: Generation) -> Self {
        let entries = BTreeMap::new();
        let contextual_entries = BTreeMap::new();
        let adapter_scopes = BTreeMap::new();
        let contextual_adapter_scopes = BTreeMap::new();
        Self {
            generation,
            digest: SnapshotDigest::from_entries(
                &entries,
                &contextual_entries,
                &adapter_scopes,
                &contextual_adapter_scopes,
            ),
            entries,
            contextual_entries,
            adapter_scopes,
            contextual_adapter_scopes,
        }
    }

    #[must_use]
    pub fn with_entry(
        mut self,
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        let location = location.into();
        let source = source.into();
        self.entries
            .entry(location.clone())
            .or_default()
            .insert(source.clone(), Arc::from(translation.into()));
        self.adapter_scopes.remove(&(location, source));
        self.refresh_digest();
        self
    }

    #[must_use]
    pub fn with_entry_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let location = location.into();
        let source = source.into();
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.entries
            .entry(location.clone())
            .or_default()
            .insert(source.clone(), Arc::from(translation.into()));
        if adapters.is_empty() {
            self.adapter_scopes.remove(&(location, source));
        } else {
            self.adapter_scopes.insert((location, source), adapters);
        }
        self.refresh_digest();
        self
    }

    #[must_use]
    pub fn with_context_entry(
        mut self,
        location: impl Into<Box<str>>,
        kind: impl Into<Box<str>>,
        key: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        let entry_key = (location.into(), kind.into(), key.into(), source.into());
        self.contextual_entries
            .insert(entry_key.clone(), Arc::from(translation.into()));
        self.contextual_adapter_scopes.remove(&entry_key);
        self.refresh_digest();
        self
    }

    #[must_use]
    pub fn with_context_entry_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        kind: impl Into<Box<str>>,
        key: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let entry_key = (location.into(), kind.into(), key.into(), source.into());
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.contextual_entries
            .insert(entry_key.clone(), Arc::from(translation.into()));
        if adapters.is_empty() {
            self.contextual_adapter_scopes.remove(&entry_key);
        } else {
            self.contextual_adapter_scopes.insert(entry_key, adapters);
        }
        self.refresh_digest();
        self
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub fn lookup(&self, location: &str, source: &str) -> Option<Arc<str>> {
        self.entries
            .get(location)
            .and_then(|entries| entries.get(source))
            .cloned()
    }

    #[must_use]
    pub fn lookup_for_adapter(
        &self,
        location: &str,
        adapter_id: &str,
        source: &str,
    ) -> Option<Arc<str>> {
        let allowed = self
            .adapter_scopes
            .get(&(location.into(), source.into()))
            .is_none_or(|adapters| adapters.contains(adapter_id));
        allowed.then(|| self.lookup(location, source)).flatten()
    }

    #[must_use]
    pub fn lookup_context(
        &self,
        location: &str,
        kind: &str,
        key: &str,
        source: &str,
    ) -> Option<Arc<str>> {
        self.contextual_entries
            .get(&(location.into(), kind.into(), key.into(), source.into()))
            .cloned()
    }

    #[must_use]
    pub fn lookup_context_for_adapter(
        &self,
        location: &str,
        context_kind: &str,
        context_key: &str,
        adapter_id: &str,
        source: &str,
    ) -> Option<Arc<str>> {
        let key = (
            location.into(),
            context_kind.into(),
            context_key.into(),
            source.into(),
        );
        let allowed = self
            .contextual_adapter_scopes
            .get(&key)
            .is_none_or(|adapters| adapters.contains(adapter_id));
        allowed
            .then(|| self.lookup_context(location, context_kind, context_key, source))
            .flatten()
    }

    #[must_use]
    pub const fn digest(&self) -> SnapshotDigest {
        self.digest
    }

    pub fn visit_entries(&self, mut visitor: impl FnMut(&str, &str, &str)) {
        for (location, translations) in &self.entries {
            for (source, translation) in translations {
                visitor(location, source, translation);
            }
        }
    }

    pub fn visit_entries_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &str, &str, &BTreeSet<Box<str>>),
    ) {
        let empty = BTreeSet::new();
        for (location, translations) in &self.entries {
            for (source, translation) in translations {
                let adapters = self
                    .adapter_scopes
                    .get(&(location.clone(), source.clone()))
                    .unwrap_or(&empty);
                visitor(location, source, translation, adapters);
            }
        }
    }

    pub fn visit_context_entries(&self, mut visitor: impl FnMut(&str, &str, &str, &str, &str)) {
        for ((location, kind, key, source), translation) in &self.contextual_entries {
            visitor(location, kind, key, source, translation);
        }
    }

    pub fn visit_context_entries_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &str, &str, &str, &str, &BTreeSet<Box<str>>),
    ) {
        let empty = BTreeSet::new();
        for ((location, kind, key, source), translation) in &self.contextual_entries {
            let adapters = self
                .contextual_adapter_scopes
                .get(&(location.clone(), kind.clone(), key.clone(), source.clone()))
                .unwrap_or(&empty);
            visitor(location, kind, key, source, translation, adapters);
        }
    }

    fn refresh_digest(&mut self) {
        self.digest = SnapshotDigest::from_entries(
            &self.entries,
            &self.contextual_entries,
            &self.adapter_scopes,
            &self.contextual_adapter_scopes,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontRule {
    Unchanged,
    Substitute(Arc<str>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontPolicy {
    by_location: BTreeMap<Box<str>, Arc<str>>,
    by_location_adapter: BTreeMap<(Box<str>, Box<str>), Arc<str>>,
    entries: BTreeMap<(Box<str>, Box<str>), FontRule>,
    entry_adapter_scopes: TranslationScopes,
    contextual_entries: BTreeMap<ContextualTranslationKey, FontRule>,
    contextual_entry_adapter_scopes: ContextualTranslationScopes,
}

impl FontPolicy {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_location(
        mut self,
        location: impl Into<Box<str>>,
        family: impl Into<Box<str>>,
    ) -> Self {
        self.by_location
            .insert(location.into(), Arc::from(family.into()));
        self
    }

    #[must_use]
    pub fn with_location_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        family: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let location = location.into();
        let family = Arc::from(family.into());
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        if adapters.is_empty() {
            self.by_location.insert(location, family);
        } else {
            for adapter_id in adapters {
                self.by_location_adapter
                    .insert((location.clone(), adapter_id), Arc::clone(&family));
            }
        }
        self
    }

    #[must_use]
    pub fn with_entry(
        mut self,
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        rule: FontRule,
    ) -> Self {
        let key = (location.into(), source.into());
        self.entries.insert(key.clone(), rule);
        self.entry_adapter_scopes.remove(&key);
        self
    }

    #[must_use]
    pub fn with_entry_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        rule: FontRule,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let key = (location.into(), source.into());
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.entries.insert(key.clone(), rule);
        if adapters.is_empty() {
            self.entry_adapter_scopes.remove(&key);
        } else {
            self.entry_adapter_scopes.insert(key, adapters);
        }
        self
    }

    #[must_use]
    pub fn with_context_entry(
        mut self,
        location: impl Into<Box<str>>,
        context_kind: impl Into<Box<str>>,
        context_key: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        rule: FontRule,
    ) -> Self {
        let key = (
            location.into(),
            context_kind.into(),
            context_key.into(),
            source.into(),
        );
        self.contextual_entries.insert(key.clone(), rule);
        self.contextual_entry_adapter_scopes.remove(&key);
        self
    }

    #[must_use]
    pub fn with_context_entry_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        context_kind: impl Into<Box<str>>,
        context_key: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        rule: FontRule,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let key = (
            location.into(),
            context_kind.into(),
            context_key.into(),
            source.into(),
        );
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.contextual_entries.insert(key.clone(), rule);
        if adapters.is_empty() {
            self.contextual_entry_adapter_scopes.remove(&key);
        } else {
            self.contextual_entry_adapter_scopes.insert(key, adapters);
        }
        self
    }

    #[must_use]
    pub fn lookup(&self, location: &str) -> Option<Arc<str>> {
        self.by_location.get(location).cloned()
    }

    #[must_use]
    pub fn digest(&self) -> FontPolicyDigest {
        FontPolicyDigest::from_policy(self)
    }

    #[must_use]
    pub fn lookup_entry_for_adapter(
        &self,
        location: &str,
        adapter_id: &str,
        source: &str,
    ) -> Option<FontRule> {
        let key = (location.into(), source.into());
        let entry = self
            .entry_adapter_scopes
            .get(&key)
            .is_none_or(|adapters| adapters.contains(adapter_id))
            .then(|| self.entries.get(&key).cloned())
            .flatten();
        entry.or_else(|| {
            self.by_location_adapter
                .get(&(location.into(), adapter_id.into()))
                .cloned()
                .or_else(|| self.lookup(location))
                .map(FontRule::Substitute)
        })
    }

    #[must_use]
    pub fn lookup_context_entry_for_adapter(
        &self,
        location: &str,
        context_kind: &str,
        context_key: &str,
        adapter_id: &str,
        source: &str,
    ) -> Option<FontRule> {
        let key = (
            location.into(),
            context_kind.into(),
            context_key.into(),
            source.into(),
        );
        let entry = self
            .contextual_entry_adapter_scopes
            .get(&key)
            .is_none_or(|adapters| adapters.contains(adapter_id))
            .then(|| self.contextual_entries.get(&key).cloned())
            .flatten();
        entry.or_else(|| {
            self.by_location_adapter
                .get(&(location.into(), adapter_id.into()))
                .cloned()
                .or_else(|| self.lookup(location))
                .map(FontRule::Substitute)
        })
    }

    pub fn visit_locations(&self, mut visitor: impl FnMut(&str, &str)) {
        for (location, family) in &self.by_location {
            visitor(location, family);
        }
    }

    pub fn visit_locations_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &str, &BTreeSet<Box<str>>),
    ) {
        let empty = BTreeSet::new();
        for (location, family) in &self.by_location {
            visitor(location, family, &empty);
        }
        for ((location, adapter_id), family) in &self.by_location_adapter {
            let adapters = BTreeSet::from([adapter_id.clone()]);
            visitor(location, family, &adapters);
        }
    }

    pub fn visit_entries_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &str, &FontRule, &BTreeSet<Box<str>>),
    ) {
        let empty = BTreeSet::new();
        for ((location, source), rule) in &self.entries {
            let adapters = self
                .entry_adapter_scopes
                .get(&(location.clone(), source.clone()))
                .unwrap_or(&empty);
            visitor(location, source, rule, adapters);
        }
    }

    pub fn visit_context_entries_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &str, &str, &str, &FontRule, &BTreeSet<Box<str>>),
    ) {
        let empty = BTreeSet::new();
        for ((location, kind, key, source), rule) in &self.contextual_entries {
            let adapters = self
                .contextual_entry_adapter_scopes
                .get(&(location.clone(), kind.clone(), key.clone(), source.clone()))
                .unwrap_or(&empty);
            visitor(location, kind, key, source, rule, adapters);
        }
    }
}

impl FontPolicyDigest {
    fn from_policy(policy: &FontPolicy) -> Self {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x8422_2325_cbf2_9ce4_u64,
            0x9e37_79b9_7f4a_7c15_u64,
            0x517c_c1b7_2722_0a95_u64,
        ];
        for (location, family) in &policy.by_location {
            SnapshotDigest::digest_fields(
                &mut lanes,
                [
                    b"default".as_slice(),
                    location.as_bytes(),
                    family.as_bytes(),
                ],
            );
        }
        for ((location, adapter), family) in &policy.by_location_adapter {
            SnapshotDigest::digest_fields(
                &mut lanes,
                [
                    b"default-adapter".as_slice(),
                    location.as_bytes(),
                    adapter.as_bytes(),
                    family.as_bytes(),
                ],
            );
        }
        for ((location, source), rule) in &policy.entries {
            digest_font_rule(
                &mut lanes,
                b"entry",
                [location.as_ref(), source.as_ref()],
                rule,
            );
            if let Some(adapters) = policy
                .entry_adapter_scopes
                .get(&(location.clone(), source.clone()))
            {
                for adapter in adapters {
                    SnapshotDigest::digest_fields(
                        &mut lanes,
                        [
                            b"entry-adapter".as_slice(),
                            location.as_bytes(),
                            source.as_bytes(),
                            adapter.as_bytes(),
                        ],
                    );
                }
            }
        }
        for ((location, kind, key, source), rule) in &policy.contextual_entries {
            digest_font_rule(
                &mut lanes,
                b"context-entry",
                [
                    location.as_ref(),
                    kind.as_ref(),
                    key.as_ref(),
                    source.as_ref(),
                ],
                rule,
            );
            if let Some(adapters) = policy.contextual_entry_adapter_scopes.get(&(
                location.clone(),
                kind.clone(),
                key.clone(),
                source.clone(),
            )) {
                for adapter in adapters {
                    SnapshotDigest::digest_fields(
                        &mut lanes,
                        [
                            b"context-entry-adapter".as_slice(),
                            location.as_bytes(),
                            kind.as_bytes(),
                            key.as_bytes(),
                            source.as_bytes(),
                            adapter.as_bytes(),
                        ],
                    );
                }
            }
        }
        let mut digest = [0_u8; 32];
        for (index, lane) in lanes.into_iter().enumerate() {
            digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_le_bytes());
        }
        Self(digest)
    }
}

fn digest_font_rule<'a, const N: usize>(
    lanes: &mut [u64; 4],
    prefix: &'a [u8],
    key: [&'a str; N],
    rule: &'a FontRule,
) {
    let mut fields = Vec::with_capacity(N + 3);
    fields.push(prefix);
    fields.extend(key.into_iter().map(str::as_bytes));
    match rule {
        FontRule::Unchanged => fields.push(b"unchanged"),
        FontRule::Substitute(family) => {
            fields.push(b"substitute");
            fields.push(family.as_bytes());
        }
    }
    SnapshotDigest::digest_fields(lanes, fields);
}

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
