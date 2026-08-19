use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiReasoningEffort, CredentialUpdate,
    CredentialVault, CredentialVaultError,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

#[derive(Clone, Default)]
struct MemoryCredentialVault {
    secrets: Arc<Mutex<BTreeMap<Box<str>, Box<str>>>>,
}

#[test]
fn codex_subscription_profile_uses_local_login_without_a_credential() {
    let root = tempdir().expect("Codex profile root");
    let mut catalog =
        AiProfileCatalog::open(root.path(), Box::new(MemoryCredentialVault::default()))
            .expect("open profile catalog");
    let profile = catalog
        .save_profile(AiProfileDraft::new(
            "profile.codex",
            "Codex subscription",
            AiProviderProtocol::CodexSubscription,
            "gpt-5.6-luna",
        ))
        .expect("save Codex profile");

    assert_eq!(profile.base_url(), "codex://local");
    assert!(!profile.credential_required());
    assert!(!profile.has_credential());
    let resolved = catalog
        .resolve_profile("profile.codex")
        .expect("resolve Codex profile without credential");
    assert_eq!(resolved.base_url(), "codex://local");
    assert_eq!(resolved.max_concurrency(), 1);
    assert!(resolved.credential().is_none());
    assert_eq!(resolved.reasoning_effort(), AiReasoningEffort::Disabled);
    assert_eq!(resolved.reasoning_effort().codex_value(), Some("none"));
}

impl CredentialVault for MemoryCredentialVault {
    fn replace(&self, credential_ref: &str, secret: &str) -> Result<(), CredentialVaultError> {
        self.secrets
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .insert(credential_ref.into(), secret.into());
        Ok(())
    }

    fn contains(&self, credential_ref: &str) -> Result<bool, CredentialVaultError> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .contains_key(credential_ref))
    }

    fn delete(&self, credential_ref: &str) -> Result<(), CredentialVaultError> {
        self.secrets
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .remove(credential_ref);
        Ok(())
    }

    fn expose(&self, credential_ref: &str) -> Result<Box<str>, CredentialVaultError> {
        self.secrets
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .get(credential_ref)
            .cloned()
            .ok_or(CredentialVaultError::Missing)
    }
}

#[test]
fn profiles_and_default_selection_survive_restart_without_persisting_plaintext_credentials() {
    let root = tempdir().expect("profile data root");
    let vault = MemoryCredentialVault::default();
    let shared_secrets = vault.secrets.clone();
    let mut catalog =
        AiProfileCatalog::open(root.path(), Box::new(vault.clone())).expect("open profile catalog");

    let openai = catalog
        .save_profile(
            AiProfileDraft::new(
                "profile.openai",
                "OpenAI",
                AiProviderProtocol::OpenAiResponses,
                "gpt-synthetic",
            )
            .with_credential(CredentialUpdate::replace("synthetic-secret-value")),
        )
        .expect("save OpenAI profile");
    catalog
        .save_profile(AiProfileDraft::new(
            "profile.ollama",
            "Local Ollama",
            AiProviderProtocol::OllamaChat,
            "qwen-synthetic",
        ))
        .expect("save Ollama profile");
    catalog
        .set_default_profile("profile.ollama")
        .expect("select default profile");

    assert!(openai.has_credential());
    assert_eq!(openai.base_url(), "https://api.openai.com/v1");
    assert_eq!(
        serde_json::to_value(&openai).expect("serialize OpenAI profile")["timeoutMs"],
        1_800_000
    );
    assert_eq!(
        serde_json::to_value(&openai).expect("serialize OpenAI profile")["maxRetries"],
        2
    );
    assert_eq!(
        serde_json::to_value(&openai).expect("serialize OpenAI profile")["maxItemsPerRequest"],
        50
    );
    let persisted = std::fs::read_to_string(root.path().join("ai-profiles.json"))
        .expect("read persisted profiles");
    assert!(persisted.contains("glyphshift.ai-profiles/3"));
    assert!(persisted.contains(r#""reasoningEffort": "disabled""#));
    assert!(persisted.contains("credentialRef"));
    assert!(persisted.contains("maxItemsPerRequest"));
    assert!(!persisted.contains("maxInputCharsPerRequest"));
    assert!(!persisted.contains("synthetic-secret-value"));
    drop(catalog);

    let mut reopened =
        AiProfileCatalog::open(root.path(), Box::new(vault)).expect("reopen profile catalog");
    let profiles = reopened.profiles().expect("list reopened profiles");
    assert_eq!(profiles.len(), 2);
    assert_eq!(reopened.default_profile_id(), Some("profile.ollama"));
    assert!(profiles
        .iter()
        .find(|profile| profile.id() == "profile.openai")
        .expect("OpenAI profile")
        .has_credential());
    assert_eq!(
        profiles
            .iter()
            .find(|profile| profile.id() == "profile.openai")
            .expect("OpenAI profile")
            .reasoning_effort(),
        AiReasoningEffort::Disabled
    );
    assert_eq!(
        profiles
            .iter()
            .find(|profile| profile.id() == "profile.ollama")
            .expect("Ollama profile")
            .reasoning_effort(),
        AiReasoningEffort::Automatic
    );
    assert_eq!(
        shared_secrets
            .lock()
            .expect("credential memory")
            .values()
            .map(Box::as_ref)
            .collect::<Vec<_>>(),
        vec!["synthetic-secret-value"]
    );

    reopened
        .delete_profile("profile.openai")
        .expect("delete profile and its credential");
    assert!(shared_secrets
        .lock()
        .expect("credential memory")
        .is_empty());
    assert_eq!(reopened.profiles().expect("list remaining profiles").len(), 1);
}

#[test]
fn existing_profiles_without_a_batch_size_gain_the_fifty_item_default() {
    let root = tempdir().expect("profile migration data root");
    std::fs::write(
        root.path().join("ai-profiles.json"),
        r#"{
  "schema": "glyphshift.ai-profiles/2",
  "defaultProfileId": "profile.local",
  "profiles": [{
    "id": "profile.local",
    "name": "Local",
    "protocol": "ollama_chat",
    "baseUrl": "http://127.0.0.1:11434/api",
    "modelId": "synthetic-model",
    "credentialRef": "glyphshift.ai-profile/profile.local",
    "timeoutMs": 300000,
    "maxConcurrency": 1,
    "maxRetries": 2,
    "filterPolicy": {}
  }]
}"#,
    )
    .expect("write existing profile artifact");

    let catalog = AiProfileCatalog::open(root.path(), Box::new(MemoryCredentialVault::default()))
        .expect("open existing profile artifact");
    let profile = catalog
        .profiles()
        .expect("list migrated profiles")
        .into_iter()
        .next()
        .expect("migrated profile");

    assert_eq!(
        serde_json::to_value(profile).expect("serialize migrated profile")["maxItemsPerRequest"],
        50
    );
    let profile = catalog
        .profiles()
        .expect("list migrated profiles")
        .into_iter()
        .next()
        .expect("migrated profile");
    assert_eq!(profile.reasoning_effort(), AiReasoningEffort::Automatic);
    let persisted = std::fs::read_to_string(root.path().join("ai-profiles.json"))
        .expect("read migrated profile artifact");
    assert!(persisted.contains("glyphshift.ai-profiles/3"));
}
