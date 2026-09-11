<script setup lang="ts">
import { workflowLifecycle, workflowOperations, workflowStateColor } from '../workflowLifecycle'
import { useWorkflowWorkspace } from '../workspace/workflows'
import { model as workspaceModel, messages as workspaceMessages } from '../workspace/state'
import { translateCommandError } from '../commandError'
import { useAppSettings } from '../appSettings'
import { latestRequestQueue } from '../latestRequestQueue'
import { useProbeAutoComplete } from '../useProbeAutoComplete'
import { invoke } from '@tauri-apps/api/core'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { open, save } from '@tauri-apps/plugin-dialog'
import type { DropdownMenuItem, TableColumn } from '@nuxt/ui'
import { useI18n } from 'vue-i18n'
import { adapterDisplayName, adapterSummary } from '../adapterPresentation'
import type {
  AdapterOption,
  DictionarySummary,
  ProbeEntryRow,
  ProbeExportFormat,
  ProbeRunSummary,
  SoftwarePreflight,
  SoftwareRecord,
} from '../model'
import { useAiTranslation, type AiTranslationPlan } from '../useAiTranslation'
import {
  editableRowIndex,
  managementActionsColumnMeta,
  managementIdentityColumnMeta,
  managementSelectionColumnMeta,
} from '../tableInteraction'
import { useCaptureScrollbar } from '../useCaptureScrollbar'
import { useProbeRuns, type ProbeTranslationFilter } from '../useProbeRuns'
import { usePageEscape } from '../usePageEscape'
import { useTableColumns } from '../useTableColumns'
import AiTranslationPreflight from './AiTranslationPreflight.vue'
import ConfirmDialog from './ConfirmDialog.vue'
import ManagementFormModal from './ManagementFormModal.vue'
import ManagementPageHeader from './ManagementPageHeader.vue'
import ManagementTableFrame from './ManagementTableFrame.vue'
import ProbeAdapterPicker from './ProbeAdapterPicker.vue'
import QuickProbeLauncher from './QuickProbeLauncher.vue'

const props = defineProps<{
  workflowId?: string | null
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  adapters: AdapterOption[]
  captureArmed: boolean
  captureShortcut: string
  captureResult: SoftwarePreflight | null
  captureError: string
}>()

const emit = defineEmits<{
  'back-workflow': []
  'workflow-settings': []
  'arm-capture': []
  'cancel-capture': []
  'open-dictionary': [id: string]
  'workspace-changed': []
  'configure-ai': []
  'open-ai-tasks': []
}>()

const { t, locale } = useI18n()
const probe = useProbeRuns()
const workflowWorkspace = useWorkflowWorkspace()
const lifecycle = computed(() => props.workflowId ? workflowLifecycle(props.workflowId) : null)
const stopRequested = computed(() => lifecycle.value?.enabled || lifecycle.value?.phase === 'stop_failed')
const workflowCommandBusy = computed(() => props.workflowId && ['connecting', 'stopping'].includes(workflowOperations.value[props.workflowId] ?? ''))
const lifecycleMessage = computed(() => {
  if (!props.workflowId) return ''
  return workspaceMessages.value[props.workflowId] || Object.values(workspaceModel.value.workflowRuntimeStatus[props.workflowId]?.errors ?? {})
    .filter(error => error.code !== 'runtime.target_not_found').map(translateCommandError).join('；')
})
const collectionEnabled = computed(() => lifecycle.value?.collectNewSources ?? true)
const hasWriter = computed(() => workspaceModel.value.workflows.find(item => item.id === props.workflowId)?.targets.some(target => target.writeDictionaryId))
async function setCollection(enabled: boolean) {
  if (props.workflowId) await workflowWorkspace.setWorkflowCollection(props.workflowId, enabled)
}

const ai = useAiTranslation()
const autoComplete = useProbeAutoComplete()
const query = ref('')
const adapterFilterIds = ref<string[]>([])
const translationFilter = ref<ProbeTranslationFilter>('all')
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
const textSettings = useAppSettings()
const selected = ref(new Set<string>())
const listQuery = ref('')
const listPage = ref(1)
const listPageSize = ref(20)
const listSelected = ref(new Set<string>())
const { columns: runVisibleColumns, toggleColumn: toggleRunColumn } = useTableColumns('glyphshift.table-columns.probes.runs', {
  software: true,
  dictionary: true,
  status: true,
  progress: true,
  updated: true,
})
const { columns: entryVisibleColumns, toggleColumn: toggleEntryColumn } = useTableColumns('glyphshift.table-columns.probes.entries', {
  status: true,
  adapters: true,
  count: true,
  lastSeen: true,
})
const pendingRemoval = ref<ProbeRunSummary[]>([])
const quickProbeOpen = ref(false)
const settingsOpen = ref(false)
const settingsName = ref('')
const pendingImport = ref<{ runId: string; path: string; format: 'json' | 'csv' } | null>(null)
const importMode = ref('overwrite')
const importModeItems = computed(() => ['overwrite', 'keep_existing', 'replace'].map(value => ({ value, label: t(`capture.importModes.${value}`) })))
const settingsDictionaryId = ref('')
const settingsExcludedDictionaryIds = ref<string[]>([])
const settingsAdapterIds = ref<string[]>([])
const settingsLivePreview = ref(false)
const settingsCompatibleAdapterIds = ref<string[] | null>(null)
const settingsCompatibilityLoading = ref(false)
const clearAllOpen = ref(false)
const mergeRules = ref(true)
const dictionaryNotice = ref('')
const refreshingText = ref(false)
const launchingSoftware = ref(false)
const disconnectingRunId = ref('')
const translationValues = ref<Record<string, string>>({})
const focusedTranslation = ref<string | null>(null)
const selectedAiProfileId = ref<string | null>(null)
const aiPreviewOpen = ref(false)
const aiPreflightOpen = ref(false)
const aiPlan = ref<AiTranslationPlan | null>(null)
const aiNotice = ref('')
const aiNoticeTone = ref<'success' | 'warning'>('success')
const aiNoticeCancelled = ref(false)
const aiRetryAvailable = ref(false)
const aiNoticeTitle = computed(() => aiNoticeCancelled.value
  ? t('ai.translationCancelled')
  : aiNoticeTone.value === 'warning'
  ? t('ai.partialCompletion')
  : t('ai.translationCompleted'))
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
let pageRequest = 0
let disposed = false
let pollingPage = false
const pageQueue = latestRequestQueue(async () => loadPageNow(pageRequest))
let settingsCompatibilityRequest = 0
let translationSaveQueue = Promise.resolve()

const observableAdapters = computed(() => props.adapters.filter(adapter => adapter.features.includes('textObserve')))
const compatibleSettingsAdapters = computed(() => settingsCompatibleAdapterIds.value === null
  ? observableAdapters.value
  : observableAdapters.value.filter(adapter => settingsCompatibleAdapterIds.value?.includes(adapter.id)))
const selectedRun = computed(() => probe.selectedRun.value)
const displayedAiJob = computed(() => {
  const job = ai.currentJob.value
  return job && selectedRun.value && job.scopeId === `probe:${selectedRun.value.id}` ? job : null
})
const activeDisplayedAiJob = computed(() => {
  const job = displayedAiJob.value
  return job && !['completed', 'completed_with_failures', 'cancelled', 'interrupted'].includes(job.status)
    ? job
    : null
})
const selectedSoftware = computed(() => props.software.find(item => item.id === selectedRun.value?.softwareId))
const selectedDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === selectedRun.value?.dictionaryId))
const selectedRunDictionaryName = computed(() => (
  selectedRun.value ? runDictionaryName(selectedRun.value) : ''
))
const settingsDictionary = computed(() => props.dictionaries.find(item => item.metadata.id === settingsDictionaryId.value))
const settingsDictionaryItems = computed(() => props.dictionaries.map(item => ({
  value: item.metadata.id,
  label: `${item.metadata.name} · ${item.metadata.sourceLocale} → ${item.metadata.targetLocale}`,
})))
const selectedRunMetadata = computed(() => {
  const run = selectedRun.value
  if (!run) return ''
  const parts = [
    selectedSoftware.value?.name ?? run.softwareId,
    selectedRunDictionaryName.value,
    t('capture.runSummary', { observed: run.observedCount, entries: run.dictionaryEntryCount }),
  ]
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
const translationFilterOptions = computed(() => (['all', 'untranslated', 'translated', 'rule_matched', 'other_dictionary'] as const).map(value => ({
  value,
  label: ['rule_matched', 'other_dictionary'].includes(value) ? t(`captureResolution.filter_${value}`) : t(`capture.translationFilter.${value}`),
})))
const translationFilterLabel = computed(() => translationFilterOptions.value.find(option => (
  option.value === translationFilter.value
))?.label ?? t('capture.translationFilter.all'))
const currentSources = computed(() => entryPage.value.rows.map(row => row.source))
const pageSelected = computed(() => Boolean(currentSources.value.length) && currentSources.value.every(source => selected.value.has(source)))
const selectedRows = computed(() => entryPage.value.rows.filter(row => selected.value.has(row.source)))
const settingsPreviewAvailable = computed(() => Boolean(settingsAdapterIds.value.length)
  && settingsAdapterIds.value.some(id => props.adapters.find(adapter => adapter.id === id)?.features.includes('textReplace')))
const settingsConfigurationLocked = computed(() => ['running', 'paused'].includes(selectedRun.value?.status ?? ''))
const clearEntriesLocked = computed(() => selectedRun.value?.status === 'running')
const selectedAiProfile = computed(() => ai.profiles.value.find(profile => (
  profile.id === (selectedAiProfileId.value ?? ai.catalog.value.defaultProfileId)
)) ?? ai.defaultProfile.value)
const autoCompleteEnabled = computed(() => Boolean(selectedRun.value && autoComplete.runs.value[selectedRun.value.id]))
const autoCompleteDisabled = computed(() => !selectedRun.value || (!autoCompleteEnabled.value && (!selectedAiProfile.value || (lifecycle.value ? lifecycle.value.phase !== 'running' : selectedRun.value.status !== 'running'))))
function toggleAutoComplete(checked: boolean) {
  const run = selectedRun.value
  if (!run) return
  if (checked && selectedAiProfile.value) autoComplete.start(run.id, selectedAiProfile.value.id, props.workflowId ?? undefined)
  else autoComplete.stop(run.id)
}
const aiMenuItems = computed<DropdownMenuItem[][]>(() => [
  [{
    label: t('ai.previewCandidates'),
    icon: 'i-tabler-list-check',
    disabled: !selectedAiProfile.value || ai.busy.value,
    onSelect: () => void previewAiTranslation(),
  }],
  ai.profiles.value.map(profile => ({
    label: t('ai.useProfile', { name: profile.name }),
    icon: selectedAiProfile.value?.id === profile.id ? 'i-tabler-check' : 'i-tabler-sparkles',
    onSelect: () => { selectedAiProfileId.value = profile.id; const run = selectedRun.value; if (run && autoComplete.runs.value[run.id]) autoComplete.start(run.id, profile.id) },
  })),
  [{
    label: t('ai.manageProfiles'),
    icon: 'i-tabler-settings',
    onSelect: () => emit('configure-ai'),
  }],
])
const settingsValid = computed(() => Boolean(
  !settingsCompatibilityLoading.value
  &&
  settingsName.value.trim()
  && settingsDictionary.value
  && settingsAdapterIds.value.length
  && (!settingsLivePreview.value || settingsPreviewAvailable.value),
))
const settingsChanged = computed(() => {
  const run = selectedRun.value
  if (!run) return false
  return (
    settingsName.value.trim() !== run.name
    || settingsDictionaryId.value !== run.dictionaryId
    || JSON.stringify(settingsExcludedDictionaryIds.value) !== JSON.stringify(run.excludedDictionaryIds ?? [])
    || JSON.stringify(settingsAdapterIds.value) !== JSON.stringify(run.adapterIds)
    || settingsLivePreview.value !== run.livePreviewEnabled
  )
})
const activeRunExists = computed(() => Boolean(probe.activityStatus.value))
const bulkRunDeletionBlocked = computed(() => [...listSelected.value].some((id) => {
  const run = probe.runs.value.find(item => item.id === id)
  return ['running', 'paused'].includes(run?.status ?? '')
}))
const filteredRuns = computed(() => {
  const needle = listQuery.value.trim().toLowerCase()
  if (!needle) return probe.runs.value
  return probe.runs.value.filter(run => [
    run.name,
    softwareName(run.softwareId),
    runDictionaryName(run),
  ].some(value => value.toLowerCase().includes(needle)))
})
const listPageItems = computed(() => filteredRuns.value.slice(
  (listPage.value - 1) * listPageSize.value,
  listPage.value * listPageSize.value,
))
const listPageSelected = computed(() => Boolean(listPageItems.value.length)
  && listPageItems.value.every(run => listSelected.value.has(run.id)))

const runColumnOptions = computed(() => [
  { key: 'software', label: t('capture.columns.software'), visible: runVisibleColumns.value.software },
  { key: 'dictionary', label: t('capture.columns.dictionary'), visible: runVisibleColumns.value.dictionary },
  { key: 'status', label: t('capture.columns.status'), visible: runVisibleColumns.value.status },
  { key: 'progress', label: t('capture.columns.progress'), visible: runVisibleColumns.value.progress },
  { key: 'updated', label: t('capture.columns.updated'), visible: runVisibleColumns.value.updated },
])
const runColumns = computed<TableColumn<ProbeRunSummary>[]>(() => [
  { id: 'select', header: '', meta: managementSelectionColumnMeta() },
  { id: 'run', header: t('capture.columns.run'), meta: managementIdentityColumnMeta('w-60') },
  ...(runVisibleColumns.value.software ? [{ id: 'software', header: t('capture.columns.software'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<ProbeRunSummary>] : []),
  ...(runVisibleColumns.value.dictionary ? [{ id: 'dictionary', header: t('capture.columns.dictionary'), meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } } satisfies TableColumn<ProbeRunSummary>] : []),
  ...(runVisibleColumns.value.status ? [{ accessorKey: 'status', header: t('capture.columns.status'), meta: { class: { th: 'w-56', td: 'w-56' } } } satisfies TableColumn<ProbeRunSummary>] : []),
  ...(runVisibleColumns.value.progress ? [{ id: 'progress', header: t('capture.columns.progress'), meta: { class: { th: 'w-32', td: 'w-32' } } } satisfies TableColumn<ProbeRunSummary>] : []),
  ...(runVisibleColumns.value.updated ? [{ accessorKey: 'updatedAtMs', header: t('capture.columns.updated'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<ProbeRunSummary>] : []),
  { id: 'actions', header: t('capture.columns.actions'), meta: managementActionsColumnMeta('w-20') },
])
const entryColumnOptions = computed(() => [
  { key: 'status', label: t('capture.columns.status'), visible: entryVisibleColumns.value.status },
  { key: 'adapters', label: t('capture.columns.adapter'), visible: entryVisibleColumns.value.adapters },
  { key: 'count', label: t('capture.columns.count'), visible: entryVisibleColumns.value.count },
  { key: 'lastSeen', label: t('capture.columns.lastSeen'), visible: entryVisibleColumns.value.lastSeen },
])
const entryColumns = computed<TableColumn<ProbeEntryRow>[]>(() => [
  { id: 'select', header: '', meta: managementSelectionColumnMeta() },
  { accessorKey: 'source', header: t('capture.columns.source'), meta: managementIdentityColumnMeta('w-60') },
  { accessorKey: 'translation', header: t('capture.columns.translation'), meta: { class: { th: 'w-[30%]', td: 'w-[30%]' } } },
  ...(entryVisibleColumns.value.status ? [{ accessorKey: 'state', header: t('capture.columns.status'), meta: { class: { th: 'w-56', td: 'w-56' } } } satisfies TableColumn<ProbeEntryRow>] : []),
  ...(entryVisibleColumns.value.adapters ? [{ id: 'adapters', header: t('capture.columns.adapter'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<ProbeEntryRow>] : []),
  ...(entryVisibleColumns.value.count ? [{ accessorKey: 'count', header: t('capture.columns.count'), meta: { class: { th: 'w-20 text-right', td: 'w-20 text-right' } } } satisfies TableColumn<ProbeEntryRow>] : []),
  ...(entryVisibleColumns.value.lastSeen ? [{ accessorKey: 'lastSeenMs', header: t('capture.columns.lastSeen'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<ProbeEntryRow>] : []),
])
const exportItems = computed<DropdownMenuItem[][]>(() => [[
  { label: t('capture.importJson'), icon: 'i-tabler-file-import', onSelect: () => void chooseImport('json') },
  { label: t('capture.importCsv'), icon: 'i-tabler-file-import', onSelect: () => void chooseImport('csv') },
], [
  { label: t('capture.exportJson'), icon: 'i-tabler-file-export', onSelect: () => void chooseExport('entries_json') },
  { label: t('capture.exportCsv'), icon: 'i-tabler-file-export', onSelect: () => void chooseExport('entries_csv') },
]])
const taskActionLabel = computed(() => t('capture.taskActions'))
const taskActionItems = computed<DropdownMenuItem[][]>(() => selectedRun.value ? [[{
  label: t('capture.launchSoftware'), icon: 'i-tabler-app-window', disabled: launchingSoftware.value || !selectedSoftware.value?.executablePath, onSelect: () => void launchSelectedSoftware(),
}, {
  label: t('capture.refreshText'), icon: 'i-tabler-refresh', disabled: refreshingText.value || probe.busy.value || !selectedRun.value.livePreviewEnabled || !['running', 'paused'].includes(selectedRun.value.status), onSelect: () => void refreshTargetText(),
}], [{ type: 'checkbox' as const, label: t('capture.mergeRules'), checked: mergeRules.value, onSelect: (event: Event) => event.preventDefault(), onUpdateChecked: (checked: boolean) => void setMergeRules(checked) }], ...(props.workflowId ? [[{ type: 'checkbox' as const, label: t('workflowLifecycle.collect'), description: t('workflowLifecycle.collectHint'), checked: collectionEnabled.value, disabled: !hasWriter.value, onSelect: (event: Event) => event.preventDefault(), onUpdateChecked: (checked: boolean) => void setCollection(checked) }]] : []), ...(!props.workflowId ? [[{ label: t('capture.settings'), icon: 'i-tabler-settings', onSelect: () => void openSettings() }]] : []), ...exportItems.value] : [])
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

watch(settingsDictionaryId, (id) => { settingsExcludedDictionaryIds.value = settingsExcludedDictionaryIds.value.filter(excluded => excluded !== id) })

watch(settingsAdapterIds, () => {
  if (!settingsPreviewAvailable.value) settingsLivePreview.value = false
}, { deep: true })

watch(() => probe.selectedRunId.value, async (id, previous) => {
  if (id === previous) return
  selected.value = new Set()
  dictionaryNotice.value = ''
  pendingImport.value = null
  restoreViewState(id)
  await loadPage()
})

watch([page, pageSize], async () => {
  selected.value = new Set()
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

watch(() => textSettings.settings.value.textFilterPolicy, async () => {
  page.value = 1
  selected.value = new Set()
  await loadPage()
}, { deep: true })

watch(translationFilter, async () => {
  page.value = 1
  selected.value = new Set()
  persistViewState()
  await loadPage()
})

watch([listQuery, listPageSize], () => {
  listPage.value = 1
  listSelected.value = new Set()
})

watch(() => selected.value.size, async () => {
  await nextTick()
  updateScrollMetrics()
})

onMounted(async () => {
  await ai.connect().catch(() => undefined)
  await probe.connect()
  restoreViewState(probe.selectedRunId.value)
  await loadPage()
  startScrollTracking()
  pollTimer = setInterval(() => void poll(), 1000)
})

onBeforeUnmount(() => {
  disposed = true
  pageRequest += 1
  pageQueue.dispose()
  if (queryTimer) clearTimeout(queryTimer)
  if (pollTimer) clearInterval(pollTimer)
  stopScrollTracking()
  if (props.captureArmed) emit('cancel-capture')
  for (const [source, timer] of editTimers) {
    clearTimeout(timer)
    void saveTranslation(source)
  }
})

async function setMergeRules(value: boolean) {
  for (const source of [...dirtyTranslations]) await saveTranslation(source)
  if (dirtyTranslations.size) return
  mergeRules.value = value
  page.value = 1
  selected.value = new Set()
  persistViewState()
  await loadPage()
}

async function loadPage() {
  pageRequest += 1
  return pageQueue.request()
}

async function loadPageNow(request: number) {
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
    const nextPage = await probe.queryEntries({
      runId,
      search: query.value,
      adapterIds: [...adapterFilterIds.value],
      translationFilter: translationFilter.value,
      mergeRules: mergeRules.value,
      page: page.value,
      pageSize: pageSize.value,
    })
    if (request !== pageRequest || runId !== probe.selectedRunId.value) return
    const editingRow = entryPage.value.rows.find(row => row.source === focusedTranslation.value)
    if (mergeRules.value && editingRow?.resolution?.editSource) {
      const replacement = nextPage.rows.find(row => row.resolution?.editSource === editingRow.resolution?.editSource
        && row.resolution?.ruleIndex === editingRow.resolution?.ruleIndex
        && JSON.stringify(row.resolution?.dictionaryIds) === JSON.stringify(editingRow.resolution?.dictionaryIds))
      if (replacement) {
        replacement.source = editingRow.source
        replacement.translation = editingRow.translation
      }
    }
    if (JSON.stringify(entryPage.value) !== JSON.stringify(nextPage)) entryPage.value = nextPage
    synchronizeTranslationValues(entryPage.value.rows)
    const maxPage = Math.max(1, Math.ceil(entryPage.value.total / pageSize.value))
    if (page.value > maxPage) page.value = maxPage
  }
  catch (error) {
    if (request === pageRequest) probe.report(error, runId)
  }
  finally {
    if (request === pageRequest) {
      loading.value = false
      await nextTick()
      updateScrollMetrics()
    }
  }
}

async function prepareAiPlan() {
  const run = selectedRun.value
  const dictionary = selectedDictionary.value
  aiNotice.value = ''
  aiNoticeCancelled.value = false
  aiRetryAvailable.value = false
  ai.error.value = ''
  if (!run || !dictionary) return null
  if (!selectedAiProfile.value) {
    emit('configure-ai')
    return null
  }
  for (const source of [...dirtyTranslations]) await saveTranslation(source)
  try {
    const plan = await ai.planProbe(run.id, selectedAiProfile.value.id, {
      snapshotRevision: entryPage.value.dictionaryRevision,
      sourceLocale: dictionary.metadata.sourceLocale,
      targetLocale: dictionary.metadata.targetLocale,
      items: entryPage.value.rows.map((row, index) => ({
        itemId: `probe-row-${index + 1}`,
        source: row.source,
        translation: (translationValues.value[row.source] ?? row.translation) || null,
        ignored: row.state === 'ignored',
      })),
    })
    aiPlan.value = plan
    return plan
  }
  catch {
    return null
  }
}

async function previewAiTranslation() {
  const plan = await prepareAiPlan()
  if (plan) aiPreviewOpen.value = true
}

function dismissAiOutcome() {
  aiNotice.value = ''
  aiNoticeCancelled.value = false
  aiRetryAvailable.value = false
  ai.dismissCurrentJob()
}

async function runAiTranslation(plan?: AiTranslationPlan | null) {
  const run = selectedRun.value
  const nextPlan = plan ?? await prepareAiPlan()
  if (!run || !nextPlan || !selectedAiProfile.value) return
  if (!nextPlan.candidates.length) {
    aiPreviewOpen.value = true
    return
  }
  aiPreviewOpen.value = false
  aiPreflightOpen.value = true
}

async function executeAiTranslation() {
  const run = selectedRun.value
  const nextPlan = aiPlan.value
  if (!run || !nextPlan || !selectedAiProfile.value) return
  aiPreflightOpen.value = false
  try {
    await ai.startBackgroundPlan(nextPlan, selectedAiProfile.value.id)
    aiPlan.value = null
  }
  catch {
    // The composable exposes the localized error near the probe header.
  }
}

async function poll() {
  const run = selectedRun.value
  if (!run || pollingPage || disposed) return
  pollingPage = true
  try {
    const previousObservation = run.observationRevision
    const previousDictionary = run.dictionaryRevision
    const previousExclusions = JSON.stringify([run.excludedDictionaryIds, run.exclusionRevisions])
    const summary = await probe.refreshSummary(run.id)
    if (summary && (
      summary.observationRevision !== previousObservation
      || summary.dictionaryRevision !== previousDictionary
      || JSON.stringify([summary.excludedDictionaryIds, summary.exclusionRevisions]) !== previousExclusions
    )) await loadPage()
  } finally { pollingPage = false }
}

async function openSettings() {
  for (const source of [...dirtyTranslations]) await saveTranslation(source)
  if (dirtyTranslations.size) return
  const run = selectedRun.value
  if (!run) return
  settingsName.value = run.name
  settingsDictionaryId.value = run.dictionaryId
  settingsExcludedDictionaryIds.value = [...(run.excludedDictionaryIds ?? [])]
  settingsAdapterIds.value = [...run.adapterIds]
  settingsLivePreview.value = run.livePreviewEnabled
  settingsCompatibleAdapterIds.value = null
  settingsOpen.value = true
  const request = ++settingsCompatibilityRequest
  settingsCompatibilityLoading.value = true
  const compatible = await probe.compatibleAdapters(run.softwareId, run.id)
  if (request !== settingsCompatibilityRequest || run.id !== selectedRun.value?.id) {
    if (request === settingsCompatibilityRequest) settingsCompatibilityLoading.value = false
    return
  }
  settingsCompatibleAdapterIds.value = compatible
  if (compatible) settingsAdapterIds.value = settingsAdapterIds.value.filter(id => compatible.includes(id))
  settingsCompatibilityLoading.value = false
}

async function handleQuickProbeStarted(runId: string) {
  emit('workspace-changed')
  probe.selectRun(runId)
  await nextTick()
  await loadPage()
}

async function openWorkflowSettings() {
  await Promise.all([...dirtyTranslations].map(saveTranslation))
  if (dirtyTranslations.size) return
  emit('workflow-settings')
}

async function closeDetail() {
  await Promise.all([...dirtyTranslations].map(saveTranslation))
  if (dirtyTranslations.size) return
  probe.selectRun('')
  selected.value = new Set()
  if (props.workflowId) emit('back-workflow')
}

async function applySettings() {
  const run = selectedRun.value
  if (!run || !settingsValid.value || !settingsChanged.value) return
  const updated = await probe.update({
    runId: run.id,
    name: settingsName.value.trim(),
    dictionaryId: settingsDictionaryId.value,
    excludedDictionaryIds: [...settingsExcludedDictionaryIds.value],
    adapterIds: [...settingsAdapterIds.value],
    livePreviewEnabled: settingsLivePreview.value,
  })
  if (!updated) return
  adapterFilterIds.value = adapterFilterIds.value.filter(id => updated.adapterIds.includes(id))
  if (updated.dictionaryId !== run.dictionaryId) {
    dictionaryNotice.value = t('capture.dictionaryBindingChanged', {
      dictionary: dictionaryName(updated.dictionaryId),
    })
  }
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
  if (action === 'clear_translations') sources = selectedRows.value.filter(row => row.resolution?.editable !== false).map(row => row.source)
  if (action === 'ignore') sources = selectedRows.value.filter(row => row.count > 0 && row.state !== 'ignored').map(row => row.source)
  if (action === 'restore') sources = selectedRows.value.filter(row => row.state === 'ignored').map(row => row.source)
  if (!sources.length) return
  try {
    if (action === 'clear_translations') {
      for (const source of sources) {
        const timer = editTimers.get(source)
        if (timer) clearTimeout(timer)
        editTimers.delete(source)
      }
      await translationSaveQueue
    }
    await probe.bulk(run.id, sources, action)
    if (action === 'clear_translations') {
      for (const source of sources) {
        const timer = editTimers.get(source)
        if (timer) clearTimeout(timer)
        editTimers.delete(source)
        dirtyTranslations.delete(source)
        translationValues.value[source] = ''
      }
      emit('workspace-changed')
      dictionaryNotice.value = t('capture.removedFromDictionary', {
        count: sources.length,
        dictionary: runDictionaryName(run),
      })
    }
    selected.value = new Set()
    await loadPage()
  }
  catch (error) {
    probe.report(error, run.id)
  }
}

function entryRowId(row: ProbeEntryRow) {
  if (mergeRules.value && row.resolution?.editSource) return JSON.stringify([
    row.resolution.dictionaryIds, row.resolution.ruleIndex, row.resolution.editSource,
  ])
  return row.source
}

function translationInputValue(row: ProbeEntryRow) {
  if (dirtyTranslations.has(row.source)) return translationValues.value[row.source] ?? ''
  if (focusedTranslation.value === row.source && row.resolution?.editSource) return row.resolution.editTranslation ?? ''
  return row.translation
}

function blurTranslation(source: string) {
  focusedTranslation.value = null
  void saveTranslation(source).then(() => { if (mergeRules.value) return loadPage() })
}

function updateTranslation(source: string, value: unknown) {
  if (entryPage.value.rows.find(row => row.source === source)?.resolution?.editable === false) return
  dictionaryNotice.value = ''
  const translation = String(value ?? '')
  translationValues.value = { ...translationValues.value, [source]: translation }
  dirtyTranslations.add(source)
  const previous = editTimers.get(source)
  if (previous) clearTimeout(previous)
  editTimers.set(source, setTimeout(() => void saveTranslation(source), 500))
}

function saveTranslation(source: string) {
  const run = selectedRun.value
  if (!run || !dirtyTranslations.has(source)) return Promise.resolve()
  const pending = editTimers.get(source)
  if (pending) clearTimeout(pending)
  editTimers.delete(source)
  const task = translationSaveQueue.then(async () => {
    if (!dirtyTranslations.has(source)) return
    try {
      const value = translationValues.value[source] ?? ''
      await probe.editTranslation(run.id, source, value)
      if (translationValues.value[source] === value) dirtyTranslations.delete(source)
      emit('workspace-changed')
      await loadPage()
    }
    catch (error) {
      probe.report(error, run.id)
    }
  })
  translationSaveQueue = task
  return task
}

async function openBoundDictionary() {
  const dictionaryId = selectedRun.value?.dictionaryId
  if (!dictionaryId) return
  for (const source of [...dirtyTranslations]) await saveTranslation(source)
  if (!dirtyTranslations.size) emit('open-dictionary', dictionaryId)
}

async function launchSelectedSoftware() {
  const run = selectedRun.value
  if (!run || launchingSoftware.value) return
  probe.clearMessage()
  launchingSoftware.value = true
  try {
    if ('__TAURI_INTERNALS__' in window) await invoke('desktop_launch_software', { softwareId: run.softwareId })
  }
  catch (error) {
    probe.report(error, run.id)
  }
  finally {
    launchingSoftware.value = false
  }
}

async function disconnectRun(run: ProbeRunSummary | null) {
  if (!run || disconnectingRunId.value || !['running', 'paused'].includes(run.status)) return
  disconnectingRunId.value = run.id
  try {
    await probe.disconnect(run.id)
  }
  finally {
    if (disconnectingRunId.value === run.id) disconnectingRunId.value = ''
  }
}

async function refreshTargetText() {
  const run = selectedRun.value
  if (!run || refreshingText.value || probe.busy.value) return
  refreshingText.value = true
  try {
    for (const source of [...dirtyTranslations]) await saveTranslation(source)
    await translationSaveQueue
    if (dirtyTranslations.size || selectedRun.value?.id !== run.id) return
    await probe.refreshText(run.id)
  }
  finally {
    refreshingText.value = false
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
      translationFilter?: ProbeTranslationFilter
      mergeRules?: boolean
      page?: number
      pageSize?: number
    }
    const available = new Set(probe.runs.value.find(run => run.id === runId)?.adapterIds ?? [])
    mergeRules.value = value.mergeRules !== false
    query.value = value.query ?? ''
    adapterFilterIds.value = Array.isArray(value.adapterIds)
      ? [...new Set(value.adapterIds.filter(id => typeof id === 'string' && available.has(id)))]
      : []
    translationFilter.value = ['all', 'untranslated', 'translated', 'rule_matched', 'other_dictionary'].includes(value.translationFilter ?? '')
      ? value.translationFilter!
      : 'all'
    page.value = Math.max(1, value.page ?? 1)
    pageSize.value = [50, 100, 200].includes(value.pageSize ?? 0) ? value.pageSize! : 50
  }
  catch {
    mergeRules.value = true
    query.value = ''
    adapterFilterIds.value = []
    translationFilter.value = 'all'
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
    translationFilter: translationFilter.value,
    mergeRules: mergeRules.value,
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
  return props.dictionaries.find(item => item.metadata.id === id)?.metadata.name ?? t('capture.dictionaryUnavailable')
}

function runDictionaryName(run: ProbeRunSummary) { return dictionaryName(run.dictionaryId) }

function adapterName(id: string) {
  const adapter = props.adapters.find(candidate => candidate.id === id)
  return adapter ? adapterDisplayName(adapter.name, t) : t('capture.adapterUnavailable')
}

function adapterDetails(run: ProbeRunSummary) {
  return run.adapterIds.map(id => {
    const adapter = props.adapters.find(item => item.id === id)
    return {
      id,
      name: adapter ? adapterDisplayName(adapter.name, t) : t('capture.adapterUnavailable'),
      technologies: adapter ? adapterSummary(adapter, t) : t('capture.technologyUnavailable'),
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

function resolutionLabel(row: ProbeEntryRow) {
  if (row.translationVariants?.length) return t('capture.translationConflict')
  return row.resolution ? t(`captureResolution.${row.resolution.kind}`) : stateLabel(row.state)
}
function resolutionSource(row: ProbeEntryRow) {
  const resolution = row.resolution
  if (!resolution) return ''
  const parts: string[] = []
  if (resolution.ruleIndex !== null) parts.push(t('captureResolution.rule', { number: resolution.ruleIndex + 1 }))
  const extraDictionaryIds = resolution.dictionaryIds.filter(id => id !== selectedRun.value?.dictionaryId)
  if (extraDictionaryIds.length) parts.push(t(resolution.kind.startsWith('rule_') ? 'captureResolution.fromDictionary' : 'captureResolution.inDictionary', { names: extraDictionaryIds.map(dictionaryName).join('、') }))
  if (resolution.kind === 'filtered') parts.push(t(resolution.skipReason ? `captureResolution.skip.${resolution.skipReason}` : 'captureResolution.textFilter'))
  if (resolution.kind === 'ignored') parts.push(t('captureResolution.manualIgnore'))
  if (!resolution.editable) parts.push(t('captureResolution.readOnly'))
  return parts.join(' · ')
}
function resolutionColor(row: ProbeEntryRow) {
  if (row.translationVariants?.length) return 'warning'
  if (!row.resolution) return stateColor(row.state)
  if (['dictionary', 'rule_translated'].includes(row.resolution.kind)) return 'success'
  if (row.resolution.kind === 'rule_pending') return 'primary'
  if (['pending', 'dictionary_pending'].includes(row.resolution.kind)) return 'warning'
  return 'neutral'
}

function stateLabel(state: string) {
  return t(`capture.entryState.${state}`)
}

function translationVariantItems(row: ProbeEntryRow) {
  return (row.translationVariants ?? []).map(variant => ({
    label: variant.translation,
    onSelect: () => { updateTranslation(row.source, variant.translation); void saveTranslation(row.source) },
  }))
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

async function chooseImport(format: 'json' | 'csv') {
  const run = selectedRun.value
  if (!run || !('__TAURI_INTERNALS__' in window)) return
  const path = await open({ title: t('capture.importTitle'), multiple: false, filters: [{ name: format.toUpperCase(), extensions: [format] }] })
  if (typeof path !== 'string' || selectedRun.value?.id !== run.id) return
  importMode.value = 'overwrite'
  pendingImport.value = { runId: run.id, path, format }
}

async function confirmImport() {
  const input = pendingImport.value
  if (!input || selectedRun.value?.id !== input.runId) return
  for (const source of [...dirtyTranslations]) await saveTranslation(source)
  await translationSaveQueue
  if (dirtyTranslations.size || selectedRun.value?.id !== input.runId) return
  if (await probe.importEntries(input.runId, input.path, input.format, importMode.value)) {
    pendingImport.value = null
    emit('workspace-changed')
    await loadPage()
  }
}

async function chooseExport(format: ProbeExportFormat) {
  const run = selectedRun.value
  if (!run || !('__TAURI_INTERNALS__' in window)) return
  const csv = format.endsWith('_csv')
  const outputPath = await save({
    title: t('capture.export'),
    defaultPath: `${run.name}.${csv ? 'csv' : 'json'}`,
    filters: [{ name: csv ? 'CSV' : 'JSON', extensions: [csv ? 'csv' : 'json'] }],
  })
  if (outputPath) await probe.exportRun(run.id, format, outputPath)
}

async function confirmRemoval() {
  const ids = pendingRemoval.value.map(run => run.id)
  const removed = await probe.remove(ids)
  emit('workspace-changed')
  listSelected.value = new Set([...listSelected.value].filter(id => probe.runs.value.some(run => run.id === id)))
  if (removed) {
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
      :back-label="workflowId ? t('workflows.title') : t('capture.backToRuns')"
      @back="closeDetail"
    >
      <template #status>
        <div class="flex items-center gap-1.5">
          <UBadge :color="lifecycle ? workflowStateColor(lifecycle.phase) : statusColor(selectedRun.status)" variant="soft" size="sm" :label="lifecycle ? t(`workflowLifecycle.${lifecycle.phase}`) : statusLabel(selectedRun.status)" />
          <UBadge
            v-if="selectedRun.runtimeCapability && (!lifecycle || lifecycle.phase === 'running')"
            data-testid="probe-runtime-capability"
            :color="runtimeCapabilityColor(selectedRun.runtimeCapability)"
            variant="soft"
            size="sm"
            :label="runtimeCapabilityLabel(selectedRun.runtimeCapability)"
          />
        </div>
      </template>
      <template #detail>
        <p
          data-testid="probe-software-path"
          class="type-metadata m-0 truncate text-[var(--text-secondary)]"
          :title="selectedSoftware?.executablePath ?? t('capture.softwarePathUnavailable')"
        >
          {{ t('capture.softwarePath', { path: selectedSoftware?.executablePath ?? t('capture.softwarePathUnavailable') }) }}
        </p>
      </template>
      <template #actions>
        <div data-testid="probe-detail-actions" class="flex items-center gap-2">
          <UButton v-if="workflowId" :color="stopRequested ? 'neutral' : 'primary'" variant="outline" size="sm" :icon="stopRequested ? 'i-tabler-player-stop' : 'i-tabler-player-play'" :label="t(stopRequested ? 'workflowLifecycle.stop' : 'workflows.start')" :loading="Boolean(workflowCommandBusy)" @click="workflowWorkspace.setWorkflowEnabled(workflowId, !stopRequested)" />
          <template v-else>
          <UButton v-if="selectedRun.status === 'running'" color="neutral" variant="outline" size="sm" icon="i-tabler-player-pause" :label="t('capture.pause')" :loading="probe.busy.value && disconnectingRunId !== selectedRun.id" :disabled="disconnectingRunId === selectedRun.id" @click="probe.setPaused(selectedRun.id, true)" />
          <UButton v-else color="primary" :variant="selectedRun.status === 'paused' ? 'soft' : 'solid'" size="sm" icon="i-tabler-player-play" :label="selectedRun.status === 'paused' ? t('capture.continue') : (workflowId ? t('workflows.start') : t('capture.resume'))" :loading="probe.busy.value && disconnectingRunId !== selectedRun.id" :disabled="disconnectingRunId === selectedRun.id" @click="selectedRun.status === 'paused' ? probe.setPaused(selectedRun.id, false) : probe.resume(selectedRun.id)" />
          <UButton v-if="['running', 'paused'].includes(selectedRun.status)" data-testid="probe-disconnect" color="neutral" variant="outline" size="sm" icon="i-tabler-plug-off" :label="t('capture.disconnect')" :loading="disconnectingRunId === selectedRun.id" :disabled="probe.busy.value && disconnectingRunId !== selectedRun.id" @click="disconnectRun(selectedRun)" />
          </template>
          <UButton v-if="workflowId" color="neutral" variant="outline" size="sm" icon="i-tabler-settings" :label="t('workflows.settings')" :disabled="probe.busy.value" @click="openWorkflowSettings" />
          <UDropdownMenu :items="taskActionItems" :content="{ align: 'end' }" :ui="{ content: 'min-w-60' }">
            <UButton data-testid="probe-task-actions" color="neutral" variant="outline" size="sm" icon="i-tabler-dots-vertical" trailing-icon="i-tabler-chevron-down" :label="taskActionLabel" :aria-label="taskActionLabel" :title="taskActionLabel" :ui="{ label: 'hidden min-[1080px]:inline' }" />
          </UDropdownMenu>
        </div>
      </template>
    </ManagementDetailHeader>
    <ManagementPageHeader v-else title-id="capture-title" icon="i-tabler-radar" :title="t('capture.title')">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('capture.createRun')" :disabled="!observableAdapters.length || activeRunExists" @click="quickProbeOpen = true" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="lifecycleMessage" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="lifecycleMessage" class="mb-3" />
    <UAlert v-if="probe.message.value" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="probe.message.value" class="mb-3">
      <template #actions><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :label="t('common.dismissMessage')" @click="probe.clearMessage()" /></template>
    </UAlert>
    <UAlert v-if="dictionaryNotice" role="status" color="success" variant="soft" icon="i-tabler-book-check" :title="t('capture.dictionaryUpdated')" :description="dictionaryNotice" class="mb-3" />

    <UAlert v-if="ai.error.value" role="alert" color="error" variant="soft" :title="t('ai.translationFailed')" :description="ai.error.value" class="mb-3">
      <template #actions><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :label="t('common.dismissMessage')" @click="ai.clearError()" /></template>
    </UAlert>
    <UAlert v-else-if="aiNotice" role="status" :color="aiNoticeTone" variant="soft" icon="i-tabler-sparkles" :title="aiNoticeTitle" :description="aiNotice" class="mb-3">
      <template #actions>
        <div class="flex items-center gap-1.5">
          <UButton v-if="aiRetryAvailable" color="primary" variant="soft" size="xs" :label="t('ai.retryRemaining')" @click="runAiTranslation()" />
          <UButton color="neutral" variant="ghost" size="xs" :label="t('ai.dismissOutcome')" @click="dismissAiOutcome" />
        </div>
      </template>
    </UAlert>
    <section
      v-if="activeDisplayedAiJob"
      data-testid="probe-ai-task-status"
      role="status"
      aria-live="polite"
      class="mb-3 flex min-h-11 items-center gap-2.5 rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2"
    >
      <UIcon name="i-tabler-sparkles" class="size-4 shrink-0 text-[var(--primary)]" aria-hidden="true" />
      <div class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <strong class="type-label text-[var(--text)]">{{ t('ai.translating') }}</strong>
        <span class="type-metadata tabular-nums text-[var(--text-muted)]">{{ t('ai.progressSummary', { completed: activeDisplayedAiJob.completedCount, total: activeDisplayedAiJob.totalCount, elapsed: ai.elapsed.value }) }}</span>
      </div>
      <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-list-check" :label="t('ai.tasks.viewCurrent')" class="shrink-0" @click="emit('open-ai-tasks')" />
    </section>

    <section
      v-if="selectedRun"
      class="mb-3 flex items-center gap-3 rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2.5"
      :aria-label="t('capture.boundDictionary')"
    >
      <UIcon name="i-tabler-book-2" class="size-5 shrink-0 text-[var(--primary)]" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <div class="flex min-w-0 items-center gap-2">
          <strong data-testid="probe-bound-dictionary-name" class="truncate text-xs text-[var(--text)]">{{ selectedRunDictionaryName }}</strong>
          <UBadge v-if="selectedDictionary" color="neutral" variant="soft" size="sm" :label="`${selectedDictionary.metadata.sourceLocale} → ${selectedDictionary.metadata.targetLocale}`" />
          <span class="type-metadata shrink-0 text-[var(--text-muted)]">{{ t('capture.dictionaryEntries', { count: selectedRun.dictionaryEntryCount }) }}</span>
        </div>
        <p class="type-metadata m-0 mt-0.5 leading-4 text-[var(--text-muted)]">{{ selectedDictionary ? t('capture.dictionaryLinkHint') : t('capture.dictionaryUnavailableHint') }}</p>
      </div>
      <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-external-link" :label="t('capture.openDictionary')" :disabled="!selectedDictionary" @click="openBoundDictionary" />
    </section>

    <template v-if="selectedRun">
      <ManagementTableFrame v-model:query="query" v-model:filter-value="translationFilter" v-model:page="page" v-model:page-size="pageSize" :page-sizes="[50, 100, 200]" :search-placeholder="t('capture.searchEntries')" :search-label="t('capture.searchLabel')" :filter-label="translationFilterLabel" :filter-aria-label="t('capture.translationFilterLabel')" :filter-options="translationFilterOptions" :column-options="entryColumnOptions" :columns-label="t('table.columns')" :selected-count="selected.size" :selected-label="t('capture.itemLabel')" :total="entryPage.total" :item-label="t('capture.itemLabel')" @toggle-column="toggleEntryColumn">
        <template #toolbar-actions>
          <UDropdownMenu v-if="selectedRunAdapters.length > 1" :items="adapterFilterItems" :content="{ align: 'end' }" :ui="{ content: 'min-w-48' }">
            <UButton :title="t('capture.adapterFilterLabel')" data-testid="capture-adapter-filter" color="neutral" variant="outline" size="sm" icon="i-tabler-filter" trailing-icon="i-tabler-chevron-down" :label="adapterFilterLabel" class="max-w-52 justify-between" :aria-label="t('capture.adapterFilterLabel')" />
          </UDropdownMenu>
          <div data-testid="probe-ai-actions" class="inline-flex">
            <UButton
              color="primary"
              variant="soft"
              size="sm"
              icon="i-tabler-sparkles"
              :label="selectedAiProfile ? t('ai.fillUntranslated') : t('ai.configure')"
              :loading="ai.busy.value"
              class="rounded-r-none"
              @click="selectedAiProfile ? runAiTranslation() : emit('configure-ai')"
            />
            <UDropdownMenu :items="aiMenuItems" :content="{ align: 'end' }">
              <UButton :title="t('ai.translationOptions')" color="primary" variant="soft" size="sm" icon="i-tabler-chevron-down" class="rounded-l-none border-l border-l-[var(--border)]" :aria-label="t('ai.translationOptions')" :disabled="ai.busy.value" />
            </UDropdownMenu>
          </div>
          <UCheckbox data-testid="probe-auto-complete" :model-value="autoCompleteEnabled" :disabled="autoCompleteDisabled" :label="t('ai.autoComplete')" @update:model-value="toggleAutoComplete(Boolean($event))" />
        </template>
        <template #bulk-actions>
          <UButton color="neutral" variant="soft" size="sm" icon="i-tabler-eye-off" :label="t('capture.bulkIgnore')" :disabled="!selectedRows.some(row => row.count > 0 && row.state !== 'ignored')" @click="bulk('ignore')" />
          <UButton color="neutral" variant="soft" size="sm" icon="i-tabler-eye" :label="t('capture.bulkRestore')" :disabled="!selectedRows.some(row => row.state === 'ignored')" @click="bulk('restore')" />
          <UButton color="error" variant="soft" size="sm" icon="i-tabler-book-off" :label="t('capture.removeSelectedFromDictionary')" :disabled="!selectedRows.some(row => row.translation && row.resolution?.editable !== false)" @click="bulk('clear_translations')" />
        </template>

        <div ref="tableShell" class="relative h-full min-h-0 overflow-hidden">
          <UTable data-testid="capture-table-scroll" role="region" tabindex="0" :aria-label="t('capture.tableLabel')" :data="entryPage.rows" :get-row-id="entryRowId" :columns="entryColumns" sticky :loading="loading" class="capture-table-scroll management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[1040px]' }" @scroll.passive="updateScrollMetrics">
            <template #select-header><UCheckbox :model-value="pageSelected" :aria-label="t('capture.selectPage')" @update:model-value="togglePageSelection" /></template>
            <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.source)" :aria-label="t('common.selectNamed', { name: row.original.source })" @update:model-value="toggleSelection(row.original.source)" /></template>
            <template #source-cell="{ row }"><div class="truncate font-medium" :title="row.original.source">{{ row.original.source }}</div></template>
            <template #translation-cell="{ row }">
              <div class="flex min-w-0 items-center gap-1">
                <UInput :model-value="translationInputValue(row.original)" size="sm" class="min-w-0 flex-1" :placeholder="row.original.translationVariants?.length ? t('capture.resolveTranslation') : t('capture.pendingTranslation')" :aria-label="t('capture.translationFor', { source: row.original.source })" :readonly="row.original.resolution?.editable === false" :title="row.original.resolution?.editable === false ? t(row.original.resolution?.kind.startsWith('rule_') ? 'captureResolution.ruleReadOnlyHint' : 'captureResolution.readOnlyHint') : row.original.resolution?.editSource ? t('captureResolution.editFixedHint', { source: row.original.resolution.editSource }) : undefined" @update:model-value="updateTranslation(row.original.source, $event)" @focus="focusedTranslation = row.original.source" @blur="blurTranslation(row.original.source)" />
                <UDropdownMenu v-if="row.original.translationVariants?.length && row.original.resolution?.editable !== false" :items="translationVariantItems(row.original)" :content="{ align: 'end' }" :ui="{ content: 'max-w-[min(32rem,90vw)]', itemLabel: 'whitespace-pre-wrap break-words' }">
                  <UButton color="warning" variant="ghost" size="xs" icon="i-tabler-copy-check" :disabled="probe.busy.value" :aria-label="t('capture.chooseExistingTranslation', { count: row.original.translationVariants.length })" :title="t('capture.translationConflictHint')" />
                </UDropdownMenu>
              </div>
            </template>
            <template #state-cell="{ row }">
              <div class="flex min-w-0 flex-col items-start gap-1" :title="resolutionSource(row.original)">
                <UBadge :color="resolutionColor(row.original)" variant="soft" size="sm" :label="resolutionLabel(row.original)" />
                <span v-if="resolutionSource(row.original)" class="type-metadata block w-48 truncate text-[var(--text-muted)]">{{ resolutionSource(row.original) }}</span>
              </div>
            </template>
            <template #adapters-cell="{ row }">
              <div v-if="row.original.adapterIds.length" class="flex min-w-0 flex-wrap gap-1">
                <UBadge v-for="adapterId in row.original.adapterIds.slice(0, 2)" :key="adapterId" color="neutral" variant="soft" size="sm" :label="adapterName(adapterId)" />
                <span v-if="row.original.adapterIds.length > 2" class="type-caption text-[var(--text-muted)]">+{{ row.original.adapterIds.length - 2 }}</span>
              </div>
              <span v-else class="type-caption text-[var(--text-muted)]">—</span>
            </template>
            <template #count-cell="{ row }"><div class="text-right tabular-nums">{{ row.original.count || '—' }}</div></template>
            <template #lastSeenMs-cell="{ row }"><span class="tabular-nums text-[var(--text-secondary)]">{{ formatTime(row.original.lastSeenMs) }}</span></template>
            <template #empty><UEmpty icon="i-tabler-radar-off" :title="query || adapterFilterIds.length || translationFilter !== 'all' ? t('capture.noMatch') : t('capture.noRecords')" :description="query || adapterFilterIds.length || translationFilter !== 'all' ? t('capture.adjustSearch') : t('capture.noRecordsHint')" /></template>
          </UTable>

          <div v-if="scrollThumbHeight" data-testid="capture-scrollbar" aria-hidden="true" class="absolute inset-y-2 right-1 z-20 w-2 cursor-pointer rounded-full bg-[var(--surface-subtle)] ring-1 ring-inset ring-[var(--border)]" @pointerdown="jumpScrollbar">
            <div data-testid="capture-scrollbar-thumb" class="absolute inset-x-0 cursor-grab rounded-full border-2 border-[var(--surface-subtle)] bg-[var(--text-muted)] active:cursor-grabbing" :style="{ height: `${scrollThumbHeight}px`, transform: `translateY(${scrollThumbTop}px)` }" @pointerdown.stop="beginScrollbarDrag" @pointermove="dragScrollbar" @pointerup="endScrollbarDrag" @pointercancel="endScrollbarDrag" />
          </div>
        </div>
      </ManagementTableFrame>
    </template>

    <ManagementTableFrame v-else v-model:query="listQuery" v-model:page="listPage" v-model:page-size="listPageSize" :search-placeholder="t('capture.searchRuns')" :search-label="t('capture.searchRuns')" :column-options="runColumnOptions" :columns-label="t('table.columns')" :selected-count="listSelected.size" :selected-label="t('capture.runItemLabel')" :total="filteredRuns.length" :item-label="t('capture.runItemLabel')" @toggle-column="toggleRunColumn">
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('capture.bulkDelete')" :disabled="bulkRunDeletionBlocked" @click="pendingRemoval = probe.runs.value.filter(run => listSelected.has(run.id))" />
      </template>
      <UTable data-testid="capture-run-management-table" role="region" tabindex="0" aria-labelledby="capture-title" :data="listPageItems" :columns="runColumns" sticky class="management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[920px]' }" @dblclick="openRunOnDoubleClick">
        <template #select-header><UCheckbox :model-value="listPageSelected" :aria-label="t('capture.selectRunPage')" @update:model-value="toggleListPageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="listSelected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleListSelection(row.original.id)" /></template>
        <template #run-cell="{ row }">
          <div class="flex min-w-0 items-center gap-1.5">
            <div class="truncate font-semibold">{{ row.original.name }}</div>
            <UPopover mode="hover" :open-delay="150" :close-delay="100" :content="{ side: 'right', align: 'start', sideOffset: 6 }" :ui="{ content: 'z-[80] w-80 p-0' }">
              <UButton :title="t('capture.viewTechnicalDetails', { name: row.original.name })" color="neutral" variant="ghost" size="xs" icon="i-tabler-info-circle" class="shrink-0" :aria-label="t('capture.viewTechnicalDetails', { name: row.original.name })" />
              <template #content>
                <div class="space-y-3 p-3">
                  <div>
                    <div class="type-label font-medium text-[var(--text-secondary)]">{{ t('capture.executablePath') }}</div>
                    <div class="type-metadata mt-1 break-all leading-4 text-[var(--text)]">{{ softwarePath(row.original.softwareId) }}</div>
                  </div>
                  <div>
                    <div class="type-label font-medium text-[var(--text-secondary)]">{{ t('capture.adapters') }}</div>
                    <ul class="m-0 mt-1 space-y-1 p-0" role="list">
                      <li v-for="adapter in adapterDetails(row.original)" :key="adapter.id" class="type-metadata list-none leading-4 text-[var(--text)]">
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
        <template #dictionary-cell="{ row }"><div class="truncate font-medium">{{ runDictionaryName(row.original) }}</div><div class="type-metadata mt-0.5 text-[var(--text-muted)]">{{ t('capture.dictionaryEntries', { count: row.original.dictionaryEntryCount }) }}</div></template>
        <template #status-cell="{ row }">
          <div class="flex flex-col items-start gap-1">
            <UBadge :color="statusColor(row.original.status)" variant="soft" size="sm" :label="statusLabel(row.original.status)" />
            <UBadge v-if="row.original.runtimeCapability" :color="runtimeCapabilityColor(row.original.runtimeCapability)" variant="soft" size="sm" :label="runtimeCapabilityLabel(row.original.runtimeCapability)" />
          </div>
        </template>
        <template #progress-cell="{ row }"><div class="tabular-nums">{{ t('capture.observedCount', { count: row.original.observedCount }) }}</div><div v-if="row.original.ignoredCount" class="type-metadata mt-0.5 text-[var(--text-muted)]">{{ t('capture.ignoredCount', { count: row.original.ignoredCount }) }}</div></template>
        <template #updatedAtMs-cell="{ row }"><span class="tabular-nums text-[var(--text-secondary)]">{{ formatTime(row.original.updatedAtMs) }}</span></template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-0.5">
            <UButton v-if="['running', 'paused'].includes(row.original.status)" data-testid="probe-list-stop" :title="t('capture.disconnect')" color="neutral" variant="ghost" size="xs" icon="i-tabler-player-stop" :aria-label="t('capture.disconnect')" :loading="disconnectingRunId === row.original.id" :disabled="probe.busy.value && disconnectingRunId !== row.original.id" @click.stop="disconnectRun(row.original)" />
            <UButton :title="t('capture.openNamed', { name: row.original.name })" color="neutral" variant="ghost" size="xs" icon="i-tabler-arrow-right" :aria-label="t('capture.openNamed', { name: row.original.name })" @click="probe.selectRun(row.original.id)" />
            <UButton :title="t('common.deleteNamed', { name: row.original.name })" color="error" variant="ghost" size="xs" icon="i-tabler-trash" :disabled="['running', 'paused'].includes(row.original.status)" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original]" />
          </div>
        </template>
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
      :open="aiPreviewOpen"
      :title="t('ai.previewTitle')"
      :description="aiPlan ? t('ai.previewSummary', { eligible: aiPlan.candidates.length, skipped: aiPlan.skipped.length }) : ''"
      :confirm-label="t('ai.translateCount', { count: aiPlan?.candidates.length ?? 0 })"
      :confirm-disabled="!aiPlan?.candidates.length"
      :busy="ai.busy.value"
      width="md"
      @update:open="aiPreviewOpen = $event"
      @confirm="runAiTranslation(aiPlan)"
    >
      <p v-if="aiPlan?.candidates.length && selectedAiProfile" class="type-metadata mb-3 mt-0 rounded-md bg-[var(--surface-subtle)] px-3 py-2 leading-4 text-[var(--text-muted)]">
        {{ t('ai.previewBatchHint', { previewed: Math.min(aiPlan.candidates.length, 20), total: aiPlan.candidates.length, items: selectedAiProfile.maxItemsPerRequest, batches: Math.ceil(aiPlan.candidates.length / selectedAiProfile.maxItemsPerRequest), concurrency: selectedAiProfile.maxConcurrency }) }}
      </p>
      <div v-if="aiPlan?.candidates.length" class="space-y-1">
        <div v-for="candidate in aiPlan.candidates.slice(0, 20)" :key="candidate.itemId" class="flex items-center gap-2 border-b border-[var(--border)] py-2 last:border-b-0">
          <UIcon name="i-tabler-arrow-right" class="size-4 shrink-0 text-[var(--accent-strong)]" aria-hidden="true" />
          <span class="min-w-0 flex-1 truncate text-[11px] text-[var(--text)]">{{ candidate.source }}</span>
        </div>
      </div>
      <UEmpty v-else icon="i-tabler-check" :title="t('ai.nothingToTranslate')" :description="t('ai.nothingToTranslateHint')" />
      <p v-if="aiPlan?.skipped.length" class="type-metadata mb-0 mt-3 leading-4 text-[var(--text-muted)]">{{ t('ai.skippedHint', { count: aiPlan.skipped.length }) }}</p>
    </ManagementFormModal>

    <UAlert v-if="selectedRun && autoComplete.stopped.value[selectedRun.id]" color="warning" :title="t('ai.autoStopped')" class="mt-2" @close="autoComplete.stopped.value[selectedRun!.id] = false" />
    <AiTranslationPreflight
      v-model:open="aiPreflightOpen"
      :plan="aiPlan"
      :profile="selectedAiProfile"
      @proceed="executeAiTranslation"
    />

    <ManagementFormModal
      :open="settingsOpen"
      :title="t('capture.settingsTitle')"
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

        <UFormField :label="t('capture.boundDictionary')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : t('capture.dictionaryBindingHint')" required>
          <USelect v-model="settingsDictionaryId" :items="settingsDictionaryItems" value-key="value" label-key="label" class="w-full" :disabled="settingsConfigurationLocked" />
        </UFormField>
        <UFormField :label="t('capture.excludedDictionaries')" :description="t('capture.excludedDictionariesHint')">
          <USelectMenu v-model="settingsExcludedDictionaryIds" :items="settingsDictionaryItems.filter(item => item.value !== settingsDictionaryId)" multiple value-key="value" :aria-label="t('capture.excludedDictionaries')" :placeholder="t('capture.noExcludedDictionaries')" class="w-full" :disabled="settingsConfigurationLocked" />
        </UFormField>

        <UFormField :label="t('capture.adapters')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : settingsCompatibilityLoading ? t('capture.loadingCompatibleAdapters') : compatibleSettingsAdapters.length ? t('capture.adaptersHint') : t('capture.noCompatibleAdapters')" required><ProbeAdapterPicker v-model="settingsAdapterIds" :adapters="compatibleSettingsAdapters" :disabled="settingsConfigurationLocked || settingsCompatibilityLoading" /></UFormField>

        <UFormField :label="t('capture.livePreview')" :hint="settingsConfigurationLocked ? t('capture.releaseToEditSettings') : settingsPreviewAvailable ? t('capture.livePreviewHint') : t('capture.livePreviewUnavailable')">
          <USwitch v-model="settingsLivePreview" :disabled="settingsConfigurationLocked || !settingsPreviewAvailable" />
        </UFormField>

        <section class="border-t border-[var(--border)] pt-4" :aria-labelledby="'capture-danger-title'">
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0">
              <h3 id="capture-danger-title" class="m-0 text-[11px] font-semibold text-[var(--text)]">{{ t('capture.dangerTitle') }}</h3>
              <p class="type-metadata mt-1 mb-0 max-w-[42ch] leading-4 text-[var(--text-muted)]">{{ clearEntriesLocked ? t('capture.clearAllLocked') : t('capture.clearAllHint') }}</p>
            </div>
            <UButton data-testid="capture-clear-all" color="error" variant="soft" size="sm" icon="i-tabler-trash-x" :label="t('capture.clearAll')" :disabled="clearEntriesLocked || (!selectedRun?.observedCount && !selectedRun?.dictionaryEntryCount)" @click="requestClearAll" />
          </div>
        </section>
      </div>
    </ManagementFormModal>

    <ManagementFormModal :open="Boolean(pendingImport)" :title="t('capture.importTitle')" :confirm-label="t('capture.importConfirm')" :busy="probe.busy.value" @update:open="$event || (pendingImport = null)" @confirm="confirmImport">
      <UFormField :label="t('capture.importConflict')">
        <USelect v-model="importMode" :items="importModeItems" value-key="value" :aria-label="t('capture.importConflict')" class="w-full" />
      </UFormField>
      <p class="type-metadata text-[var(--text-muted)]">{{ t(importMode === 'replace' ? 'capture.importReplaceHint' : 'capture.importMergeHint') }}</p>
      <UAlert v-if="probe.message.value" role="alert" color="error" :description="probe.message.value" />
    </ManagementFormModal>

    <ConfirmDialog :open="Boolean(pendingRemoval.length)" :title="t('capture.deleteTitle')" :description="t('capture.deleteDescription', { count: pendingRemoval.length })" :confirm-label="t('capture.deleteConfirm')" :busy="probe.busy.value" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
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
