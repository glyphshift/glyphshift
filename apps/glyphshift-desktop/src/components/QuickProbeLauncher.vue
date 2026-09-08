<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { translateCommandError } from '../commandError'
import type { DictionarySummary, SoftwarePreflight, SoftwareRecord } from '../model'
import { useProbeRuns } from '../useProbeRuns'
import ManagementFormModal from './ManagementFormModal.vue'

type TargetMode = 'library' | 'active'
type DictionaryMode = 'library' | 'new'

const props = defineProps<{
  open: boolean
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
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

const { t } = useI18n()
const probe = useProbeRuns()
const targetMode = ref<TargetMode>('library')
const softwareId = ref('')
const dictionaryMode = ref<DictionaryMode>('new')
const dictionaryId = ref('')
const excludedDictionaryIds = ref<string[]>([])
const exclusionItems = computed(() => props.dictionaries.filter(item => dictionaryMode.value !== 'library' || item.metadata.id !== dictionaryId.value).map(item => ({ value: item.metadata.id, label: item.metadata.name })))
watch([dictionaryId, dictionaryMode], () => { excludedDictionaryIds.value = excludedDictionaryIds.value.filter(id => exclusionItems.value.some(item => item.value === id)) })
const sourceLocale = ref('en-US')
const targetLocale = ref('zh-CN')
const runName = ref('')
const activeTarget = ref<SoftwarePreflight | null>(null)
const runningTargets = ref<SoftwarePreflight[]>([])
const runningTargetPath = ref('')
const runningTargetsLoading = ref(false)
const runningTargetsLoaded = ref(false)
const runningTargetsError = ref('')
let runningTargetsRequest = 0

function normalizedPath(path: string) {
  return path.trim().replace(/^\\\\\?\\/, '').replace(/\//g, '\\').toLocaleLowerCase()
}

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

const librarySoftware = computed(() => props.software.filter(item => item.executablePath))
const selectedSoftware = computed(() => librarySoftware.value.find(item => item.id === softwareId.value))
const selectedDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === dictionaryId.value))
const capturedLibrarySoftware = computed(() => {
  const path = activeTarget.value?.executablePath
  return path
    ? props.software.find(item => item.executablePath
      && normalizedPath(item.executablePath) === normalizedPath(path))
    : undefined
})
const runningTargetItems = computed(() => runningTargets.value.map(target => ({
  value: target.executablePath,
  label: `${target.existingName ?? target.suggestedName} · ${target.executableName} · ${target.architecture}`,
})))
const activeTargetReady = computed(() => Boolean(
  activeTarget.value?.running
  && ['ready', 'already_added'].includes(activeTarget.value.state),
))
const targetReady = computed(() => targetMode.value === 'library'
  ? Boolean(selectedSoftware.value?.executablePath)
  : activeTargetReady.value)
const dictionaryReady = computed(() => dictionaryMode.value === 'library'
  ? Boolean(selectedDictionary.value)
  : Boolean(sourceLocale.value.trim() && targetLocale.value.trim()))
const canStart = computed(() => targetReady.value && dictionaryReady.value && !probe.busy.value)
const activePresentation = computed(() => {
  const result = activeTarget.value
  if (!result) return null
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
    not_running: ['i-tabler-player-pause', 'notRunning'] as const,
    runtime_unavailable: ['i-tabler-plug-connected-x', 'runtimeUnavailable'] as const,
    self_target: ['i-tabler-app-window', 'selfTarget'] as const,
    unsupported_architecture: ['i-tabler-cpu-off', 'unsupportedArchitecture'] as const,
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

async function loadRunningTargets(force = false) {
  if (runningTargetsLoading.value || (runningTargetsLoaded.value && !force)) return
  const request = ++runningTargetsRequest
  runningTargetsLoading.value = true
  runningTargetsError.value = ''
  try {
    const result = hasDesktopRuntime()
      ? await invoke<SoftwarePreflight[]>('desktop_running_software_targets')
      : []
    if (request !== runningTargetsRequest) return
    runningTargets.value = Array.isArray(result) ? result : []
    runningTargetsLoaded.value = true
    const current = activeTarget.value?.executablePath
    const selected = runningTargets.value.find(item => current
      && normalizedPath(item.executablePath) === normalizedPath(current))
      ?? runningTargets.value[0]
    runningTargetPath.value = selected?.executablePath ?? ''
    activeTarget.value = selected ?? null
  }
  catch (error) {
    if (request !== runningTargetsRequest) return
    runningTargets.value = []
    runningTargetPath.value = ''
    activeTarget.value = null
    runningTargetsLoaded.value = true
    runningTargetsError.value = translateCommandError(error)
  }
  finally {
    if (request === runningTargetsRequest) runningTargetsLoading.value = false
  }
}

function chooseRunningTarget(value: unknown) {
  runningTargetPath.value = String(value ?? '')
  activeTarget.value = runningTargets.value.find(item => item.executablePath === runningTargetPath.value) ?? null
  probe.clearMessage()
}

watch(() => props.open, (open) => {
  if (!open) return
  targetMode.value = librarySoftware.value.length ? 'library' : 'active'
  softwareId.value = librarySoftware.value[0]?.id ?? ''
  dictionaryMode.value = 'new'
  excludedDictionaryIds.value = []
  dictionaryId.value = props.dictionaries[0]?.metadata.id ?? ''
  sourceLocale.value = 'en-US'
  targetLocale.value = 'zh-CN'
  runName.value = ''
  activeTarget.value = null
  runningTargets.value = []
  runningTargetPath.value = ''
  runningTargetsLoaded.value = false
  runningTargetsError.value = ''
  probe.clearMessage()
  if (targetMode.value === 'active') void loadRunningTargets()
})

watch(() => props.captureResult, (result, previous) => {
  if (!props.open || !result || result === previous) return
  const existingIndex = runningTargets.value.findIndex(item => normalizedPath(item.executablePath) === normalizedPath(result.executablePath))
  if (existingIndex >= 0) runningTargets.value.splice(existingIndex, 1, result)
  else runningTargets.value.unshift(result)
  runningTargetPath.value = result.executablePath
  activeTarget.value = result
  targetMode.value = 'active'
  probe.clearMessage()
})

function selectTargetMode(mode: TargetMode) {
  targetMode.value = mode
  if (mode === 'library') {
    if (props.captureArmed) emit('cancel-capture')
    return
  }
  void loadRunningTargets()
}

function selectDictionaryMode(mode: DictionaryMode) {
  dictionaryMode.value = mode
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
  if (!canStart.value) return
  const software = selectedSoftware.value
  const active = activeTarget.value
  const softwareName = targetMode.value === 'library'
    ? software?.name
    : capturedLibrarySoftware.value?.name ?? active?.suggestedName
  if (!softwareName) return

  const started = await probe.createFromSources({
    target: targetMode.value === 'library'
      ? { kind: 'library', softwareId: software!.id }
      : { kind: 'active_process', executablePath: active!.executablePath },
    dictionary: dictionaryMode.value === 'library'
      ? { kind: 'library', dictionaryId: selectedDictionary.value!.metadata.id }
      : {
          kind: 'new',
          sourceLocale: sourceLocale.value.trim(),
          targetLocale: targetLocale.value.trim(),
        },
    name: runName.value.trim() || undefined,
    excludedDictionaryIds: [...excludedDictionaryIds.value],
    adapterIds: [],
    livePreviewEnabled: false,
  }, {
    softwareId: targetMode.value === 'library' ? software?.id : capturedLibrarySoftware.value?.id,
    softwareName,
    dictionaryId: dictionaryMode.value === 'library' ? selectedDictionary.value?.metadata.id : undefined,
  })
  if (!started) return
  emit('started', started.id)
  setOpen(false)
}
</script>

<template>
  <ManagementFormModal
    :open="open"
    :title="t('capture.createRun')"
    :confirm-label="t('capture.createConfirm')"
    :confirm-disabled="!canStart"
    :busy="probe.busy.value"
    width="lg"
    @update:open="setOpen"
    @confirm="start"
  >
    <div class="space-y-4">
      <UFormField :label="t('capture.runName')" :hint="t('capture.runNameOptional')">
        <UInput v-model="runName" :maxlength="128" class="w-full" />
      </UFormField>

      <UFormField :label="t('capture.targetSource')" required>
        <div class="mb-2 flex rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('capture.targetSource')">
          <UButton class="flex-1" :color="targetMode === 'library' ? 'primary' : 'neutral'" size="xs" :variant="targetMode === 'library' ? 'soft' : 'ghost'" :label="t('capture.targetFromLibrary')" :aria-pressed="targetMode === 'library'" @click="selectTargetMode('library')" />
          <UButton class="flex-1" :color="targetMode === 'active' ? 'primary' : 'neutral'" size="xs" :variant="targetMode === 'active' ? 'soft' : 'ghost'" :label="t('capture.targetFromActive')" :aria-pressed="targetMode === 'active'" @click="selectTargetMode('active')" />
        </div>

        <USelect v-if="targetMode === 'library' && librarySoftware.length" v-model="softwareId" :items="librarySoftware.map(item => ({ value: item.id, label: item.name }))" value-key="value" label-key="label" :aria-label="t('capture.chooseSoftware')" class="w-full" />
        <UAlert v-else-if="targetMode === 'library'" color="neutral" variant="soft" icon="i-tabler-library" :title="t('capture.softwareLibraryEmptyTitle')" :description="t('capture.softwareLibraryEmptyDescription')" />

        <div v-else class="space-y-2">
          <div class="flex gap-2">
            <USelect
              :model-value="runningTargetPath"
              :items="runningTargetItems"
              value-key="value"
              label-key="label"
              :aria-label="t('capture.runningSoftware')"
              :placeholder="t('capture.runningSoftwarePlaceholder')"
              :loading="runningTargetsLoading"
              :disabled="runningTargetsLoading || !runningTargetItems.length"
              class="min-w-0 flex-1"
              @update:model-value="chooseRunningTarget"
            />
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :aria-label="t('capture.refreshRunningSoftware')" :loading="runningTargetsLoading" @click="loadRunningTargets(true)" />
          </div>
          <p class="type-metadata m-0 leading-4 text-[var(--text-muted)]">{{ t('capture.runningSoftwareHint') }}</p>
          <UAlert v-if="runningTargetsError" role="alert" color="error" variant="soft" icon="i-tabler-alert-circle" :title="t('capture.error')" :description="runningTargetsError" />
          <UAlert v-else-if="runningTargetsLoaded && !runningTargets.length" color="neutral" variant="soft" icon="i-tabler-apps-off" :title="t('capture.runningSoftwareEmptyTitle')" :description="t('capture.runningSoftwareEmptyDescription')" />
          <div class="flex items-center justify-between gap-3 rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2.5">
            <p class="type-metadata m-0 min-w-0 leading-4 text-[var(--text-muted)]">{{ t('capture.shortcutCaptureHint', { shortcut: captureShortcut }) }}</p>
            <UButton color="neutral" :variant="captureArmed ? 'soft' : 'outline'" size="sm" :icon="captureArmed ? 'i-tabler-x' : 'i-tabler-focus-centered'" :label="captureArmed ? t('capture.quickProbe.cancelCapture') : t('capture.quickProbe.captureForeground')" @click="toggleForegroundCapture" />
          </div>
        </div>
      </UFormField>

      <UAlert v-if="targetMode === 'active' && captureArmed" color="primary" variant="soft" icon="i-tabler-keyboard" :title="t('capture.quickProbe.captureArmedTitle')" :description="t('capture.quickProbe.captureArmedDescription', { shortcut: captureShortcut })" />
      <UAlert v-else-if="targetMode === 'active' && activePresentation" data-testid="quick-probe-preflight" :color="activePresentation.color" variant="soft" :icon="activePresentation.icon" :title="activePresentation.title" :description="activePresentation.description" />

      <UFormField :label="t('capture.dictionaryBinding')" required>
        <div class="mb-2 flex rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('capture.dictionaryBinding')">
          <UButton class="flex-1" :color="dictionaryMode === 'new' ? 'primary' : 'neutral'" size="xs" :variant="dictionaryMode === 'new' ? 'soft' : 'ghost'" :label="t('capture.useTemporaryDictionary')" :aria-pressed="dictionaryMode === 'new'" @click="selectDictionaryMode('new')" />
          <UButton class="flex-1" :color="dictionaryMode === 'library' ? 'primary' : 'neutral'" size="xs" :variant="dictionaryMode === 'library' ? 'soft' : 'ghost'" :label="t('capture.useExistingDictionary')" :aria-pressed="dictionaryMode === 'library'" @click="selectDictionaryMode('library')" />
        </div>
        <USelect v-if="dictionaryMode === 'library' && dictionaries.length" v-model="dictionaryId" :items="dictionaries.map(item => ({ value: item.metadata.id, label: item.metadata.name }))" value-key="value" label-key="label" :aria-label="t('capture.useExistingDictionary')" class="w-full" />
        <UAlert v-else-if="dictionaryMode === 'library'" color="neutral" variant="soft" icon="i-tabler-books" :title="t('capture.dictionaryLibraryEmptyTitle')" :description="t('capture.dictionaryLibraryEmptyDescription')" />
        <UAlert v-else color="neutral" variant="soft" icon="i-tabler-wand" :title="t('capture.temporaryDictionaryTitle')" :description="t('capture.temporaryDictionaryHint')" />
      </UFormField>

      <UFormField :label="t('capture.excludedDictionaries')" :description="t('capture.excludedDictionariesHint')">
        <USelectMenu v-model="excludedDictionaryIds" :items="exclusionItems" multiple value-key="value" :aria-label="t('capture.excludedDictionaries')" :placeholder="t('capture.noExcludedDictionaries')" class="w-full" />
      </UFormField>

      <div v-if="dictionaryMode === 'new'" class="grid grid-cols-2 gap-3 @max-[560px]:grid-cols-1">
        <UFormField :label="t('capture.sourceLocale')" :hint="t('capture.quickProbe.sourceLocaleHint')" required>
          <UInput v-model="sourceLocale" :aria-label="t('capture.sourceLocale')" class="w-full" maxlength="64" />
        </UFormField>
        <UFormField :label="t('capture.targetLocale')" :hint="t('capture.quickProbe.targetLocaleHint')" required>
          <UInput v-model="targetLocale" :aria-label="t('capture.targetLocale')" class="w-full" maxlength="64" />
        </UFormField>
      </div>

      <UAlert v-if="captureError || probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="captureError || probe.message.value" />
    </div>
  </ManagementFormModal>
</template>
