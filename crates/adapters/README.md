# Adapter crates

This directory is the physical entry point for the Adapter capability family. Cargo package names, ABI boundaries,
crate types, and runtime discovery remain stable across the folder split.

## Platform

`platform/` owns contracts and shared loading/catalog behavior used across text technologies:

- `sdk/` — package `glyphshift-adapter-sdk`
- `registry/` — package `glyphshift-adapter-registry`
- `native-abi/` — package `glyphshift-adapter-native-abi`
- `native-host/` — package `glyphshift-adapter-native-host`

## Implementations

`implementations/` owns technology-specific descriptors and their deployable companions. Its second level is a
navigation aid based on how text is acquired; it does not introduce another runtime interface:

- `native/` — Console, Direct2D, DirectWrite, Win32 DrawText/GDI, and GDI+ hooks.
- `framework/` — GTK3/Pango, Qt Painter, Qt Quick retained labels, raylib, and MonoGame hooks.
- `accessibility/` — UI Automation descriptor and isolated Worker.
- `fallback/` — OCR descriptor and isolated Worker.

Descriptor, Native DLL, shared native support, and isolated Worker crates stay separate when they own distinct test,
ABI, or process seams. Companions for one technology remain adjacent inside the same group. Empty future categories
are not created before a concrete Adapter exists.

Adapters are organized by text technology, not by software brand. Product orchestration discovers concrete
implementations at runtime and must not add static dependencies on them.
