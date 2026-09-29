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

Public Core keeps only the five first-party Windows text implementations required by the base application:
Win32 DrawText/GDI TextOut/ExtTextOut, GDI+, and DirectWrite. Their descriptor/native companions live under
`implementations/native/` together with the shared GDI native support they require.

Official framework, engine, accessibility, OCR, Console, and Direct2D implementations are maintained outside this
public repository and are distributed as GSP plugins. Public Core intentionally does not vendor source snapshots of
those implementations.

Adapters are organized by text technology, not by software brand. Product orchestration discovers concrete
implementations at runtime and must not add static dependencies on them.

See [the local GSP pilot](../../docs/plugins.md) for package manifests, developer commands,
explicit local trust, and version selection. Native plugin packages do not duplicate the shared Runtime.

Official Adapter repositories consume the same public SDK/ABI and produce versioned GSP artifacts. Product integration
tests must consume those artifacts through the plugin contract instead of importing sibling source trees. See
[SDK and source ownership](../../docs/adapter-sdk.md). The App build in this repository always contains only the five
Windows Core adapters.
