//! Validation and discovery for host-independent software extension packages.

use glyphshift_adapter_registry::AdapterRequirement;
pub use glyphshift_domain::{RouteLimits, RouteOperator, RouteProgram};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROUTE_STATE_ENTRIES: u16 = 1_024;
const MAX_ROUTE_STEPS: u16 = 4_096;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionId(Box<str>);

impl ExtensionId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl ExtensionVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionVersionRequirement {
    Exact(ExtensionVersion),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionRequirement {
    extension_id: ExtensionId,
    version: ExtensionVersionRequirement,
}

impl ExtensionRequirement {
    #[must_use]
    pub const fn new(extension_id: ExtensionId, version: ExtensionVersionRequirement) -> Self {
        Self {
            extension_id,
            version,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionPackageKind {
    DictionaryOnly,
    Software,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationCatalog {
    locale: Box<str>,
}

impl TranslationCatalog {
    #[must_use]
    pub fn new(locale: impl Into<Box<str>>) -> Self {
        Self {
            locale: locale.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerDescriptor {
    Generic,
    Code(ControllerCodeIdentity),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerArtifactId(Box<str>);

impl ControllerArtifactId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerSignerId(Box<str>);

impl ControllerSignerId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeHash([u8; 32]);

impl CodeHash {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    major: u16,
    minor: u16,
}

impl ProtocolVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerCodeIdentity {
    artifact_id: ControllerArtifactId,
    signer_id: ControllerSignerId,
    hash: CodeHash,
    protocol: ProtocolVersion,
}

impl ControllerCodeIdentity {
    #[must_use]
    pub const fn new(
        artifact_id: ControllerArtifactId,
        signer_id: ControllerSignerId,
        hash: CodeHash,
        protocol: ProtocolVersion,
    ) -> Self {
        Self {
            artifact_id,
            signer_id,
            hash,
            protocol,
        }
    }

    #[must_use]
    pub const fn artifact_id(&self) -> &ControllerArtifactId {
        &self.artifact_id
    }

    #[must_use]
    pub const fn signer_id(&self) -> &ControllerSignerId {
        &self.signer_id
    }

    #[must_use]
    pub const fn hash(&self) -> CodeHash {
        self.hash
    }

    #[must_use]
    pub const fn protocol(&self) -> ProtocolVersion {
        self.protocol
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareIdentity {
    name: Box<str>,
    vendor: Box<str>,
    executables: Vec<Box<str>>,
}

impl SoftwareIdentity {
    #[must_use]
    pub fn new(
        name: impl Into<Box<str>>,
        vendor: impl Into<Box<str>>,
        executables: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            name: name.into(),
            vendor: vendor.into(),
            executables: executables.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationLocation {
    id: Box<str>,
    label: Box<str>,
    context: Option<ContextSchema>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextSchema {
    kind: Box<str>,
    label: Box<str>,
}

impl ContextSchema {
    #[must_use]
    pub fn new(kind: impl Into<Box<str>>, label: impl Into<Box<str>>) -> Self {
        Self {
            kind: kind.into(),
            label: label.into(),
        }
    }
}

impl TranslationLocation {
    #[must_use]
    pub fn without_context(id: impl Into<Box<str>>, label: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context: None,
        }
    }

    #[must_use]
    pub fn with_context(
        id: impl Into<Box<str>>,
        label: impl Into<Box<str>>,
        context: ContextSchema,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context: Some(context),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExtensionArtifactKind {
    Data,
    Executable,
    NativeLibrary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionArtifact {
    id: Box<str>,
    kind: ExtensionArtifactKind,
}

impl ExtensionArtifact {
    #[must_use]
    pub fn data(id: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            kind: ExtensionArtifactKind::Data,
        }
    }

    #[must_use]
    pub fn executable(id: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            kind: ExtensionArtifactKind::Executable,
        }
    }

    #[must_use]
    pub fn native_library(id: impl Into<Box<str>>) -> Self {
        Self {
            id: id.into(),
            kind: ExtensionArtifactKind::NativeLibrary,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestDirective {
    FunctionAddress(u64),
    HookCode,
    Script,
}

impl ManifestDirective {
    #[must_use]
    pub const fn function_address(address: u64) -> Self {
        Self::FunctionAddress(address)
    }

    #[must_use]
    pub const fn hook_code() -> Self {
        Self::HookCode
    }

    #[must_use]
    pub const fn script() -> Self {
        Self::Script
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionPackage {
    extension_id: ExtensionId,
    version: ExtensionVersion,
    kind: ExtensionPackageKind,
    translations: Vec<TranslationCatalog>,
    controller: Option<ControllerDescriptor>,
    capability_requirements: Vec<AdapterRequirement>,
    software: Option<SoftwareIdentity>,
    locations: Vec<TranslationLocation>,
    route_program: Option<RouteProgram>,
    artifacts: Vec<ExtensionArtifact>,
    manifest_directives: Vec<ManifestDirective>,
}

impl ExtensionPackage {
    #[must_use]
    pub fn dictionary_only(
        extension_id: ExtensionId,
        version: ExtensionVersion,
        translations: impl IntoIterator<Item = TranslationCatalog>,
    ) -> Self {
        Self {
            extension_id,
            version,
            kind: ExtensionPackageKind::DictionaryOnly,
            translations: translations.into_iter().collect(),
            controller: None,
            capability_requirements: Vec::new(),
            software: None,
            locations: Vec::new(),
            route_program: None,
            artifacts: Vec::new(),
            manifest_directives: Vec::new(),
        }
    }

    #[must_use]
    pub fn software(
        extension_id: ExtensionId,
        version: ExtensionVersion,
        software: SoftwareIdentity,
        capability_requirements: impl IntoIterator<Item = AdapterRequirement>,
        locations: impl IntoIterator<Item = TranslationLocation>,
    ) -> Self {
        Self {
            extension_id,
            version,
            kind: ExtensionPackageKind::Software,
            translations: Vec::new(),
            controller: Some(ControllerDescriptor::Generic),
            capability_requirements: capability_requirements.into_iter().collect(),
            software: Some(software),
            locations: locations.into_iter().collect(),
            route_program: None,
            artifacts: Vec::new(),
            manifest_directives: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_controller(mut self, controller: ControllerDescriptor) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn with_route_program(mut self, route_program: RouteProgram) -> Self {
        self.route_program = Some(route_program);
        self
    }

    #[must_use]
    pub fn with_artifacts(
        mut self,
        artifacts: impl IntoIterator<Item = ExtensionArtifact>,
    ) -> Self {
        self.artifacts = artifacts.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_manifest_directives(
        mut self,
        directives: impl IntoIterator<Item = ManifestDirective>,
    ) -> Self {
        self.manifest_directives = directives.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionPackageSet {
    packages: Vec<ExtensionPackage>,
}

impl ExtensionPackageSet {
    #[must_use]
    pub fn new(packages: impl IntoIterator<Item = ExtensionPackage>) -> Self {
        Self {
            packages: packages.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedExtension {
    package: ExtensionPackage,
}

impl ResolvedExtension {
    #[must_use]
    pub const fn kind(&self) -> ExtensionPackageKind {
        self.package.kind
    }

    #[must_use]
    pub const fn controller(&self) -> Option<&ControllerDescriptor> {
        self.package.controller.as_ref()
    }

    #[must_use]
    pub fn capability_requirements(&self) -> &[AdapterRequirement] {
        &self.package.capability_requirements
    }

    #[must_use]
    pub const fn has_code_authority(&self) -> bool {
        matches!(
            self.package.controller.as_ref(),
            Some(ControllerDescriptor::Code(_))
        )
    }

    #[must_use]
    pub const fn software(&self) -> Option<&SoftwareIdentity> {
        self.package.software.as_ref()
    }

    #[must_use]
    pub fn locations(&self) -> &[TranslationLocation] {
        &self.package.locations
    }

    #[must_use]
    pub const fn route_program(&self) -> Option<&RouteProgram> {
        self.package.route_program.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteRejection {
    UnknownOperator(Box<str>),
    ExecutableCode,
    Io,
    InvalidLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestRejection {
    FunctionAddress,
    HookCode,
    Script,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionRegistryError {
    DictionaryOnlyContainsCode {
        extension_id: ExtensionId,
        artifact: Box<str>,
    },
    InvalidRouteProgram {
        extension_id: ExtensionId,
        reason: RouteRejection,
    },
    ForbiddenManifestDirective {
        extension_id: ExtensionId,
        reason: ManifestRejection,
    },
    DuplicateLocation {
        extension_id: ExtensionId,
        location: Box<str>,
    },
    InvalidContextSchema {
        extension_id: ExtensionId,
        location: Box<str>,
    },
    DuplicateExtensionVersion {
        extension_id: ExtensionId,
        version: ExtensionVersion,
    },
    ExtensionNotFound(ExtensionId),
    ExtensionVersionNotFound {
        extension_id: ExtensionId,
        version: ExtensionVersion,
    },
}

#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    revision: u64,
    packages: BTreeMap<ExtensionId, BTreeMap<ExtensionVersion, ExtensionPackage>>,
}

impl ExtensionRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            revision: 0,
            packages: BTreeMap::new(),
        }
    }

    pub fn reload(
        &mut self,
        package_set: ExtensionPackageSet,
    ) -> Result<u64, ExtensionRegistryError> {
        let mut next_packages: BTreeMap<ExtensionId, BTreeMap<ExtensionVersion, ExtensionPackage>> =
            BTreeMap::new();
        for package in package_set.packages {
            let extension_id = package.extension_id.clone();
            let version = package.version;
            if let Some(directive) = package.manifest_directives.first() {
                let reason = match directive {
                    ManifestDirective::FunctionAddress(_) => ManifestRejection::FunctionAddress,
                    ManifestDirective::HookCode => ManifestRejection::HookCode,
                    ManifestDirective::Script => ManifestRejection::Script,
                };
                return Err(ExtensionRegistryError::ForbiddenManifestDirective {
                    extension_id,
                    reason,
                });
            }
            let mut location_ids = BTreeSet::new();
            for location in &package.locations {
                if !location_ids.insert(location.id.clone()) {
                    return Err(ExtensionRegistryError::DuplicateLocation {
                        extension_id,
                        location: location.id.clone(),
                    });
                }
                if let Some(context) = &location.context {
                    if context.kind.trim().is_empty() || context.label.trim().is_empty() {
                        return Err(ExtensionRegistryError::InvalidContextSchema {
                            extension_id,
                            location: location.id.clone(),
                        });
                    }
                }
            }
            if let Some(route_program) = &package.route_program {
                let limits = route_program.limits();
                if limits.max_state_entries() == 0
                    || limits.max_state_entries() > MAX_ROUTE_STATE_ENTRIES
                    || limits.max_steps() == 0
                    || limits.max_steps() > MAX_ROUTE_STEPS
                {
                    return Err(ExtensionRegistryError::InvalidRouteProgram {
                        extension_id,
                        reason: RouteRejection::InvalidLimits,
                    });
                }
                for operator in route_program.operators() {
                    let rejection = match operator {
                        RouteOperator::Unknown { operator } => {
                            Some(RouteRejection::UnknownOperator(operator.clone()))
                        }
                        RouteOperator::NativeCode | RouteOperator::Script => {
                            Some(RouteRejection::ExecutableCode)
                        }
                        RouteOperator::Io => Some(RouteRejection::Io),
                        RouteOperator::Direct { .. }
                        | RouteOperator::Fallback { .. }
                        | RouteOperator::ContextualHeading { .. } => None,
                    };
                    if let Some(reason) = rejection {
                        return Err(ExtensionRegistryError::InvalidRouteProgram {
                            extension_id,
                            reason,
                        });
                    }
                }
            }
            if package.kind == ExtensionPackageKind::DictionaryOnly {
                if let Some(artifact) = package.artifacts.iter().find(|artifact| {
                    matches!(
                        artifact.kind,
                        ExtensionArtifactKind::Executable | ExtensionArtifactKind::NativeLibrary
                    )
                }) {
                    return Err(ExtensionRegistryError::DictionaryOnlyContainsCode {
                        extension_id,
                        artifact: artifact.id.clone(),
                    });
                }
            }
            let versions = next_packages.entry(extension_id.clone()).or_default();
            if versions.insert(version, package).is_some() {
                return Err(ExtensionRegistryError::DuplicateExtensionVersion {
                    extension_id,
                    version,
                });
            }
        }

        self.packages = next_packages;
        self.revision += 1;
        Ok(self.revision)
    }

    pub fn resolve(
        &self,
        requirement: &ExtensionRequirement,
    ) -> Result<ResolvedExtension, ExtensionRegistryError> {
        let versions = self
            .packages
            .get(&requirement.extension_id)
            .ok_or_else(|| {
                ExtensionRegistryError::ExtensionNotFound(requirement.extension_id.clone())
            })?;
        let ExtensionVersionRequirement::Exact(version) = requirement.version;
        let package = versions.get(&version).ok_or_else(|| {
            ExtensionRegistryError::ExtensionVersionNotFound {
                extension_id: requirement.extension_id.clone(),
                version,
            }
        })?;
        Ok(ResolvedExtension {
            package: package.clone(),
        })
    }
}
