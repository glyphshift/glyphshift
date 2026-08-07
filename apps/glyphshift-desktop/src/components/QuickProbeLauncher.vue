<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { DictionarySummary, SoftwarePreflight, SoftwareRecord } from '../model'
import { useProbeRuns } from '../useProbeRuns'
import ManagementFormModal from './ManagementFormModal.vue'

type TargetMode = 'library' | 'active'
type DictionaryMode = 'library' | 'temporary'

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

const { t, locale } = useI18n()
const probe = useProbeRuns()
const targetMode = ref<TargetMode>('library')
const softwareId = ref('')
const dictionaryMode = ref<DictionaryMode>('library')
const dictionaryId = ref('')
const targetLocale = ref('zh-CN')
const runName = ref('')
const advancedOpen = ref(false)
const activeTarget = ref<SoftwarePreflight | null>(null)

function normalizedPath(path: string) {
  return path.trim().replace(/^\\\\\?\\/, '').replace(/\//g, '\\').toLocaleLowerCase()
}

const selectedSoftware = computed(() => props.software.find(item => item.id === softwareId.value))
const selectedDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === dictionaryId.value))
const capturedLibrarySoftware = computed(() => {
  const path = activeTarget.value?.executablePath
  return path
    ? props.software.find(item => item.executablePath
      && normalizedPath(item.executablePath) === normalizedPath(path))
    : undefined
})
const activeTargetReady = computed(() => Boolean(
  activeTarget.value?.running
  && ['ready', 'already_added'].includes(activeTarget.value.state),
))
const targetReady = computed(() => targetMode.value === 'library'
  ? Boolean(selectedSoftware.value?.executablePath)
  : activeTargetReady.value)
const dictionaryReady = computed(() => dictionaryMode.value === 'library'
  ? Boolean(selectedDictionary.value)
  : Boolean(targetLocale.value.trim()))
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

watch(() => props.open, (open) => {
  if (!open) return
  targetMode.value = props.software.length ? 'library' : 'active'
  softwareId.value = props.software.find(item => item.executablePath)?.id ?? ''
  dictionaryMode.value = props.dictionaries.length ? 'library' : 'temporary'
  dictionaryId.value = props.dictionaries[0]?.metadata.id ?? ''
  targetLocale.value = locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'
  runName.value = ''
  advancedOpen.value = false
  activeTarget.value = null
  probe.clearMessage()
})

watch(() => props.captureResult, (result, previous) => {
  if (!props.open || !result || result === previous) return
  activeTarget.value = result
  targetMode.value = 'active'
  probe.clearMessage()
})

function selectTargetMode(mode: TargetMode) {
  if (mode === 'library' && !props.software.length) return
  targetMode.value = mode
  if (mode === 'library' && props.captureArmed) emit('cancel-capture')
}

function selectDictionaryMode(mode: DictionaryMode) {
  if (mode === 'library' && !props.dictionaries.length) return
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
      : { kind: 'temporary', targetLocale: targetLocale.value.trim() },
    name: runName.value.trim() || undefined,
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
    :description="t('capture.createDescription')"
    :confirm-label="t('capture.createConfirm')"
    :confirm-disabled="!canStart"
    :busy="probe.busy.value"
    width="lg"
    @update:open="setOpen"
    @confirm="start"
  >
    <div class="space-y-4">
      <UFormField :label="t('capture.targetSource')" required>
        <div class="mb-2 flex rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('capture.targetSource')">
          <UButton class="flex-1" :color="targetMode === 'library' ? 'primary' : 'neutral'" size="xs" :variant="targetMode === 'library' ? 'soft' : 'ghost'" :label="t('capture.targetFromLibrary')" :disabled="!software.length" :aria-pressed="targetMode === 'library'" @click="selectTargetMode('library')" />
          <UButton class="flex-1" :color="targetMode === 'active' ? 'primary' : 'neutral'" size="xs" :variant="targetMode === 'active' ? 'soft' : 'ghost'" :label="t('capture.targetFromActive')" :aria-pressed="targetMode === 'active'" @click="selectTargetMode('active')" />
        </div>

        <USelect v-if="targetMode === 'library'" v-model="softwareId" :items="software.filter(item => item.executablePath).map(item => ({ value: item.id, label: item.name }))" value-key="value" label-key="label" class="w-full" />
        <div v-else class="rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
          <div class="flex items-center justify-between gap-3">
            <div>
              <p class="m-0 text-[10px] font-semibold text-[var(--text)]">{{ activeTarget?.suggestedName ?? t('capture.activeTargetPending') }}</p>
              <p class="m-0 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('capture.activeTargetHint') }}</p>
            </div>
            <UButton color="neutral" :variant="captureArmed ? 'soft' : 'outline'" size="sm" :icon="captureArmed ? 'i-tabler-x' : 'i-tabler-focus-centered'" :label="captureArmed ? t('capture.quickProbe.cancelCapture') : t('capture.quickProbe.captureForeground')" @click="toggleForegroundCapture" />
          </div>
        </div>
      </UFormField>

      <UAlert v-if="targetMode === 'active' && captureArmed" color="primary" variant="soft" icon="i-tabler-keyboard" :title="t('capture.quickProbe.captureArmedTitle')" :description="t('capture.quickProbe.captureArmedDescription', { shortcut: captureShortcut })" />
      <UAlert v-else-if="targetMode === 'active' && activePresentation" data-testid="quick-probe-preflight" :color="activePresentation.color" variant="soft" :icon="activePresentation.icon" :title="activePresentation.title" :description="activePresentation.description" />

      <UFormField :label="t('capture.dictionaryBinding')" required>
        <div class="mb-2 flex rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('capture.dictionaryBinding')">
          <UButton class="flex-1" :color="dictionaryMode === 'library' ? 'primary' : 'neutral'" size="xs" :variant="dictionaryMode === 'library' ? 'soft' : 'ghost'" :label="t('capture.useExistingDictionary')" :disabled="!dictionaries.length" :aria-pressed="dictionaryMode === 'library'" @click="selectDictionaryMode('library')" />
          <UButton class="flex-1" :color="dictionaryMode === 'temporary' ? 'primary' : 'neutral'" size="xs" :variant="dictionaryMode === 'temporary' ? 'soft' : 'ghost'" :label="t('capture.useTemporaryDictionary')" :aria-pressed="dictionaryMode === 'temporary'" @click="selectDictionaryMode('temporary')" />
        </div>
        <USelect v-if="dictionaryMode === 'library'" v-model="dictionaryId" :items="dictionaries.map(item => ({ value: item.metadata.id, label: item.metadata.name }))" value-key="value" label-key="label" class="w-full" />
        <UAlert v-else color="neutral" variant="soft" icon="i-tabler-wand" :title="t('capture.temporaryDictionaryTitle')" :description="t('capture.temporaryDictionaryHint')" />
      </UFormField>

      <UAlert v-if="captureError || probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="captureError || probe.message.value" />

      <UCollapsible v-model:open="advancedOpen">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-adjustments-horizontal" :trailing-icon="advancedOpen ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'" :label="t('capture.quickProbe.advanced')" :aria-expanded="advancedOpen" />
        <template #content>
          <div class="mt-2 grid gap-3 border-t border-[var(--border)] pt-3 sm:grid-cols-2">
            <UFormField :label="t('capture.runName')" :hint="t('capture.runNameOptional')"><UInput v-model="runName" :maxlength="128" class="w-full" /></UFormField>
            <UFormField v-if="dictionaryMode === 'temporary'" :label="t('capture.targetLocale')" :hint="t('capture.quickProbe.targetLocaleHint')"><UInput v-model="targetLocale" class="w-full" maxlength="64" /></UFormField>
          </div>
        </template>
      </UCollapsible>
    </div>
  </ManagementFormModal>
</template>
