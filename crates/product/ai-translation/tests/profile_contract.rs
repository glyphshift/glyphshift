use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiReasoningEffort, CredentialUpdate,
};
use tempfile::tempdir;

#[test]
fn codex_subscription_profile_uses_local_login_without_a_credential() {
    let root = tempdir().expect("Codex profile root");
    let mut catalog = AiProfileCatalog::open(root.path()).expect("open profile catalog");
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

#[test]
fn profiles_persist_plaintext_credentials_and_expose_them_for_editing() {
    let root = tempdir().expect("profile data root");
    let mut catalog = AiProfileCatalog::open(root.path()).expect("open profile catalog");

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
    assert_eq!(
        serde_json::to_value(&openai).expect("serialize OpenAI profile")["credential"],
        "synthetic-secret-value"
    );
    let persisted = std::fs::read_to_string(root.path().join("ai-profiles.json"))
        .expect("read persisted profiles");
    assert!(persisted.contains("glyphshift.ai-profiles/4"));
    assert!(persisted.contains(r#""reasoningEffort": "disabled""#));
    assert!(!persisted.contains("credentialRef"));
    assert!(persisted.contains("maxItemsPerRequest"));
    assert!(!persisted.contains("maxInputCharsPerRequest"));
    assert!(persisted.contains("synthetic-secret-value"));
    drop(catalog);

    let mut reopened = AiProfileCatalog::open(root.path()).expect("reopen profile catalog");
    let profiles = reopened.profiles().expect("list reopened profiles");
    assert_eq!(profiles.len(), 2);
    assert_eq!(reopened.default_profile_id(), Some("profile.ollama"));
    assert!(profiles
        .iter()
        .find(|profile| profile.id() == "profile.openai")
        .expect("OpenAI profile")
        .has_credential());
    assert_eq!(
        serde_json::to_value(
            profiles
                .iter()
                .find(|profile| profile.id() == "profile.openai")
                .expect("OpenAI profile")
        )
        .expect("serialize reopened profile")["credential"],
        "synthetic-secret-value"
    );
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
    reopened
        .delete_profile("profile.openai")
        .expect("delete profile and its credential");
    assert_eq!(
        reopened.profiles().expect("list remaining profiles").len(),
        1
    );
}

#[test]
fn codex_profile_normalizes_the_reversed_sol_model_id() {
    let root = tempdir().expect("Codex profile root");
    let mut catalog = AiProfileCatalog::open(root.path()).expect("open profile catalog");
    let profile = catalog
        .save_profile(AiProfileDraft::new(
            "profile.codex-sol",
            "Codex Sol",
            AiProviderProtocol::CodexSubscription,
            "gpt-sol-5.6",
        ))
        .expect("save Codex profile");

    assert_eq!(profile.model_id(), "gpt-5.6-sol");
}

#[test]
fn profile_catalog_keeps_valid_profiles_when_other_records_or_fields_are_invalid() {
    let root = tempdir().expect("profile tolerance root");
    std::fs::write(
        root.path().join("ai-profiles.json"),
        r#"{
  "schema": "glyphshift.ai-profiles/4",
  "defaultProfileId": "profile.valid",
  "unknownFutureField": true,
  "profiles": [
    {
      "id": "profile.valid",
      "name": "Valid",
      "protocol": "ollama_chat",
      "baseUrl": 42,
      "modelId": "valid-model",
      "reasoningEffort": 42,
      "timeoutMs": "invalid",
      "maxItemsPerRequest": 50,
      "maxConcurrency": 1,
      "maxRetries": "invalid",
      "filterPolicy": {
        "skipUrls": "invalid",
        "skipEmails": false,
        "unknownFilterField": true
      },
      "unknownProfileField": "ignored"
    },
    {
      "id": "profile.invalid",
      "name": "Invalid",
      "protocol": "ollama_chat",
      "baseUrl": "http://127.0.0.1:11434/api",
      "modelId": 42,
      "timeoutMs": 300000,
      "maxItemsPerRequest": 50,
      "maxConcurrency": 1,
      "maxRetries": 2,
      "filterPolicy": {}
    }
  ]
}"#,
    )
    .expect("write partially invalid profile artifact");

    let catalog = AiProfileCatalog::open(root.path()).expect("open remaining valid profiles");
    let profiles = catalog.profiles().expect("list valid profiles");

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id(), "profile.valid");
    assert_eq!(catalog.default_profile_id(), Some("profile.valid"));
    let profile = serde_json::to_value(&profiles[0]).expect("serialize recovered profile");
    assert_eq!(profile["baseUrl"], "http://127.0.0.1:11434/api");
    assert_eq!(profile["reasoningEffort"], "automatic");
    assert_eq!(profile["timeoutMs"], 1_800_000);
    assert_eq!(profile["maxRetries"], 2);
    assert_eq!(profile["filterPolicy"]["skipUrls"], true);
    assert_eq!(profile["filterPolicy"]["skipEmails"], false);
}

#[test]
fn malformed_profile_artifact_opens_empty_and_preserves_the_original_file() {
    let root = tempdir().expect("profile recovery root");
    std::fs::write(root.path().join("ai-profiles.json"), b"{")
        .expect("write malformed profile artifact");

    let catalog =
        AiProfileCatalog::open(root.path()).expect("open malformed profile artifact safely");

    assert!(catalog.profiles().expect("list profiles").is_empty());
    assert!(root.path().join("ai-profiles.json").exists());
    assert!(root.path().join("ai-profiles.invalid.json").exists());
}
