# Adapter crates

This directory is the physical entry point for the Adapter capability family. Cargo package names, ABI boundaries,
crate types, and runtime discovery remain stable across the folder split.

## Platform

`platform/` owns contracts and shared loading/catalog behavior used across text technologies:

- `sdk/` — package `glyphshift-adapter-sdk`
- `registry/` — package `glyphshift-adapter-registry`
- `package/` — package `glyphshift-plugin-package`, GSP archives and immutable local installations
- `native-abi/` — package `glyphshift-adapter-native-abi`
- `native-host/` — package `glyphshift-adapter-native-host`

## Implementations

Public Core contains no concrete Adapter implementation source. All implementations live in
independent `glyphshift/adapter-*` repositories and are distributed as versioned GSP packages.

The five free base Adapter IDs come from three public repositories:

- `adapter-win32-text` — GDI ExtTextOut/TextOut and User32 DrawText
- `adapter-gdiplus` — GDI+ DrawString
- `adapter-directwrite` — DirectWrite TextLayout

The base App still bundles those five capabilities. `scripts/base-adapters.lock.json` pins the
Release URL and SHA-256 for each GSP; the Runtime Bundle build verifies and extracts the pinned
artifacts instead of compiling Adapter source in this repository.

Adapters are organized by text technology, not by software brand. Product orchestration discovers concrete
implementations at runtime and must not add static dependencies on them.

See [the local GSP pilot](../../docs/plugins.md) for package manifests, developer commands,
explicit local trust, and version selection. Native plugin packages do not duplicate the shared Runtime.

Official Adapter repositories consume the same public SDK/ABI and produce versioned GSP artifacts. Product integration
must consume those artifacts through the plugin contract instead of importing sibling source trees. See
[SDK and source ownership](../../docs/adapter-sdk.md). The App build in this repository always bundles only the five
free Windows base Adapter IDs.
