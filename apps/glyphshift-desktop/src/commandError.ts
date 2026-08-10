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
  'acquisition.invalid_request': 'errors.acquisition.invalidRequest',
  'acquisition.state_unavailable': 'errors.acquisition.stateUnavailable',
  'acquisition.request_in_progress': 'errors.acquisition.requestInProgress',
  'acquisition.request_not_found': 'errors.acquisition.requestNotFound',
  'acquisition.execution_failed': 'errors.acquisition.executionFailed',
  'acquisition.software_not_found': 'errors.acquisition.softwareNotFound',
  'acquisition.runtime_spec_unavailable': 'errors.acquisition.runtimeSpecUnavailable',
  'acquisition.target_not_found': 'errors.acquisition.targetNotFound',
  'acquisition.invalid_state': 'errors.acquisition.invalidState',
  'acquisition.controller_unavailable': 'errors.acquisition.controllerUnavailable',
  'acquisition.target_unavailable': 'errors.acquisition.targetUnavailable',
  'acquisition.adapter_unavailable': 'errors.acquisition.adapterUnavailable',
  'acquisition.permission_denied': 'errors.acquisition.permissionDenied',
  'acquisition.no_text': 'errors.acquisition.noText',
  'acquisition.provider_unavailable': 'errors.acquisition.providerUnavailable',
  'acquisition.timed_out': 'errors.acquisition.timedOut',
  'acquisition.cancelled': 'errors.acquisition.cancelled',
  'interactive_translation.invalid_request': 'errors.interactiveTranslation.invalidRequest',
  'interactive_translation.shortcut_unavailable': 'errors.interactiveTranslation.shortcutUnavailable',
  'interactive_translation.request_in_progress': 'errors.interactiveTranslation.requestInProgress',
  'interactive_translation.ocr_not_eligible': 'errors.interactiveTranslation.ocrNotEligible',
  'interactive_translation.ocr_unavailable': 'errors.interactiveTranslation.ocrUnavailable',
  'interactive_translation.state_unavailable': 'errors.interactiveTranslation.stateUnavailable',
  'interactive_translation.software_not_found': 'errors.interactiveTranslation.softwareNotFound',
  'interactive_translation.software_not_configured': 'errors.interactiveTranslation.softwareNotConfigured',
  'interactive_translation.dictionary_not_found': 'errors.interactiveTranslation.dictionaryNotFound',
  'interactive_translation.dictionary_invalid': 'errors.interactiveTranslation.dictionaryInvalid',
  'interactive_translation.runtime_spec_unavailable': 'errors.interactiveTranslation.runtimeSpecUnavailable',
  'interactive_translation.foreground_mismatch': 'errors.interactiveTranslation.foregroundMismatch',
  'interactive_translation.foreground_unavailable': 'errors.interactiveTranslation.foregroundUnavailable',
  'interactive_translation.cancelled': 'errors.interactiveTranslation.cancelled',
  'interactive_translation.acquisition_failed': 'errors.interactiveTranslation.acquisitionFailed',
  'interactive_translation.provider_timed_out': 'errors.interactiveTranslation.providerTimedOut',
  'interactive_translation.translation_unavailable': 'errors.interactiveTranslation.translationUnavailable',
  'interactive_translation.presentation_failed': 'errors.interactiveTranslation.presentationFailed',
  'interactive_translation.execution_failed': 'errors.interactiveTranslation.executionFailed',
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
  return i18n.global.t(key, error.args)
}
