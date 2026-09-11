use glyphshift_domain::Generation;
use crate::RegexTranslationRules;
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
    regex_rules: Vec<(Box<str>, RegexTranslationRules)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontPolicyDigest([u8; 32]);

impl SnapshotDigest {
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    fn from_entries(
        entries: &BTreeMap<Box<str>, BTreeMap<Box<str>, Arc<str>>>,
        contextual_entries: &ContextualTranslations,
        adapter_scopes: &TranslationScopes,
        contextual_adapter_scopes: &ContextualTranslationScopes,
        regex_rules: &Vec<(Box<str>, RegexTranslationRules)>,
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
        for (location, rules) in regex_rules {
        Self::digest_fields(&mut lanes, [b"rule-owner".as_slice(), location.as_bytes()]);
        for rule in rules.rules() {
            Self::digest_fields(&mut lanes, [b"regex-rule".as_slice(), rule.pattern.as_bytes(), rule.replacement.as_bytes(), if rule.enabled { b"1" } else { b"0" }]);
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
                &Vec::new(),
            ),
            entries,
            contextual_entries,
            adapter_scopes,
            contextual_adapter_scopes,
            regex_rules: Vec::new(),
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

    #[must_use]
    pub fn with_dictionary_rules(mut self, location: impl Into<Box<str>>, rules: RegexTranslationRules) -> Self {
        let location = location.into();
        if let Some((_, existing)) = self.regex_rules.iter_mut().find(|(id, _)| id == &location) { *existing = rules; }
        else { self.regex_rules.push((location, rules)); }
        self.refresh_digest();
        self
    }

    pub fn dictionary_rules(&self) -> &Vec<(Box<str>, RegexTranslationRules)> { &self.regex_rules }

    fn refresh_digest(&mut self) {
        self.digest = SnapshotDigest::from_entries(
            &self.entries,
            &self.contextual_entries,
            &self.adapter_scopes,
            &self.contextual_adapter_scopes,
            &self.regex_rules,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontRule {
    Scaled {
        family: Option<Arc<str>>,
        percent: u16,
    },
    Unchanged,
    Substitute(Arc<str>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontPolicy {
    scoped_entries: BTreeMap<(Box<str>, Box<str>, Box<str>), FontRule>,
    by_location: BTreeMap<Box<str>, FontRule>,
    by_location_adapter: BTreeMap<(Box<str>, Box<str>), FontRule>,
    entries: BTreeMap<(Box<str>, Box<str>), FontRule>,
    entry_adapter_scopes: TranslationScopes,
    contextual_entries: BTreeMap<ContextualTranslationKey, FontRule>,
    contextual_entry_adapter_scopes: ContextualTranslationScopes,
}

impl FontPolicy {
    #[must_use]
    pub fn with_entry_for_adapter(
        mut self,
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        adapter: impl Into<Box<str>>,
        rule: FontRule,
    ) -> Self {
        self.scoped_entries
            .insert((location.into(), source.into(), adapter.into()), rule);
        self
    }
    pub fn visit_adapter_entries(&self, mut visitor: impl FnMut(&str, &str, &str, &FontRule)) {
        for ((location, source, adapter), rule) in &self.scoped_entries {
            visitor(location, source, adapter, rule);
        }
    }

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
        self.by_location.insert(
            location.into(),
            FontRule::Substitute(Arc::from(family.into())),
        );
        self
    }

    #[must_use]
    pub fn with_location_for_adapters(
        self,
        location: impl Into<Box<str>>,
        family: impl Into<Box<str>>,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.with_location_rule_for_adapters(
            location,
            FontRule::Substitute(Arc::from(family.into())),
            adapter_ids,
        )
    }

    #[must_use]
    pub fn with_location_rule_for_adapters(
        mut self,
        location: impl Into<Box<str>>,
        rule: FontRule,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        let location = location.into();
        let adapters = adapter_ids
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        if adapters.is_empty() {
            self.by_location.insert(location, rule);
        } else {
            for adapter in adapters {
                self.by_location_adapter
                    .insert((location.clone(), adapter), rule.clone());
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
        match self.by_location.get(location) {
            Some(FontRule::Substitute(family))
            | Some(FontRule::Scaled {
                family: Some(family),
                ..
            }) => Some(family.clone()),
            _ => None,
        }
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
        if let Some(rule) =
            self.scoped_entries
                .get(&(location.into(), source.into(), adapter_id.into()))
        {
            return Some(rule.clone());
        }
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
                .or_else(|| self.by_location.get(location).cloned())
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
                .or_else(|| self.by_location.get(location).cloned())
        })
    }

    pub fn visit_locations(&self, mut visitor: impl FnMut(&str, &str)) {
        for (location, family) in &self.by_location {
            if let FontRule::Substitute(family)
            | FontRule::Scaled {
                family: Some(family),
                ..
            } = family
            {
                visitor(location, family);
            }
        }
    }

    pub fn visit_location_rules_with_adapters(
        &self,
        mut visitor: impl FnMut(&str, &FontRule, &BTreeSet<Box<str>>),
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
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    fn from_policy(policy: &FontPolicy) -> Self {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x8422_2325_cbf2_9ce4_u64,
            0x9e37_79b9_7f4a_7c15_u64,
            0x517c_c1b7_2722_0a95_u64,
        ];
        for ((location, source, adapter), rule) in &policy.scoped_entries {
            digest_font_rule(
                &mut lanes,
                b"scoped-entry",
                [location.as_ref(), source.as_ref(), adapter.as_ref()],
                rule,
            );
        }
        for (location, rule) in &policy.by_location {
            digest_font_rule(&mut lanes, b"default", [location.as_ref()], rule);
        }
        for ((location, adapter), rule) in &policy.by_location_adapter {
            digest_font_rule(
                &mut lanes,
                b"default-adapter",
                [location.as_ref(), adapter.as_ref()],
                rule,
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
    let percent_bytes;
    match rule {
        FontRule::Scaled { family, percent } => {
            percent_bytes = percent.to_le_bytes();
            fields.push(b"scaled");
            fields.push(family.as_deref().unwrap_or("").as_bytes());
            fields.push(&percent_bytes);
        }
        FontRule::Unchanged => fields.push(b"unchanged"),
        FontRule::Substitute(family) => {
            fields.push(b"substitute");
            fields.push(family.as_bytes());
        }
    }
    SnapshotDigest::digest_fields(lanes, fields);
}
