use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_domain::{AdapterId, Feature};
use glyphshift_extension::{
    CodeHash, ContextSchema, ControllerArtifactId, ControllerCodeIdentity, ControllerDescriptor,
    ControllerSignerId, ExtensionArtifact, ExtensionId, ExtensionPackage, ExtensionPackageKind,
    ExtensionPackageSet, ExtensionRegistry, ExtensionRegistryError, ExtensionRequirement,
    ExtensionVersion, ExtensionVersionRequirement, ManifestDirective, ManifestRejection,
    ProtocolVersion, RouteLimits, RouteOperator, RouteProgram, RouteRejection, SoftwareIdentity,
    TranslationCatalog, TranslationLocation,
};

#[test]
fn ext_001_resolves_a_dictionary_only_package_without_code_authority() {
    let extension_id = ExtensionId::new("org.example.dictionary");
    let version = ExtensionVersion::new(1, 0, 0);
    let package = ExtensionPackage::dictionary_only(
        extension_id.clone(),
        version,
        [TranslationCatalog::new("zh-CN")],
    );
    let mut registry = ExtensionRegistry::new();

    registry
        .reload(ExtensionPackageSet::new([package]))
        .expect("dictionary-only package should load");
    let resolved = registry
        .resolve(&ExtensionRequirement::new(
            extension_id,
            ExtensionVersionRequirement::Exact(version),
        ))
        .expect("dictionary-only package should resolve");

    assert_eq!(resolved.kind(), ExtensionPackageKind::DictionaryOnly);
    assert!(resolved.controller().is_none());
    assert!(resolved.capability_requirements().is_empty());
    assert!(!resolved.has_code_authority());
}

#[test]
fn ext_002_discovers_generic_software_and_preserves_declared_capabilities() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ExtensionVersion::new(1, 0, 0);
    let adapter_requirement = AdapterRequirement::new(
        AdapterId::new("example.synthetic.writeback"),
        AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
        [Feature::TextReplace],
    );
    let package = ExtensionPackage::software(
        extension_id.clone(),
        version,
        SoftwareIdentity::new("Example Editor", "Example Vendor", ["ExampleEditor.exe"]),
        [adapter_requirement.clone()],
        [TranslationLocation::without_context("menu", "Menu")],
    );
    let mut registry = ExtensionRegistry::new();

    registry
        .reload(ExtensionPackageSet::new([package]))
        .expect("generic software package should load");
    let resolved = registry
        .resolve(&ExtensionRequirement::new(
            extension_id,
            ExtensionVersionRequirement::Exact(version),
        ))
        .expect("generic software package should resolve");

    assert_eq!(resolved.kind(), ExtensionPackageKind::Software);
    assert_eq!(
        resolved
            .software()
            .expect("software metadata should be preserved")
            .name(),
        "Example Editor"
    );
    assert_eq!(resolved.capability_requirements(), &[adapter_requirement]);
    assert_eq!(resolved.locations()[0].id(), "menu");
}

#[test]
fn ext_003_preserves_controller_code_identity_hash_and_protocol_constraint() {
    let extension_id = ExtensionId::new("org.example.controller-editor");
    let version = ExtensionVersion::new(1, 0, 0);
    let controller = ControllerCodeIdentity::new(
        ControllerArtifactId::new("controller/win-x64/controller"),
        ControllerSignerId::new("example.signer.controller"),
        CodeHash::new([0x31; 32]),
        ProtocolVersion::new(1, 2),
    );
    let package = ExtensionPackage::software(
        extension_id.clone(),
        version,
        SoftwareIdentity::new(
            "Controller Editor",
            "Example Vendor",
            ["ControllerEditor.exe"],
        ),
        std::iter::empty(),
        [TranslationLocation::without_context("panel", "Panel")],
    )
    .with_controller(ControllerDescriptor::Code(controller.clone()));
    let mut registry = ExtensionRegistry::new();

    registry
        .reload(ExtensionPackageSet::new([package]))
        .expect("controller package should load");
    let resolved = registry
        .resolve(&ExtensionRequirement::new(
            extension_id,
            ExtensionVersionRequirement::Exact(version),
        ))
        .expect("controller package should resolve");

    assert_eq!(
        resolved.controller(),
        Some(&ControllerDescriptor::Code(controller))
    );
    assert!(resolved.has_code_authority());
}

#[test]
fn ext_004_preserves_a_valid_bounded_declarative_route_program() {
    let extension_id = ExtensionId::new("org.example.routed-editor");
    let version = ExtensionVersion::new(1, 0, 0);
    let route_program = RouteProgram::new(
        [
            RouteOperator::direct("menu"),
            RouteOperator::fallback(["panel", "menu"]),
            RouteOperator::contextual_heading("panel", "tool"),
        ],
        RouteLimits::new(32, 64),
    );
    let package = ExtensionPackage::software(
        extension_id.clone(),
        version,
        SoftwareIdentity::new("Routed Editor", "Example Vendor", ["RoutedEditor.exe"]),
        std::iter::empty(),
        [
            TranslationLocation::without_context("menu", "Menu"),
            TranslationLocation::without_context("panel", "Panel"),
        ],
    )
    .with_route_program(route_program.clone());
    let mut registry = ExtensionRegistry::new();

    registry
        .reload(ExtensionPackageSet::new([package]))
        .expect("bounded declarative routes should load");
    let resolved = registry
        .resolve(&ExtensionRequirement::new(
            extension_id,
            ExtensionVersionRequirement::Exact(version),
        ))
        .expect("routed extension should resolve");

    assert_eq!(resolved.route_program(), Some(&route_program));
}

#[test]
fn ext_004_rejects_zero_or_excessive_route_limits() {
    let limits = [
        RouteLimits::new(0, 16),
        RouteLimits::new(8, 0),
        RouteLimits::new(1_025, 16),
        RouteLimits::new(8, 4_097),
    ];

    for (index, limits) in limits.into_iter().enumerate() {
        let extension_id = ExtensionId::new(format!("org.example.route-limits-{index}"));
        let package = ExtensionPackage::software(
            extension_id.clone(),
            ExtensionVersion::new(1, 0, 0),
            SoftwareIdentity::new(
                "Route Limits Editor",
                "Example Vendor",
                ["RouteLimitsEditor.exe"],
            ),
            std::iter::empty(),
            [TranslationLocation::without_context("menu", "Menu")],
        )
        .with_route_program(RouteProgram::new([RouteOperator::direct("menu")], limits));
        let mut registry = ExtensionRegistry::new();

        assert_eq!(
            registry.reload(ExtensionPackageSet::new([package])),
            Err(ExtensionRegistryError::InvalidRouteProgram {
                extension_id,
                reason: RouteRejection::InvalidLimits,
            })
        );
    }
}

#[test]
fn ext_005_rejects_a_dictionary_only_package_that_contains_executable_code() {
    let extension_id = ExtensionId::new("org.example.unsafe-dictionary");
    let version = ExtensionVersion::new(1, 0, 0);
    let package = ExtensionPackage::dictionary_only(
        extension_id.clone(),
        version,
        [TranslationCatalog::new("zh-CN")],
    )
    .with_artifacts([
        ExtensionArtifact::executable("controller/controller"),
        ExtensionArtifact::native_library("runtime/adapter"),
    ]);
    let mut registry = ExtensionRegistry::new();

    let rejection = registry
        .reload(ExtensionPackageSet::new([package]))
        .expect_err("dictionary-only packages must never gain code authority");

    assert_eq!(
        rejection,
        ExtensionRegistryError::DictionaryOnlyContainsCode {
            extension_id,
            artifact: "controller/controller".into(),
        }
    );
}

#[test]
fn ext_006_rejects_unknown_executable_or_io_route_operators() {
    let cases = [
        (
            RouteOperator::unknown("future-operator"),
            RouteRejection::UnknownOperator("future-operator".into()),
        ),
        (RouteOperator::native_code(), RouteRejection::ExecutableCode),
        (RouteOperator::script(), RouteRejection::ExecutableCode),
        (RouteOperator::io(), RouteRejection::Io),
    ];

    for (index, (operator, expected_reason)) in cases.into_iter().enumerate() {
        let extension_id = ExtensionId::new(format!("org.example.invalid-route-{index}"));
        let package = ExtensionPackage::software(
            extension_id.clone(),
            ExtensionVersion::new(1, 0, 0),
            SoftwareIdentity::new(
                "Invalid Route Editor",
                "Example Vendor",
                ["InvalidRouteEditor.exe"],
            ),
            std::iter::empty(),
            [TranslationLocation::without_context("menu", "Menu")],
        )
        .with_route_program(RouteProgram::new([operator], RouteLimits::new(8, 16)));
        let mut registry = ExtensionRegistry::new();

        let rejection = registry
            .reload(ExtensionPackageSet::new([package]))
            .expect_err("unsafe route declarations must be rejected");

        assert_eq!(
            rejection,
            ExtensionRegistryError::InvalidRouteProgram {
                extension_id,
                reason: expected_reason,
            }
        );
    }
}

#[test]
fn ext_007_rejects_addresses_hook_code_and_scripts_in_software_manifests() {
    let cases = [
        (
            ManifestDirective::function_address(0x1234),
            ManifestRejection::FunctionAddress,
        ),
        (ManifestDirective::hook_code(), ManifestRejection::HookCode),
        (ManifestDirective::script(), ManifestRejection::Script),
    ];

    for (index, (directive, expected_reason)) in cases.into_iter().enumerate() {
        let extension_id = ExtensionId::new(format!("org.example.invalid-manifest-{index}"));
        let package = ExtensionPackage::software(
            extension_id.clone(),
            ExtensionVersion::new(1, 0, 0),
            SoftwareIdentity::new(
                "Invalid Manifest Editor",
                "Example Vendor",
                ["InvalidManifestEditor.exe"],
            ),
            std::iter::empty(),
            [TranslationLocation::without_context("menu", "Menu")],
        )
        .with_manifest_directives([directive]);
        let mut registry = ExtensionRegistry::new();

        let rejection = registry
            .reload(ExtensionPackageSet::new([package]))
            .expect_err("manifest-native runtime instructions must be rejected");

        assert_eq!(
            rejection,
            ExtensionRegistryError::ForbiddenManifestDirective {
                extension_id,
                reason: expected_reason,
            }
        );
    }
}

#[test]
fn ext_008_rejects_invalid_locations_without_replacing_the_previous_revision() {
    let extension_id = ExtensionId::new("org.example.location-editor");
    let stable_version = ExtensionVersion::new(1, 0, 0);
    let stable_package = ExtensionPackage::software(
        extension_id.clone(),
        stable_version,
        SoftwareIdentity::new("Location Editor", "Example Vendor", ["LocationEditor.exe"]),
        std::iter::empty(),
        [TranslationLocation::without_context("menu", "Menu")],
    );
    let mut registry = ExtensionRegistry::new();
    registry
        .reload(ExtensionPackageSet::new([stable_package]))
        .expect("stable package should load");

    let duplicate_location = ExtensionPackage::software(
        extension_id.clone(),
        ExtensionVersion::new(2, 0, 0),
        SoftwareIdentity::new("Location Editor", "Example Vendor", ["LocationEditor.exe"]),
        std::iter::empty(),
        [
            TranslationLocation::without_context("menu", "Menu"),
            TranslationLocation::without_context("menu", "Duplicate Menu"),
        ],
    );
    assert_eq!(
        registry.reload(ExtensionPackageSet::new([duplicate_location])),
        Err(ExtensionRegistryError::DuplicateLocation {
            extension_id: extension_id.clone(),
            location: "menu".into(),
        })
    );

    let invalid_context = ExtensionPackage::software(
        extension_id.clone(),
        ExtensionVersion::new(2, 0, 0),
        SoftwareIdentity::new("Location Editor", "Example Vendor", ["LocationEditor.exe"]),
        std::iter::empty(),
        [TranslationLocation::with_context(
            "parameter",
            "Parameter",
            ContextSchema::new("", "Tool"),
        )],
    );
    assert_eq!(
        registry.reload(ExtensionPackageSet::new([invalid_context])),
        Err(ExtensionRegistryError::InvalidContextSchema {
            extension_id: extension_id.clone(),
            location: "parameter".into(),
        })
    );

    assert!(
        registry
            .resolve(&ExtensionRequirement::new(
                extension_id,
                ExtensionVersionRequirement::Exact(stable_version),
            ))
            .is_ok(),
        "a rejected reload must preserve the last-known-good revision"
    );
}

#[test]
fn ext_009_removes_one_software_package_without_affecting_other_extensions() {
    let editor_id = ExtensionId::new("org.example.editor");
    let viewer_id = ExtensionId::new("org.example.viewer");
    let version = ExtensionVersion::new(1, 0, 0);
    let editor = ExtensionPackage::software(
        editor_id.clone(),
        version,
        SoftwareIdentity::new("Example Editor", "Example Vendor", ["ExampleEditor.exe"]),
        std::iter::empty(),
        [TranslationLocation::without_context("menu", "Menu")],
    );
    let viewer = ExtensionPackage::software(
        viewer_id.clone(),
        version,
        SoftwareIdentity::new("Example Viewer", "Example Vendor", ["ExampleViewer.exe"]),
        std::iter::empty(),
        [TranslationLocation::without_context("panel", "Panel")],
    );
    let mut registry = ExtensionRegistry::new();
    registry
        .reload(ExtensionPackageSet::new([editor, viewer.clone()]))
        .expect("both software packages should load");

    registry
        .reload(ExtensionPackageSet::new([viewer]))
        .expect("removing one package should publish a new revision");

    assert_eq!(
        registry.resolve(&ExtensionRequirement::new(
            editor_id.clone(),
            ExtensionVersionRequirement::Exact(version),
        )),
        Err(ExtensionRegistryError::ExtensionNotFound(editor_id))
    );
    assert!(
        registry
            .resolve(&ExtensionRequirement::new(
                viewer_id,
                ExtensionVersionRequirement::Exact(version),
            ))
            .is_ok(),
        "unrelated software must remain resolvable"
    );
}

#[test]
fn ext_010_resolves_unknown_software_added_at_runtime_with_existing_capabilities() {
    let editor_id = ExtensionId::new("org.example.editor");
    let viewer_id = ExtensionId::new("org.example.viewer");
    let version = ExtensionVersion::new(1, 0, 0);
    let editor = ExtensionPackage::software(
        editor_id,
        version,
        SoftwareIdentity::new("Example Editor", "Example Vendor", ["ExampleEditor.exe"]),
        std::iter::empty(),
        [TranslationLocation::without_context("menu", "Menu")],
    );
    let adapter_requirement = AdapterRequirement::new(
        AdapterId::new("example.synthetic.writeback"),
        AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
        [Feature::TextReplace, Feature::FontSubstitute],
    );
    let viewer = ExtensionPackage::software(
        viewer_id.clone(),
        version,
        SoftwareIdentity::new("Example Viewer", "Example Vendor", ["ExampleViewer.exe"]),
        [adapter_requirement.clone()],
        [TranslationLocation::without_context("panel", "Panel")],
    );
    let mut registry = ExtensionRegistry::new();
    registry
        .reload(ExtensionPackageSet::new([editor.clone()]))
        .expect("initial package set should load");

    registry
        .reload(ExtensionPackageSet::new([editor, viewer]))
        .expect("runtime package discovery should publish the unknown software");
    let resolved = registry
        .resolve(&ExtensionRequirement::new(
            viewer_id,
            ExtensionVersionRequirement::Exact(version),
        ))
        .expect("runtime-added software should resolve");

    assert_eq!(resolved.capability_requirements(), &[adapter_requirement]);
}
