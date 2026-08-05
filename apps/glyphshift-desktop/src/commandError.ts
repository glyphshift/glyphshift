import { i18n } from './i18n'

export type CommandErrorArg = string | number | boolean

export interface CommandError {
  schemaVersion: 1
  code: string
  args: Record<string, CommandErrorArg>
  diagnosticId?: string
}

interface PresentationError {
  presentationMessage: string
}

const messageKeys: Record<string, string> = {
  'settings.invalid_data': 'errors.settings.invalidData',
  'settings.unavailable': 'errors.settings.unavailable',
  'settings.write_failed': 'errors.settings.writeFailed',
  'settings.startup_failed': 'errors.settings.startupFailed',
  'settings.privilege_unavailable': 'errors.settings.privilegeUnavailable',
  'settings.elevation_failed': 'errors.settings.elevationFailed',
  'workspace.unavailable': 'errors.workspaceUnavailable',
  'storage.write_failed': 'errors.storageWriteFailed',
  'font.cache_write_failed': 'errors.fontCacheWriteFailed',
  'dictionary.not_found': 'errors.dictionary.notFound',
  'dictionary.invalid_create': 'errors.dictionary.invalidCreate',
  'dictionary.invalid_update': 'errors.dictionary.invalidUpdate',
  'dictionary.referenced': 'errors.dictionary.referenced',
  'dictionary.import_duplicate': 'errors.dictionary.importDuplicate',
  'dictionary.import_invalid': 'errors.dictionary.importInvalid',
  'dictionary.import_failed': 'errors.dictionary.importFailed',
  'dictionary.export_failed': 'errors.dictionary.exportFailed',
  'dictionary.catalog_invalid': 'errors.dictionary.catalogInvalid',
  'dictionary.catalog_unavailable': 'errors.dictionary.catalogUnavailable',
  'dictionary.release_missing': 'errors.dictionary.releaseMissing',
  'dictionary.artifact_too_large': 'errors.dictionary.artifactTooLarge',
  'dictionary.artifact_size_mismatch': 'errors.dictionary.artifactSizeMismatch',
  'dictionary.artifact_digest_mismatch': 'errors.dictionary.artifactDigestMismatch',
  'dictionary.signature_invalid': 'errors.dictionary.signatureInvalid',
  'dictionary.publisher_untrusted': 'errors.dictionary.publisherUntrusted',
  'dictionary.publisher_identity_mismatch': 'errors.dictionary.publisherIdentityMismatch',
  'dictionary.trust_unavailable': 'errors.dictionary.trustUnavailable',
  'dictionary.payload_invalid': 'errors.dictionary.payloadInvalid',
  'dictionary.release_identity_mismatch': 'errors.dictionary.releaseIdentityMismatch',
  'dictionary.local_changes_conflict': 'errors.dictionary.localChangesConflict',
  'dictionary.installation_storage_failure': 'errors.dictionary.installationStorageFailure',
  'workflow.no_effective_rules': 'errors.workflow.noEffectiveRules',
  'workflow.empty_adapter_plan': 'errors.workflow.emptyAdapterPlan',
  'workflow.locale_mismatch': 'errors.workflow.localeMismatch',
  'workflow.unknown_software': 'errors.workflow.unknownSoftware',
  'workflow.unknown_dictionary': 'errors.workflow.unknownDictionary',
  'workflow.unknown_adapter': 'errors.workflow.unknownAdapter',
  'workflow.empty_font_families': 'errors.workflow.emptyFontFamilies',
  'workflow.font_unavailable': 'errors.workflow.fontUnavailable',
  'workflow.feature_unavailable': 'errors.workflow.featureUnavailable',
  'workflow.software_occupied': 'errors.workflow.softwareOccupied',
  'workflow.not_found': 'errors.workflow.notFound',
  'workflow.invalid_create': 'errors.workflow.invalidCreate',
  'workflow.invalid_update': 'errors.workflow.invalidUpdate',
  'workflow.invalid_copy': 'errors.workflow.invalidCopy',
  'workflow.enabled_delete': 'errors.workflow.enabledDelete',
  'workflow.reconcile_invalid': 'errors.workflow.reconcileInvalid',
  'workflow.refresh_invalid': 'errors.workflow.refreshInvalid',
  'workflow.disable_invalid': 'errors.workflow.disableInvalid',
  'workflow.disable_failed': 'errors.workflow.disableFailed',
  'workflow.invalid': 'errors.workflow.invalid',
  'software.invalid_executable': 'errors.software.invalidExecutable',
  'software.preflight_required': 'errors.software.preflightRequired',
  'software.already_added': 'errors.software.alreadyAdded',
  'software.not_running': 'errors.software.notRunning',
  'software.runtime_unavailable': 'errors.software.runtimeUnavailable',
  'software.self_target': 'errors.software.selfTarget',
  'software.unsupported_architecture': 'errors.software.unsupportedArchitecture',
  'software.quick_capture_unavailable': 'errors.software.quickCaptureUnavailable',
  'software.quick_capture_foreground_unavailable': 'errors.software.quickCaptureForegroundUnavailable',
  'software.quick_capture_self': 'errors.software.quickCaptureSelf',
  'software.quick_capture_failed': 'errors.software.quickCaptureFailed',
  'software.select_failed': 'errors.software.selectFailed',
  'software.runtime_stop_unconfirmed': 'errors.software.runtimeStopUnconfirmed',
  'software.referenced': 'errors.software.referencedByWorkflowAndProbe',
  'software.delete_failed': 'errors.software.deleteFailed',
  'software.invalid_update': 'errors.software.invalidUpdate',
  'runtime.target_not_found': 'errors.runtime.targetNotFound',
  'runtime.session_rejected': 'errors.runtime.sessionRejected',
  'runtime.bundle_unavailable': 'errors.runtime.bundleUnavailable',
  'runtime.target_access_failed': 'errors.runtime.targetAccessFailed',
  'runtime.component_load_failed': 'errors.runtime.componentLoadFailed',
  'runtime.component_incompatible': 'errors.runtime.componentIncompatible',
  'runtime.activation_timed_out': 'errors.runtime.activationTimedOut',
  'runtime.activation_failed': 'errors.runtime.activationFailed',
  'runtime.stop_unconfirmed': 'errors.runtime.stopUnconfirmed',
  'runtime.unavailable': 'errors.runtime.unavailable',
  'runtime.diagnostics_inactive': 'errors.runtime.diagnosticsInactive',
  'runtime.diagnostics_unavailable': 'errors.runtime.diagnosticsUnavailable',
  'runtime.diagnostics_failed': 'errors.runtime.diagnosticsFailed',
  'capture.already_active': 'errors.capture.alreadyActive',
  'capture.not_active': 'errors.capture.notActive',
  'capture.not_completed': 'errors.capture.notCompleted',
  'capture.unknown_software': 'errors.capture.unknownSoftware',
  'capture.unknown_adapter': 'errors.capture.unknownAdapter',
  'capture.adapters_required': 'errors.capture.adaptersRequired',
  'capture.adapter_cannot_observe': 'errors.capture.adapterCannotObserve',
  'capture.invalid_configuration': 'errors.capture.invalidConfiguration',
  'capture.read_failed': 'errors.capture.readFailed',
  'capture.write_failed': 'errors.capture.writeFailed',
  'capture.workspace_not_found': 'errors.capture.workspaceNotFound',
  'capture.workspace_exists': 'errors.capture.workspaceExists',
  'capture.invalid_state': 'errors.capture.invalidState',
  'capture.invalid_workspace': 'errors.capture.invalidWorkspace',
  'capture.preview_unavailable': 'errors.capture.previewUnavailable',
  'capture.preview_publish_failed': 'errors.capture.previewPublishFailed',
  'capture.export_failed': 'errors.capture.exportFailed',
}

function isCommandError(value: unknown): value is CommandError {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Partial<CommandError>
  return candidate.schemaVersion === 1
    && typeof candidate.code === 'string'
    && Boolean(candidate.args)
    && typeof candidate.args === 'object'
}

function isPresentationError(value: unknown): value is PresentationError {
  return Boolean(value)
    && typeof value === 'object'
    && typeof (value as Partial<PresentationError>).presentationMessage === 'string'
}

export function presentationError(message: string): PresentationError {
  return { presentationMessage: message }
}

export function translateCommandCode(code: string, args: Record<string, CommandErrorArg> = {}): string {
  return translateCommandError({ schemaVersion: 1, code, args })
}

export function translateCommandError(error: unknown): string {
  if (isPresentationError(error)) return error.presentationMessage
  if (!isCommandError(error)) return i18n.global.t('errors.unknown')
  let key = messageKeys[error.code] ?? 'errors.unknown'
  if (error.code === 'software.referenced') {
    const workflowCount = Number(error.args.workflowCount ?? 0)
    const probeCount = Number(error.args.probeCount ?? 0)
    if (workflowCount > 0 && probeCount === 0) key = 'errors.software.referencedByWorkflow'
    else if (probeCount > 0 && workflowCount === 0) key = 'errors.software.referencedByProbe'
  }
  return i18n.global.t(key, error.args)
}
