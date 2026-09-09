# Glyphshift

[User guide (Chinese)](docs/index.md)

[简体中文](README.md) · English

Glyphshift is a runtime interface translation tool for Windows desktop applications and games. While an application is running,
it captures source UI text and replaces it with the target language selected by a dictionary.
Dictionaries can use any source and target language pair. Glyphshift never modifies the target application's installed
files.

## How to use it

1. Create a workflow and set its application: browse an executable, select a running or recent application, or capture it with the shortcut. The workflow name is filled automatically.
2. Select compatibility methods and dictionaries. Recent history can be cleared without changing existing workflows.
3. To collect new source text, choose one selected dictionary as the write destination, or create a new dictionary. Sources already present in any other selected dictionary are excluded even when their translations are blank.
4. Start the workflow and open Collect and translate. Enter translations or use AI completion.
5. Check the result in the application. Pause collection or stop the workflow when needed; dictionaries are retained.

Each workflow serves one application. Copy it to configure another application. Collection and translation share the workflow; there is no separate probe task.

Dictionaries and workflow entries support JSON and CSV import/export. JSON retains metadata; CSV requires language settings. Imports support keeping existing entries, overwriting matching sources, and replacing all entries.

This workflow update does not load old workflows, activation state, or probe tasks. Application records, dictionaries, and AI profiles are retained.

## AI translation and privacy

Each AI connection stores its service URL, model, reasoning mode, batching, simultaneous requests, timeout, retries, and
local skip rules. Extra reasoning is off by default to avoid unnecessary wait time and cost for routine interface text.
API keys are stored as plain text with the local AI connection. Settings masks them by default and can reveal them on
demand; use this only on a computer you trust.

A Codex subscription uses Codex already signed in on this computer, and translations use its subscription allowance.
Only the target dictionary is read-only while a task runs; the rest of the app remains available. Quitting interrupts
the task and never resends it automatically.

Only candidate source text selected by the current plan is sent to the AI service you choose. Existing translations and
locally filtered numbers, paths, URLs, shortcuts, and similar content are skipped before the request. Review the
AI service's own data-handling and billing terms before using a cloud model.

## Scope and limitations

- The current release targets Windows desktop applications and games.
- Compatibility methods cover native Windows text, Qt, and selected game-engine text paths. VGUI and CatSystem2 remain experimental: capture, replacement, and restoration have worked in real applications, but validation across games is incomplete. Coverage does not extend to every surface using the same engine.
- Coverage depends on the rendering technology used by the target. Glyphshift cannot guarantee every application,
  window, or piece of text.
- Glyphshift is not an installer patch and is not a whole-screen OCR translation overlay.
- Process discovery, text observation, and a replacement decision do not by themselves prove that the final pixels are
  visible. Verify the result in the target application.
- Application updates can change rendering paths and may require another probe pass.

Export backups of important dictionaries and verify capture, replacement, refresh, and restoration after stopping in the target application.

## License

Copyright (c) 2026 Yuelioi

Glyphshift is released under the [GNU General Public License v3.0](LICENSE) (`GPL-3.0-only`). Third-party dependencies and vendored code retain their respective licenses.
