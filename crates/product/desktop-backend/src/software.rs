use super::*;

const EXTENSION_SCHEMA: &str = "glyphshift.extension/1";
const DESKTOP_STATE_SCHEMA: &str = "glyphshift.desktop-state/1";
pub(super) const DEFAULT_LOCALE: &str = "zh-CN";

pub(super) struct LoadedSoftware {
    pub(super) software: BTreeMap<Box<str>, SoftwareState>,
    pub(super) local_software: BTreeMap<Box<str>, DesktopSoftwareArtifact>,
    pub(super) selected_software_id: Option<Box<str>>,
    pub(super) requirements: Vec<AdapterRequirement>,
    pub(super) warnings: Vec<ArtifactWarningView>,
}

pub(super) fn load(root: &Path) -> Result<LoadedSoftware, BackendError> {
    let extension_root = root.join("extensions");
    fs::create_dir_all(&extension_root)
        .map_err(|_| BackendError::Storage("create-extension-directory"))?;
    let mut extension_paths = fs::read_dir(&extension_root)
        .map_err(|_| BackendError::Storage("read-extension-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    extension_paths.sort();

    let mut software = BTreeMap::new();
    let mut warnings = Vec::new();
    for path in extension_paths {
        let artifact_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unreadable-file-name");
        let Ok(artifact) = read_json::<ExtensionArtifact>(&path, "extension-json") else {
            warnings.push(ArtifactWarningView::software(artifact_id, "unreadable"));
            continue;
        };
        if validate_extension(&artifact, &path).is_err() {
            warnings.push(ArtifactWarningView::software(artifact_id, "invalid"));
            continue;
        }
        if software.contains_key(&artifact.id) {
            warnings.push(ArtifactWarningView::software(
                artifact_id,
                "duplicate_identity",
            ));
            continue;
        }
        software.insert(
            artifact.id.clone(),
            SoftwareState {
                artifact,
                locale: DEFAULT_LOCALE.into(),
            },
        );
    }
    let requirements = software
        .values()
        .filter_map(|state| state.artifact.runtime.as_ref())
        .flat_map(|runtime| runtime.capabilities.iter())
        .map(runtime_requirement)
        .collect::<Result<Vec<_>, _>>()?;
    let desktop_state = read_desktop_state(root)?;
    let selected_software_id = desktop_state
        .selected_software_id
        .and_then(|selected| software.contains_key(&selected).then_some(selected))
        .or_else(|| software.keys().next().cloned());
    let local_software = desktop_state
        .software
        .into_iter()
        .filter(|(extension_id, _)| software.contains_key(extension_id))
        .collect();
    Ok(LoadedSoftware {
        software,
        local_software,
        selected_software_id,
        requirements,
        warnings,
    })
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    state: &'static str,
    enabled: bool,
    coverage: u8,
    detail: &'static str,
    generation: Option<u64>,
}

impl CapabilityView {
    fn unavailable(detail: &'static str) -> Self {
        Self {
            state: "unavailable",
            enabled: false,
            coverage: 0,
            detail,
            generation: None,
        }
    }

    fn pending_observation() -> Self {
        Self {
            state: "limited",
            enabled: false,
            coverage: 0,
            detail: "capability.compatibility-pending",
            generation: None,
        }
    }
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareView {
    id: Box<str>,
    name: Box<str>,
    description: Box<str>,
    vendor: Box<str>,
    version: Box<str>,
    executable_name: Box<str>,
    executable_path: Option<Box<str>>,
    monogram: Box<str>,
    last_used: Option<Box<str>>,
    locale: Box<str>,
    connected: bool,
    translation: CapabilityView,
    font: CapabilityView,
    observe: CapabilityView,
}

impl SoftwareView {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn executable_name(&self) -> &str {
        &self.executable_name
    }

    #[must_use]
    pub fn executable_path(&self) -> Option<&str> {
        self.executable_path.as_deref()
    }

    #[must_use]
    pub const fn translation_capability(&self) -> &CapabilityView {
        &self.translation
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableSelection {
    path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareEdit {
    extension_id: Box<str>,
    display_name: Box<str>,
    description: Box<str>,
    executable_path: PathBuf,
}

impl SoftwareEdit {
    #[must_use]
    pub fn new(
        extension_id: impl Into<Box<str>>,
        display_name: impl Into<Box<str>>,
        executable_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            extension_id: extension_id.into(),
            display_name: display_name.into(),
            description: "".into(),
            executable_path: executable_path.into(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.description = description.into();
        self
    }
}

impl ExecutableSelection {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ExtensionArtifact {
    pub(super) schema: Box<str>,
    pub(super) id: Box<str>,
    pub(super) version: Box<str>,
    pub(super) name: Box<str>,
    pub(super) vendor: Box<str>,
    #[serde(default)]
    pub(super) executables: Vec<Box<str>>,
    #[serde(default)]
    pub(super) descendant_executables: Vec<Box<str>>,
    #[serde(default)]
    pub(super) runtime: Option<ExtensionRuntimeArtifact>,
    pub(super) locations: Vec<ExtensionLocationArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ExtensionRuntimeArtifact {
    #[serde(default)]
    pub(super) capabilities: Vec<RuntimeCapabilityArtifact>,
    pub(super) route: RuntimeRouteArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RuntimeCapabilityArtifact {
    pub(super) adapter: Box<str>,
    pub(super) version: [u16; 3],
    pub(super) features: Vec<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RuntimeRouteArtifact {
    pub(super) kind: Box<str>,
    pub(super) locations: Vec<Box<str>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ExtensionLocationArtifact {
    pub(super) id: Box<str>,
    pub(super) label: Box<str>,
    #[serde(default)]
    pub(super) context: Option<ContextSchemaArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ContextSchemaArtifact {
    pub(super) kind: Box<str>,
    pub(super) label: Box<str>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopStateArtifact {
    pub(super) schema: Box<str>,
    pub(super) selected_software_id: Option<Box<str>>,
    #[serde(default)]
    pub(super) software: BTreeMap<Box<str>, DesktopSoftwareArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopSoftwareArtifact {
    pub(super) display_name: Box<str>,
    #[serde(default)]
    pub(super) description: Box<str>,
    pub(super) executable_path: Box<str>,
}

pub(super) struct SoftwareState {
    pub(super) artifact: ExtensionArtifact,
    pub(super) locale: Box<str>,
}
#[derive(Clone, Debug)]
pub struct DesktopRuntimeSpec {
    pub(super) executable_names: Vec<Box<str>>,
    pub(super) executable_paths: Vec<Box<str>>,
    pub(super) descendant_executable_names: Vec<Box<str>>,
    pub(super) requirements: Vec<AdapterRequirement>,
    pub(super) publication: RuntimePublication,
}

impl DesktopRuntimeSpec {
    /// Restrict a compiled intent to explicit executable identities (including a
    /// software family with architecture-specific launchers). The Controller
    /// validates these paths and binds live process instances before deployment.
    #[must_use]
    pub fn with_executable_paths(
        mut self,
        paths: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.executable_paths = paths.into_iter().map(Into::into).collect();
        self
    }
    #[must_use]
    pub fn executable_names(&self) -> &[Box<str>] {
        &self.executable_names
    }

    #[must_use]
    pub fn executable_paths(&self) -> &[Box<str>] {
        &self.executable_paths
    }

    #[must_use]
    pub fn descendant_executable_names(&self) -> &[Box<str>] {
        &self.descendant_executable_names
    }

    #[must_use]
    pub fn requirements(&self) -> &[AdapterRequirement] {
        &self.requirements
    }

    #[must_use]
    pub const fn publication(&self) -> &RuntimePublication {
        &self.publication
    }
}
impl DesktopBackend {
    pub fn runtime_spec(&self, extension_id: &str) -> Result<DesktopRuntimeSpec, BackendError> {
        let state = self
            .software
            .get(extension_id)
            .ok_or_else(|| BackendError::UnknownSoftware(extension_id.into()))?;
        if state.artifact.executables.is_empty() {
            return Err(BackendError::InvalidArtifact("runtime-executable-missing"));
        }
        let (requirements, route) = state.artifact.runtime.as_ref().map_or_else(
            || {
                let location = state
                    .artifact
                    .locations
                    .first()
                    .ok_or(BackendError::InvalidArtifact("runtime-location-missing"))?;
                Ok((Vec::new(), RouteProgram::direct(location.id.clone())))
            },
            |runtime| {
                if runtime.capabilities.is_empty() {
                    return Err(BackendError::InvalidArtifact("runtime-capability-empty"));
                }
                let requirements = runtime
                    .capabilities
                    .iter()
                    .map(runtime_requirement)
                    .collect::<Result<Vec<_>, _>>()?;
                let route = runtime_route(&runtime.route, &state.artifact.locations)?;
                Ok((requirements, route))
            },
        )?;
        Ok(DesktopRuntimeSpec {
            executable_names: state.artifact.executables.clone(),
            executable_paths: self
                .local_software
                .get(extension_id)
                .map(|software| vec![software.executable_path.clone()])
                .unwrap_or_default(),
            descendant_executable_names: state.artifact.descendant_executables.clone(),
            requirements,
            publication: RuntimePublication::new(
                route,
                TranslationSnapshot::empty(Generation::new(0)),
                FontPolicy::empty(),
            ),
        })
    }

    pub fn capture_runtime_spec(
        &self,
        extension_id: &str,
        adapter_ids: &[Box<str>],
    ) -> Result<DesktopRuntimeSpec, BackendError> {
        if adapter_ids.is_empty() {
            return Err(BackendError::InvalidInput("capture-adapter-empty"));
        }
        let unique = adapter_ids.iter().collect::<BTreeSet<_>>();
        if unique.len() != adapter_ids.len() {
            return Err(BackendError::InvalidInput("capture-adapter-duplicate"));
        }
        let requirements = adapter_ids
            .iter()
            .map(|adapter_id| {
                let requirement = self
                    .environment
                    .adapter_requirements
                    .get(adapter_id.as_ref())
                    .ok_or_else(|| BackendError::UnknownAdapter(adapter_id.clone()))?;
                if !requirement
                    .features()
                    .any(|feature| feature == Feature::TextObserve)
                {
                    return Err(BackendError::InvalidInput("capture-adapter-cannot-observe"));
                }
                let mut features = vec![Feature::TextObserve];
                if requirement
                    .features()
                    .any(|feature| feature == Feature::TextReplace)
                {
                    features.push(Feature::TextReplace);
                }
                Ok(AdapterRequirement::new(
                    requirement.adapter_id().clone(),
                    requirement.version_requirement(),
                    features,
                ))
            })
            .collect::<Result<Vec<_>, BackendError>>()?;
        let mut spec = self.runtime_spec(extension_id)?;
        spec.requirements = requirements;
        Ok(spec)
    }

    pub(super) fn target_requirements(
        &self,
        target: &glyphshift_workflow::CompiledTarget,
    ) -> Result<Vec<AdapterRequirement>, BackendError> {
        target
            .adapter_ids()
            .iter()
            .map(|adapter_id| {
                let requirement = self
                    .environment
                    .adapter_requirements
                    .get(adapter_id)
                    .ok_or(BackendError::InvalidArtifact("adapter-requirement-missing"))?;
                let supported = requirement.features().collect::<BTreeSet<_>>();
                Ok(AdapterRequirement::new(
                    requirement.adapter_id().clone(),
                    requirement.version_requirement(),
                    target
                        .requested_features()
                        .iter()
                        .copied()
                        .filter(|feature| supported.contains(feature)),
                ))
            })
            .collect()
    }
    pub fn add_software(
        &mut self,
        selection: ExecutableSelection,
    ) -> Result<DesktopSnapshot, BackendError> {
        let metadata = fs::metadata(&selection.path)
            .map_err(|_| BackendError::InvalidInput("software-executable"))?;
        let executable_name = selection
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|_| metadata.is_file())
            .filter(|value| {
                Path::new(value)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            })
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let name = Path::new(executable_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let extension_id = next_local_extension_id(&self.software, name);
        let executable_path = selection
            .path
            .to_str()
            .filter(|path| Path::new(path).is_absolute())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let artifact = ExtensionArtifact {
            schema: EXTENSION_SCHEMA.into(),
            id: extension_id.clone(),
            version: "0.1.0".into(),
            name: name.into(),
            vendor: "—".into(),
            executables: vec![executable_name.into()],
            descendant_executables: Vec::new(),
            runtime: None,
            locations: vec![ExtensionLocationArtifact {
                id: "internal-default".into(),
                label: "internal-default".into(),
                context: None,
            }],
        };
        let serialized = serde_json::to_string(&artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-extension"))?;
        let extension_path = self
            .root
            .join("extensions")
            .join(format!("{extension_id}.json"));
        write_atomic(&extension_path, &serialized)?;
        self.local_software.insert(
            extension_id.clone(),
            DesktopSoftwareArtifact {
                display_name: name.into(),
                description: "".into(),
                executable_path: executable_path.into(),
            },
        );
        write_desktop_state(&self.root, Some(&extension_id), &self.local_software)?;

        self.software.insert(
            extension_id.clone(),
            SoftwareState {
                artifact,
                locale: DEFAULT_LOCALE.into(),
            },
        );
        self.selected_software_id = Some(extension_id);
        Ok(self.snapshot())
    }

    pub fn validate_software_edit(&self, edit: &SoftwareEdit) -> Result<(), BackendError> {
        self.validated_software_edit(edit).map(|_| ())
    }

    pub fn update_software(&mut self, edit: SoftwareEdit) -> Result<DesktopSnapshot, BackendError> {
        let (display_name, description, executable_name, executable_path) =
            self.validated_software_edit(&edit)?;
        let state = self
            .software
            .get_mut(&edit.extension_id)
            .ok_or_else(|| BackendError::UnknownSoftware(edit.extension_id.clone()))?;
        state.artifact.executables = vec![executable_name.into()];
        let serialized = serde_json::to_string(&state.artifact)
            .map_err(|_| BackendError::InvalidArtifact("serialize-extension"))?;
        write_atomic(
            &self
                .root
                .join("extensions")
                .join(format!("{}.json", edit.extension_id)),
            &serialized,
        )?;
        self.local_software.insert(
            edit.extension_id.clone(),
            DesktopSoftwareArtifact {
                display_name: display_name.into(),
                description: description.into(),
                executable_path: executable_path.into(),
            },
        );
        write_desktop_state(
            &self.root,
            self.selected_software_id.as_deref(),
            &self.local_software,
        )?;
        Ok(self.snapshot())
    }

    fn validated_software_edit<'a>(
        &self,
        edit: &'a SoftwareEdit,
    ) -> Result<(&'a str, &'a str, &'a str, &'a str), BackendError> {
        if !self.software.contains_key(&edit.extension_id) {
            return Err(BackendError::UnknownSoftware(edit.extension_id.clone()));
        }
        let display_name = edit.display_name.trim();
        if display_name.is_empty() || display_name.chars().count() > 128 {
            return Err(BackendError::InvalidInput("software-display-name"));
        }
        let description = edit.description.trim();
        if description.chars().count() > 512 {
            return Err(BackendError::InvalidInput("software-description"));
        }
        let metadata = fs::metadata(&edit.executable_path)
            .map_err(|_| BackendError::InvalidInput("software-executable"))?;
        let executable_name = edit
            .executable_path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|_| metadata.is_file())
            .filter(|value| {
                Path::new(value)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            })
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        let executable_path = edit
            .executable_path
            .to_str()
            .filter(|path| Path::new(path).is_absolute())
            .ok_or(BackendError::InvalidInput("software-executable"))?;
        Ok((display_name, description, executable_name, executable_path))
    }

    pub fn select_software(&mut self, extension_id: &str) -> Result<DesktopSnapshot, BackendError> {
        if !self.software.contains_key(extension_id) {
            return Err(BackendError::UnknownSoftware(extension_id.into()));
        }
        write_desktop_state(&self.root, Some(extension_id), &self.local_software)?;
        self.selected_software_id = Some(extension_id.into());
        Ok(self.snapshot())
    }

    pub fn remove_software(&mut self, extension_id: &str) -> Result<DesktopSnapshot, BackendError> {
        if !safe_identifier(extension_id) || !self.software.contains_key(extension_id) {
            return Err(BackendError::UnknownSoftware(extension_id.into()));
        }
        let workflow_ids = self
            .workflows
            .values()
            .filter(|workflow| {
                workflow
                    .targets
                    .iter()
                    .any(|target| target.software_id.as_ref() == extension_id)
            })
            .map(|workflow| workflow.id.clone())
            .collect::<Vec<_>>();
        if !workflow_ids.is_empty() {
            return Err(BackendError::SoftwareReferenced {
                software_id: extension_id.into(),
                workflow_ids,
            });
        }
        let extension_path = self
            .root
            .join("extensions")
            .join(format!("{extension_id}.json"));
        if extension_path.exists() {
            fs::remove_file(&extension_path)
                .map_err(|_| BackendError::Storage("remove-extension"))?;
        }
        self.software.remove(extension_id);
        self.local_software.remove(extension_id);
        if self.selected_software_id.as_deref() == Some(extension_id) {
            self.selected_software_id = self.software.keys().next().cloned();
        }
        write_desktop_state(
            &self.root,
            self.selected_software_id.as_deref(),
            &self.local_software,
        )?;
        Ok(self.snapshot())
    }
}

pub(super) fn validate_extension(
    artifact: &ExtensionArtifact,
    path: &Path,
) -> Result<(), BackendError> {
    if artifact.schema.as_ref() != EXTENSION_SCHEMA
        || !safe_identifier(&artifact.id)
        || artifact.name.trim().is_empty()
        || artifact.version.trim().is_empty()
        || path.file_stem().and_then(|value| value.to_str()) != Some(&artifact.id)
        || artifact.executables.iter().any(|executable| {
            executable.trim().is_empty()
                || Path::new(executable.as_ref())
                    .file_name()
                    .and_then(|value| value.to_str())
                    != Some(executable)
        })
        || artifact.descendant_executables.iter().any(|executable| {
            executable.trim().is_empty()
                || Path::new(executable.as_ref())
                    .file_name()
                    .and_then(|value| value.to_str())
                    != Some(executable)
        })
        || artifact
            .locations
            .iter()
            .any(|location| !safe_identifier(&location.id) || location.label.trim().is_empty())
    {
        return Err(BackendError::InvalidArtifact("extension-contract"));
    }
    Ok(())
}

pub(super) fn runtime_requirement(
    artifact: &RuntimeCapabilityArtifact,
) -> Result<AdapterRequirement, BackendError> {
    if !safe_identifier(&artifact.adapter) || artifact.features.is_empty() {
        return Err(BackendError::InvalidArtifact("runtime-capability"));
    }
    let features = artifact
        .features
        .iter()
        .map(|feature| match feature.as_ref() {
            "text_observe" => Ok(Feature::TextObserve),
            "text_replace" => Ok(Feature::TextReplace),
            "font_substitute" => Ok(Feature::FontSubstitute),
            "layout_adjust" => Ok(Feature::LayoutAdjust),
            "resource_replace" => Ok(Feature::ResourceReplace),
            _ => Err(BackendError::InvalidArtifact("runtime-feature")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdapterRequirement::new(
        AdapterId::new(artifact.adapter.clone()),
        AdapterVersionRequirement::Exact(AdapterVersion::new(
            artifact.version[0],
            artifact.version[1],
            artifact.version[2],
        )),
        features,
    ))
}

pub(super) fn runtime_route(
    route: &RuntimeRouteArtifact,
    locations: &[ExtensionLocationArtifact],
) -> Result<RouteProgram, BackendError> {
    if route.locations.is_empty()
        || route.locations.iter().any(|route_location| {
            !locations
                .iter()
                .any(|location| location.id == *route_location)
        })
    {
        return Err(BackendError::InvalidArtifact("runtime-route"));
    }
    match route.kind.as_ref() {
        "direct" if route.locations.len() == 1 => {
            Ok(RouteProgram::direct(route.locations[0].clone()))
        }
        "fallback" => Ok(RouteProgram::new(
            [RouteOperator::fallback(route.locations.iter().cloned())],
            RouteLimits::new(32, 64),
        )),
        _ => Err(BackendError::InvalidArtifact("runtime-route")),
    }
}

pub(super) fn software_view(
    state: &SoftwareState,
    local_software: Option<&DesktopSoftwareArtifact>,
) -> SoftwareView {
    let display_name = local_software
        .map(|software| software.display_name.clone())
        .unwrap_or_else(|| state.artifact.name.clone());
    let monogram = display_name
        .chars()
        .take(2)
        .collect::<String>()
        .to_uppercase()
        .into_boxed_str();
    SoftwareView {
        id: state.artifact.id.clone(),
        name: display_name,
        description: local_software
            .map(|software| software.description.clone())
            .unwrap_or_default(),
        vendor: state.artifact.vendor.clone(),
        version: "—".into(),
        executable_name: state
            .artifact
            .executables
            .first()
            .cloned()
            .unwrap_or_default(),
        executable_path: local_software.map(|software| software.executable_path.clone()),
        monogram,
        last_used: None,
        locale: state.locale.clone(),
        connected: false,
        translation: CapabilityView::unavailable("capability.text-unavailable"),
        font: CapabilityView::unavailable("capability.font-unavailable"),
        observe: CapabilityView::pending_observation(),
    }
}
fn next_local_extension_id(software: &BTreeMap<Box<str>, SoftwareState>, name: &str) -> Box<str> {
    let mut slug = String::new();
    let mut pending_separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("software");
    }
    let base = format!("local.{slug}");
    if !software.contains_key(base.as_str()) {
        return base.into();
    }
    for suffix in 2_u32.. {
        let candidate = format!("{base}.{suffix}");
        if !software.contains_key(candidate.as_str()) {
            return candidate.into();
        }
    }
    unreachable!("an unbounded numeric suffix always has another candidate")
}

pub(super) fn read_desktop_state(root: &Path) -> Result<DesktopStateArtifact, BackendError> {
    let path = root.join("desktop-state.json");
    if !path.exists() {
        return Ok(DesktopStateArtifact {
            schema: DESKTOP_STATE_SCHEMA.into(),
            selected_software_id: None,
            software: BTreeMap::new(),
        });
    }
    let source =
        fs::read_to_string(&path).map_err(|_| BackendError::Storage("read-desktop-state"))?;
    let value = match serde_json::from_str::<serde_json::Value>(&source) {
        Ok(value) => value,
        Err(_) => {
            let backup = path.with_extension("invalid.json");
            if !backup.exists() {
                let _ = fs::copy(&path, backup);
            }
            serde_json::Value::Null
        }
    };
    Ok(desktop_state_from_value(&value))
}

fn desktop_state_from_value(value: &serde_json::Value) -> DesktopStateArtifact {
    let Some(object) = value.as_object() else {
        return DesktopStateArtifact {
            schema: DESKTOP_STATE_SCHEMA.into(),
            selected_software_id: None,
            software: BTreeMap::new(),
        };
    };
    let software = object
        .get("software")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(extension_id, value)| {
            if !safe_identifier(extension_id) {
                return None;
            }
            let record = value.as_object()?;
            let executable_path = record.get("executablePath")?.as_str()?;
            if !Path::new(executable_path).is_absolute() {
                return None;
            }
            let display_name = record
                .get("displayName")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty() && name.chars().count() <= 128)
                .unwrap_or(extension_id);
            let description = record
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            Some((
                Box::<str>::from(extension_id.as_str()),
                DesktopSoftwareArtifact {
                    display_name: display_name.into(),
                    description: description.into(),
                    executable_path: executable_path.into(),
                },
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let selected_software_id = object
        .get("selectedSoftwareId")
        .and_then(serde_json::Value::as_str)
        .filter(|selected| software.contains_key(*selected))
        .map(Into::into);
    DesktopStateArtifact {
        schema: DESKTOP_STATE_SCHEMA.into(),
        selected_software_id,
        software,
    }
}

fn write_desktop_state(
    root: &Path,
    selected_software_id: Option<&str>,
    software: &BTreeMap<Box<str>, DesktopSoftwareArtifact>,
) -> Result<(), BackendError> {
    let state = DesktopStateArtifact {
        schema: DESKTOP_STATE_SCHEMA.into(),
        selected_software_id: selected_software_id.map(Into::into),
        software: software.clone(),
    };
    let serialized = serde_json::to_string(&state)
        .map_err(|_| BackendError::InvalidArtifact("serialize-desktop-state"))?;
    write_atomic(&root.join("desktop-state.json"), &serialized)
}
