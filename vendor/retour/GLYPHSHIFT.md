# Local compatibility patch

This is the library source of retour 0.3.1, distributed under its original BSD-2-Clause license. Upstream: <https://github.com/Hpmason/retour-rs>.

The compatibility changes gate the `win64` function-trait implementation on `target_arch = "x86_64"` and the optional `thiscall` implementation on x86. Current Rust rejects win64 when compiling i686. The win64 guard follows upstream commit `94be4740ec0c624f61a074f439e6351fefc0e697`, without migrating every existing Adapter to the newer development API.

The `thiscall-abi` feature no longer enables the obsolete nightly `abi_thiscall` gate: this ABI is supported by the project's stable toolchain. Qt's MSVC x86 member functions use this feature; independent C++ ABI fixtures verify the real calls on x86 and the existing x64 path. Static-detour features retain their upstream nightly requirements.

Upstream examples/development dependencies are omitted from this downstream library copy. Do not edit a contributor's global Cargo registry. Remove this patch when a compatible release is adopted and both GDI architectures pass their native contracts.
