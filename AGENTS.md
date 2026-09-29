# Repository instructions

- Collaborate with project contributors in Chinese.
- When running from Codex CLI, validate local GUI surfaces with the repository's Playwright CLI/test runner. Do not search for or depend on an in-app browser.
- For every request to build or launch the latest desktop app for review, use `scripts/review-app.ps1`.
  It rebuilds the desktop shell and Runtime Bundle from the same checkout and verifies the bundle
  with the production loader before launch. Never launch a shell produced by a standalone
  `cargo build`/`tauri build` or pair a newly built shell with a pre-existing Runtime directory.

## Engine-level adaptation scope

- The base App includes only conventional Windows text APIs: GDI ExtTextOut/TextOut,
  User32 DrawText, GDI+ DrawString, and DirectWrite TextLayout, plus shared Controller/Target Runtime.
- Framework, vendor, and game-engine adapters ship as separate `.gsp` plugins, including Qt,
  SideFX (Houdini), Unity, and any future Unreal Engine adapter. A `windows.*` Adapter ID
  does not make a framework adapter part of the base App. Group game plugins by engine,
  not individual game. Full bundled engine builds are explicit research/integration tools only;
  they must not become the default desktop or installer distribution.

- Official engine/framework, Console, Direct2D, UIA, and OCR Adapter implementations are owned by
  private `glyphshift/adapter-*` repositories and must not be vendored or source-snapshotted into
  this Public Core repository. Integrate them only through versioned GSP/Release/Registry artifacts
  and the public SDK/ABI contracts. Public Core CI rejects tracked private Adapter source paths.

- Build reusable engine/framework adapters across applications and games. Individual games are validation samples, not implementation targets.
- Do not hardcode game titles, scenario names, dialogue, installation paths, or game-specific addresses into production adapters. Detect and declare engine API, architecture, and version capabilities instead.
- Validate shared contracts with synthetic fixtures and multiple independent engine projects. A successful single-game experiment does not establish general engine support.

## Private Adapter boundary

- UIA, OCR, Console, standalone Direct2D, and official engine/framework implementations live only in
  private `glyphshift/adapter-*` repositories. Do not restore their source, reverse-engineering notes,
  real-engine fixtures, or source-sync tooling to Public Core.
- Public Core may expose package IDs, compatibility metadata, market UI, SDK/ABI contracts, signature
  verification, installation, selection, and lifecycle code. It must consume paid official Adapters as
  versioned GSP/Release/Registry artifacts.
- Run `python -B scripts/check-public-core-boundary.py` before commits that touch Adapter boundaries.
  Release CI and architecture checks run the same gate.
- Keep repository tests scoped to Public Core packages and synthetic reference fixtures. Integration
  tests requiring a paid Adapter belong with that private Adapter or the private integration-fixture repo.

## Build cache and local organization

- Use the global `CARGO_TARGET_DIR` as the cache root and isolate this application in its `glyphshift` child directory. Repository build/test scripts initialize this through `scripts/cargo-target.ps1`; direct Cargo commands must dot-source that helper and call `Get-GlyphshiftCargoTargetDirectory` first. Explicit target-directory overrides remain exact. Never clean the shared global root as if it belonged only to this project.
- Keep `local-test/` root limited to navigation and machine configuration. Place reusable local tools under `tools/`, software-specific experiments under `software/<name>/`, disposable caches under `cache/`, and evidence under `evidence/<topic>/`. After an experiment, promote verified portable conclusions to tracked project documentation and retain only useful local reproductions/evidence.

## Local testing and privacy

- Put every machine-specific test input and output under `local-test/`. This directory is
  local-only and must never be added, force-added, committed, or referenced as a required repository
  resource.
- Use `local-test/machine.ps1` (or another file below the same directory) for local executable
  paths and environment variables such as `GLYPHSHIFT_AE_EXE`, `GLYPHSHIFT_PREMIERE_EXE`,
  `GLYPHSHIFT_QQ_EXE`, attached PIDs, and temporary data roots.
- Put machine-specific screenshots, videos, raw logs, runtime descriptors, process/module dumps,
  captured text, temporary catalogs/packages, copied dictionaries/plugins, WebView profiles, and
  real AE/PR/QQ smoke-test results under `local-test/evidence/`; do not put them in any tracked
  directory.
- User-approved release screenshots may live under `preview/` and be referenced by README files only
  after a privacy review confirms they contain no local paths, usernames, PIDs, window titles, or
  other machine-specific evidence. All other screenshots remain local-only.
- Tracked test code may contain deterministic harnesses and synthetic fixtures only. Real software
  locations and live-process values must be supplied through environment variables; do not add a
  developer-machine fallback path. Path-parsing unit tests may use clearly synthetic paths that
  cannot be mistaken for a contributor's real layout.
- Do not write local usernames, drive layouts, absolute repository paths, installed-software paths,
  PIDs, window titles, or temporary-directory names into tracked source, tests, docs, comments,
  snapshots, or commit messages. Use placeholders such as `<repo>`, `<authorized-executable>`, and
  `<local-test-root>` when an example needs a path.
- Tracked project documentation may record portable conclusions, commands, pass/fail counts, and
  residual risks, but not raw local evidence or enough machine-specific detail to reconstruct the
  local environment.
- Before every commit, inspect staged paths and staged text for local artifacts. At minimum, reject
  anything under `local-test/` and scan for absolute drive paths, usernames, PIDs, and links to
  local screenshots.
