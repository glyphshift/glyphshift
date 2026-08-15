use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, CredentialUpdate, CredentialVault,
    CredentialVaultError,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

#[derive(Clone, Default)]
struct MemoryCredentialVault {
    secrets: Arc<Mutex<BTreeMap<Box<str>, Box<str>>>>,
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
        300_000
    );
    assert_eq!(
        serde_json::to_value(&openai).expect("serialize OpenAI profile")["maxRetries"],
        2
    );
    let persisted = std::fs::read_to_string(root.path().join("ai-profiles.json"))
        .expect("read persisted profiles");
    assert!(persisted.contains("glyphshift.ai-profiles/2"));
    assert!(persisted.contains("credentialRef"));
    assert!(!persisted.contains("maxItemsPerRequest"));
    assert!(!persisted.contains("maxInputCharsPerRequest"));
    assert!(!persisted.contains("synthetic-secret-value"));
    drop(catalog);

    let reopened =
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
        shared_secrets
            .lock()
            .expect("credential memory")
            .values()
            .map(Box::as_ref)
            .collect::<Vec<_>>(),
        vec!["synthetic-secret-value"]
    );
}
