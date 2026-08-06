//! Discovery and resolution for signed Capability Adapter packages.

pub use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, Placement, RegistryRevision, TargetFacts,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArtifactHash([u8; 32]);

impl ArtifactHash {
    #[must_use]
    pub const fn sha256(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignerId(Box<str>);

impl SignerId {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageArtifactId(Box<str>);

impl PackageArtifactId {
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
pub enum AdapterHostBinding {
    TargetProcess { library: PackageArtifactId },
    IsolatedWorker { executable: PackageArtifactId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterTrustPolicy {
    trusted_signers: BTreeSet<SignerId>,
    authorized_adapters: BTreeSet<AdapterId>,
}

impl AdapterTrustPolicy {
    #[must_use]
    pub fn new(
        trusted_signers: impl IntoIterator<Item = SignerId>,
        authorized_adapters: impl IntoIterator<Item = AdapterId>,
    ) -> Self {
        Self {
            trusted_signers: trusted_signers.into_iter().collect(),
            authorized_adapters: authorized_adapters.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterVersionRequirement {
    Exact(AdapterVersion),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterPackage {
    descriptor: AdapterDescriptor,
    artifact_id: PackageArtifactId,
    signer: SignerId,
    declared_hash: ArtifactHash,
    observed_hash: ArtifactHash,
}

impl AdapterPackage {
    #[must_use]
    pub const fn new(
        descriptor: AdapterDescriptor,
        artifact_id: PackageArtifactId,
        signer: SignerId,
        declared_hash: ArtifactHash,
        observed_hash: ArtifactHash,
    ) -> Self {
        Self {
            descriptor,
            artifact_id,
            signer,
            declared_hash,
            observed_hash,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdapterPackageSet {
    packages: Vec<AdapterPackage>,
}

impl AdapterPackageSet {
    #[must_use]
    pub fn new(packages: impl IntoIterator<Item = AdapterPackage>) -> Self {
        Self {
            packages: packages.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterRequirement {
    adapter_id: AdapterId,
    version: AdapterVersionRequirement,
    features: BTreeSet<Feature>,
}

impl AdapterRequirement {
    #[must_use]
    pub fn new(
        adapter_id: AdapterId,
        version: AdapterVersionRequirement,
        features: impl IntoIterator<Item = Feature>,
    ) -> Self {
        Self {
            adapter_id,
            version,
            features: features.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn version_requirement(&self) -> AdapterVersionRequirement {
        self.version
    }

    pub fn features(&self) -> impl Iterator<Item = Feature> + '_ {
        self.features.iter().copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterBinding {
    pub descriptor: AdapterDescriptor,
    pub adapter_id: AdapterId,
    pub version: AdapterVersion,
    pub apply_model: ApplyModel,
    pub artifact_hash: ArtifactHash,
    pub host: AdapterHostBinding,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    RegistryUnavailable,
    DuplicateAdapterVersion {
        adapter_id: AdapterId,
        version: AdapterVersion,
    },
    ConflictingAdapterContent {
        adapter_id: AdapterId,
        version: AdapterVersion,
        registered: ArtifactHash,
        incoming: ArtifactHash,
    },
    AdapterNotFound(AdapterId),
    AdapterVersionNotFound {
        adapter_id: AdapterId,
        version: AdapterVersion,
    },
    FeatureUnavailable {
        adapter_id: AdapterId,
        feature: Feature,
    },
    UnsupportedHostModel {
        apply_model: ApplyModel,
        placement: Placement,
    },
    ObserveOnlyWriteback {
        adapter_id: AdapterId,
        feature: Feature,
    },
    UnsupportedPlatform {
        adapter_id: AdapterId,
        platform: Box<str>,
    },
    UnsupportedArchitecture {
        adapter_id: AdapterId,
        architecture: Box<str>,
    },
    IncompatibleAbi {
        adapter_id: AdapterId,
        host: AbiVersion,
        adapter: AbiVersion,
    },
    ArtifactHashMismatch {
        adapter_id: AdapterId,
        declared: ArtifactHash,
        observed: ArtifactHash,
    },
    UntrustedSigner {
        adapter_id: AdapterId,
        signer: SignerId,
    },
    AuthorizationMissing {
        adapter_id: AdapterId,
    },
}

#[derive(Debug)]
struct RegistryState {
    revision: u64,
    packages: BTreeMap<AdapterId, BTreeMap<AdapterVersion, AdapterPackage>>,
}

#[derive(Clone, Debug)]
pub struct AdapterRegistry {
    supported_abi: AbiVersion,
    trust_policy: AdapterTrustPolicy,
    state: Arc<RwLock<RegistryState>>,
}

impl AdapterRegistry {
    #[must_use]
    pub fn new(trust_policy: AdapterTrustPolicy) -> Self {
        Self {
            supported_abi: AbiVersion::new(1, 0),
            trust_policy,
            state: Arc::new(RwLock::new(RegistryState {
                revision: 0,
                packages: BTreeMap::new(),
            })),
        }
    }

    pub fn reload(
        &mut self,
        packages: AdapterPackageSet,
    ) -> Result<RegistryRevision, RegistryError> {
        let mut next_packages: BTreeMap<AdapterId, BTreeMap<AdapterVersion, AdapterPackage>> =
            BTreeMap::new();

        for package in packages.packages {
            let descriptor = &package.descriptor;
            let adapter_id = descriptor.adapter_id().clone();
            let version = descriptor.version();
            if descriptor.apply_model() == ApplyModel::ObserveOnly {
                if let Some(feature) = descriptor
                    .features()
                    .find(|feature| *feature != Feature::TextObserve)
                {
                    return Err(RegistryError::ObserveOnlyWriteback {
                        adapter_id,
                        feature,
                    });
                }
            }

            let versions = next_packages.entry(adapter_id.clone()).or_default();
            if let Some(registered) = versions.get(&version) {
                if registered.declared_hash != package.declared_hash {
                    return Err(RegistryError::ConflictingAdapterContent {
                        adapter_id,
                        version,
                        registered: registered.declared_hash,
                        incoming: package.declared_hash,
                    });
                }
                return Err(RegistryError::DuplicateAdapterVersion {
                    adapter_id,
                    version,
                });
            }
            versions.insert(version, package);
        }

        let mut state = self
            .state
            .write()
            .map_err(|_| RegistryError::RegistryUnavailable)?;
        state.packages = next_packages;
        state.revision += 1;

        Ok(RegistryRevision::new(state.revision))
    }

    pub fn resolve(
        &self,
        requirement: &AdapterRequirement,
        target: &TargetFacts,
    ) -> Result<AdapterBinding, RegistryError> {
        let state = self
            .state
            .read()
            .map_err(|_| RegistryError::RegistryUnavailable)?;
        let versions = state
            .packages
            .get(&requirement.adapter_id)
            .ok_or_else(|| RegistryError::AdapterNotFound(requirement.adapter_id.clone()))?;
        let AdapterVersionRequirement::Exact(requested_version) = requirement.version;
        let package = versions.get(&requested_version).ok_or_else(|| {
            RegistryError::AdapterVersionNotFound {
                adapter_id: requirement.adapter_id.clone(),
                version: requested_version,
            }
        })?;
        let descriptor = &package.descriptor;

        if package.declared_hash != package.observed_hash {
            return Err(RegistryError::ArtifactHashMismatch {
                adapter_id: descriptor.adapter_id().clone(),
                declared: package.declared_hash,
                observed: package.observed_hash,
            });
        }

        if !self.trust_policy.trusted_signers.contains(&package.signer) {
            return Err(RegistryError::UntrustedSigner {
                adapter_id: descriptor.adapter_id().clone(),
                signer: package.signer.clone(),
            });
        }

        if !self
            .trust_policy
            .authorized_adapters
            .contains(descriptor.adapter_id())
        {
            return Err(RegistryError::AuthorizationMissing {
                adapter_id: descriptor.adapter_id().clone(),
            });
        }

        let host_model_supported = matches!(
            (descriptor.apply_model(), descriptor.placement()),
            (
                ApplyModel::InlineRender | ApplyModel::RetainedObject,
                Placement::TargetProcess
            ) | (ApplyModel::ExternalProtocol, Placement::IsolatedWorker)
                | (ApplyModel::ObserveOnly, _)
        );
        if !host_model_supported {
            return Err(RegistryError::UnsupportedHostModel {
                apply_model: descriptor.apply_model(),
                placement: descriptor.placement(),
            });
        }

        if descriptor.abi().major() != self.supported_abi.major() {
            return Err(RegistryError::IncompatibleAbi {
                adapter_id: descriptor.adapter_id().clone(),
                host: self.supported_abi,
                adapter: descriptor.abi(),
            });
        }

        let platforms = descriptor.platforms().collect::<Vec<_>>();
        if !platforms.is_empty() && !platforms.contains(&target.operating_system()) {
            return Err(RegistryError::UnsupportedPlatform {
                adapter_id: descriptor.adapter_id().clone(),
                platform: target.operating_system().into(),
            });
        }

        let architectures = descriptor.architectures().collect::<Vec<_>>();
        if !architectures.is_empty() && !architectures.contains(&target.architecture()) {
            return Err(RegistryError::UnsupportedArchitecture {
                adapter_id: descriptor.adapter_id().clone(),
                architecture: target.architecture().into(),
            });
        }

        for feature in &requirement.features {
            if !descriptor.features().any(|declared| declared == *feature) {
                return Err(RegistryError::FeatureUnavailable {
                    adapter_id: requirement.adapter_id.clone(),
                    feature: *feature,
                });
            }
        }

        Ok(AdapterBinding {
            descriptor: descriptor.clone(),
            adapter_id: descriptor.adapter_id().clone(),
            version: descriptor.version(),
            apply_model: descriptor.apply_model(),
            artifact_hash: package.declared_hash,
            host: match descriptor.placement() {
                Placement::TargetProcess => AdapterHostBinding::TargetProcess {
                    library: package.artifact_id.clone(),
                },
                Placement::IsolatedWorker => AdapterHostBinding::IsolatedWorker {
                    executable: package.artifact_id.clone(),
                },
            },
            features: requirement.features.iter().copied().collect(),
        })
    }
}
