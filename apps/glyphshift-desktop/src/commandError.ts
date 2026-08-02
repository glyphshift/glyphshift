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
  'workspace.unavailable': 'errors.workspaceUnavailable',
  'storage.write_failed': 'errors.storageWriteFailed',
  'dictionary.not_found': 'errors.dictionary.notFound',
  'dictionary.invalid_create': 'errors.dictionary.invalidCreate',
  'dictionary.invalid_update': 'errors.dictionary.invalidUpdate',
  'dictionary.referenced': 'errors.dictionary.referenced',
  'font_profile.not_found': 'errors.fontProfile.notFound',
  'font_profile.invalid_create': 'errors.fontProfile.invalidCreate',
  'font_profile.invalid_update': 'errors.fontProfile.invalidUpdate',
  'font_profile.referenced': 'errors.fontProfile.referenced',
  'workflow.no_effective_rules': 'errors.workflow.noEffectiveRules',
  'workflow.empty_adapter_plan': 'errors.workflow.emptyAdapterPlan',
  'workflow.locale_mismatch': 'errors.workflow.localeMismatch',
  'workflow.unknown_location': 'errors.workflow.unknownLocation',
  'workflow.unknown_software': 'errors.workflow.unknownSoftware',
  'workflow.unknown_dictionary': 'errors.workflow.unknownDictionary',
  'workflow.unknown_font_profile': 'errors.workflow.unknownFontProfile',
  'workflow.unknown_adapter': 'errors.workflow.unknownAdapter',
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
  'software.select_failed': 'errors.software.selectFailed',
  'software.runtime_stop_unconfirmed': 'errors.software.runtimeStopUnconfirmed',
  'software.delete_failed': 'errors.software.deleteFailed',
  'software.invalid_update': 'errors.software.invalidUpdate',
  'runtime.target_not_found': 'errors.runtime.targetNotFound',
  'runtime.session_rejected': 'errors.runtime.sessionRejected',
  'runtime.bundle_unavailable': 'errors.runtime.bundleUnavailable',
  'runtime.activation_failed': 'errors.runtime.activationFailed',
  'runtime.stop_unconfirmed': 'errors.runtime.stopUnconfirmed',
  'runtime.unavailable': 'errors.runtime.unavailable',
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

export function translateCommandError(error: unknown): string {
  if (isPresentationError(error)) return error.presentationMessage
  if (!isCommandError(error)) return i18n.global.t('errors.unknown')
  const key = messageKeys[error.code] ?? 'errors.unknown'
  return i18n.global.t(key, error.args)
}
