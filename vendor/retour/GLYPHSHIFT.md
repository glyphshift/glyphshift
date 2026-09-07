# Local compatibility patch

This is the library source of retour 0.3.1, distributed under its original BSD-2-Clause license. Upstream: <https://github.com/Hpmason/retour-rs>.

The only source change gates the `win64` function-trait implementation on `target_arch = "x86_64"`. Current Rust rejects that calling convention when compiling i686. This follows the architecture guard present in upstream commit `94be4740ec0c624f61a074f439e6351fefc0e697`, without migrating every existing Adapter to the newer development API.

Upstream examples/development dependencies are omitted from this downstream library copy. Do not edit a contributor's global Cargo registry. Remove this patch when a compatible release is adopted and both GDI architectures pass their native contracts.
