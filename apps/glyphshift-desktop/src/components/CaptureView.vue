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
  SoftwareRecord,
} from '../model'
import { editableRowIndex } from '../tableInteraction'
import { useProbeRuns } from '../useProbeRuns'
import { usePageEscape } from '../usePageEscape'
import ConfirmDialog from './ConfirmDialog.vue'
import ManagementFormModal from './ManagementFormModal.vue'
import ManagementPageHeader from './ManagementPageHeader.vue'
import ManagementTableFrame from './ManagementTableFrame.vue'
import ProbeAdapterPicker from './ProbeAdapterPicker.vue'

const props = defineProps<{
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  adapters: AdapterOption[]
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
const creating = ref(false)
const createName = ref('')
const createSoftwareId = ref('')
const createAdapterIds = ref<string[]>([])
const createLivePreview = ref(false)
const createDictionaryMode = ref<'existing' | 'new'>('existing')
const createDictionaryId = ref('')
const newDictionaryName = ref('')
const newSourceLocale = ref('en-US')
const newTargetLocale = ref('zh-CN')
const settingsOpen = ref(false)
const settingsName = ref('')
const settingsAdapterIds = ref<string[]>([])
const settingsLivePreview = ref(false)
const clearAllOpen = ref(false)
const translationValues = ref<Record<string, string>>({})
const tableShell = ref<HTMLElement>()
const tableScrollState = ref({ top: 0, clientHeight: 0, scrollHeight: 0 })
const dirtyTranslations = new Set<string>()
const editTimers = new Map<string, ReturnType<typeof setTimeout>>()
let queryTimer: ReturnType<typeof setTimeout> | undefined
let pollTimer: ReturnType<typeof setInterval> | undefined
let scrollDrag: { pointerId: number; startY: number; startTop: number } | undefined

const observableAdapters = computed(() => props.adapters.filter(adapter => adapter.features.includes('textObserve')))
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
const createPreviewAvailable = computed(() => Boolean(createAdapterIds.value.length)
  && createAdapterIds.value.some(id => props.adapters.find(adapter => adapter.id === id)?.features.includes('textReplace')))
const settingsPreviewAvailable = computed(() => Boolean(settingsAdapterIds.value.length)
  && settingsAdapterIds.value.some(id => props.adapters.find(adapter => adapter.id === id)?.features.includes('textReplace')))
const settingsConfigurationLocked = computed(() => ['running', 'paused'].includes(selectedRun.value?.status ?? ''))
const settingsValid = computed(() => Boolean(
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
const createValid = computed(() => Boolean(
  createName.value.trim()
  && createSoftwareId.value
  && createAdapterIds.value.length
  && (createDictionaryMode.value === 'existing'
    ? createDictionaryId.value
    : newDictionaryName.value.trim()
      && newSourceLocale.value.trim()
      && newTargetLocale.value.trim()),
))
const activeRunExists = computed(() => probe.runs.value.some(run => ['running', 'paused'].includes(run.status)))
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
const scrollTrackHeight = computed(() => Math.max(0, tableScrollState.value.clientHeight - 16))
const scrollThumbHeight = computed(() => {
  const { clientHeight, scrollHeight } = tableScrollState.value
  if (!clientHeight || scrollHeight <= clientHeight) return 0
  return Math.max(32, scrollTrackHeight.value * clientHeight / scrollHeight)
})
const scrollThumbTop = computed(() => {
  const { top, clientHeight, scrollHeight } = tableScrollState.value
  const scrollRange = scrollHeight - clientHeight
  const thumbRange = scrollTrackHeight.value - scrollThumbHeight.value
  return scrollRange > 0 && thumbRange > 0 ? top / scrollRange * thumbRange : 0
})

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

watch(createAdapterIds, () => {
  createLivePreview.value = createPreviewAvailable.value
}, { deep: true })

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
  window.addEventListener('resize', updateScrollMetrics)
  pollTimer = setInterval(() => void poll(), 1000)
})

onBeforeUnmount(() => {
  if (queryTimer) clearTimeout(queryTimer)
  if (pollTimer) clearInterval(pollTimer)
  window.removeEventListener('resize', updateScrollMetrics)
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

function resetCreateForm() {
  createName.value = ''
  createSoftwareId.value = ''
  createAdapterIds.value = []
  createLivePreview.value = false
  createDictionaryMode.value = 'existing'
  createDictionaryId.value = ''
  newDictionaryName.value = ''
  newSourceLocale.value = 'en-US'
  newTargetLocale.value = 'zh-CN'
}

function closeCreate() {
  creating.value = false
  resetCreateForm()
}

function openCreate() {
  resetCreateForm()
  const software = props.software[0]
  createSoftwareId.value = software?.id ?? ''
  createAdapterIds.value = observableAdapters.value.map(adapter => adapter.id)
  createLivePreview.value = createPreviewAvailable.value
  createDictionaryMode.value = props.dictionaries.length ? 'existing' : 'new'
  createDictionaryId.value = props.dictionaries[0]?.metadata.id ?? ''
  creating.value = true
}

function openSettings() {
  const run = selectedRun.value
  if (!run) return
  settingsName.value = run.name
  settingsAdapterIds.value = [...run.adapterIds]
  settingsLivePreview.value = run.livePreviewEnabled
  settingsOpen.value = true
}

async function createRun() {
  if (!createValid.value) return
  const dictionary = createDictionaryMode.value === 'existing'
    ? { kind: 'existing' as const, dictionaryId: createDictionaryId.value }
    : {
        kind: 'new' as const,
        id: `dictionary.probe-${crypto.randomUUID()}`,
        name: newDictionaryName.value.trim(),
        description: '',
        sourceLocale: newSourceLocale.value.trim(),
        targetLocale: newTargetLocale.value.trim(),
      }
  const created = await probe.create({
    id: `probe-${crypto.randomUUID()}`,
    name: createName.value.trim(),
    softwareId: createSoftwareId.value,
    adapterIds: [...createAdapterIds.value],
    livePreviewEnabled: createLivePreview.value,
    dictionary,
  })
  if (created) {
    closeCreate()
    await loadPage()
  }
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

function tableScrollElement() {
  return tableShell.value?.querySelector<HTMLElement>('[data-testid="capture-table-scroll"]')
}

function updateScrollMetrics() {
  const element = tableScrollElement()
  if (!element) return
  tableScrollState.value = {
    top: element.scrollTop,
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
  }
}

function jumpScrollbar(event: PointerEvent) {
  const element = tableScrollElement()
  const track = event.currentTarget as HTMLElement
  if (!element || !scrollThumbHeight.value) return
  const bounds = track.getBoundingClientRect()
  const thumbRange = Math.max(1, bounds.height - scrollThumbHeight.value)
  const target = Math.min(thumbRange, Math.max(0, event.clientY - bounds.top - scrollThumbHeight.value / 2))
  element.scrollTop = target / thumbRange * (element.scrollHeight - element.clientHeight)
  updateScrollMetrics()
}

function beginScrollbarDrag(event: PointerEvent) {
  const element = tableScrollElement()
  if (!element) return
  scrollDrag = { pointerId: event.pointerId, startY: event.clientY, startTop: element.scrollTop }
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  event.preventDefault()
}

function dragScrollbar(event: PointerEvent) {
  const element = tableScrollElement()
  if (!element || !scrollDrag || scrollDrag.pointerId !== event.pointerId) return
  const scrollRange = element.scrollHeight - element.clientHeight
  const thumbRange = scrollTrackHeight.value - scrollThumbHeight.value
  if (thumbRange <= 0) return
  element.scrollTop = scrollDrag.startTop + (event.clientY - scrollDrag.startY) * scrollRange / thumbRange
  updateScrollMetrics()
}

function endScrollbarDrag(event: PointerEvent) {
  if (scrollDrag?.pointerId === event.pointerId) scrollDrag = undefined
}

function softwareName(id: string) {
  return props.software.find(item => item.id === id)?.name ?? id
}

function dictionaryName(id: string) {
  return props.dictionaries.find(item => item.metadata.id === id)?.metadata.name ?? id
}

function adapterName(id: string) {
  return props.adapters.find(adapter => adapter.id === id)?.name ?? id
}

function statusLabel(status: string) {
  return t(`capture.status.${status}`)
}

function statusColor(status: string) {
  if (status === 'running') return 'success'
  if (status === 'paused') return 'warning'
  if (status === 'interrupted') return 'error'
  return 'neutral'
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
        <UBadge :color="statusColor(selectedRun.status)" variant="soft" size="sm" :label="statusLabel(selectedRun.status)" />
      </template>
      <template #actions>
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-settings" :label="t('capture.settings')" @click="openSettings" />
        <UDropdownMenu :items="exportItems" :content="{ align: 'end' }">
          <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-download" trailing-icon="i-tabler-chevron-down" :label="t('capture.export')" />
        </UDropdownMenu>
        <UButton v-if="['running', 'paused'].includes(selectedRun.status)" color="neutral" variant="ghost" size="sm" icon="i-tabler-plug-off" :aria-label="t('capture.disconnect')" @click="probe.disconnect(selectedRun.id)" />
        <UButton v-if="selectedRun.status === 'running'" color="neutral" variant="outline" size="sm" icon="i-tabler-player-pause" :label="t('capture.pause')" :loading="probe.busy.value" @click="probe.setPaused(selectedRun.id, true)" />
        <UButton v-else color="primary" :variant="selectedRun.status === 'paused' ? 'soft' : 'solid'" size="sm" icon="i-tabler-player-play" :label="selectedRun.status === 'paused' ? t('capture.continue') : t('capture.resume')" :loading="probe.busy.value" @click="selectedRun.status === 'paused' ? probe.setPaused(selectedRun.id, false) : probe.resume(selectedRun.id)" />
      </template>
    </ManagementDetailHeader>
    <ManagementPageHeader v-else title-id="capture-title" icon="i-tabler-radar" :title="t('capture.title')" :description="t('capture.description')">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('capture.createRun')" :disabled="!software.length || !observableAdapters.length || activeRunExists" @click="openCreate" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="probe.message.value" class="mb-3" />

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
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('capture.bulkDelete')" :disabled="[...listSelected].some(id => ['running', 'paused'].includes(probe.runs.value.find(run => run.id === id)?.status ?? ''))" @click="pendingRemoval = probe.runs.value.filter(run => listSelected.has(run.id))" />
      </template>
      <UTable :data="listPageItems" :columns="runColumns" sticky :ui="{ base: 'min-w-[920px]' }" @dblclick="openRunOnDoubleClick">
        <template #select-header><UCheckbox :model-value="listPageSelected" :aria-label="t('capture.selectRunPage')" @update:model-value="toggleListPageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="listSelected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleListSelection(row.original.id)" /></template>
        <template #run-cell="{ row }"><div class="truncate font-semibold">{{ row.original.name }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ row.original.adapterIds.map(adapterName).join(' · ') }}</div></template>
        <template #software-cell="{ row }"><div class="truncate">{{ softwareName(row.original.softwareId) }}</div></template>
        <template #dictionary-cell="{ row }"><div class="truncate font-medium">{{ dictionaryName(row.original.dictionaryId) }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('capture.dictionaryEntries', { count: row.original.dictionaryEntryCount }) }}</div></template>
        <template #status-cell="{ row }"><UBadge :color="statusColor(row.original.status)" variant="soft" size="sm" :label="statusLabel(row.original.status)" /></template>
        <template #progress-cell="{ row }"><div class="tabular-nums">{{ t('capture.observedCount', { count: row.original.observedCount }) }}</div><div v-if="row.original.ignoredCount" class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('capture.ignoredCount', { count: row.original.ignoredCount }) }}</div></template>
        <template #updatedAtMs-cell="{ row }"><span class="tabular-nums text-[var(--text-secondary)]">{{ formatTime(row.original.updatedAtMs) }}</span></template>
        <template #actions-cell="{ row }"><div class="flex justify-center gap-0.5"><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-arrow-right" :aria-label="t('capture.openNamed', { name: row.original.name })" @click="probe.selectRun(row.original.id)" /><UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :disabled="['running', 'paused'].includes(row.original.status)" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original]" /></div></template>
        <template #empty><UEmpty icon="i-tabler-radar-off" :title="listQuery ? t('capture.noRunMatch') : t('capture.empty')" :description="listQuery ? t('capture.adjustSearch') : t('capture.emptyHint')" /></template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal :open="creating" :title="t('capture.createRun')" :description="t('capture.createDescription')" :confirm-label="t('capture.createConfirm')" :confirm-disabled="probe.busy.value || !createValid" :busy="probe.busy.value" width="lg" @update:open="$event || closeCreate()" @confirm="createRun">
      <div class="space-y-3">
        <UFormField :label="t('capture.runName')" required><UInput v-model="createName" :maxlength="128" class="w-full" /></UFormField>
        <UFormField :label="t('capture.chooseSoftware')" required><USelect v-model="createSoftwareId" :items="software.map(item => ({ value: item.id, label: item.name }))" value-key="value" label-key="label" class="w-full" /></UFormField>
        <UFormField :label="t('capture.dictionaryBinding')" required>
          <div class="mb-2 flex rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('capture.dictionaryBinding')">
            <UButton
              class="flex-1"
              :color="createDictionaryMode === 'existing' ? 'primary' : 'neutral'"
              size="xs"
              :variant="createDictionaryMode === 'existing' ? 'soft' : 'ghost'"
              :label="t('capture.useExistingDictionary')"
              :disabled="!dictionaries.length"
              :aria-pressed="createDictionaryMode === 'existing'"
              @click="createDictionaryMode = 'existing'"
            />
            <UButton
              class="flex-1"
              :color="createDictionaryMode === 'new' ? 'primary' : 'neutral'"
              size="xs"
              :variant="createDictionaryMode === 'new' ? 'soft' : 'ghost'"
              :label="t('capture.createDictionary')"
              :aria-pressed="createDictionaryMode === 'new'"
              @click="createDictionaryMode = 'new'"
            />
          </div>
          <USelect v-if="createDictionaryMode === 'existing'" v-model="createDictionaryId" :items="dictionaries.map(item => ({ value: item.metadata.id, label: item.metadata.name }))" value-key="value" label-key="label" class="w-full" />
          <div v-else class="space-y-3 rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
            <p class="m-0 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('capture.createDictionaryHint') }}</p>
            <UFormField :label="t('capture.dictionaryName')" required>
              <UInput v-model="newDictionaryName" :maxlength="128" class="w-full" />
            </UFormField>
            <div class="grid grid-cols-2 gap-3">
              <UFormField :label="t('capture.sourceLocale')" required><UInput v-model="newSourceLocale" class="w-full" /></UFormField>
              <UFormField :label="t('capture.targetLocale')" required><UInput v-model="newTargetLocale" class="w-full" /></UFormField>
            </div>
          </div>
        </UFormField>
        <UFormField :label="t('capture.adapters')" :hint="t('capture.adaptersHint')" required><ProbeAdapterPicker v-model="createAdapterIds" :adapters="observableAdapters" /></UFormField>
        <UFormField :label="t('capture.livePreview')" :hint="createPreviewAvailable ? t('capture.livePreviewHint') : t('capture.livePreviewUnavailable')"><USwitch v-model="createLivePreview" :disabled="!createPreviewAvailable" /></UFormField>
      </div>
    </ManagementFormModal>

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

        <UFormField :label="t('capture.adapters')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : t('capture.adaptersHint')" required><ProbeAdapterPicker v-model="settingsAdapterIds" :adapters="observableAdapters" :disabled="settingsConfigurationLocked" /></UFormField>

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
