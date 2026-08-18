use crate::{
    AiProviderProtocol, AiTranslation, CancellationToken, ProviderBatchResult, ProviderError,
    ProviderErrorCategory, ProviderRequest, ProviderTranslation, ProviderUsage,
    TranslationProvider,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const MAX_CAPTURED_OUTPUT_BYTES: usize = 4 << 20;

#[derive(Clone, Copy, Default)]
pub struct CodexExecTranslationProvider;

impl AiTranslation {
    pub fn register_codex_subscription_provider(&mut self) {
        self.register_provider(
            AiProviderProtocol::CodexSubscription,
            Arc::new(CodexExecTranslationProvider),
        );
    }
}

impl TranslationProvider for CodexExecTranslationProvider {
    fn translate(
        &self,
        request: &ProviderRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        run_codex_translation(request, cancellation)
    }
}

fn run_codex_translation(
    request: &ProviderRequest<'_>,
    cancellation: &CancellationToken,
) -> Result<ProviderBatchResult, ProviderError> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }
    let workspace =
        tempfile::tempdir().map_err(|_| local_error("could not prepare Codex workspace"))?;
    let schema_path = workspace.path().join("translation-output.schema.json");
    let output_path = workspace.path().join("translation-output.json");
    let schema = output_schema(request.items().len());
    std::fs::write(
        &schema_path,
        serde_json::to_vec(&schema)
            .map_err(|_| local_error("could not encode Codex output schema"))?,
    )
    .map_err(|_| local_error("could not prepare Codex output schema"))?;
    let prompt = translation_prompt(request)?;
    let mut command = codex_command();
    command.current_dir(workspace.path()).args([
        "exec",
        "--ephemeral",
        "--ignore-user-config",
        "--ignore-rules",
        "--sandbox",
        "read-only",
        "--skip-git-repo-check",
        "--json",
        "--color",
        "never",
        "--model",
        request.profile().model_id(),
    ]);
    if let Some(effort) = request.profile().reasoning_effort().codex_value() {
        command
            .arg("--config")
            .arg(format!("model_reasoning_effort=\"{effort}\""));
    }
    command
        .arg("--output-schema")
        .arg(&schema_path)
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_command_window(&mut command);
    let mut child = command.spawn().map_err(|_| {
        ProviderError::new(
            ProviderErrorCategory::InvalidRequest,
            false,
            "Codex CLI was not found; install Codex and sign in with ChatGPT",
        )
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| local_error("could not open Codex input"))?;
    stdin
        .write_all(prompt.as_bytes())
        .and_then(|()| stdin.flush())
        .map_err(|_| local_error("could not send translation input to Codex"))?;
    drop(stdin);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| local_error("could not read Codex events"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| local_error("could not read Codex diagnostics"))?;
    let stdout_reader = thread::spawn(move || read_tail(stdout, MAX_CAPTURED_OUTPUT_BYTES));
    let stderr_reader = thread::spawn(move || read_tail(stderr, 64 << 10));
    let started = Instant::now();
    let timeout = Duration::from_millis(request.profile().timeout_ms());
    let status = loop {
        if cancellation.is_cancelled() {
            terminate_process_tree(&mut child);
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(cancelled_error());
        }
        if started.elapsed() >= timeout {
            terminate_process_tree(&mut child);
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(ProviderError::new(
                ProviderErrorCategory::Timeout,
                true,
                "Codex translation request timed out",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => {
                terminate_process_tree(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(local_error("could not wait for Codex translation"));
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    if !status.success() {
        return Err(codex_failure(&stderr));
    }
    let output = std::fs::read(&output_path)
        .map_err(|_| malformed_error("Codex did not return structured translation output"))?;
    let response = serde_json::from_slice::<CodexTranslationOutput>(&output)
        .map_err(|_| malformed_error("Codex returned invalid structured translation output"))?;
    let translations = response
        .translations
        .into_iter()
        .map(|translation| ProviderTranslation::new(translation.item_id, translation.translation));
    let mut result = ProviderBatchResult::new(translations);
    if let Some(usage) = codex_usage(&stdout) {
        result = result.with_usage(usage);
    }
    Ok(result)
}

fn translation_prompt(request: &ProviderRequest<'_>) -> Result<String, ProviderError> {
    let input = json!({
        "sourceLocale": request.source_locale(),
        "targetLocale": request.target_locale(),
        "items": request.items().iter().map(|item| json!({
            "itemId": item.item_id(),
            "source": item.source(),
        })).collect::<Vec<_>>(),
    });
    let encoded = serde_json::to_string(&input)
        .map_err(|_| local_error("could not encode Codex translation input"))?;
    Ok(format!(
        "You are the translation model inside Glyphshift. Translate every UI text from {} to {}. Return only the JSON object required by the supplied output schema. Preserve every itemId exactly and return exactly one translation for every input item. Never reorder, merge, split, omit, duplicate, explain, number, or label translations. Preserve placeholders, format specifiers, escape sequences, keyboard shortcuts, and protected tokens exactly. Do not use shell, files, web search, tools, skills, plugins, or MCP.\n\nINPUT_JSON:\n{}",
        request.source_locale(),
        request.target_locale(),
        encoded,
    ))
}

fn output_schema(item_count: usize) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["translations"],
        "properties": {
            "translations": {
                "type": "array",
                "minItems": item_count,
                "maxItems": item_count,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["itemId", "translation"],
                    "properties": {
                        "itemId": {"type": "string"},
                        "translation": {"type": "string"}
                    }
                }
            }
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexTranslationOutput {
    translations: Vec<CodexTranslation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexTranslation {
    item_id: Box<str>,
    translation: Box<str>,
}

fn codex_usage(events: &[u8]) -> Option<ProviderUsage> {
    let text = String::from_utf8_lossy(events);
    text.lines().rev().find_map(|line| {
        let event = serde_json::from_str::<Value>(line).ok()?;
        if event.get("type").and_then(Value::as_str) != Some("turn.completed") {
            return None;
        }
        let usage = event.get("usage")?;
        let input = usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cached = usage
            .get("cached_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let reasoning = usage
            .get("reasoning_output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Some(ProviderUsage::new(
            input,
            output,
            reasoning,
            cached,
            input.saturating_add(output),
        ))
    })
}

fn codex_failure(stderr: &[u8]) -> ProviderError {
    let diagnostic = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if diagnostic.contains("not logged in") || diagnostic.contains("login") {
        ProviderError::new(
            ProviderErrorCategory::Authentication,
            false,
            "Codex is not signed in with ChatGPT; run codex login and retry",
        )
    } else if diagnostic.contains("model") && diagnostic.contains("not") {
        ProviderError::new(
            ProviderErrorCategory::ModelNotFound,
            false,
            "The selected model is not available to this Codex subscription",
        )
    } else if diagnostic.contains("rate limit") || diagnostic.contains("usage limit") {
        ProviderError::new(
            ProviderErrorCategory::RateLimited,
            true,
            "The Codex subscription usage limit was reached; retry after it resets",
        )
    } else {
        ProviderError::new(
            ProviderErrorCategory::ProviderInternal,
            true,
            "Codex could not complete the translation request",
        )
    }
}

fn cancelled_error() -> ProviderError {
    ProviderError::new(
        ProviderErrorCategory::Cancelled,
        false,
        "Codex translation request was cancelled",
    )
}

fn local_error(message: &'static str) -> ProviderError {
    ProviderError::new(ProviderErrorCategory::ProviderInternal, false, message)
}

fn malformed_error(message: &'static str) -> ProviderError {
    ProviderError::new(ProviderErrorCategory::MalformedOutput, true, message)
}

fn read_tail(mut reader: impl Read, maximum: usize) -> Vec<u8> {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 16 << 10];
    loop {
        let Ok(read) = reader.read(&mut buffer) else {
            break;
        };
        if read == 0 {
            break;
        }
        captured.extend_from_slice(&buffer[..read]);
        if captured.len() > maximum {
            captured.drain(..captured.len() - maximum);
        }
    }
    captured
}

#[cfg(windows)]
fn codex_command() -> Command {
    let mut resolver = Command::new("where.exe");
    resolver
        .arg("codex")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_command_window(&mut resolver);
    let resolved = resolver.output().ok().and_then(|output| {
        let candidates = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        candidates
            .iter()
            .find(|candidate| candidate.to_ascii_lowercase().ends_with(".exe"))
            .or_else(|| {
                candidates
                    .iter()
                    .find(|candidate| candidate.to_ascii_lowercase().ends_with(".cmd"))
            })
            .cloned()
    });
    Command::new(resolved.unwrap_or_else(|| "codex.cmd".to_owned()))
}

#[cfg(not(windows))]
fn codex_command() -> Command {
    Command::new("codex")
}

#[cfg(windows)]
fn hide_command_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_command_window(_command: &mut Command) {}

#[cfg(windows)]
fn terminate_process_tree(child: &mut Child) {
    use std::os::windows::process::CommandExt;
    let mut stop = Command::new("taskkill.exe");
    stop.args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000);
    let _ = stop.status();
    let _ = child.kill();
}

#[cfg(not(windows))]
fn terminate_process_tree(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_last_completed_turn_usage() {
        let events = br#"{"type":"turn.started"}
{"type":"turn.completed","usage":{"input_tokens":120,"cached_input_tokens":80,"output_tokens":25,"reasoning_output_tokens":7}}
"#;
        assert_eq!(
            codex_usage(events),
            Some(ProviderUsage::new(120, 25, 7, 80, 145))
        );
    }

    #[test]
    fn schema_requires_exact_item_count() {
        let schema = output_schema(3);
        assert_eq!(
            schema.pointer("/properties/translations/minItems"),
            Some(&json!(3))
        );
        assert_eq!(
            schema.pointer("/properties/translations/maxItems"),
            Some(&json!(3))
        );
    }
}
