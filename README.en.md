# Glyphshift

**Translate Windows applications and games with your own dictionaries, with optional AI help.**

[Download](https://www.yuelili.com/apps/glyphshift) · [Online guide (Chinese)](https://docs.yuelili.com/glyphshift) · [GitHub Releases](https://github.com/Yuelioi/glyphshift/releases) · [简体中文](README.md)

Glyphshift reads text while an application is running, looks up translations in a dictionary, and passes them back to the application for display. Collect text, edit translations, and configure fonts in one workflow, without changing the application's installed files.

Use it to maintain translations for everyday tools or try translating game menus and dialogue. Coverage depends on how the target displays text and what its adapters support.

## Get started

1. **Install and open Glyphshift.** Download the Windows installer from the links above.
2. **Create a workflow and select an application.** Choose a running application, browse for its executable, or use recent history. The workflow name fills automatically. Adapters are selected by default; keep the defaults if unsure.
3. **Add dictionaries.** Create a dictionary or select existing ones, then confirm the source and target languages. Choose one write dictionary to collect new text.
4. **Click Start.** If the target is closed, the workflow waits for it to open, then attempts to connect.
5. **Open View text and translate as you use the application.** Open menus, panels, or dialogue to collect source text. Enter translations yourself or use AI completion, then check the result in the target.

Each workflow targets one application. Create or copy workflows for other applications; workflows for different applications can run together. There is no separate software library or probe task to maintain.

## Workflow behavior

| Action | Result |
| --- | --- |
| Start while the target is closed | Waits for the application to open |
| Close the target application | Returns to waiting; attempts to reconnect when it opens again |
| Turn off Collect new text | Stops collecting new text; existing translations and font replacement continue |
| Stop workflow | Stops the workflow; opening the target again will not reconnect it until you start the workflow again |
| A stop cannot be confirmed | Shows Stop failed and lets you retry |

The header, workflow list, and detail view share the same runtime status. Returning to the workflow page or bringing Glyphshift to the foreground refreshes it, alongside periodic background checks. Previously collected text does not prove the target is still running.

Saving a translation does not always repaint the target immediately. Refresh target text from Task actions, or reopen a menu or advance the dialogue. Restoration after stopping also depends on the adapter; some components remain loaded until the target exits.

## Combining dictionaries

Dictionaries store source text, translations, languages, and other metadata. Edit, share, and reuse them across workflows. A dictionary can use any source and target language pair.

A workflow can select multiple dictionaries:

- **Write dictionary:** receives newly collected sources and the workflow's AI completion results.
- **Other dictionaries:** if any already contains the same source, that source is not appended to the write dictionary—even if its translation is blank.
- **Priority:** translations are looked up from top to bottom. If several dictionaries translate the same source, the earlier dictionary takes priority. Use the arrows to reorder them.

Both dictionary and workflow text lists can filter translated or untranslated entries and hide entries matched by skip rules. Manage those rules centrally in Settings.

### Import and export

- **Import JSON, UTF-8 CSV, or SRT.** Select multiple files, create separate dictionaries or merge them, and confirm names and languages.
- **Extract SRT subtitle text.** Prepare and translate a dictionary before using it in a workflow. This does not rewrite the subtitle file.
- **Import into an existing dictionary.** Append new entries, overwrite matching sources, or replace all entries after confirmation.
- **Export JSON or CSV.** JSON preserves dictionary metadata; CSV is useful for spreadsheet editing. SRT is currently import-only.

CSV needs `source` and `translation` columns. Translations may be blank; sources must be nonempty and unique within the file. Import errors include the filename, reason, and a row number where available.

## AI completion

Supported connections include OpenAI, Anthropic, Gemini, OpenAI-compatible services, Ollama, and a locally signed-in Codex subscription.

Add an AI connection in Settings, enter the service URL and model, and test it before translating. Configure batch size, maximum simultaneous requests, timeout, and retries. Speed depends on the model, hardware, and service; automatic completion does not guarantee real-time translation.

- Only untranslated candidates are completed; entries matched by skip rules are skipped.
- Tasks run in the background while you browse other pages. Inspect batches, progress, elapsed time, and token usage reported by the service.
- One translation task runs at a time, with requests using the connection's concurrency setting. Automatic completion from multiple workflows queues fairly.
- Automatic completion supports intervals from 0 to 60 seconds. Zero means processing new content as soon as possible, with a minimum background check interval of 250ms.
- Closing the target stops new rounds while allowing the current round to finish. Explicitly stopping a workflow cancels its own automatic task, without cancelling manually started dictionary tasks or other workflows.
- Restarting Glyphshift does not resume AI requests automatically.

**Cloud AI receives the text selected for translation. API keys are currently stored in plain text in local configuration.** A local Codex subscription connection uses its subscription allowance. You can also maintain dictionaries entirely by hand without AI.

## Fonts and settings

Font and size settings belong to workflows. Set a default font and size scale, with optional overrides for individual dictionaries, so reusing a dictionary does not carry those display settings with it.

Settings also manages recent applications, favorite fonts, per-language missing-glyph fallback fonts, and the language list. Font replacement, scaling, and missing-glyph support depend on the adapter and are not available in every application.

On first use, installed recommendations such as Microsoft YaHei, SimSun, SimHei, and Segoe UI are added to favorites. Existing users can choose Add recommended fonts in font settings without replacing their favorites.

Glyphshift's interface supports Simplified Chinese and English, with light, dark, and system themes. These interface choices do not restrict the languages a dictionary can translate. Update notifications can be turned off in Settings.

## Application support

Adapters cover traditional Windows text, GDI+, selected DirectWrite paths, Qt Widgets, Qt Quick, GTK 3 / Pango, raylib, MonoGame, Unity Mono, and other supported text paths. VGUI localization and CatSystem2 dialogue adapters remain experimental and need testing across more games.

Support for a framework does not mean every application built with it can be fully translated. Unity Mono support does not include IL2CPP, for example. Text in images, custom glyph rendering, or cached textures may not be replaceable. Check each adapter's in-app description for supported versions and limitations.

If translation does not work, check that the target is open and both applications have matching privilege levels, then check whether source text was collected. If text was collected but the display did not change, try reopening that surface. Changing resident adapters or Runtime versions may require fully closing and reopening the target.

## Documentation and releases

Read the [online guide](https://docs.yuelili.com/glyphshift) or browse [docs/index.md](docs/index.md) in the repository. The user guide is currently maintained in Simplified Chinese.

The [Release Action](.github/workflows/release.yml) runs when a `vMAJOR.MINOR.PATCH` tag is pushed. It checks application versions, builds the Windows installer, and packages `docs/` from the same checkout into `docs.zip`. Release assets include the installer, candidate manifest, documentation archive, and SHA-256 checksums.

The documentation site can subscribe to the Release's `docs.zip` attachment. The Action packages and uploads the archive; site synchronization requires a separate binding. See [documentation publishing](scripts/docs-publishing.md) for setup and maintenance.

## License

Copyright (c) 2026 Yuelioi

Glyphshift is released under the [GNU General Public License v3.0](LICENSE) (`GPL-3.0-only`). Third-party dependencies and vendored code retain their respective licenses.
