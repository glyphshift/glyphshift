<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { translateCommandError } from '../commandError'
import type {
  DictionarySummary,
  InteractiveTranslationEvent,
  InteractiveTranslationResult,
  SoftwareRecord,
} from '../model'

type Phase = 'setup' | 'armed' | 'capturing' | 'result' | 'error'

const props = defineProps<{
  open: boolean
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
}>()

const emit = defineEmits<{ 'update:open': [value: boolean] }>()
const { t } = useI18n()
const phase = ref<Phase>('setup')
const softwareId = ref('')
const dictionaryId = ref('')
const shortcut = ref('Ctrl+Shift+F9')
const result = ref<InteractiveTranslationResult | null>(null)
const error = ref('')
const copiedIndex = ref<number | null>(null)
const busy = ref(false)
let unlistenNative: UnlistenFn | null = null
let copyReset: ReturnType<typeof setTimeout> | undefined

const softwareOptions = computed(() => props.software
  .filter(item => Boolean(item.executablePath))
  .map(item => ({ value: item.id, label: item.name, description: item.executableName })))
const dictionaryOptions = computed(() => props.dictionaries.map(item => ({
  value: item.metadata.id,
  label: item.metadata.name,
  description: `${item.metadata.sourceLocale} → ${item.metadata.targetLocale} · ${t('interactiveTranslation.entries', { count: item.entryCount })}`,
})))
const selectedSoftware = computed(() => props.software.find(item => item.id === softwareId.value))
const selectedDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === dictionaryId.value))
const canArm = computed(() => Boolean(softwareId.value && dictionaryId.value && !busy.value))
const translatedCount = computed(() => result.value?.blocks.filter(block => block.translationState === 'translated').length ?? 0)

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function reset() {
  phase.value = 'setup'
  result.value = null
  error.value = ''
  copiedIndex.value = null
  busy.value = false
}

function chooseDefaults() {
  if (!props.software.some(item => item.id === softwareId.value && item.executablePath)) {
    softwareId.value = softwareOptions.value[0]?.value ?? ''
  }
  if (!props.dictionaries.some(item => item.metadata.id === dictionaryId.value)) {
    dictionaryId.value = dictionaryOptions.value[0]?.value ?? ''
  }
}

watch(() => props.open, open => {
  if (!open) return
  chooseDefaults()
  reset()
})

function receive(event: InteractiveTranslationEvent) {
  shortcut.value = event.shortcut
  busy.value = false
  if (event.state === 'capturing') {
    phase.value = 'capturing'
    error.value = ''
    return
  }
  if (event.state === 'presented') {
    result.value = event.result
    phase.value = 'result'
    error.value = ''
    return
  }
  error.value = translateCommandError(event.error)
  phase.value = 'error'
}

function receiveBrowserEvent(event: Event) {
  receive((event as CustomEvent<InteractiveTranslationEvent>).detail)
}

function browserResult(): InteractiveTranslationResult {
  const translated = (selectedDictionary.value?.entryCount ?? 0) > 0
  return {
    partial: !translated,
    blocks: [{
      source: 'Open',
      anchors: [{ left: 10, top: 20, right: 80, bottom: 44 }],
      granularity: 'control',
      provenance: 'structured',
      translation: translated ? '打开' : undefined,
      translationState: translated ? 'translated' : 'missing',
      origin: translated ? 'dictionary' : undefined,
    }],
  }
}

function handleBrowserShortcut(event: KeyboardEvent) {
  if (!props.open || phase.value !== 'armed' || !event.ctrlKey || !event.shiftKey || event.key !== 'F9') return
  event.preventDefault()
  receive({ state: 'capturing', shortcut: shortcut.value })
  window.setTimeout(() => receive({
    state: 'presented',
    shortcut: shortcut.value,
    result: browserResult(),
  }), 0)
}

async function connectEvents() {
  window.addEventListener('glyphshift:interactive-translation', receiveBrowserEvent)
  if (!hasDesktopRuntime()) return
  try {
    unlistenNative = await listen<InteractiveTranslationEvent>('interactive-translation', event => receive(event.payload))
  }
  catch {
    // The arm command still reports native shortcut registration failures.
  }
}

async function arm() {
  if (!canArm.value) return
  busy.value = true
  error.value = ''
  result.value = null
  try {
    shortcut.value = hasDesktopRuntime()
      ? await invoke<string>('desktop_arm_interactive_translation', {
          request: { softwareId: softwareId.value, dictionaryId: dictionaryId.value },
        })
      : 'Ctrl+Shift+F9'
    phase.value = 'armed'
  }
  catch (cause) {
    error.value = translateCommandError(cause)
    phase.value = 'error'
  }
  finally {
    busy.value = false
  }
}

async function cancelNative() {
  if (!hasDesktopRuntime() || !['armed', 'capturing'].includes(phase.value)) return
  try {
    await invoke('desktop_cancel_interactive_translation')
  }
  catch {
    // Closing the bounded result surface remains safe even if native state already completed.
  }
}

async function close() {
  await cancelNative()
  reset()
  emit('update:open', false)
}

async function retry() {
  await cancelNative()
  phase.value = 'setup'
  await arm()
}

async function copyBlock(index: number) {
  const block = result.value?.blocks[index]
  if (!block) return
  const text = block.translation ? `${block.source}\n${block.translation}` : block.source
  try {
    await navigator.clipboard.writeText(text)
    copiedIndex.value = index
    if (copyReset) clearTimeout(copyReset)
    copyReset = setTimeout(() => { copiedIndex.value = null }, 1800)
  }
  catch {
    error.value = t('interactiveTranslation.copyFailed')
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleBrowserShortcut)
  void connectEvents()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleBrowserShortcut)
  window.removeEventListener('glyphshift:interactive-translation', receiveBrowserEvent)
  unlistenNative?.()
  if (copyReset) clearTimeout(copyReset)
  void cancelNative()
})
</script>

<template>
  <UModal
    :open="open"
    :title="t('interactiveTranslation.title')"
    :description="t('interactiveTranslation.description')"
    :dismissible="phase !== 'capturing'"
    :ui="{
      content: 'flex max-h-[calc(100dvh-32px)] max-w-[620px] flex-col',
      header: 'min-h-0 shrink-0 border-b border-[var(--border)] px-5 py-4',
      title: 'text-[15px]',
      description: 'mt-1 text-[10px] leading-4',
      body: 'min-h-0 flex-1 overflow-y-auto px-5 py-4',
      footer: 'shrink-0 border-t border-[var(--border)] px-5 py-3',
    }"
    @update:open="$event || close()"
  >
    <template #body>
      <div v-if="phase === 'setup'" class="space-y-4" data-testid="interactive-translation-setup">
        <div class="grid grid-cols-2 gap-3 max-sm:grid-cols-1">
          <UFormField :label="t('interactiveTranslation.software')" required>
            <USelectMenu
              v-model="softwareId"
              :items="softwareOptions"
              value-key="value"
              label-key="label"
              description-key="description"
              :placeholder="t('interactiveTranslation.chooseSoftware')"
              class="w-full"
            />
          </UFormField>
          <UFormField :label="t('interactiveTranslation.dictionary')" required>
            <USelectMenu
              v-model="dictionaryId"
              :items="dictionaryOptions"
              value-key="value"
              label-key="label"
              description-key="description"
              :placeholder="t('interactiveTranslation.chooseDictionary')"
              class="w-full"
            />
          </UFormField>
        </div>

        <div class="rounded-[7px] border border-[var(--border)] bg-[var(--surface-inset)] px-4 py-3">
          <div class="flex items-start gap-3">
            <span class="grid h-8 w-8 shrink-0 place-items-center rounded-[6px] bg-[var(--selection)] text-[var(--accent)]"><UIcon name="i-tabler-focus-2" class="size-4" /></span>
            <div class="min-w-0">
              <p class="m-0 text-[11px] font-semibold text-[var(--text)]">{{ t('interactiveTranslation.oneShotTitle') }}</p>
              <p class="m-0 mt-1 text-[10px] leading-4 text-[var(--text-muted)]">{{ t('interactiveTranslation.oneShotDescription') }}</p>
            </div>
          </div>
        </div>

        <UAlert
          v-if="!softwareOptions.length || !dictionaryOptions.length"
          color="warning"
          variant="soft"
          icon="i-tabler-alert-triangle"
          :title="t('interactiveTranslation.notReadyTitle')"
          :description="t(!softwareOptions.length ? 'interactiveTranslation.noSoftware' : 'interactiveTranslation.noDictionary')"
        />
      </div>

      <div v-else-if="phase === 'armed'" class="py-6 text-center" data-testid="interactive-translation-armed">
        <span class="mx-auto grid h-12 w-12 place-items-center rounded-full border border-[var(--accent)]/30 bg-[var(--selection)] text-[var(--accent)]"><UIcon name="i-tabler-crosshair" class="size-6" /></span>
        <h3 class="mt-4 text-[14px] font-semibold text-[var(--text)]">{{ t('interactiveTranslation.armedTitle') }}</h3>
        <p class="mx-auto mt-2 max-w-[420px] text-[10px] leading-5 text-[var(--text-secondary)]">{{ t('interactiveTranslation.armedDescription', { software: selectedSoftware?.name }) }}</p>
        <UKbd class="mt-4">{{ shortcut }}</UKbd>
        <p class="mt-3 text-[9px] text-[var(--text-muted)]">{{ t('interactiveTranslation.noMouseHook') }}</p>
      </div>

      <div v-else-if="phase === 'capturing'" class="grid min-h-52 place-items-center text-center" data-testid="interactive-translation-capturing">
        <div>
          <UIcon name="i-tabler-loader-2" class="mx-auto size-7 animate-spin text-[var(--accent)]" />
          <h3 class="mt-4 text-[13px] font-semibold">{{ t('interactiveTranslation.capturingTitle') }}</h3>
          <p class="mt-1 text-[10px] text-[var(--text-muted)]">{{ t('interactiveTranslation.capturingDescription') }}</p>
        </div>
      </div>

      <div v-else-if="phase === 'result' && result" class="space-y-3" data-testid="interactive-translation-result">
        <div class="flex items-center justify-between gap-3">
          <div>
            <p class="m-0 text-[11px] font-semibold">{{ translatedCount ? t('interactiveTranslation.resultTitle') : t('interactiveTranslation.missingTitle') }}</p>
            <p class="m-0 mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('interactiveTranslation.resultSummary', { translated: translatedCount, total: result.blocks.length }) }}</p>
          </div>
          <UBadge color="neutral" variant="outline" size="sm" icon="i-tabler-accessible" :label="t('interactiveTranslation.uiaSource')" />
        </div>

        <article v-for="(block, index) in result.blocks" :key="`${index}-${block.source}`" class="overflow-hidden rounded-[7px] border border-[var(--border)] bg-[var(--surface)]">
          <div class="grid grid-cols-2 divide-x divide-[var(--border)] max-sm:grid-cols-1 max-sm:divide-x-0 max-sm:divide-y">
            <div class="min-w-0 p-3">
              <span class="text-[8px] font-semibold uppercase tracking-[0.08em] text-[var(--text-muted)]">{{ t('interactiveTranslation.original') }}</span>
              <p class="m-0 mt-1 whitespace-pre-wrap break-words text-[12px] leading-5 text-[var(--text)]">{{ block.source }}</p>
            </div>
            <div class="min-w-0 bg-[var(--surface-subtle)] p-3">
              <span class="text-[8px] font-semibold uppercase tracking-[0.08em] text-[var(--text-muted)]">{{ t('interactiveTranslation.translation') }}</span>
              <p v-if="block.translation" class="m-0 mt-1 whitespace-pre-wrap break-words text-[12px] font-medium leading-5 text-[var(--text)]">{{ block.translation }}</p>
              <div v-else class="mt-1 flex items-center gap-1.5 text-[10px] text-[var(--warning)]">
                <UIcon name="i-tabler-book-off" class="size-3.5" />
                <span>{{ t('interactiveTranslation.missing') }}</span>
              </div>
            </div>
          </div>
          <div class="flex items-center border-t border-[var(--border)] bg-[var(--surface-inset)] px-3 py-1.5">
            <span class="text-[8px] text-[var(--text-muted)]">{{ block.origin === 'dictionary' ? selectedDictionary?.metadata.name : t('interactiveTranslation.noOrigin') }}</span>
            <UButton
              class="ml-auto"
              color="neutral"
              variant="ghost"
              size="xs"
              :icon="copiedIndex === index ? 'i-tabler-check' : 'i-tabler-copy'"
              :label="copiedIndex === index ? t('interactiveTranslation.copied') : t('interactiveTranslation.copy')"
              @click="copyBlock(index)"
            />
          </div>
        </article>

        <UAlert v-if="result.partial" color="warning" variant="soft" icon="i-tabler-info-circle" :description="t('interactiveTranslation.providerUnavailable')" />
      </div>

      <div v-else class="space-y-3 py-4" data-testid="interactive-translation-error">
        <UAlert role="alert" color="error" variant="soft" icon="i-tabler-alert-circle" :title="t('interactiveTranslation.errorTitle')" :description="error" />
        <p class="m-0 text-[10px] leading-4 text-[var(--text-muted)]">{{ t('interactiveTranslation.errorHint') }}</p>
      </div>
    </template>

    <template #footer>
      <span v-if="phase === 'setup'" class="min-w-0 truncate text-[9px] text-[var(--text-muted)]">{{ selectedSoftware?.name || t('interactiveTranslation.noSelection') }} · {{ selectedDictionary?.metadata.name || t('interactiveTranslation.noSelection') }}</span>
      <span v-else-if="phase === 'armed'" class="text-[9px] text-[var(--text-muted)]">{{ t('interactiveTranslation.waiting') }}</span>
      <span v-else-if="phase === 'capturing'" class="text-[9px] text-[var(--text-muted)]">{{ t('interactiveTranslation.doNotClose') }}</span>
      <UButton v-if="phase === 'setup'" class="ml-auto" color="primary" size="sm" icon="i-tabler-crosshair" :label="t('interactiveTranslation.start')" :loading="busy" :disabled="!canArm" @click="arm" />
      <UButton v-else-if="phase === 'armed'" class="ml-auto" color="neutral" variant="outline" size="sm" :label="t('common.cancel')" @click="close" />
      <template v-else-if="phase !== 'capturing'">
        <UButton class="ml-auto" color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="t('common.retry')" :loading="busy" @click="retry" />
        <UButton color="primary" size="sm" :label="t('interactiveTranslation.close')" @click="close" />
      </template>
    </template>
  </UModal>
</template>
