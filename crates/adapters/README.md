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

`implementations/` owns technology-specific descriptors and their deployable companions: Console, Direct2D,
DirectWrite, Win32 DrawText/GDI, GDI+, GTK3/Pango, Qt Painter, raylib, and UI Automation. Descriptor, Native DLL,
shared native support, and isolated Worker crates stay separate when they own distinct test, ABI, or process seams.
Leaf folders use technology names such as `gdi/`, `gdi-native/` and `uia-worker/`; the parent family supplies the
otherwise repeated `glyphshift-adapter-` path context.

Adapters are organized by text technology, not by software brand. Product orchestration discovers concrete
implementations at runtime and must not add static dependencies on them.
