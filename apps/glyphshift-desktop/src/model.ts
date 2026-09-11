export type CapabilityState = 'active' | 'ready' | 'candidate' | 'limited' | 'unavailable'
export interface CapabilityEvidence {
  state: CapabilityState
  enabled: boolean
  coverage: number
  detail: string
  generation: number | null
}

export interface SoftwareRecord {
  id: string
  name: string
  description: string
  vendor: string
  version: string
  executableName: string
  executablePath: string | null
  monogram: string
  lastUsed: string | null
  locale: string
  connected: boolean
  translation: CapabilityEvidence
  font: CapabilityEvidence
  observe: CapabilityEvidence
}

export type SoftwarePreflightState
  = 'ready'
    | 'already_added'
    | 'not_running'
    | 'runtime_unavailable'
    | 'self_target'
    | 'unsupported_architecture'

export interface SoftwarePreflight {
  executablePath: string
  executableName: string
  suggestedName: string
  architecture: string
  running: boolean
  canAdd: boolean
  state: SoftwarePreflightState
  existingName: string | null
}

export type SoftwareQuickCaptureEvent
  = { state: 'armed'; shortcut: string }
    | { state: 'captured'; shortcut: string; preflight: SoftwarePreflight }
    | { state: 'failed'; shortcut: string; errorCode: string }

export interface DictionaryMetadata {
  textRules?: import('./regexRules').RegexRule[]
  fontFamilies?: string[]
  fontScalePercent?: number | null
  id: string
  releaseVersion: string
  name: string
  description: string
  sourceLocale: string
  targetLocale: string
  authors: string[]
  license: string | null
  homepage: string | null
  tags: string[]
}

export interface DictionarySummary {
  metadata: DictionaryMetadata
  revision: number
  entryCount: number
  installation: DictionaryInstallationSummary
}

export type DictionaryInstallationState = 'verified' | 'modified' | 'missing' | 'unmanaged'

export interface DictionaryInstallationSummary {
  state: DictionaryInstallationState
  installedRelease: string | null
  verifiedPublisher: string | null
  updateRelease: string | null
}

export interface DictionaryCatalogRelease {
  catalogId: string
  dictionaryId: string
  releaseVersion: string
  sourceLocale: string
  targetLocale: string
  effectivePresentationLocale: string
  name: string
  summary: string
  tags: string[]
  publisherIdentity: string
}

export interface DictionaryCatalogPage {
  releases: DictionaryCatalogRelease[]
  nextCursor: string | null
}

export interface DictionaryCatalogQueryRequest {
  text: string
  sourceLocale: string | null
  targetLocale: string | null
  tag: string | null
  cursor: string | null
  pageSize: number
  requestedPresentationLocale: string
}

export interface DictionaryCatalogInstallRequest {
  catalogId: string
  dictionaryId: string
  releaseVersion: string
  replacement: 'reject_existing' | 'replace_verified' | 'replace_any'
}

export interface DictionaryEntry {
  source: string
  translation: string
}

export interface DictionaryDetail {
  metadata: DictionaryMetadata
  revision: number
  entries: DictionaryEntry[]
}

export interface AdapterOption {
  id: string
  name: string
  version: string
  summary: string
  platforms: string[]
  technologies: string[]
  features: string[]
  technicalTarget: string
  documentationUrl: string | null
  configuration: 'none'
  processResidentAfterDeactivate: boolean
}

export type ProbeRunStatus = 'ready' | 'running' | 'paused' | 'interrupted'
export type ProbeRuntimeCapability = 'direct_replace' | 'collection_only' | 'no_signal'

export interface ProbeRunSummary {
  workflowRuntime?: WorkflowRuntimeStatus | null
  workflowId?: string | null
  excludedDictionaryIds?: string[]
  exclusionRevisions?: number[]
  id: string
  name: string
  softwareId: string
  dictionaryId: string
  adapterIds: string[]
  status: ProbeRunStatus
  livePreviewEnabled: boolean
  observationRevision: number
  observedCount: number
  ignoredCount: number
  droppedObservations: number
  previewGeneration: number
  createdAtMs: number
  updatedAtMs: number
  dictionaryRevision: number
  dictionaryEntryCount: number
  runtimeCapability: ProbeRuntimeCapability | null
  quickProbe: boolean
}

export interface ProbeEntryResolution {
  skipReason?: string | null
  kind: 'dictionary' | 'dictionary_pending' | 'rule_translated' | 'rule_pending' | 'rule_skipped' | 'filtered' | 'ignored' | 'conflict' | 'pending'
  dictionaryIds: string[]
  ruleIndex: number | null
  editable: boolean
  editSource?: string | null
  editTranslation?: string | null
}

export interface ProbeEntryRow {
  resolution?: ProbeEntryResolution
  source: string
  translation: string
  state: 'pending' | 'translated' | 'unobserved' | 'ignored'
  adapterIds: string[]
  count: number
  firstSeenMs: number
  lastSeenMs: number
  translationVariants?: { source: string; translation: string }[]
}

export interface ProbeEntryPage {
  observationRevision: number
  dictionaryRevision: number
  page: number
  pageSize: number
  total: number
  rows: ProbeEntryRow[]
}

export type ProbeExportFormat
  = 'entries_json'
    | 'observations_json'
    | 'observations_csv'
    | 'entries_csv'
    | 'dictionary_json'

export interface WorkflowAdapterPlan {
  strategy: 'parallel'
  adapterIds: string[]
}

export type FontCoverage = 'dictionary_matches' | 'all_observations'

export interface WorkflowDictionaryFont {
  families: string[]
  scalePercent?: number | null
}
export interface WorkflowFontPolicy {
  dictionaryOverrides?: Record<string, WorkflowDictionaryFont>
  scalePercent?: number
  families: string[]
  coverage: FontCoverage
}

export interface WorkflowTarget {
  collectNewSources?: boolean | null
  softwareId: string
  adapterPlan: WorkflowAdapterPlan
  dictionaryIds: string[]
  writeDictionaryId?: string | null
  fontPolicy: WorkflowFontPolicy | null
}

export interface WorkflowSummary {
  globalShortcut?: string
  id: string
  name: string
  description: string
  revision: number
  softwareIds: string[]
  dictionaryIds: string[]
  targets: WorkflowTarget[]
}

export interface WorkflowDetail {
  globalShortcut?: string
  id: string
  name: string
  description: string
  revision: number
  targets: WorkflowTarget[]
}

export interface WorkflowActivation {
  workflowId: string
  revision: number
}

export interface WorkflowTargetRuntime {
  softwareId: string
  discovered: boolean
  active: boolean
  translationRequested: boolean
  fontRequested: boolean
  translationActive: boolean
  fontActive: boolean
  appliedGeneration: number | null
}

export interface WorkflowLifecycle {
  phase: "stopped" | "waiting" | "connecting" | "running" | "stopping" | "failed" | "stop_failed" | "unknown"
  enabled: boolean
  collectNewSources: boolean
  checkedAtMs: number
  revision: number
}

export interface WorkflowRuntimeStatus {
  lifecycle?: WorkflowLifecycle | null
  checkedAtMs?: number
  revision?: number
  workflowId: string
  targets: WorkflowTargetRuntime[]
  errors: Record<string, CommandError>
}

export type RuntimeTraceStatus
  = 'no_match'
    | 'matched'
    | 'context_recorded'
    | 'invalid_observation'
    | 'invalid_route_program'
    | 'execution_limit_exceeded'
    | 'state_limit_exceeded'

export interface WorkflowRuntimeTrace {
  softwareId: string
  softwareName: string
  adapterName: string
  sourceText: string
  status: RuntimeTraceStatus
  text: 'unmatched' | 'replaced'
  font: 'unmatched' | 'protected' | 'substituted'
  generation: number
  publicationIdentity: string
  translationDigest: string
  fontPolicyDigest: string
}

export interface WorkflowRuntimeDiagnostics {
  workflowId: string
  records: WorkflowRuntimeTrace[]
  dropped: number
}

export interface ArtifactWarning {
  artifactKind: 'dictionary' | 'workflow'
  artifactId: string
  issue: 'invalid_runtime' | 'invalid' | 'unreadable' | 'invalid_identity' | 'migration_write_failed' | 'duplicate_identity' | 'disabled_invalid_dependency'
}

export interface DesktopSnapshot {
  selectedSoftwareId: string | null
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  workflows: WorkflowSummary[]
  activations: WorkflowActivation[]
  workflowRuntimeStatus: Record<string, WorkflowRuntimeStatus>
  adapters: AdapterOption[]
  fontFamilies: string[]
  artifactWarnings: ArtifactWarning[]
}

export interface WorkflowCommandResult {
  definition: WorkflowDetail
  activation: { workflowId: string; enabled: boolean }
  runtime: WorkflowRuntimeStatus
}

export interface DesktopModel extends DesktopSnapshot {
  dictionaryDetails: Record<string, DictionaryDetail>
  workflowDetails: Record<string, WorkflowDetail>
}

export const STORAGE_KEY = 'glyphshift.composable-product-model.v3'

export function emptyModel(): DesktopModel {
  return {
    selectedSoftwareId: null,
    software: [],
    dictionaries: [],
    workflows: [],
    activations: [],
    workflowRuntimeStatus: {},
    adapters: [],
    fontFamilies: [],
    artifactWarnings: [],
    dictionaryDetails: {},
    workflowDetails: {},
  }
}
import type { CommandError } from './commandError'
