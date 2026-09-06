use super::*;

const BUNDLE_SCHEMA: &str = "glyphshift.runtime-bundle/3";
const FIRST_PARTY_BUNDLE_AUTHORITY: &str = "app.glyphshift.runtime.first-party";
const CONTROLLER_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_ACQUISITION_SUPPORT_FILES: usize = 64;

#[derive(Deserialize)]
pub(super) struct BundleManifest {
    schema: Box<str>,
    authority: Box<str>,
    controller: ControllerManifest,
    runtime: ArtifactManifest,
    adapters: Vec<ArtifactManifest>,
    #[serde(default)]
    isolated_workers: Vec<IsolatedWorkerManifest>,
    pub(super) acquisition_workers: Vec<AcquisitionWorkerManifest>,
}

#[derive(Deserialize)]
struct ControllerManifest {
    artifact: Box<str>,
    file: Box<str>,
    sha256: Box<str>,
    protocol: [u16; 2],
}

#[derive(Deserialize)]
struct ArtifactManifest {
    file: Box<str>,
    sha256: Box<str>,
    #[serde(default)]
    name: Option<Box<str>>,
    #[serde(default)]
    summary: Option<Box<str>>,
    #[serde(default)]
    technology: Option<Box<str>>,
    #[serde(default)]
    #[serde(alias = "technicalTarget")]
    technical_target: Option<Box<str>>,
    #[serde(default, alias = "documentationUrl")]
    documentation_url: Option<Box<str>>,
    #[serde(default, alias = "processResidentAfterDeactivate")]
    process_resident_after_deactivate: bool,
}

#[derive(Deserialize)]
struct IsolatedWorkerManifest {
    file: Box<str>,
    sha256: Box<str>,
    adapter_id: Box<str>,
    version: [u16; 3],
    features: Vec<Box<str>>,
    platforms: Vec<Box<str>>,
    architectures: Vec<Box<str>>,
    #[serde(default)]
    name: Option<Box<str>>,
    #[serde(default)]
    summary: Option<Box<str>>,
    #[serde(default)]
    technology: Option<Box<str>>,
    #[serde(default, alias = "technicalTarget")]
    technical_target: Option<Box<str>>,
    #[serde(default, alias = "documentationUrl")]
    documentation_url: Option<Box<str>>,
}

#[derive(Deserialize)]
pub(super) struct AcquisitionWorkerManifest {
    pub(super) file: Box<str>,
    pub(super) sha256: Box<str>,
    pub(super) adapter_id: Box<str>,
    pub(super) support_files: Vec<AcquisitionSupportFileManifest>,
}

#[derive(Clone, Deserialize)]
pub(super) struct AcquisitionSupportFileManifest {
    pub(super) file: Box<str>,
    pub(super) sha256: Box<str>,
}

#[derive(Clone, Default)]
pub(super) struct AcquisitionWorkerCatalog {
    artifacts: BTreeMap<Box<str>, AcquisitionWorkerArtifact>,
}

impl AcquisitionWorkerCatalog {
    pub(super) fn load(
        root: &Path,
        manifests: &[AcquisitionWorkerManifest],
    ) -> Result<Self, DesktopRuntimeError> {
        if manifests.len() > 64 {
            return Err(DesktopRuntimeError::InvalidManifest);
        }
        let mut artifacts = BTreeMap::new();
        for manifest in manifests {
            if !valid_acquisition_adapter_id(&manifest.adapter_id)
                || !valid_acquisition_file_name(&manifest.file)
                || manifest.support_files.len() > MAX_ACQUISITION_SUPPORT_FILES
                || manifest
                    .support_files
                    .iter()
                    .any(|support| !valid_acquisition_file_name(&support.file))
            {
                return Err(DesktopRuntimeError::InvalidManifest);
            }
            let mut declared_files =
                BTreeSet::from([manifest.file.to_ascii_lowercase().into_boxed_str()]);
            for support in &manifest.support_files {
                if !declared_files.insert(support.file.to_ascii_lowercase().into_boxed_str()) {
                    return Err(DesktopRuntimeError::InvalidManifest);
                }
            }
            for support in &manifest.support_files {
                let hash = parse_hash(&support.sha256)?;
                verified_artifact(root, &support.file, hash)?;
            }
            let hash = parse_hash(&manifest.sha256)?;
            let path = verified_artifact(root, &manifest.file, hash)?;
            let artifact = AcquisitionWorkerArtifact::open(path)
                .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
            if artifacts
                .insert(manifest.adapter_id.clone(), artifact)
                .is_some()
            {
                return Err(DesktopRuntimeError::InvalidManifest);
            }
        }
        Ok(Self { artifacts })
    }

    pub(super) fn adapter_ids(&self) -> Vec<Box<str>> {
        self.artifacts.keys().cloned().collect()
    }

    pub(super) fn host(
        &self,
        adapter_id: &str,
    ) -> Result<AcquisitionWorkerHost, DesktopRuntimeError> {
        let artifact = self
            .artifacts
            .get(adapter_id)
            .cloned()
            .ok_or(DesktopRuntimeError::AcquisitionWorkerUnavailable)?;
        AcquisitionWorkerHost::new(artifact, ACQUISITION_WORKER_TIMEOUT)
            .map_err(|_| DesktopRuntimeError::AcquisitionWorkerUnavailable)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAdapterOption {
    id: Box<str>,
    name: Box<str>,
    version: Box<str>,
    summary: Box<str>,
    platforms: Vec<Box<str>>,
    architectures: Vec<Box<str>>,
    placement: Placement,
    technologies: Vec<Box<str>>,
    features: Vec<Feature>,
    technical_target: Box<str>,
    documentation_url: Option<Box<str>>,
    configuration: Box<str>,
    process_resident_after_deactivate: bool,
    source_policy: glyphshift_domain::SourceTextPolicy,
}

impl RuntimeAdapterOption {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn platforms(&self) -> &[Box<str>] {
        &self.platforms
    }

    #[must_use]
    pub fn architectures(&self) -> &[Box<str>] {
        &self.architectures
    }

    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    #[must_use]
    pub fn technologies(&self) -> &[Box<str>] {
        &self.technologies
    }

    #[must_use]
    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    #[must_use]
    pub fn technical_target(&self) -> &str {
        &self.technical_target
    }

    #[must_use]
    pub fn documentation_url(&self) -> Option<&str> {
        self.documentation_url.as_deref()
    }

    #[must_use]
    pub fn configuration(&self) -> &str {
        &self.configuration
    }

    #[must_use]
    pub const fn process_resident_after_deactivate(&self) -> bool {
        self.process_resident_after_deactivate
    }
    pub const fn source_policy(&self) -> glyphshift_domain::SourceTextPolicy { self.source_policy }
}

/// A verified, path-private set of production runtime artifacts.
pub struct RuntimeBundle {
    controller: VerifiedControllerArtifact,
    controller_protocol: ProtocolVersion,
    registry: AdapterRegistry,
    artifacts: TargetArtifactCatalog,
    worker_artifacts: WorkerArtifactCatalog,
    acquisition_workers: AcquisitionWorkerCatalog,
    isolated_adapter_ids: BTreeSet<AdapterId>,
    discovered_requirements: Vec<AdapterRequirement>,
    adapter_options: Vec<RuntimeAdapterOption>,
    nonce_ledger: NonceLedger,
    nonce_sequence: u64,
}

impl RuntimeBundle {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, DesktopRuntimeError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        let manifest: BundleManifest = serde_json::from_str(
            &fs::read_to_string(root.join("runtime-bundle.json"))
                .map_err(|_| DesktopRuntimeError::BundleUnavailable)?,
        )
        .map_err(|_| DesktopRuntimeError::InvalidManifest)?;
        if manifest.schema.as_ref() != BUNDLE_SCHEMA
            || manifest.authority.as_ref() != FIRST_PARTY_BUNDLE_AUTHORITY
            || manifest.controller.artifact.trim().is_empty()
            || manifest.adapters.is_empty()
        {
            return Err(DesktopRuntimeError::InvalidManifest);
        }

        let signer = SignerId::new(manifest.authority.clone());
        let controller_signer = ControllerSignerId::new(manifest.authority.clone());
        let controller_hash = parse_hash(&manifest.controller.sha256)?;
        let controller_path = artifact_path(&root, &manifest.controller.file)?;
        let controller_protocol = ProtocolVersion::new(
            manifest.controller.protocol[0],
            manifest.controller.protocol[1],
        );
        let controller_identity = ControllerCodeIdentity::new(
            ControllerArtifactId::new(manifest.controller.artifact),
            controller_signer,
            CodeHash::new(controller_hash),
            controller_protocol,
        );
        let controller = VerifiedControllerArtifact::verify(
            controller_path,
            &controller_identity,
            &ControllerTrustPolicy::new([manifest.authority.clone()]),
        )
        .map_err(|_| DesktopRuntimeError::ControllerRejected)?;

        let runtime_hash = parse_hash(&manifest.runtime.sha256)?;
        let runtime_path = verified_artifact(&root, &manifest.runtime.file, runtime_hash)?;
        let runtime_artifact = RuntimeArtifact::new(runtime_path, runtime_hash);

        let mut packages = Vec::new();
        let mut catalog_adapters = Vec::new();
        let mut authorized_adapters = Vec::new();
        let mut discovered_requirements = Vec::new();
        let mut adapter_options = Vec::new();
        for (index, adapter) in manifest.adapters.iter().enumerate() {
            let hash = parse_hash(&adapter.sha256)?;
            let path = verified_artifact(&root, &adapter.file, hash)?;
            // SAFETY: `verified_artifact` measured the exact file against the bundle manifest hash
            // before native code is loaded. The bundle authority is fixed by the product above.
            let (descriptor, source_policy) =
                unsafe { LoadedNativeAdapter::inspect_with_source_policy(&path) }.map_err(adapter_inspection_error)?;
            let features = descriptor
                .features()
                .filter(|feature| {
                    matches!(
                        feature,
                        Feature::TextObserve | Feature::TextReplace | Feature::FontSubstitute
                    )
                })
                .collect::<Vec<_>>();
            if !features.is_empty() {
                adapter_options.push(RuntimeAdapterOption {
                    id: descriptor.adapter_id().as_str().into(),
                    name: adapter
                        .name
                        .clone()
                        .unwrap_or_else(|| descriptor.adapter_id().as_str().into()),
                    version: format!(
                        "{}.{}.{}",
                        descriptor.version().major(),
                        descriptor.version().minor(),
                        descriptor.version().patch()
                    )
                    .into(),
                    summary: adapter
                        .summary
                        .clone()
                        .unwrap_or_else(|| "运行时文字与字体拦截适配器".into()),
                    platforms: descriptor.platforms().map(Into::into).collect(),
                    architectures: descriptor.architectures().map(Into::into).collect(),
                    placement: descriptor.placement(),
                    technologies: adapter.technology.iter().cloned().collect(),
                    features: features.clone(),
                    technical_target: adapter
                        .technical_target
                        .clone()
                        .unwrap_or_else(|| descriptor.adapter_id().as_str().into()),
                    documentation_url: parse_documentation_url(
                        adapter.documentation_url.as_deref(),
                    )?,
                    configuration: "none".into(),
                    process_resident_after_deactivate: adapter.process_resident_after_deactivate,
                    source_policy,
                });
            }
            let artifact_id = PackageArtifactId::new(format!("adapters/{index}"));
            discovered_requirements.push(AdapterRequirement::new(
                descriptor.adapter_id().clone(),
                AdapterVersionRequirement::Exact(descriptor.version()),
                features,
            ));
            authorized_adapters.push(descriptor.adapter_id().clone());
            packages.push(AdapterPackage::new(
                descriptor,
                artifact_id.clone(),
                signer.clone(),
                ArtifactHash::sha256(hash),
                ArtifactHash::sha256(hash),
            ));
            catalog_adapters.push((artifact_id, path));
        }

        let mut catalog_workers = Vec::new();
        let mut isolated_adapter_ids = BTreeSet::new();
        for (index, worker) in manifest.isolated_workers.iter().enumerate() {
            let hash = parse_hash(&worker.sha256)?;
            let path = verified_artifact(&root, &worker.file, hash)?;
            let features = worker
                .features
                .iter()
                .map(|feature| match feature.as_ref() {
                    "text-observe" => Ok(Feature::TextObserve),
                    _ => Err(DesktopRuntimeError::InvalidManifest),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if worker.adapter_id.trim().is_empty()
                || features.is_empty()
                || worker.platforms.is_empty()
                || worker.architectures.is_empty()
            {
                return Err(DesktopRuntimeError::InvalidManifest);
            }
            let adapter_id = AdapterId::new(worker.adapter_id.clone());
            let version =
                AdapterVersion::new(worker.version[0], worker.version[1], worker.version[2]);
            let descriptor = AdapterDescriptor::new(
                adapter_id.clone(),
                version,
                ApplyModel::ObserveOnly,
                Placement::IsolatedWorker,
                features.iter().copied(),
            )
            .with_platforms(worker.platforms.iter().cloned())
            .with_architectures(worker.architectures.iter().cloned());
            let artifact_id = PackageArtifactId::new(format!("isolated-workers/{index}"));
            discovered_requirements.push(AdapterRequirement::new(
                adapter_id.clone(),
                AdapterVersionRequirement::Exact(version),
                features.iter().copied(),
            ));
            adapter_options.push(RuntimeAdapterOption {
                id: adapter_id.as_str().into(),
                name: worker
                    .name
                    .clone()
                    .unwrap_or_else(|| adapter_id.as_str().into()),
                version: format!(
                    "{}.{}.{}",
                    worker.version[0], worker.version[1], worker.version[2]
                )
                .into(),
                summary: worker
                    .summary
                    .clone()
                    .unwrap_or_else(|| "独立进程文字观察适配器".into()),
                platforms: worker.platforms.clone(),
                architectures: worker.architectures.clone(),
                placement: Placement::IsolatedWorker,
                technologies: worker.technology.iter().cloned().collect(),
                features: features.clone(),
                technical_target: worker
                    .technical_target
                    .clone()
                    .unwrap_or_else(|| adapter_id.as_str().into()),
                documentation_url: parse_documentation_url(worker.documentation_url.as_deref())?,
                configuration: "none".into(),
                process_resident_after_deactivate: false,
                source_policy: glyphshift_domain::SourceTextPolicy::Exact,
            });
            packages.push(AdapterPackage::new(
                descriptor,
                artifact_id.clone(),
                signer.clone(),
                ArtifactHash::sha256(hash),
                ArtifactHash::sha256(hash),
            ));
            authorized_adapters.push(adapter_id.clone());
            isolated_adapter_ids.insert(adapter_id);
            catalog_workers.push((artifact_id, path));
        }

        let mut registry =
            AdapterRegistry::new(AdapterTrustPolicy::new([signer], authorized_adapters));
        registry
            .reload(AdapterPackageSet::new(packages))
            .map_err(|_| DesktopRuntimeError::AdapterRegistryRejected)?;
        let artifacts = TargetArtifactCatalog::new(runtime_artifact, catalog_adapters)
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        let worker_artifacts = WorkerArtifactCatalog::new(catalog_workers)
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        let acquisition_workers =
            AcquisitionWorkerCatalog::load(&root, &manifest.acquisition_workers)?;

        Ok(Self {
            controller,
            controller_protocol,
            registry,
            artifacts,
            worker_artifacts,
            acquisition_workers,
            isolated_adapter_ids,
            discovered_requirements,
            adapter_options,
            nonce_ledger: NonceLedger::new(),
            nonce_sequence: 0,
        })
    }

    #[must_use]
    pub fn translation_adapter_ids(&self) -> Vec<Box<str>> {
        self.discovered_requirements
            .iter()
            .filter(|requirement| {
                requirement
                    .features()
                    .any(|feature| feature == Feature::TextReplace)
            })
            .map(|requirement| requirement.adapter_id().as_str().into())
            .collect()
    }

    #[must_use]
    pub fn adapter_options(&self) -> &[RuntimeAdapterOption] {
        &self.adapter_options
    }

    #[must_use]
    pub fn adapter_requirements(&self) -> &[AdapterRequirement] {
        &self.discovered_requirements
    }

    #[must_use]
    pub fn acquisition_worker_ids(&self) -> Vec<Box<str>> {
        self.acquisition_workers.adapter_ids()
    }

    pub fn acquisition_worker_host(
        &self,
        adapter_id: &str,
    ) -> Result<AcquisitionWorkerHost, DesktopRuntimeError> {
        self.acquisition_workers.host(adapter_id)
    }

    pub fn discover(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<WindowsDesktopRuntime, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requirements = if spec.requirements().is_empty() {
            self.discovered_requirements.clone()
        } else {
            spec.requirements().to_vec()
        };
        let transport = ProcessControllerTransport::spawn_configured(
            self.controller.clone(),
            CONTROLLER_TIMEOUT,
            ControllerStartupConfig::new(
                spec.executable_names().iter().cloned(),
                requirements.clone(),
            )
            .with_executable_paths(spec.executable_paths().iter().cloned())
            .with_descendant_executable_names(spec.descendant_executable_names().iter().cloned()),
        )
        .map_err(|_| DesktopRuntimeError::ControllerUnavailable)?;
        self.nonce_sequence = self.nonce_sequence.saturating_add(1);
        DesktopRuntime::connect(
            transport,
            application_id,
            requirements,
            spec.publication().clone(),
            self.registry.clone(),
            self.artifacts.clone(),
            self.worker_artifacts.clone(),
            Box::new(self.acquisition_workers.clone()),
            self.isolated_adapter_ids.clone(),
            self.controller_protocol,
            next_nonce(self.nonce_sequence),
            &mut self.nonce_ledger,
        )
    }
}

pub(super) fn adapter_inspection_error(error: NativeHostError) -> DesktopRuntimeError {
    match error {
        NativeHostError::ApiSizeMismatch | NativeHostError::DescriptorSizeMismatch => {
            DesktopRuntimeError::AdapterAbiMismatch
        }
        _ => DesktopRuntimeError::AdapterInspectionFailed,
    }
}

fn artifact_path(root: &Path, file: &str) -> Result<PathBuf, DesktopRuntimeError> {
    let path = Path::new(file);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(DesktopRuntimeError::InvalidArtifactPath);
    }
    Ok(root.join(path))
}

fn verified_artifact(
    root: &Path,
    file: &str,
    declared_hash: [u8; 32],
) -> Result<PathBuf, DesktopRuntimeError> {
    let path = artifact_path(root, file)?;
    if measure_hash(&path)? != declared_hash {
        return Err(DesktopRuntimeError::ArtifactHashMismatch);
    }
    Ok(path)
}

fn measure_hash(path: &Path) -> Result<[u8; 32], DesktopRuntimeError> {
    let mut file = File::open(path).map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DesktopRuntimeError::BundleUnavailable)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

pub(super) fn parse_hash(value: &str) -> Result<[u8; 32], DesktopRuntimeError> {
    if value.len() != 64 {
        return Err(DesktopRuntimeError::InvalidArtifactHash);
    }
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| DesktopRuntimeError::InvalidArtifactHash)?;
    }
    Ok(hash)
}

pub(super) fn parse_documentation_url(
    value: Option<&str>,
) -> Result<Option<Box<str>>, DesktopRuntimeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() || value.trim() != value {
        return Err(DesktopRuntimeError::InvalidManifest);
    }
    let parsed = url::Url::parse(value).map_err(|_| DesktopRuntimeError::InvalidManifest)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(DesktopRuntimeError::InvalidManifest);
    }
    Ok(Some(value.into()))
}

fn next_nonce(sequence: u64) -> ControllerNonce {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&sequence.to_le_bytes());
    bytes[8..24].copy_from_slice(&timestamp.to_le_bytes());
    let process = u64::from(std::process::id());
    bytes[24..].copy_from_slice(&process.to_le_bytes());
    ControllerNonce::new(bytes)
}

fn valid_acquisition_adapter_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_acquisition_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
