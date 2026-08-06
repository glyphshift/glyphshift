# Crate map

Package names remain the stable identity. Physical folders provide capability navigation, while architecture layers
independently constrain dependency direction; a folder must not be read as permission to depend on every sibling.

All crate packages now use capability-family entry points and short leaf directories. Cargo package names retain the
`glyphshift-` identity; for example, package `glyphshift-protocol` lives at `runtime/protocol/`. Directory names are
navigation, not a second dependency policy.

## Capability families

- Core policy: `core/{domain,translation,capture,decision,workflow,extension}`.
- Dictionary assets: `dictionary/{package,distribution}`.
- Adapter platform: `adapters/platform/` owns `glyphshift-adapter-sdk`, `glyphshift-adapter-registry`,
  `glyphshift-adapter-native-abi`, and `glyphshift-adapter-native-host`.
- Adapter implementations: `adapters/implementations/` owns every other `glyphshift-adapter-*` package. A text
  technology may have a safe descriptor, a Native DLL, or an isolated Worker companion; each deployment artifact
  remains a separate package.
- Runtime orchestration: `runtime/{protocol,contract,kernel,session,desktop}` plus
  `runtime/controller/{sdk,host,windows}`, `runtime/worker/{sdk,host}` and
  `runtime/targets/{contract,process-host,runtime}`. The plural avoids Cargo's reserved/ignored `target/` build
  directory name.
- Product model: `product/desktop-backend`. Application composition roots remain under `apps/`.
- Deterministic and authorized fixtures remain under `test-support/`; production packages must not depend on them.

## Dependency direction

The normal direction is Shell → Product → Host/Kernel → deployment contracts → policy/catalog → foundation/SDK.
Concrete Adapters may depend on foundation, SDK/ABI/contracts, and their own technology companion, but production
orchestration discovers them at runtime and must not statically depend on a concrete implementation. Test-support may
depend on production interfaces; production code must never depend on test-support.

`architecture-tests/check.ps1` is the executable source of truth for package coverage, exact direct dependencies, and
these cross-layer rules. Its source scan resolves package roots from Cargo metadata rather than assuming a flat
directory. Do not create a `glyphshift-core`, `common`, or `utils` package: shared code needs one named capability and
one clear owner. All current workspace packages inherit `publish = false`; a future public package must explicitly
establish its registry and versioning contract.
