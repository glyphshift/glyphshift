# Yueli Distribution Rust SDK snapshot

This directory is a vendored snapshot of `yueli-official/distribution/sdk/rust` version `0.1.0`.
Glyphshift consumes the shared SDK through this snapshot so a standalone checkout does not depend on
the developer machine's neighboring repository layout while the SDK is still unpublished.

Keep this directory source-equivalent to the Distribution SDK. Product-specific updater behavior
belongs in Glyphshift's adapter around the SDK, not in a fork of the HTTP contract here.
