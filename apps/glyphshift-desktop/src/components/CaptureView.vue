<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { save } from '@tauri-apps/plugin-dialog'
import type { DropdownMenuItem, TableColumn } from '@nuxt/ui'
import { useI18n } from 'vue-i18n'
import type {
  AdapterOption,
  DictionarySummary,
  ProbeEntryRow,
  ProbeExportFormat,
  ProbeRunSummary,
  SoftwarePreflight,
  SoftwareRecord,
} from '../model'
import { editableRowIndex } from '../tableInteraction'
import { useCaptureScrollbar } from '../useCaptureScrollbar'
import { useProbeRuns, type QuickProbeCleanupResult } from '../useProbeRuns'
import { usePageEscape } from '../usePageEscape'
import ConfirmDialog from './ConfirmDialog.vue'
import ManagementFormModal from './ManagementFormModal.vue'
import ManagementPageHeader from './ManagementPageHeader.vue'
import ManagementTableFrame from './ManagementTableFrame.vue'
import ProbeAdapterPicker from './ProbeAdapterPicker.vue'
import QuickProbeLauncher from './QuickProbeLauncher.vue'

const props = defineProps<{
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  adapters: AdapterOption[]
  captureArmed: boolean
  captureShortcut: string
  captureResult: SoftwarePreflight | null
  captureError: string
}>()

const emit = defineEmits<{
  'arm-capture': []
  'cancel-capture': []
  'workspace-changed': []
}>()

const { t, locale } = useI18n()
const probe = useProbeRuns()
const query = ref('')
const adapterFilterIds = ref<string[]>([])
const page = ref(1)
const pageSize = ref(50)

function openRunOnDoubleClick(event: MouseEvent) {
  const index = editableRowIndex(event)
  const item = index === null ? null : listPageItems.value[index]
  if (item) probe.selectRun(item.id)
}
const entryPage = ref({
  observationRevision: 0,
  dictionaryRevision: 1,
  page: 1,
  pageSize: 50,
  total: 0,
  rows: [] as ProbeEntryRow[],
})
const loading = ref(false)
const selected = ref(new Set<string>())
const listQuery = ref('')
const listPage = ref(1)
const listPageSize = ref(20)
const listSelected = ref(new Set<string>())
const pendingRemoval = ref<ProbeRunSummary[]>([])
const quickProbeOpen = ref(false)
const pendingQuickCleanup = ref<ProbeRunSummary | null>(null)
const quickProbeNotice = ref('')
const settingsOpen = ref(false)
const settingsName = ref('')
const settingsAdapterIds = ref<string[]>([])
const settingsLivePreview = ref(false)
const settingsCompatibleAdapterIds = ref<string[] | null>(null)
const settingsCompatibilityLoading = ref(false)
const clearAllOpen = ref(false)
const translationValues = ref<Record<string, string>>({})
const {
  tableShell,
  scrollThumbHeight,
  scrollThumbTop,
  updateScrollMetrics,
  jumpScrollbar,
  beginScrollbarDrag,
  dragScrollbar,
  endScrollbarDrag,
  startScrollTracking,
  stopScrollTracking,
} = useCaptureScrollbar()
const dirtyTranslations = new Set<string>()
const editTimers = new Map<string, ReturnType<typeof setTimeout>>()
let queryTimer: ReturnType<typeof setTimeout> | undefined
let pollTimer: ReturnType<typeof setInterval> | undefined
let settingsCompatibilityRequest = 0

const observableAdapters = computed(() => props.adapters.filter(adapter => adapter.features.includes('textObserve')))
const compatibleSettingsAdapters = computed(() => settingsCompatibleAdapterIds.value === null
  ? observableAdapters.value
  : observableAdapters.value.filter(adapter => settingsCompatibleAdapterIds.value?.includes(adapter.id)))
const selectedRun = computed(() => probe.selectedRun.value)
const selectedSoftware = computed(() => props.software.find(item => item.id === selectedRun.value?.softwareId))
const selectedDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === selectedRun.value?.dictionaryId))
const selectedRunMetadata = computed(() => {
  const run = selectedRun.value
  if (!run) return ''
  const parts = [
    selectedSoftware.value?.name ?? run.softwareId,
    selectedDictionary.value?.metadata.name ?? run.dictionaryId,
    t('capture.runSummary', { observed: run.observedCount, entries: run.dictionaryEntryCount }),
  ]
  if (run.livePreviewEnabled) parts.push(t('capture.previewGeneration', { generation: run.previewGeneration }))
  if (run.runtimeCapability) parts.push(runtimeCapabilityDescription(run.runtimeCapability))
  return parts.join(' · ')
})
const selectedRunAdapters = computed(() => (selectedRun.value?.adapterIds ?? []).map(id => ({
  id,
  name: adapterName(id),
})))
const adapterFilterLabel = computed(() => {
  if (!adapterFilterIds.value.length) return t('capture.adapterFilterAll')
  if (adapterFilterIds.value.length === 1) return adapterName(adapterFilterIds.value[0]!)
  return t('capture.adapterFilterSelected', { count: adapterFilterIds.value.length })
})
const currentSources = computed(() => entryPage.value.rows.map(row => row.source))
const pageSelected = computed(() => Boolean(currentSources.value.length) && currentSources.value.every(source => selected.value.has(source)))
const selectedRows = computed(() => entryPage.value.rows.filter(row => selected.value.has(row.source)))
const settingsPreviewAvailable = computed(() => Boolean(settingsAdapterIds.value.length)
  && settingsAdapterIds.value.some(id => props.adapters.find(adapter => adapter.id === id)?.features.includes('textReplace')))
const settingsConfigurationLocked = computed(() => ['running', 'paused'].includes(selectedRun.value?.status ?? ''))
const settingsValid = computed(() => Boolean(
  !settingsCompatibilityLoading.value
  &&
  settingsName.value.trim()
  && settingsAdapterIds.value.length
  && (!settingsLivePreview.value || settingsPreviewAvailable.value),
))
const settingsChanged = computed(() => {
  const run = selectedRun.value
  if (!run) return false
  return (
    settingsName.value.trim() !== run.name
    || JSON.stringify(settingsAdapterIds.value) !== JSON.stringify(run.adapterIds)
    || settingsLivePreview.value !== run.livePreviewEnabled
  )
})
const activeRunExists = computed(() => probe.runs.value.some(run => ['running', 'paused'].includes(run.status)))
const bulkRunDeletionBlocked = computed(() => [...listSelected.value].some((id) => {
  const run = probe.runs.value.find(item => item.id === id)
  return Boolean(run?.quickProbe) || ['running', 'paused'].includes(run?.status ?? '')
}))
const filteredRuns = computed(() => {
  const needle = listQuery.value.trim().toLowerCase()
  if (!needle) return probe.runs.value
  return probe.runs.value.filter(run => [
    run.name,
    softwareName(run.softwareId),
    dictionaryName(run.dictionaryId),
  ].some(value => value.toLowerCase().includes(needle)))
})
const listPageItems = computed(() => filteredRuns.value.slice(
  (listPage.value - 1) * listPageSize.value,
  listPage.value * listPageSize.value,
))
const listPageSelected = computed(() => Boolean(listPageItems.value.length)
  && listPageItems.value.every(run => listSelected.value.has(run.id)))

const runColumns = computed<TableColumn<ProbeRunSummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'run', header: t('capture.columns.run'), meta: { class: { th: 'w-[25%]', td: 'w-[25%]' } } },
  { id: 'software', header: t('capture.columns.software'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { id: 'dictionary', header: t('capture.columns.dictionary'), meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  { accessorKey: 'status', header: t('capture.columns.status'), meta: { class: { th: 'w-24', td: 'w-24' } } },
  { id: 'progress', header: t('capture.columns.progress'), meta: { class: { th: 'w-32', td: 'w-32' } } },
  { accessorKey: 'updatedAtMs', header: t('capture.columns.updated'), meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'actions', header: t('capture.columns.actions'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
])
const entryColumns = computed<TableColumn<ProbeEntryRow>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { accessorKey: 'source', header: t('capture.columns.source'), meta: { class: { th: 'w-[24%]', td: 'w-[24%]' } } },
  { accessorKey: 'translation', header: t('capture.columns.translation'), meta: { class: { th: 'w-[30%]', td: 'w-[30%]' } } },
  { accessorKey: 'state', header: t('capture.columns.status'), meta: { class: { th: 'w-24', td: 'w-24' } } },
  { id: 'adapters', header: t('capture.columns.adapter'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { accessorKey: 'count', header: t('capture.columns.count'), meta: { class: { th: 'w-20 text-right', td: 'w-20 text-right' } } },
  { accessorKey: 'lastSeenMs', header: t('capture.columns.lastSeen'), meta: { class: { th: 'w-28', td: 'w-28' } } },
])
const exportItems = computed<DropdownMenuItem[][]>(() => [[
  { label: t('capture.exportEntriesCsv'), icon: 'i-tabler-file-type-csv', onSelect: () => void chooseExport('entries_csv') },
  { label: t('capture.exportDictionary'), icon: 'i-tabler-language', disabled: !selectedRun.value?.dictionaryEntryCount, onSelect: () => void chooseExport('dictionary_json') },
], [
  { label: t('capture.exportObservationsCsv'), icon: 'i-tabler-file-type-csv', onSelect: () => void chooseExport('observations_csv') },
  { label: t('capture.exportObservationsJson'), icon: 'i-tabler-braces', onSelect: () => void chooseExport('observations_json') },
]])
const adapterFilterItems = computed<DropdownMenuItem[][]>(() => [
  selectedRunAdapters.value.map(adapter => ({
    type: 'checkbox' as const,
    label: adapter.name,
    checked: adapterFilterIds.value.includes(adapter.id),
    onSelect: event => event.preventDefault(),
    onUpdateChecked: checked => toggleAdapterFilter(adapter.id, checked),
  })),
  [{
    label: t('capture.clearAdapterFilter'),
    icon: 'i-tabler-filter-off',
    disabled: !adapterFilterIds.value.length,
    onSelect: () => { adapterFilterIds.value = [] },
  }],
])

watch(settingsAdapterIds, () => {
  if (!settingsPreviewAvailable.value) settingsLivePreview.value = false
}, { deep: true })

watch(() => probe.selectedRunId.value, async (id, previous) => {
  if (id === previous) return
  selected.value = new Set()
  restoreViewState(id)
  await loadPage()
})

watch([page, pageSize], async () => {
  persistViewState()
  await loadPage()
})

watch(query, () => {
  if (queryTimer) clearTimeout(queryTimer)
  queryTimer = setTimeout(async () => {
    page.value = 1
    selected.value = new Set()
    persistViewState()
    await loadPage()
  }, 250)
})

watch(adapterFilterIds, async () => {
  page.value = 1
  selected.value = new Set()
  persistViewState()
  await loadPage()
}, { deep: true })

watch([listQuery, listPageSize], () => {
  listPage.value = 1
  listSelected.value = new Set()
})

watch(() => selected.value.size, async () => {
  await nextTick()
  updateScrollMetrics()
})

onMounted(async () => {
  await probe.connect()
  restoreViewState(probe.selectedRunId.value)
  await loadPage()
  startScrollTracking()
  pollTimer = setInterval(() => void poll(), 1000)
})

onBeforeUnmount(() => {
  if (queryTimer) clearTimeout(queryTimer)
  if (pollTimer) clearInterval(pollTimer)
  stopScrollTracking()
  if (props.captureArmed) emit('cancel-capture')
  for (const [source, timer] of editTimers) {
    clearTimeout(timer)
    void saveTranslation(source)
  }
})

async function loadPage() {
  const runId = probe.selectedRunId.value
  if (!runId) {
    entryPage.value = {
      observationRevision: 0,
      dictionaryRevision: 1,
      page: 1,
      pageSize: pageSize.value,
      total: 0,
      rows: [],
    }
    return
  }
  loading.value = true
  try {
    entryPage.value = await probe.queryEntries({
      runId,
      search: query.value,
      adapterIds: [...adapterFilterIds.value],
      page: page.value,
      pageSize: pageSize.value,
    })
    synchronizeTranslationValues(entryPage.value.rows)
    const maxPage = Math.max(1, Math.ceil(entryPage.value.total / pageSize.value))
    if (page.value > maxPage) page.value = maxPage
  }
  catch (error) {
    probe.report(error)
  }
  finally {
    loading.value = false
    await nextTick()
    updateScrollMetrics()
  }
}

async function poll() {
  const run = selectedRun.value
  if (!run) return
  const previousObservation = run.observationRevision
  const previousDictionary = run.dictionaryRevision
  const summary = await probe.refreshSummary(run.id)
  if (summary && (
    summary.observationRevision !== previousObservation
    || summary.dictionaryRevision !== previousDictionary
  )) await loadPage()
}

async function openSettings() {
  const run = selectedRun.value
  if (!run) return
  settingsName.value = run.name
  settingsAdapterIds.value = [...run.adapterIds]
  settingsLivePreview.value = run.livePreviewEnabled
  settingsCompatibleAdapterIds.value = null
  settingsOpen.value = true
  const request = ++settingsCompatibilityRequest
  settingsCompatibilityLoading.value = true
  const compatible = await probe.compatibleAdapters(run.softwareId)
  if (request !== settingsCompatibilityRequest || run.id !== selectedRun.value?.id) {
    if (request === settingsCompatibilityRequest) settingsCompatibilityLoading.value = false
    return
  }
  settingsCompatibleAdapterIds.value = compatible
  if (compatible) settingsAdapterIds.value = settingsAdapterIds.value.filter(id => compatible.includes(id))
  settingsCompatibilityLoading.value = false
}

async function handleQuickProbeStarted(runId: string) {
  quickProbeNotice.value = ''
  emit('workspace-changed')
  probe.selectRun(runId)
  await nextTick()
  await loadPage()
}

async function retainQuickProbe() {
  const run = selectedRun.value
  if (!run?.quickProbe) return
  if (await probe.retainQuickProbe(run.id)) emit('workspace-changed')
}

async function confirmQuickProbeCleanup() {
  const run = pendingQuickCleanup.value
  if (!run) return
  const result = await probe.cleanupQuickProbe(run.id)
  if (result) {
    pendingQuickCleanup.value = null
    quickProbeNotice.value = quickProbeCleanupNotice(result)
    emit('workspace-changed')
  }
}

function quickProbeCleanupNotice(result: QuickProbeCleanupResult) {
  if (result.software === 'retained' || result.dictionary === 'retained') {
    return t('capture.quickProbe.cleanupReferencedNotice')
  }
  if (result.software === 'reused' || result.dictionary === 'reused') {
    return t('capture.quickProbe.cleanupReusedNotice')
  }
  return t('capture.quickProbe.cleanupCompleteNotice')
}

async function closeDetail() {
  await Promise.all([...dirtyTranslations].map(saveTranslation))
  if (dirtyTranslations.size) return
  probe.selectRun('')
  selected.value = new Set()
}

async function applySettings() {
  const run = selectedRun.value
  if (!run || !settingsValid.value || !settingsChanged.value) return
  const updated = await probe.update({
    runId: run.id,
    name: settingsName.value.trim(),
    adapterIds: [...settingsAdapterIds.value],
    livePreviewEnabled: settingsLivePreview.value,
  })
  if (!updated) return
  adapterFilterIds.value = adapterFilterIds.value.filter(id => updated.adapterIds.includes(id))
  settingsOpen.value = false
  await loadPage()
}

function requestClearAll() {
  settingsOpen.value = false
  clearAllOpen.value = true
}

async function confirmClearAll() {
  const run = selectedRun.value
  if (!run) return
  await Promise.all([...dirtyTranslations].map(saveTranslation))
  if (dirtyTranslations.size) return
  const cleared = await probe.clearEntries(run.id)
  if (!cleared) return
  for (const timer of editTimers.values()) clearTimeout(timer)
  editTimers.clear()
  dirtyTranslations.clear()
  translationValues.value = {}
  selected.value = new Set()
  page.value = 1
  clearAllOpen.value = false
  await loadPage()
}

function toggleAdapterFilter(id: string, checked: boolean) {
  const available = selectedRun.value?.adapterIds ?? []
  const next = new Set(adapterFilterIds.value)
  if (checked) next.add(id)
  else next.delete(id)
  adapterFilterIds.value = available.filter(adapterId => next.has(adapterId))
}

function toggleSelection(source: string) {
  const next = new Set(selected.value)
  next.has(source) ? next.delete(source) : next.add(source)
  selected.value = next
}

function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) currentSources.value.forEach(source => next.delete(source))
  else currentSources.value.forEach(source => next.add(source))
  selected.value = next
}

function toggleListSelection(id: string) {
  const next = new Set(listSelected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  listSelected.value = next
}

function toggleListPageSelection() {
  const next = new Set(listSelected.value)
  if (listPageSelected.value) listPageItems.value.forEach(run => next.delete(run.id))
  else listPageItems.value.forEach(run => next.add(run.id))
  listSelected.value = next
}

async function bulk(action: 'ignore' | 'restore' | 'clear_translations') {
  const run = selectedRun.value
  if (!run || !selected.value.size) return
  let sources = [...selected.value]
  if (action === 'ignore') sources = selectedRows.value.filter(row => row.count > 0 && row.state !== 'ignored').map(row => row.source)
  if (action === 'restore') sources = selectedRows.value.filter(row => row.state === 'ignored').map(row => row.source)
  if (!sources.length) return
  try {
    await probe.bulk(run.id, sources, action)
    selected.value = new Set()
    await loadPage()
  }
  catch (error) {
    probe.report(error)
  }
}

function updateTranslation(source: string, value: unknown) {
  const translation = String(value ?? '')
  translationValues.value = { ...translationValues.value, [source]: translation }
  dirtyTranslations.add(source)
  const previous = editTimers.get(source)
  if (previous) clearTimeout(previous)
  editTimers.set(source, setTimeout(() => void saveTranslation(source), 500))
}

async function saveTranslation(source: string) {
  const run = selectedRun.value
  if (!run || !dirtyTranslations.has(source)) return
  const pending = editTimers.get(source)
  if (pending) clearTimeout(pending)
  editTimers.delete(source)
  try {
    await probe.editTranslation(run.id, source, translationValues.value[source] ?? '')
    dirtyTranslations.delete(source)
    await loadPage()
  }
  catch (error) {
    probe.report(error)
  }
}

function synchronizeTranslationValues(rows: readonly ProbeEntryRow[]) {
  const next = { ...translationValues.value }
  for (const row of rows) if (!dirtyTranslations.has(row.source)) next[row.source] = row.translation
  translationValues.value = next
}

function restoreViewState(runId: string) {
  if (!runId) return
  try {
    const value = JSON.parse(localStorage.getItem(`glyphshift.probe.view.${runId}`) ?? '{}') as {
      query?: string
      adapterIds?: string[]
      page?: number
      pageSize?: number
    }
    const available = new Set(probe.runs.value.find(run => run.id === runId)?.adapterIds ?? [])
    query.value = value.query ?? ''
    adapterFilterIds.value = Array.isArray(value.adapterIds)
      ? [...new Set(value.adapterIds.filter(id => typeof id === 'string' && available.has(id)))]
      : []
    page.value = Math.max(1, value.page ?? 1)
    pageSize.value = [20, 50, 100].includes(value.pageSize ?? 0) ? value.pageSize! : 50
  }
  catch {
    query.value = ''
    adapterFilterIds.value = []
    page.value = 1
    pageSize.value = 50
  }
}

function persistViewState() {
  const runId = probe.selectedRunId.value
  if (!runId) return
  localStorage.setItem(`glyphshift.probe.view.${runId}`, JSON.stringify({
    query: query.value,
    adapterIds: adapterFilterIds.value,
    page: page.value,
    pageSize: pageSize.value,
  }))
}

function softwareName(id: string) {
  return props.software.find(item => item.id === id)?.name ?? id
}

function softwarePath(id: string) {
  return props.software.find(item => item.id === id)?.executablePath ?? t('capture.pathUnavailable')
}

function dictionaryName(id: string) {
  return props.dictionaries.find(item => item.metadata.id === id)?.metadata.name ?? id
}

function adapterName(id: string) {
  return props.adapters.find(adapter => adapter.id === id)?.name ?? id
}

function adapterDetails(run: ProbeRunSummary) {
  return run.adapterIds.map(id => {
    const adapter = props.adapters.find(item => item.id === id)
    return {
      id,
      name: adapter?.name ?? id,
      technologies: adapter?.technologies.join(' · ') || t('capture.technologyUnavailable'),
    }
  })
}

function statusLabel(status: string) {
  return t(['running', 'paused'].includes(status) ? 'capture.status.connected' : 'capture.status.disconnected')
}

function statusColor(status: string) {
  return ['running', 'paused'].includes(status) ? 'success' : 'neutral'
}

function runtimeCapabilityLabel(capability: NonNullable<ProbeRunSummary['runtimeCapability']>) {
  return t(`capture.runtimeCapability.${capability}.label`)
}

function runtimeCapabilityDescription(capability: NonNullable<ProbeRunSummary['runtimeCapability']>) {
  return t(`capture.runtimeCapability.${capability}.description`)
}

function runtimeCapabilityColor(capability: NonNullable<ProbeRunSummary['runtimeCapability']>) {
  if (capability === 'direct_replace') return 'success'
  if (capability === 'collection_only') return 'warning'
  return 'error'
}

function stateLabel(state: string) {
  return t(`capture.entryState.${state}`)
}

function stateColor(state: string) {
  if (state === 'translated') return 'success'
  if (state === 'pending') return 'warning'
  return 'neutral'
}

function formatTime(value: number) {
  if (!value) return '—'
  return new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit' }).format(value)
}

async function chooseExport(format: ProbeExportFormat) {
  const run = selectedRun.value
  if (!run || !('__TAURI_INTERNALS__' in window)) return
  const csv = format.endsWith('_csv')
  const outputPath = await save({
    title: t('capture.export'),
    defaultPath: `${run.name}-${format}.${csv ? 'csv' : 'json'}`,
    filters: [{ name: csv ? 'CSV' : 'JSON', extensions: [csv ? 'csv' : 'json'] }],
  })
  if (outputPath) await probe.exportRun(run.id, format, outputPath)
}

async function confirmRemoval() {
  const ids = pendingRemoval.value.map(run => run.id)
  if (await probe.remove(ids)) {
    listSelected.value = new Set([...listSelected.value].filter(id => !ids.includes(id)))
    pendingRemoval.value = []
  }
}

usePageEscape(() => Boolean(selectedRun.value), () => void closeDetail())
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="capture-title">
    <ManagementDetailHeader
      v-if="selectedRun"
      title-id="capture-title"
      :title="selectedRun.name"
      :description="selectedRunMetadata"
      :back-label="t('capture.backToRuns')"
      @back="closeDetail"
    >
      <template #status>
        <div class="flex items-center gap-1.5">
          <UBadge :color="statusColor(selectedRun.status)" variant="soft" size="sm" :label="statusLabel(selectedRun.status)" />
          <UBadge
            v-if="selectedRun.runtimeCapability"
            data-testid="probe-runtime-capability"
            :color="runtimeCapabilityColor(selectedRun.runtimeCapability)"
            variant="soft"
            size="sm"
            :label="runtimeCapabilityLabel(selectedRun.runtimeCapability)"
          />
        </div>
      </template>
      <template #actions>
        <template v-if="selectedRun.quickProbe">
          <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-bookmark" :label="t('capture.quickProbe.retain')" :loading="probe.busy.value" @click="retainQuickProbe" />
          <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash-x" :label="t('capture.quickProbe.cleanup')" :disabled="probe.busy.value" @click="pendingQuickCleanup = selectedRun" />
          <UButton v-if="selectedRun.status === 'running'" color="neutral" variant="ghost" size="sm" icon="i-tabler-player-pause" :aria-label="t('capture.pause')" :loading="probe.busy.value" @click="probe.setPaused(selectedRun.id, true)" />
          <UButton v-else color="neutral" variant="ghost" size="sm" icon="i-tabler-player-play" :aria-label="selectedRun.status === 'paused' ? t('capture.continue') : t('capture.resume')" :loading="probe.busy.value" @click="selectedRun.status === 'paused' ? probe.setPaused(selectedRun.id, false) : probe.resume(selectedRun.id)" />
        </template>
        <template v-else>
          <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-settings" :label="t('capture.settings')" @click="openSettings" />
          <UDropdownMenu :items="exportItems" :content="{ align: 'end' }">
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-download" trailing-icon="i-tabler-chevron-down" :label="t('capture.export')" />
          </UDropdownMenu>
          <UButton v-if="['running', 'paused'].includes(selectedRun.status)" color="neutral" variant="ghost" size="sm" icon="i-tabler-plug-off" :aria-label="t('capture.disconnect')" @click="probe.disconnect(selectedRun.id)" />
          <UButton v-if="selectedRun.status === 'running'" color="neutral" variant="outline" size="sm" icon="i-tabler-player-pause" :label="t('capture.pause')" :loading="probe.busy.value" @click="probe.setPaused(selectedRun.id, true)" />
          <UButton v-else color="primary" :variant="selectedRun.status === 'paused' ? 'soft' : 'solid'" size="sm" icon="i-tabler-player-play" :label="selectedRun.status === 'paused' ? t('capture.continue') : t('capture.resume')" :loading="probe.busy.value" @click="selectedRun.status === 'paused' ? probe.setPaused(selectedRun.id, false) : probe.resume(selectedRun.id)" />
        </template>
      </template>
    </ManagementDetailHeader>
    <ManagementPageHeader v-else title-id="capture-title" icon="i-tabler-radar" :title="t('capture.title')" :description="t('capture.description')">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('capture.createRun')" :disabled="!observableAdapters.length || activeRunExists" @click="quickProbeOpen = true" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="probe.message.value" class="mb-3" />
    <UAlert v-else-if="quickProbeNotice" role="status" color="success" variant="soft" :title="t('capture.quickProbe.cleanupCompleteTitle')" :description="quickProbeNotice" class="mb-3" />

    <template v-if="selectedRun">
      <ManagementTableFrame v-model:query="query" v-model:page="page" v-model:page-size="pageSize" :search-placeholder="t('capture.searchEntries')" :search-label="t('capture.searchLabel')" :selected-count="selected.size" :selected-label="t('capture.itemLabel')" :total="entryPage.total" :item-label="t('capture.itemLabel')">
        <template #toolbar-actions>
          <UDropdownMenu v-if="selectedRunAdapters.length > 1" :items="adapterFilterItems" :content="{ align: 'end' }" :ui="{ content: 'min-w-48' }">
            <UButton data-testid="capture-adapter-filter" color="neutral" variant="outline" size="sm" icon="i-tabler-filter" trailing-icon="i-tabler-chevron-down" :label="adapterFilterLabel" class="max-w-52 justify-between" :aria-label="t('capture.adapterFilterLabel')" />
          </UDropdownMenu>
        </template>
        <template #bulk-actions>
          <UButton color="neutral" variant="soft" size="sm" icon="i-tabler-eye-off" :label="t('capture.bulkIgnore')" :disabled="!selectedRows.some(row => row.count > 0 && row.state !== 'ignored')" @click="bulk('ignore')" />
          <UButton color="neutral" variant="soft" size="sm" icon="i-tabler-eye" :label="t('capture.bulkRestore')" :disabled="!selectedRows.some(row => row.state === 'ignored')" @click="bulk('restore')" />
          <UButton color="error" variant="soft" size="sm" icon="i-tabler-eraser" :label="t('capture.bulkClear')" :disabled="!selectedRows.some(row => row.translation)" @click="bulk('clear_translations')" />
        </template>

        <div ref="tableShell" class="relative h-full min-h-0 overflow-hidden">
          <UTable data-testid="capture-table-scroll" role="region" tabindex="0" :aria-label="t('capture.tableLabel')" :data="entryPage.rows" :columns="entryColumns" sticky :loading="loading" class="capture-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[1040px]' }" @scroll.passive="updateScrollMetrics">
            <template #select-header><UCheckbox :model-value="pageSelected" :aria-label="t('capture.selectPage')" @update:model-value="togglePageSelection" /></template>
            <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.source)" :aria-label="t('common.selectNamed', { name: row.original.source })" @update:model-value="toggleSelection(row.original.source)" /></template>
            <template #source-cell="{ row }"><div class="truncate font-medium" :title="row.original.source">{{ row.original.source }}</div></template>
            <template #translation-cell="{ row }">
              <UInput :model-value="translationValues[row.original.source] ?? row.original.translation" size="sm" class="w-full" :placeholder="t('capture.pendingTranslation')" :aria-label="t('capture.translationFor', { source: row.original.source })" @update:model-value="updateTranslation(row.original.source, $event)" @blur="saveTranslation(row.original.source)" />
            </template>
            <template #state-cell="{ row }"><UBadge :color="stateColor(row.original.state)" variant="soft" size="sm" :label="stateLabel(row.original.state)" /></template>
            <template #adapters-cell="{ row }">
              <div v-if="row.original.adapterIds.length" class="flex min-w-0 flex-wrap gap-1">
                <UBadge v-for="adapterId in row.original.adapterIds.slice(0, 2)" :key="adapterId" color="neutral" variant="soft" size="sm" :label="adapterName(adapterId)" />
                <span v-if="row.original.adapterIds.length > 2" class="text-[9px] text-[var(--text-muted)]">+{{ row.original.adapterIds.length - 2 }}</span>
              </div>
              <span v-else class="text-[9px] text-[var(--text-muted)]">—</span>
            </template>
            <template #count-cell="{ row }"><div class="text-right tabular-nums">{{ row.original.count || '—' }}</div></template>
            <template #lastSeenMs-cell="{ row }"><span class="tabular-nums text-[var(--text-secondary)]">{{ formatTime(row.original.lastSeenMs) }}</span></template>
            <template #empty><UEmpty icon="i-tabler-radar-off" :title="query || adapterFilterIds.length ? t('capture.noMatch') : t('capture.noRecords')" :description="query || adapterFilterIds.length ? t('capture.adjustSearch') : t('capture.noRecordsHint')" /></template>
          </UTable>

          <div v-if="scrollThumbHeight" data-testid="capture-scrollbar" aria-hidden="true" class="absolute inset-y-2 right-1 z-20 w-2 cursor-pointer rounded-full bg-[var(--surface-subtle)] ring-1 ring-inset ring-[var(--border)]" @pointerdown="jumpScrollbar">
            <div data-testid="capture-scrollbar-thumb" class="absolute inset-x-0 cursor-grab rounded-full border-2 border-[var(--surface-subtle)] bg-[var(--text-muted)] active:cursor-grabbing" :style="{ height: `${scrollThumbHeight}px`, transform: `translateY(${scrollThumbTop}px)` }" @pointerdown.stop="beginScrollbarDrag" @pointermove="dragScrollbar" @pointerup="endScrollbarDrag" @pointercancel="endScrollbarDrag" />
          </div>
        </div>
      </ManagementTableFrame>
    </template>

    <ManagementTableFrame v-else v-model:query="listQuery" v-model:page="listPage" v-model:page-size="listPageSize" :search-placeholder="t('capture.searchRuns')" :search-label="t('capture.searchRuns')" :selected-count="listSelected.size" :selected-label="t('capture.runItemLabel')" :total="filteredRuns.length" :item-label="t('capture.runItemLabel')">
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('capture.bulkDelete')" :disabled="bulkRunDeletionBlocked" @click="pendingRemoval = probe.runs.value.filter(run => listSelected.has(run.id))" />
      </template>
      <UTable :data="listPageItems" :columns="runColumns" sticky :ui="{ base: 'min-w-[920px]' }" @dblclick="openRunOnDoubleClick">
        <template #select-header><UCheckbox :model-value="listPageSelected" :aria-label="t('capture.selectRunPage')" @update:model-value="toggleListPageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="listSelected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleListSelection(row.original.id)" /></template>
        <template #run-cell="{ row }">
          <div class="flex min-w-0 items-center gap-1.5">
            <div class="truncate font-semibold">{{ row.original.name }}</div>
            <UBadge v-if="row.original.quickProbe" color="primary" variant="soft" size="sm" :label="t('capture.quickProbe.badge')" />
            <UPopover mode="hover" :open-delay="150" :close-delay="100" :content="{ side: 'right', align: 'start', sideOffset: 6 }" :ui="{ content: 'z-[80] w-80 p-0' }">
              <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-info-circle" class="shrink-0" :aria-label="t('capture.viewTechnicalDetails', { name: row.original.name })" />
              <template #content>
                <div class="space-y-3 p-3">
                  <div>
                    <div class="text-[9px] font-medium text-[var(--text-secondary)]">{{ t('capture.executablePath') }}</div>
                    <div class="mt-1 break-all text-[10px] leading-4 text-[var(--text)]">{{ softwarePath(row.original.softwareId) }}</div>
                  </div>
                  <div>
                    <div class="text-[9px] font-medium text-[var(--text-secondary)]">{{ t('capture.adapters') }}</div>
                    <ul class="m-0 mt-1 space-y-1 p-0" role="list">
                      <li v-for="adapter in adapterDetails(row.original)" :key="adapter.id" class="list-none text-[10px] leading-4 text-[var(--text)]">
                        <span class="text-[var(--text-secondary)]">{{ adapter.technologies }}</span>
                        <span aria-hidden="true"> · </span>{{ adapter.name }}
                      </li>
                    </ul>
                  </div>
                </div>
              </template>
            </UPopover>
          </div>
        </template>
        <template #software-cell="{ row }"><div class="truncate">{{ softwareName(row.original.softwareId) }}</div></template>
        <template #dictionary-cell="{ row }"><div class="truncate font-medium">{{ dictionaryName(row.original.dictionaryId) }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('capture.dictionaryEntries', { count: row.original.dictionaryEntryCount }) }}</div></template>
        <template #status-cell="{ row }">
          <div class="flex flex-col items-start gap-1">
            <UBadge :color="statusColor(row.original.status)" variant="soft" size="sm" :label="statusLabel(row.original.status)" />
            <UBadge v-if="row.original.runtimeCapability" :color="runtimeCapabilityColor(row.original.runtimeCapability)" variant="soft" size="sm" :label="runtimeCapabilityLabel(row.original.runtimeCapability)" />
          </div>
        </template>
        <template #progress-cell="{ row }"><div class="tabular-nums">{{ t('capture.observedCount', { count: row.original.observedCount }) }}</div><div v-if="row.original.ignoredCount" class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('capture.ignoredCount', { count: row.original.ignoredCount }) }}</div></template>
        <template #updatedAtMs-cell="{ row }"><span class="tabular-nums text-[var(--text-secondary)]">{{ formatTime(row.original.updatedAtMs) }}</span></template>
        <template #actions-cell="{ row }"><div class="flex justify-center gap-0.5"><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-arrow-right" :aria-label="t('capture.openNamed', { name: row.original.name })" @click="probe.selectRun(row.original.id)" /><UButton v-if="row.original.quickProbe" color="error" variant="ghost" size="xs" icon="i-tabler-trash-x" :aria-label="t('capture.quickProbe.cleanupNamed', { name: row.original.name })" @click="pendingQuickCleanup = row.original" /><UButton v-else color="error" variant="ghost" size="xs" icon="i-tabler-trash" :disabled="['running', 'paused'].includes(row.original.status)" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original]" /></div></template>
        <template #empty><UEmpty icon="i-tabler-radar-off" :title="listQuery ? t('capture.noRunMatch') : t('capture.empty')" :description="listQuery ? t('capture.adjustSearch') : t('capture.emptyHint')" /></template>
      </UTable>
    </ManagementTableFrame>

    <QuickProbeLauncher
      v-model:open="quickProbeOpen"
      :software="software"
      :dictionaries="dictionaries"
      :capture-armed="captureArmed"
      :capture-shortcut="captureShortcut"
      :capture-result="captureResult"
      :capture-error="captureError"
      @arm-capture="emit('arm-capture')"
      @cancel-capture="emit('cancel-capture')"
      @started="handleQuickProbeStarted"
    />

    <ManagementFormModal
      :open="settingsOpen"
      :title="t('capture.settingsTitle')"
      :description="t('capture.settingsDescription')"
      :confirm-label="t('capture.saveSettings')"
      :confirm-disabled="probe.busy.value || !settingsValid || !settingsChanged"
      :busy="probe.busy.value"
      width="md"
      @update:open="$event || (settingsOpen = false)"
      @confirm="applySettings"
    >
      <div class="space-y-4">
        <UFormField :label="t('capture.runName')" required>
          <UInput v-model="settingsName" :maxlength="128" class="w-full" />
        </UFormField>

        <UFormField :label="t('capture.adapters')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : settingsCompatibilityLoading ? t('capture.loadingCompatibleAdapters') : compatibleSettingsAdapters.length ? t('capture.adaptersHint') : t('capture.noCompatibleAdapters')" required><ProbeAdapterPicker v-model="settingsAdapterIds" :adapters="compatibleSettingsAdapters" :disabled="settingsConfigurationLocked || settingsCompatibilityLoading" /></UFormField>

        <UFormField :label="t('capture.livePreview')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : settingsPreviewAvailable ? t('capture.livePreviewHint') : t('capture.livePreviewUnavailable')">
          <USwitch v-model="settingsLivePreview" :disabled="settingsConfigurationLocked || !settingsPreviewAvailable" />
        </UFormField>

        <section class="border-t border-[var(--border)] pt-4" :aria-labelledby="'capture-danger-title'">
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0">
              <h3 id="capture-danger-title" class="m-0 text-[11px] font-semibold text-[var(--text)]">{{ t('capture.dangerTitle') }}</h3>
              <p class="mt-1 mb-0 max-w-[42ch] text-[9px] leading-4 text-[var(--text-muted)]">{{ settingsConfigurationLocked ? t('capture.clearAllLocked') : t('capture.clearAllHint') }}</p>
            </div>
            <UButton data-testid="capture-clear-all" color="error" variant="soft" size="sm" icon="i-tabler-trash-x" :label="t('capture.clearAll')" :disabled="settingsConfigurationLocked || (!selectedRun?.observedCount && !selectedRun?.dictionaryEntryCount)" @click="requestClearAll" />
          </div>
        </section>
      </div>
    </ManagementFormModal>

    <ConfirmDialog :open="Boolean(pendingRemoval.length)" :title="t('capture.deleteTitle')" :description="t('capture.deleteDescription', { count: pendingRemoval.length })" :busy="probe.busy.value" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
    <ConfirmDialog
      :open="Boolean(pendingQuickCleanup)"
      :title="t('capture.quickProbe.cleanupTitle')"
      :description="t('capture.quickProbe.cleanupDescription')"
      :confirm-label="t('capture.quickProbe.cleanupConfirm')"
      :busy="probe.busy.value"
      @update:open="$event || (pendingQuickCleanup = null)"
      @confirm="confirmQuickProbeCleanup"
    />
    <ConfirmDialog
      :open="clearAllOpen"
      :title="t('capture.clearAllTitle')"
      :description="t('capture.clearAllDescription', { dictionary: selectedDictionary?.metadata.name ?? selectedRun?.dictionaryId ?? '' })"
      :confirm-label="t('capture.clearAllConfirm')"
      :busy="probe.busy.value"
      @update:open="$event || (clearAllOpen = false)"
      @confirm="confirmClearAll"
    />
  </section>
</template>

<style scoped>
.capture-table-scroll {
  scrollbar-color: var(--text-muted) var(--surface-subtle);
}

.capture-table-scroll::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

.capture-table-scroll::-webkit-scrollbar-track {
  background: var(--surface-subtle);
}

.capture-table-scroll::-webkit-scrollbar-thumb {
  border: 2px solid var(--surface-subtle);
  background: var(--text-secondary);
  background-clip: content-box;
}
</style>
