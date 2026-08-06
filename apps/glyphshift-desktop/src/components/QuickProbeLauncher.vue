<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import type { SoftwarePreflight, SoftwareRecord } from '../model'
import { translateCommandError } from '../commandError'
import { useProbeRuns } from '../useProbeRuns'
import ManagementFormModal from './ManagementFormModal.vue'

const props = defineProps<{
  open: boolean
  software: SoftwareRecord[]
  captureArmed: boolean
  captureShortcut: string
  captureResult: SoftwarePreflight | null
  captureError: string
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  'arm-capture': []
  'cancel-capture': []
  started: [runId: string]
}>()

const { t, locale } = useI18n()
const probe = useProbeRuns()
const executablePath = ref('')
const targetLocale = ref('zh-CN')
const preflight = ref<SoftwarePreflight | null>(null)
const checking = ref(false)
const advancedOpen = ref(false)
const localError = ref('')

function normalizedPath(path: string) {
  return path.trim().replace(/^\\\\\?\\/, '').replace(/\//g, '\\').toLocaleLowerCase()
}

const activePreflight = computed(() => {
  const result = preflight.value
  return result && normalizedPath(result.executablePath) === normalizedPath(executablePath.value)
    ? result
    : null
})
const existingSoftware = computed(() => props.software.find(software =>
  software.executablePath
  && normalizedPath(software.executablePath) === normalizedPath(executablePath.value)))
const canStart = computed(() => Boolean(
  activePreflight.value?.running
  && ['ready', 'already_added'].includes(activePreflight.value.state)
  && targetLocale.value.trim(),
))
const preflightPresentation = computed(() => {
  const result = activePreflight.value
  if (!result) return null
  if (!result.running) {
    return {
      color: 'warning' as const,
      icon: 'i-tabler-player-pause',
      title: t('capture.quickProbe.preflight.notRunningTitle'),
      description: t('capture.quickProbe.preflight.notRunningDescription'),
    }
  }
  if (result.state === 'ready' || result.state === 'already_added') {
    return {
      color: 'success' as const,
      icon: 'i-tabler-circle-check',
      title: t('capture.quickProbe.preflight.readyTitle', { name: result.suggestedName }),
      description: t(result.state === 'already_added'
        ? 'capture.quickProbe.preflight.reuseDescription'
        : 'capture.quickProbe.preflight.readyDescription', {
        architecture: result.architecture,
        name: result.existingName ?? result.suggestedName,
      }),
    }
  }
  const presentation = {
    runtime_unavailable: ['i-tabler-plug-connected-x', 'runtimeUnavailable'] as const,
    self_target: ['i-tabler-app-window', 'selfTarget'] as const,
    unsupported_architecture: ['i-tabler-cpu-off', 'unsupportedArchitecture'] as const,
    not_running: ['i-tabler-player-pause', 'notRunning'] as const,
  }[result.state]
  if (!presentation) return null
  return {
    color: result.state === 'not_running' ? 'warning' as const : 'error' as const,
    icon: presentation[0],
    title: t(`capture.quickProbe.preflight.${presentation[1]}Title`),
    description: t(`capture.quickProbe.preflight.${presentation[1]}Description`, {
      architecture: result.architecture,
    }),
  }
})

watch(() => props.open, (open) => {
  if (!open) return
  executablePath.value = ''
  targetLocale.value = locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'
  preflight.value = null
  advancedOpen.value = false
  localError.value = ''
  probe.clearMessage()
})

watch(() => props.captureResult, (result, previous) => {
  if (!props.open || !result || result === previous) return
  executablePath.value = result.executablePath
  preflight.value = result
  localError.value = ''
})

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function browserPreflight(path: string): SoftwarePreflight {
  const executableName = path.split(/[\\/]/).pop() || path
  const existing = props.software.find(software => software.executablePath
    && normalizedPath(software.executablePath) === normalizedPath(path))
  return {
    executablePath: path,
    executableName,
    suggestedName: executableName.replace(/\.exe$/i, ''),
    architecture: 'x86_64',
    running: true,
    canAdd: !existing,
    state: existing ? 'already_added' : 'ready',
    existingName: existing?.name ?? null,
  }
}

function updateExecutablePath(value: unknown) {
  executablePath.value = String(value ?? '')
  preflight.value = null
  localError.value = ''
}

async function validateExecutable() {
  const path = executablePath.value.trim()
  if (!path || checking.value) return
  checking.value = true
  localError.value = ''
  probe.clearMessage()
  try {
    preflight.value = hasDesktopRuntime()
      ? await invoke<SoftwarePreflight>('desktop_preflight_software', { executablePath: path })
      : browserPreflight(path)
  }
  catch (error) {
    preflight.value = null
    localError.value = translateCommandError(error)
  }
  finally {
    checking.value = false
  }
}

async function browseExecutable() {
  try {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: t('capture.quickProbe.selectProgram'),
      filters: [{ name: t('software.windowsApp'), extensions: ['exe'] }],
    })
    if (typeof selected !== 'string') return
    updateExecutablePath(selected)
    await validateExecutable()
  }
  catch {
    // Browser previews keep the path field editable; the desktop shell supplies the native picker.
  }
}

function toggleForegroundCapture() {
  if (props.captureArmed) emit('cancel-capture')
  else emit('arm-capture')
}

function setOpen(open: boolean) {
  if (!open && props.captureArmed) emit('cancel-capture')
  emit('update:open', open)
}

async function start() {
  const result = activePreflight.value
  if (!result || !canStart.value) return
  const softwareName = existingSoftware.value?.name ?? result.suggestedName
  const started = await probe.startQuickProbe({
    executablePath: executablePath.value.trim(),
    targetLocale: targetLocale.value.trim(),
  }, {
    softwareId: existingSoftware.value?.id,
    softwareName,
  })
  if (!started) return
  emit('started', started.id)
  setOpen(false)
}
</script>

<template>
  <ManagementFormModal
    :open="open"
    :title="t('capture.quickProbe.title')"
    :description="t('capture.quickProbe.description')"
    :confirm-label="t('capture.quickProbe.start')"
    :confirm-disabled="checking || probe.busy.value || !canStart"
    :busy="probe.busy.value"
    width="lg"
    @update:open="setOpen"
    @confirm="start"
  >
    <div class="space-y-4">
      <UFormField :label="t('capture.quickProbe.program')" required>
        <div class="flex flex-wrap gap-2">
          <UInput
            :model-value="executablePath"
            autofocus
            class="min-w-[16rem] flex-1"
            :placeholder="t('software.pathPlaceholder')"
            :aria-label="t('capture.quickProbe.program')"
            @update:model-value="updateExecutablePath"
            @keydown.enter="validateExecutable"
          />
          <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-folder-open" :label="t('software.browse')" @click="browseExecutable" />
          <UButton color="neutral" variant="outline" size="sm" :loading="checking" :label="t('software.validate')" :disabled="!executablePath.trim()" @click="validateExecutable" />
        </div>
      </UFormField>

      <div class="flex items-center gap-2">
        <UButton
          color="neutral"
          :variant="captureArmed ? 'soft' : 'ghost'"
          size="sm"
          :icon="captureArmed ? 'i-tabler-x' : 'i-tabler-focus-centered'"
          :label="captureArmed ? t('capture.quickProbe.cancelCapture') : t('capture.quickProbe.captureForeground')"
          @click="toggleForegroundCapture"
        />
        <span v-if="!captureArmed" class="text-[10px] text-[var(--text-muted)]">{{ t('capture.quickProbe.captureHint') }}</span>
      </div>

      <UAlert
        v-if="captureArmed"
        color="primary"
        variant="soft"
        icon="i-tabler-keyboard"
        :title="t('capture.quickProbe.captureArmedTitle')"
        :description="t('capture.quickProbe.captureArmedDescription', { shortcut: captureShortcut })"
      />
      <UAlert
        v-else-if="preflightPresentation"
        data-testid="quick-probe-preflight"
        :color="preflightPresentation.color"
        variant="soft"
        :icon="preflightPresentation.icon"
        :title="preflightPresentation.title"
        :description="preflightPresentation.description"
      />
      <UAlert v-if="localError || captureError || probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="localError || captureError || probe.message.value" />

      <UCollapsible v-model:open="advancedOpen">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-tabler-adjustments-horizontal"
          :trailing-icon="advancedOpen ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'"
          :label="t('capture.quickProbe.advanced')"
          :aria-expanded="advancedOpen"
        />
        <template #content>
          <div class="mt-2 border-t border-[var(--border)] pt-3">
            <UFormField :label="t('capture.targetLocale')" :hint="t('capture.quickProbe.targetLocaleHint')">
              <UInput v-model="targetLocale" class="w-52" maxlength="64" />
            </UFormField>
          </div>
        </template>
      </UCollapsible>
    </div>
  </ManagementFormModal>
</template>
