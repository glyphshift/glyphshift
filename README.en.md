# Glyphshift

[简体中文](README.md) · English

Glyphshift is a runtime interface translation tool for Windows desktop applications. It captures and replaces UI text
while an application is running, making tools with missing or incomplete localization easier to use without modifying
their installed files.

## What problem does Glyphshift solve?

Many professional tools, legacy applications, and independent desktop products do not provide complete localization.
Editing packaged resources is fragile, gets overwritten by updates, and often cannot reach custom-drawn menus, panels,
or text created at runtime. Glyphshift turns that problem into three reusable, verifiable parts:

- Probe the application to see which text is drawn and which adapter can cover that rendering path.
- Keep source text and translations in independent dictionaries that can be edited, imported, exported, and reused.
- Combine an application, dictionaries, adapters, and an optional font policy in a workflow that maintains translation
  while the target is running.

It is intended both for people who rely on untranslated desktop software and for translators or tool authors who
maintain reusable interface dictionaries.

## How it works

1. **Add the application** by selecting a running process, capturing the foreground application with a shortcut, or
   binding an executable manually.
2. **Probe and collect interface text** by opening the target menus, dialogs, and panels. Confirm a working adapter,
   then add translations to the bound dictionary manually or fill blank entries with AI.
3. **Enable a workflow** that combines the application, dictionaries, adapters, and optional font policy. Glyphshift
   keeps that translation intent active while the target is running.

A probe answers “can this surface be captured and replaced?” A workflow answers “which translation should remain
active from now on?”

## Core capabilities

- **Runtime text replacement** without modifying the target installation. Stop the workflow to remove Glyphshift's
  runtime effect.
- **Recoverable probes** that collect source text, occurrence evidence, and rendering sources with pause, resume,
  filtering, and dictionary binding.
- **Reusable dictionaries** with language, release metadata, and source-to-translation entries that can be shared by
  multiple workflows.
- **Composable workflows** where every application target selects its own adapters, ordered dictionaries, and optional
  font substitution policy.
- **Background AI filling** through OpenAI, Anthropic, Gemini, OpenAI-compatible, Ollama, and a local Codex
  subscription. Only one task runs at a time; page changes do not stop it, and reported tokens, batches, and duration
  remain visible.
- **Multiple rendering paths** through built-in adapters for several native Windows and framework UI technologies.
  Actual coverage must be confirmed with a probe.
- **Chinese and English UI**, with dark, light, and system themes.

## Get started

1. Start Glyphshift and the application you want to translate. They need matching privilege levels; an elevated target
   normally requires an elevated Glyphshift instance.
2. Open Probe and create a task. The target can come from the software library or a currently running application. Use
   an existing dictionary or let Glyphshift create a temporary empty one.
3. Open each target menu, dialog, and panel you want to translate and confirm that its source text appears in Probe.
4. Enter translations manually, or configure an AI profile in Settings and use AI Fill.
5. After confirming the translated UI in the target application, keep the temporary assets and create or enable a
   workflow for long-term use.

If no text is captured, keep the target surface visible and try another compatible adapter in Probe settings. The Help
page includes recovery paths for an application that is not running, privilege mismatch, and missing observations.

## AI translation and privacy

An AI profile stores the provider protocol, service URL, model, reasoning effort, batching, concurrency, timeout,
retries, and local filtering rules. Reasoning is off by default so routine interface translation does not spend large
reasoning-token budgets; each profile can opt back into automatic or higher effort. API keys are stored only in Windows
Credential Manager and are not written in plaintext to profiles, dictionaries, or logs.

A Codex subscription profile reuses the ChatGPT account already signed in through the local Codex CLI. Glyphshift does
not read, copy, or store its account token. Only the target dictionary is read-only while a task runs; the rest of the
app remains available. Quitting interrupts the task and never resends it automatically.

Only candidate source text selected by the current plan is sent to the AI service you choose. Existing translations and
locally filtered numbers, paths, URLs, shortcuts, and similar content are skipped before the request. Review the
provider's own data-handling and billing terms before using a cloud model.

## Scope and limitations

- The current release targets Windows desktop applications.
- Coverage depends on the rendering technology used by the target. Glyphshift cannot guarantee every application,
  window, or piece of text.
- Glyphshift is not an installer patch and is not a whole-screen OCR translation overlay.
- Process discovery, text observation, and a replacement decision do not by themselves prove that the final pixels are
  visible. Verify the result in the target application.
- Application updates can change rendering paths and may require another probe pass.

Glyphshift is currently a release candidate. Export backups of important dictionaries and validate compatibility in a
non-critical environment before relying on it for production work.
