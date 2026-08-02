# Repository instructions

- Route durable multi-session work through `flightdeck/deck.md` and the focused Work page.
- Collaborate with project contributors in Chinese.
- When running from Codex CLI, validate local GUI surfaces with the repository's Playwright CLI/test runner. Do not search for or depend on an in-app browser.

## Local testing and privacy

- Put every machine-specific test input and output under `target/local-test/`. This directory is
  local-only and must never be added, force-added, committed, or referenced as a required repository
  resource.
- Use `target/local-test/machine.ps1` (or another file below the same directory) for local executable
  paths and environment variables such as `GLYPHSHIFT_AE_EXE`, `GLYPHSHIFT_PREMIERE_EXE`,
  `GLYPHSHIFT_QQ_EXE`, attached PIDs, and temporary data roots.
- Put screenshots, videos, raw logs, runtime descriptors, process/module dumps, captured text,
  temporary catalogs/packages, copied dictionaries/plugins, WebView profiles, and real AE/PR/QQ
  smoke-test results under `target/local-test/evidence/`; do not put them in Flightdeck or any other
  tracked directory.
- Tracked test code may contain deterministic harnesses and synthetic fixtures only. Real software
  locations and live-process values must be supplied through environment variables; do not add a
  developer-machine fallback path. Path-parsing unit tests may use clearly synthetic paths that
  cannot be mistaken for a contributor's real layout.
- Do not write local usernames, drive layouts, absolute repository paths, installed-software paths,
  PIDs, window titles, or temporary-directory names into tracked source, tests, docs, comments,
  snapshots, or commit messages. Use placeholders such as `<repo>`, `<authorized-executable>`, and
  `<local-test-root>` when an example needs a path.
- Flightdeck may record portable conclusions, commands, pass/fail counts, and residual risks, but not
  raw local evidence or enough machine-specific detail to reconstruct the local environment.
- Before every commit, inspect staged paths and staged text for local artifacts. At minimum, reject
  anything under `target/local-test/` and scan for absolute drive paths, usernames, PIDs, and links to
  local screenshots.
