use super::*;
use crate::{DictionaryEntry, DictionaryPage, DictionarySearch};

impl RegistryClient {
    /// Search display metadata; only an installation operation verifies the chosen original.
    pub fn search_dictionaries(&self, query: &DictionarySearch) -> Result<DictionaryPage, Error> {
        query.validate()?;
        let mut url = self
            .origin
            .join("v1/dictionaries")
            .map_err(|_| Error::Configuration)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("size", &query.size.to_string());
            if !query.text.trim().is_empty() {
                pairs.append_pair("text", query.text.trim());
            }
            for (key, value) in [
                ("sourceLocale", &query.source_locale),
                ("targetLocale", &query.target_locale),
                ("tag", &query.tag),
                ("cursor", &query.cursor),
            ] {
                if let Some(value) = value {
                    pairs.append_pair(key, value);
                }
            }
        }
        let bytes = self.get(url, 1024 * 1024, "application/json")?;
        let page: DictionaryPage = crate::strict_json::parse(&bytes)?;
        if page.items.len() > usize::from(query.size)
            || page.next_cursor.as_deref().is_some_and(|c| {
                !crate::discovery::valid_cursor(c) || Some(c) == query.cursor.as_deref()
            })
        {
            return Err(Error::InvalidArtifact);
        }
        let mut previous = None;
        for entry in &page.items {
            entry.validate()?;
            if previous.is_some_and(|id: &str| id >= entry.package_id.as_str())
                || !semver::Version::parse(&entry.version)
                    .map_err(|_| Error::InvalidArtifact)?
                    .pre
                    .is_empty()
            {
                return Err(Error::InvalidArtifact);
            }
            previous = Some(entry.package_id.as_str());
        }
        Ok(page)
    }

    /// Exact-version display metadata with caller-pinned identity; not an installation receipt.
    pub fn dictionary_detail(&self, request: &ReleaseRequest) -> Result<DictionaryEntry, Error> {
        if request.kind() != "dictionary" {
            return Err(Error::Configuration);
        }
        let mut url = self.origin.clone();
        url.path_segments_mut()
            .map_err(|_| Error::Configuration)?
            .clear()
            .extend([
                "v1",
                "dictionaries",
                request.package_id(),
                "releases",
                request.version(),
            ]);
        let bytes = self.get(url, 16384, "application/json")?;
        let entry: DictionaryEntry = crate::strict_json::parse(&bytes)?;
        entry.validate()?;
        if entry.package_id != request.package_id()
            || entry.version != request.version()
            || entry.publisher_user_key != request.publisher()
        {
            return Err(Error::IdentityMismatch);
        }
        Ok(entry)
    }
}
