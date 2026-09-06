import { i18n } from './i18n'

export type CommandErrorArg = string | number | boolean | string[]

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
  'workflow.shortcut_conflict': 'errors.workflow.shortcutConflict',
  'settings.invalid_data': 'errors.settings.invalidData',
  'settings.unavailable': 'errors.settings.unavailable',
  'settings.write_failed': 'errors.settings.writeFailed',
  'settings.startup_failed': 'errors.settings.startupFailed',
  'settings.privilege_unavailable': 'errors.settings.privilegeUnavailable',
  'settings.elevation_failed': 'errors.settings.elevationFailed',
  'settings.shortcut_invalid': 'errors.settings.shortcutInvalid',
  'settings.shortcut_busy': 'errors.settings.shortcutBusy',
  'settings.shortcut_unavailable': 'errors.settings.shortcutUnavailable',
  'settings.shortcut_update_failed': 'errors.settings.shortcutUpdateFailed',
  'workspace.unavailable': 'errors.workspaceUnavailable',
  'storage.write_failed': 'errors.storageWriteFailed',
  'font.cache_write_failed': 'errors.fontCacheWriteFailed',
  'dictionary.not_found': 'errors.dictionary.notFound',
  'dictionary.invalid_create': 'errors.dictionary.invalidCreate',
  'dictionary.invalid_update': 'errors.dictionary.invalidUpdate',
  'dictionary.ai_translation_locked': 'errors.ai.taskAlreadyActive',
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
  'software.running_targets_unavailable': 'errors.software.runningTargetsUnavailable',
  'software.quick_capture_unavailable': 'errors.software.quickCaptureUnavailable',
  'software.quick_capture_foreground_unavailable': 'errors.software.quickCaptureForegroundUnavailable',
  'software.quick_capture_self': 'errors.software.quickCaptureSelf',
  'software.quick_capture_failed': 'errors.software.quickCaptureFailed',
  'software.select_failed': 'errors.software.selectFailed',
  'software.not_found': 'errors.software.notFound',
  'software.executable_missing': 'errors.software.executableMissing',
  'software.launch_failed': 'errors.software.launchFailed',
  'software.runtime_stop_unconfirmed': 'errors.software.runtimeStopUnconfirmed',
  'software.referenced': 'errors.software.referencedByWorkflowAndProbe',
  'software.delete_failed': 'errors.software.deleteFailed',
  'software.invalid_update': 'errors.software.invalidUpdate',
  'runtime.target_not_found': 'errors.runtime.targetNotFound',
  'runtime.session_rejected': 'errors.runtime.sessionRejected',
  'runtime.bundle_unavailable': 'errors.runtime.bundleUnavailable',
  'runtime.bundle_incompatible': 'errors.runtime.bundleIncompatible',
  'runtime.target_access_failed': 'errors.runtime.targetAccessFailed',
  'runtime.component_load_failed': 'errors.runtime.componentLoadFailed',
  'runtime.component_incompatible': 'errors.runtime.componentIncompatible',
  'runtime.target_restart_required': 'errors.runtime.targetRestartRequired',
  'runtime.activation_timed_out': 'errors.runtime.activationTimedOut',
  'runtime.activation_failed': 'errors.runtime.activationFailed',
  'runtime.target_in_use_by_probe': 'errors.runtime.targetInUseByProbe',
  'runtime.target_in_use_by_workflow': 'errors.runtime.targetInUseByWorkflow',
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
  'capture.target_in_use_by_workflow': 'errors.capture.targetInUseByWorkflow',
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
  'quick_probe.storage_failed': 'errors.quickProbe.storageFailed',
  'quick_probe.invalid_ledger': 'errors.quickProbe.invalidLedger',
  'quick_probe.invalid_locale': 'errors.quickProbe.invalidLocale',
  'quick_probe.start_failed': 'errors.quickProbe.startFailed',
  'quick_probe.target_stopped': 'errors.quickProbe.targetStopped',
  'quick_probe.not_found': 'errors.quickProbe.notFound',
  'quick_probe.cleanup_failed': 'errors.quickProbe.cleanupFailed',
  'ai.profile_storage_failed': 'errors.ai.profileStorageFailed',
  'ai.profile_invalid': 'errors.ai.profileInvalid',
  'ai.profile_not_found': 'errors.ai.profileNotFound',
  'ai.profile_required': 'errors.ai.profileRequired',
  'ai.credential_missing': 'errors.ai.credentialMissing',
  'ai.credential_unavailable': 'errors.ai.credentialUnavailable',
  'ai.credential_rejected': 'errors.ai.credentialRejected',
  'ai.filter_pattern_invalid': 'errors.ai.filterPatternInvalid',
  'ai.plan_not_found': 'errors.ai.planNotFound',
  'ai.provider_unavailable': 'errors.ai.providerUnavailable',
  'ai.task_already_active': 'errors.ai.taskAlreadyActive',
  'ai.task_scope_invalid': 'errors.ai.taskScopeInvalid',
  'ai.plan_stale': 'errors.ai.planStale',
  'ai.job_not_found': 'errors.ai.jobNotFound',
  'ai.job_state_unavailable': 'errors.ai.jobStateUnavailable',
  'ai.state_unavailable': 'errors.ai.stateUnavailable',
  'ai.writeback_revision_invalid': 'errors.ai.writebackRevisionInvalid',
  'ai.writeback_invalid': 'errors.ai.writebackInvalid',
  'ai.writeback_conflict': 'errors.ai.writebackConflict',
  'ai.writeback_failed': 'errors.ai.writebackFailed',
}

export function isCommandError(value: unknown): value is CommandError {
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
  if (error.code === 'dictionary.referenced') {
    const workflowNames = Array.isArray(error.args.workflowNames) ? error.args.workflowNames : []
    const probeNames = Array.isArray(error.args.probeNames) ? error.args.probeNames : []
    if (workflowNames.length > 0 && probeNames.length > 0) key = 'errors.dictionary.referencedByWorkflowAndProbe'
    else if (workflowNames.length > 0) key = 'errors.dictionary.referencedByWorkflow'
    else if (probeNames.length > 0) key = 'errors.dictionary.referencedByProbe'
  }
  if (error.code === 'runtime.target_access_failed') {
    const operation = String(error.args.operation ?? '')
    const controllerElevated = error.args.controllerElevated === true
    if (operation === 'targetProcess') key = 'errors.runtime.targetProcessUnavailable'
    else if (operation === 'observer') {
      key = controllerElevated
        ? 'errors.runtime.observerAccessFailedElevated'
        : 'errors.runtime.observerAccessFailed'
    }
    else if (operation === 'remoteMemory' && controllerElevated) {
      key = 'errors.runtime.targetMemoryAccessFailedElevated'
    }
    else if (operation === 'remoteThread' && controllerElevated) {
      key = 'errors.runtime.targetThreadAccessFailedElevated'
    }
    else if (controllerElevated) key = 'errors.runtime.targetAccessFailedElevated'
  }
  const localizedArgs = Object.fromEntries(Object.entries(error.args).map(([name, value]) => [
    name,
    Array.isArray(value)
      ? new Intl.ListFormat(i18n.global.locale.value, { style: 'long', type: 'conjunction' }).format(value)
      : value,
  ]))
  return i18n.global.t(key, localizedArgs)
}
