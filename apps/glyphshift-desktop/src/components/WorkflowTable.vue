<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import WorkflowSoftwarePicker from './WorkflowSoftwarePicker.vue'
import WorkflowDictionarySelection from './WorkflowDictionarySelection.vue'
import type { DropdownMenuItem } from '@nuxt/ui'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import { adapterDisplayName } from '../adapterPresentation'
import RuntimeDiagnosticsModal from './RuntimeDiagnosticsModal.vue'
import { translateCommandError } from '../commandError'
import type {
  AdapterOption,
  DictionarySummary,
  SoftwareRecord,
  WorkflowDetail,
  WorkflowRuntimeStatus,
  WorkflowSummary,
  WorkflowTarget,
} from '../model'
import {
  editableRowIndex,
  managementActionsColumnMeta,
  managementIdentityColumnMeta,
  managementSelectionColumnMeta,
} from '../tableInteraction'
import { usePageEscape } from '../usePageEscape'
import { useTableColumns } from '../useTableColumns'
import AdapterSelectionTable from './AdapterSelectionTable.vue'
import SelectedFontTags from './SelectedFontTags.vue'
import ShortcutRecorder from './ShortcutRecorder.vue'

const props = defineProps<{
  items: WorkflowSummary[]
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  installedFamilies: string[]
  adapters: AdapterOption[]
  activationIds: Set<string>
  runtimeStatus: Record<string, WorkflowRuntimeStatus>
  busy: boolean
  refreshing: boolean
  fontRefreshing: boolean
  messages: Record<string, string>
  editing: WorkflowDetail | null
}>()

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}
const emit = defineEmits<{
  toggle: [id: string, enabled: boolean]
  toggleMany: [ids: string[], enabled: boolean]
  refresh: []
  refreshFonts: []
  open: [id: string]
  collect: [id: string]
  create: [name: string, description: string, targets: WorkflowTarget[], globalShortcut: string, done: (saved: boolean) => void]
  save: [detail: WorkflowDetail, done: (saved: boolean) => void]
  closeEdit: []
  copy: [id: string]
  remove: [ids: string[]]
  navigate: [view: 'software' | 'dictionaries']
  'dirty-change': [dirty: boolean]
}>()
const { t, locale } = useI18n()
const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const statusFilter = ref('all')
const { columns: visibleColumns, toggleColumn } = useTableColumns('glyphshift.table-columns.workflows', {
  software: true,
  adapters: true,
  assets: true,
  status: true,
  enabled: true,
})
const creating = ref(false)
const name = ref('')
const description = ref('')
const globalShortcut = ref('')
const targets = ref<WorkflowTarget[]>([])
const activeSoftwareId = ref<string | null>(null)
const softwarePickerOpen = ref(false)
const lastSuggestedName = ref('')
function openSoftwarePicker() { softwarePickerOpen.value = true }
function selectWorkflowSoftware(id: string) {
  if (id !== activeSoftwareId.value) toggleSoftware(id)
  const suggested = softwareName(id)
  if (!name.value.trim() || name.value === lastSuggestedName.value) name.value = suggested
  lastSuggestedName.value = suggested
}
const fontQuery = ref('')
const fontFilter = ref('all')
const activeEditorTab = ref('software')
const pendingRemoval = ref<WorkflowSummary[]>([])
const diagnosticWorkflow = ref<WorkflowSummary | null>(null)
const formBaseline = ref('')
const discardFormOpen = ref(false)

function serializeForm() {
  return JSON.stringify({ name: name.value, description: description.value, targets: targets.value, globalShortcut: globalShortcut.value })
}

const statusFilterOptions = computed(() => [
  { value: 'all', label: t('workflows.filters.all') },
  { value: 'enabled', label: t('workflows.filters.enabled') },
  { value: 'disabled', label: t('workflows.filters.disabled') },
])
const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter((item) => {
    const enabled = props.activationIds.has(item.id)
    if (statusFilter.value === 'enabled' && !enabled) return false
    if (statusFilter.value === 'disabled' && enabled) return false
    return !needle || `${item.name} ${item.description} ${softwareNames(item)} ${dictionaryNames(item)} ${adapterNames(item)}`.toLocaleLowerCase().includes(needle)
  })
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.id)))
const activeTarget = computed(() => targets.value.find(target => target.softwareId === activeSoftwareId.value) ?? null)
const visibleFontFamilies = computed(() => {
  const needle = fontQuery.value.trim().toLocaleLowerCase()
  const selectedFamilies = activeTarget.value?.fontPolicy?.families ?? []
  return props.installedFamilies
    .filter(family => (!needle || family.toLocaleLowerCase().includes(needle))
      && (fontFilter.value !== 'selected' || selectedFamilies.includes(family)))
})
const formOpen = computed(() => creating.value || Boolean(props.editing))
const editingWorkflow = computed(() => Boolean(props.editing))
const formDirty = computed(() => formOpen.value && serializeForm() !== formBaseline.value)
const editorTitle = computed(() => props.editing ? (name.value.trim() || props.editing.name) : t('workflows.create'))

watch(formDirty, dirty => emit('dirty-change', dirty), { immediate: true })
const basicProblems = computed(() => name.value.trim() ? [] : [t('workflows.problems.name')])
const softwareProblems = computed(() => {
  const problems: string[] = []
  if (!targets.value.length) problems.push(t('workflows.problems.target'))
  for (const target of targets.value) {
    const label = softwareName(target.softwareId)
    if (!target.adapterPlan.adapterIds.length) problems.push(t('workflows.problems.adapter', { name: label }))
  }
  return [...new Set(problems)]
})
const dictionaryProblems = computed(() => {
  const problems: string[] = []
  for (const target of targets.value) {
    const label = softwareName(target.softwareId)
    if (!target.dictionaryIds.length && !target.fontPolicy) problems.push(t('workflows.problems.asset', { name: label }))
  }
  return [...new Set(problems)]
})
const fontProblems = computed(() => {
  const problems: string[] = []
  for (const target of targets.value) {
    const label = softwareName(target.softwareId)
    if (!target.fontPolicy) continue
    if (!target.fontPolicy.families.length && !target.fontPolicy.preferDictionary && (target.fontPolicy.scalePercent ?? 100) === 100) problems.push(t('workflows.problems.fontFamily', { name: label }))
    const scale = target.fontPolicy.scalePercent ?? 100
    if (!Number.isInteger(scale) || scale < 50 || scale > 200) problems.push(t('workflows.fontScaleRange'))
    if (target.fontPolicy.coverage === 'dictionary_matches' && !target.dictionaryIds.length) {
      problems.push(t('workflows.problems.fontDictionary', { name: label }))
    }
  }
  return [...new Set(problems)]
})
const formProblems = computed(() => [...new Set([
  ...basicProblems.value,
  ...softwareProblems.value,
  ...dictionaryProblems.value,
  ...fontProblems.value,
])])
const formValid = computed(() => formProblems.value.length === 0)
const editorTabs = computed(() => [
  { value: 'software', slot: 'software', label: t('workflows.tabs.software'), icon: 'i-tabler-app-window', badge: problemBadge(softwareProblems.value) },
  { value: 'basic', slot: 'basic', label: t('workflows.tabs.basic'), icon: 'i-tabler-adjustments-horizontal', badge: problemBadge(basicProblems.value) },
  { value: 'dictionary', slot: 'dictionary', label: t('workflows.tabs.dictionary'), icon: 'i-tabler-language', badge: problemBadge(dictionaryProblems.value) },
  { value: 'font', slot: 'font', label: t('workflows.tabs.font'), icon: 'i-tabler-typography', badge: problemBadge(fontProblems.value) },
])
const catalogFilterOptions = computed(() => [
  { value: 'all', label: t('workflows.catalogFilters.all') },
  { value: 'selected', label: t('workflows.catalogFilters.selected') },
])
const fontCoverageOptions = computed(() => [
  { value: 'dictionary_matches' as const, label: t('workflows.fontDictionaryMatches') },
  { value: 'all_observations' as const, label: t('workflows.fontAllObservations') },
])
const workflowMessage = computed(() => props.messages.workflows || props.items.map(item => props.messages[item.id]).find(Boolean) || '')
const workflowAdapters = computed(() => props.adapters.filter(adapter =>
  adapter.features.includes('textReplace') || adapter.features.includes('fontSubstitute')))
const columnOptions = computed(() => [
  { key: 'software', label: t('workflows.columns.software'), visible: visibleColumns.value.software },
  { key: 'assets', label: t('workflows.columns.assets'), visible: visibleColumns.value.assets },
  { key: 'status', label: t('workflows.columns.status'), visible: visibleColumns.value.status },
  { key: 'enabled', label: t('workflows.columns.enabled'), visible: visibleColumns.value.enabled },
])
const tableColumns = computed<TableColumn<WorkflowSummary>[]>(() => [
  { id: 'select', header: '', meta: managementSelectionColumnMeta() },
  { id: 'workflow', header: t('workflows.columns.workflow'), meta: { class: { th: `${managementIdentityColumnMeta('w-52').class.th} !w-auto`, td: managementIdentityColumnMeta('w-52').class.td } } },
  ...(visibleColumns.value.software ? [{ id: 'software', header: t('workflows.columns.software'), meta: { class: { th: 'w-28 min-w-28 whitespace-nowrap', td: 'w-28 min-w-28' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.assets ? [{ id: 'assets', header: t('workflows.columns.assets'), meta: { class: { th: 'w-36 whitespace-nowrap', td: 'w-36 max-w-36' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.status ? [{ id: 'status', header: t('workflows.columns.status'), meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.enabled ? [{ id: 'enabled', header: t('workflows.columns.enabled'), meta: { class: { th: 'w-16 text-center', td: 'w-16 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  { id: 'actions', header: t('workflows.columns.actions'), meta: managementActionsColumnMeta('w-48') },
])
function workflowOverflowItems(item: WorkflowSummary): DropdownMenuItem[][] {
  return [[{
    label: t('workflows.diagnostics.openNamed', { name: item.name }),
    icon: 'i-tabler-activity-heartbeat',
    disabled: actualStatus(item) !== 'running',
    onSelect: () => { diagnosticWorkflow.value = item },
  }, {
    label: t('common.copy'),
    icon: 'i-tabler-copy',
    onSelect: () => emit('copy', item.id),
  }], [{
    label: t('common.delete'),
    icon: 'i-tabler-trash',
    color: 'error',
    onSelect: () => { pendingRemoval.value = [item] },
  }]]
}
const removalDescription = computed(() => pendingRemoval.value.length === 1
  ? t('workflows.deleteOne', { name: pendingRemoval.value[0]?.name ?? '' })
  : t('workflows.deleteMany', { count: pendingRemoval.value.length }))

watch([query, statusFilter, pageSize], () => { page.value = 1 })
watch(activeSoftwareId, () => {
  fontQuery.value = ''
  fontFilter.value = 'all'
})
watch(() => props.editing, (detail, previous) => {
  if (!detail) return
  name.value = detail.name
  description.value = detail.description
  globalShortcut.value = detail.globalShortcut ?? ''
  targets.value = clone(detail.targets)
  if (detail.id !== previous?.id) {
    activeSoftwareId.value = targets.value[0]?.softwareId ?? null
    activeEditorTab.value = 'software'
  } else if (!targets.value.some(target => target.softwareId === activeSoftwareId.value)) {
    activeSoftwareId.value = targets.value[0]?.softwareId ?? null
  }
  formBaseline.value = serializeForm()
}, { immediate: true })

function softwareName(id: string) {
  return props.software.find(item => item.id === id)?.name ?? id
}
function targetForSoftware(id: string) {
  return targets.value.find(target => target.softwareId === id) ?? null
}
function softwareNames(item: WorkflowSummary) {
  return item.softwareIds.map(softwareName).join(locale.value === 'zh-CN' ? '、' : ', ')
}
function dictionaryName(id: string) {
  return props.dictionaries.find(candidate => candidate.metadata.id === id)?.metadata.name ?? id
}
function dictionaryNames(item: WorkflowSummary) {
  return item.dictionaryIds.map(dictionaryName).join(locale.value === 'zh-CN' ? '、' : ', ')
}
function adapterNames(item: WorkflowSummary) {
  const ids = [...new Set(item.targets.flatMap(target => target.adapterPlan.adapterIds))]
  return ids.map((id) => {
    const adapter = props.adapters.find(candidate => candidate.id === id)
    return adapter ? adapterDisplayName(adapter.name, t) : id
  }).join(locale.value === 'zh-CN' ? '、' : ', ')
}
function fontNames(item: WorkflowSummary) {
  const families = [...new Set(item.targets.flatMap(target => target.fontPolicy?.families.slice(0, 1) ?? []))]
  return families.join(locale.value === 'zh-CN' ? '、' : ', ')
}
function problemBadge(problems: string[]) {
  return problems.length
    ? { label: String(problems.length), color: 'warning' as const, variant: 'soft' as const }
    : undefined
}
function describeProblems(problems: string[]) {
  const visible = problems.slice(0, 3).join(locale.value === 'zh-CN' ? '；' : '; ')
  const remaining = problems.length - 3
  return remaining > 0 ? t('workflows.moreProblems', { visible, count: remaining }) : visible
}
function targetIsConfigured(target: WorkflowTarget) {
  if (!target.adapterPlan.adapterIds.length) return false
  if (!target.dictionaryIds.length && !target.fontPolicy) return false
  if (!target.fontPolicy) return true
  if (!target.fontPolicy.families.length && !target.fontPolicy.preferDictionary && (target.fontPolicy.scalePercent ?? 100) === 100) return false
  return target.fontPolicy.coverage !== 'dictionary_matches' || Boolean(target.dictionaryIds.length)
}
function actualStatus(item: WorkflowSummary): 'disabled' | 'running' | 'failed' | 'waiting' {
  const status = props.runtimeStatus[item.id]
  if (!props.activationIds.has(item.id)) return 'disabled'
  if (status && status.targets.length > 0 && status.targets.every(target => target.active)) return 'running'
  if (status && Object.keys(status.errors).length) return 'failed'
  return 'waiting'
}
function runtimeIssues(item: WorkflowSummary) {
  return Object.entries(props.runtimeStatus[item.id]?.errors ?? {}).map(([softwareId, error]) => ({
    softwareId,
    softwareName: props.software.find(candidate => candidate.id === softwareId)?.name ?? t('workflows.runtimeIssue.unknownSoftware'),
    error,
  }))
}
function runtimeIssueKind(item: WorkflowSummary) {
  const codes = [...new Set(runtimeIssues(item).map(issue => issue.error.code))]
  if (codes.length !== 1) return 'multiple'
  const code = codes[0] ?? ''
  if (code === 'runtime.target_access_failed' && runtimeIssues(item).every(issue => issue.error.args.operation === 'targetProcess')) return 'softwareStopped'
  const knownKinds: Record<string, string> = {
    'runtime.target_not_found': 'softwareStopped',
    'runtime.session_rejected': 'sessionRejected',
    'runtime.bundle_unavailable': 'bundleUnavailable',
    'runtime.target_access_failed': 'accessFailed',
    'runtime.component_load_failed': 'componentLoadFailed',
    'runtime.component_incompatible': 'componentIncompatible',
    'runtime.target_restart_required': 'targetRestartRequired',
    'runtime.activation_timed_out': 'activationTimedOut',
    'runtime.activation_failed': 'activationFailed',
    'runtime.target_in_use_by_probe': 'targetInUseByProbe',
    'runtime.target_in_use_by_workflow': 'targetInUseByWorkflow',
    'runtime.stop_unconfirmed': 'stopUnconfirmed',
    'runtime.unavailable': 'unavailable',
  }
  return knownKinds[code] ?? 'failed'
}
function actualLabel(item: WorkflowSummary) {
  return actualStatus(item) === 'failed'
    ? t(`workflows.runtimeIssue.status.${runtimeIssueKind(item)}`)
    : t(`workflows.status.${actualStatus(item)}`)
}
function actualColor(item: WorkflowSummary): 'success' | 'warning' | 'error' | 'neutral' {
  const status = actualStatus(item)
  if (status === 'running') return 'success'
  if (status === 'failed') return runtimeIssueKind(item) === 'softwareStopped' ? 'warning' : 'error'
  return 'neutral'
}

function toggleSelection(id: string) {
  const next = new Set(selected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selected.value = next
}
function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) pageItems.value.forEach(item => next.delete(item.id))
  else pageItems.value.forEach(item => next.add(item.id))
  selected.value = next
}
function resetForm() {
  softwarePickerOpen.value = false
  globalShortcut.value = ''
  name.value = ''
  description.value = ''
  targets.value = []
  activeSoftwareId.value = null
  lastSuggestedName.value = ''
  fontQuery.value = ''
  fontFilter.value = 'all'
  activeEditorTab.value = 'software'
}
function startCreate() {
  resetForm()
  formBaseline.value = serializeForm()
  creating.value = true
}
function closeForm() {
  discardFormOpen.value = false
  if (props.editing) emit('closeEdit')
  else creating.value = false
  resetForm()
}
function requestCloseForm() {
  if (formDirty.value) discardFormOpen.value = true
  else closeForm()
}
function confirmDiscardForm() {
  closeForm()
}
function toggleSoftware(id: string) {
  const index = targets.value.findIndex(target => target.softwareId === id)
  if (index >= 0) {
    targets.value.splice(index, 1)
    if (activeSoftwareId.value === id) activeSoftwareId.value = targets.value[0]?.softwareId ?? null
    return
  }
  const previous = targets.value[0]
  targets.value = [{
    softwareId: id,
    adapterPlan: { strategy: 'parallel', adapterIds: [] },
    dictionaryIds: previous?.dictionaryIds ?? [],
    writeDictionaryId: previous?.writeDictionaryId ?? null,
    fontPolicy: previous?.fontPolicy ?? null,
  }]
  activeSoftwareId.value = id
}
const fontModeOptions = computed(() => [
  { value: 'dictionary', label: t('workflows.fontModes.dictionary') },
  { value: 'workflow', label: t('workflows.fontModes.workflow') },
  { value: 'original', label: t('workflows.fontModes.original') },
])
function setFontMode(mode: string) {
  if (!activeTarget.value) return
  activeTarget.value.fontPolicy = mode === 'original' ? null : {
    families: activeTarget.value.fontPolicy?.families ?? [],
    coverage: activeTarget.value.fontPolicy?.coverage ?? 'dictionary_matches',
    preferDictionary: mode === 'dictionary',
    scalePercent: activeTarget.value.fontPolicy?.scalePercent ?? 100,
  }
}

function toggleFontFamily(family: string) {
  const policy = activeTarget.value?.fontPolicy
  if (!policy) return
  policy.families = policy.families.includes(family)
    ? policy.families.filter(candidate => candidate !== family)
    : [...policy.families, family]
}
function toggleFontFamilyFromRow(event: MouseEvent, family: string) {
  if (event.target instanceof Element && event.target.closest('button')) return
  toggleFontFamily(family)
}
function moveFontFamily(family: string, offset: number) {
  const policy = activeTarget.value?.fontPolicy
  if (!policy) return
  const current = policy.families.indexOf(family)
  const destination = current + offset
  if (current < 0 || destination < 0 || destination >= policy.families.length) return
  const next = [...policy.families]
  ;[next[current], next[destination]] = [next[destination], next[current]]
  policy.families = next
}
function submitForm() {
  if (!formValid.value) return
  const nextTargets = clone(targets.value).map(target => ({ ...target, writeDictionaryId: target.dictionaryIds.includes(target.writeDictionaryId ?? '') ? target.writeDictionaryId : target.dictionaryIds[0] ?? null }))
  const done = (saved: boolean) => { if (saved && creating.value) { creating.value = false; resetForm() } }
  if (props.editing) emit('save', { ...props.editing, name: name.value.trim(), description: description.value.trim(), targets: nextTargets, globalShortcut: globalShortcut.value }, done)
  else emit('create', name.value.trim(), description.value.trim(), nextTargets, globalShortcut.value, done)
}
function confirmRemoval() {
  const ids = pendingRemoval.value.map(item => item.id)
  emit('remove', ids)
  selected.value = new Set([...selected.value].filter(id => !ids.includes(id)))
  pendingRemoval.value = []
}
function openOnDoubleClick(event: MouseEvent) {
  const index = editableRowIndex(event)
  const item = index === null ? null : pageItems.value[index]
  if (item) emit('collect', item.id)
}

usePageEscape(() => formOpen.value, requestCloseForm)
</script>

<template>
  <section :data-testid="formOpen ? 'workflow-editor' : undefined" class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" :aria-labelledby="formOpen ? 'workflow-editor-title' : 'workflow-title'">
    <template v-if="!formOpen">
    <ManagementPageHeader title-id="workflow-title" :title="t('workflows.title')" icon="i-tabler-git-branch">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('workflows.create')" :disabled="busy" @click="startCreate" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="refreshing ? t('workflows.refreshing') : t('workflows.refresh')" :loading="refreshing" :disabled="refreshing" @click="emit('refresh')" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="workflowMessage" role="alert" color="error" variant="soft" :title="t('workflows.error')" :description="workflowMessage" class="mb-3" />
    <ManagementTableFrame v-model:query="query" v-model:filter-value="statusFilter" v-model:page="page" v-model:page-size="pageSize" :search-placeholder="t('workflows.searchPlaceholder')" :search-label="t('workflows.searchLabel')" :filter-label="statusFilterOptions.find(option => option.value === statusFilter)?.label" :filter-aria-label="t('workflows.filterLabel')" :filter-options="statusFilterOptions" :column-options="columnOptions" :columns-label="t('table.columns')" :selected-count="selected.size" :selected-label="t('workflows.itemLabel')" :total="filtered.length" :item-label="t('workflows.itemLabel')" @toggle-column="toggleColumn">
      <template #bulk-actions>
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-play" :label="t('workflows.bulkEnable')" :disabled="busy" @click="emit('toggleMany', [...selected], true)" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-stop" :label="t('workflows.bulkDisable')" :disabled="busy" @click="emit('toggleMany', [...selected], false)" />
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('workflows.bulkDelete')" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.id))" />
      </template>
      <UTable data-testid="workflow-management-table" role="region" tabindex="0" aria-labelledby="workflow-title" :data="pageItems" :columns="tableColumns" sticky class="management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'table-fixed min-w-[880px]' }" @dblclick="openOnDoubleClick">
        <template #select-header><UCheckbox :model-value="pageSelected" :aria-label="t('workflows.selectPage')" @update:model-value="togglePageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleSelection(row.original.id)" /></template>
        <template #workflow-cell="{ row }"><button type="button" class="max-w-full truncate text-left font-semibold hover:text-[var(--accent)] hover:underline focus-visible:outline-2 focus-visible:outline-[var(--accent)]" :disabled="busy" @click="emit('collect', row.original.id)">{{ row.original.name }}</button><div v-if="row.original.description" class="type-metadata mt-0.5 truncate text-[var(--text-muted)]">{{ row.original.description }}</div></template>
        <template #software-cell="{ row }"><div class="truncate" :title="softwareNames(row.original)">{{ softwareNames(row.original) }}</div><div class="type-metadata mt-1 text-[var(--text-muted)]" :title="adapterNames(row.original)">{{ t('workflows.adapterCount', { count: new Set(row.original.targets.flatMap((target: WorkflowTarget) => target.adapterPlan.adapterIds)).size }) }}</div></template>
        <template #assets-cell="{ row }"><UTooltip :text="dictionaryNames(row.original) || t('workflows.none')"><span tabindex="0" class="inline-block">{{ t('workflows.dictionaryCount', { count: row.original.dictionaryIds.length }) }}</span></UTooltip><div class="type-metadata mt-0.5 truncate text-[var(--text-muted)]" :title="fontNames(row.original)">{{ t('workflows.fonts', { names: fontNames(row.original) || t('workflows.none') }) }}</div></template>
        <template #status-cell="{ row }">
          <UPopover v-if="runtimeIssues(row.original).length" :ui="{ content: 'z-[80]' }">
            <UButton :color="actualColor(row.original)" variant="soft" size="xs" :label="actualLabel(row.original)" />
            <template #content>
              <section data-testid="workflow-runtime-issues" class="w-80 p-3" :aria-label="t('workflows.runtimeIssue.title')">
                <h2 class="m-0 text-[11px] font-semibold">{{ t('workflows.runtimeIssue.title') }}</h2>
                <p class="type-metadata m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ t('workflows.runtimeIssue.description') }}</p>
                <div class="mt-3 divide-y divide-[var(--border)] border-y border-[var(--border)]">
                  <div v-for="issue in runtimeIssues(row.original)" :key="issue.softwareId" class="py-2.5">
                    <div class="type-label font-semibold">{{ issue.softwareName }}</div>
                    <p class="type-metadata m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ translateCommandError(issue.error) }}</p>
                  </div>
                </div>
                <div class="mt-3 flex justify-end">
                  <UButton color="neutral" variant="outline" size="xs" icon="i-tabler-refresh" :label="t('workflows.runtimeIssue.refresh')" :loading="refreshing" :disabled="refreshing" @click="emit('refresh')" />
                </div>
              </section>
            </template>
          </UPopover>
          <UBadge v-else :color="actualColor(row.original)" variant="soft" size="sm" :label="actualLabel(row.original)" />
        </template>
        <template #enabled-cell="{ row }"><USwitch :model-value="activationIds.has(row.original.id)" :disabled="busy" :aria-label="t('workflows.enableNamed', { name: row.original.name })" @update:model-value="emit('toggle', row.original.id, Boolean($event))" /></template>
        <template #actions-cell="{ row }"><div class="flex items-center justify-center gap-1 whitespace-nowrap"><UButton color="primary" variant="ghost" size="xs" icon="i-tabler-language" :label="t('workflows.collectAndTranslate')" :disabled="busy" @click="emit('collect', row.original.id)" /><UButton :title="t('workflows.settings')" color="neutral" variant="ghost" size="xs" icon="i-tabler-settings" :aria-label="t('common.editNamed', { name: row.original.name })" @click="emit('open', row.original.id)" /><UDropdownMenu :items="workflowOverflowItems(row.original)" :content="{ align: 'end' }"><UButton :title="t('common.moreActionsNamed', { name: row.original.name })" color="neutral" variant="ghost" size="xs" icon="i-tabler-dots" :aria-label="t('common.moreActionsNamed', { name: row.original.name })" /></UDropdownMenu></div></template>
        <template #empty><UEmpty icon="i-tabler-git-branch" :title="items.length ? t('workflows.noMatch') : t('workflows.empty')" :description="t('workflows.emptyDescription')" /></template>
      </UTable>
    </ManagementTableFrame>

    <RuntimeDiagnosticsModal :open="Boolean(diagnosticWorkflow)" :workflow="diagnosticWorkflow" @update:open="$event || (diagnosticWorkflow = null)" />
    </template>

    <template v-else>
      <ManagementDetailHeader
        title-id="workflow-editor-title"
        :title="editorTitle"
        :back-label="t('workflows.backToList')"
        @back="requestCloseForm"
      >
        <template #status><UBadge v-if="formDirty" color="warning" variant="subtle" size="sm" :label="t('common.unsaved')" /></template>
        <template #actions>
          <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="editingWorkflow ? t('workflows.save') : t('workflows.createConfirm')" :loading="busy" :disabled="busy || !formValid" @click="submitForm" />
        </template>
      </ManagementDetailHeader>
      <UAlert v-if="workflowMessage" role="alert" color="warning" variant="soft" :title="t('workflows.settings')" :description="workflowMessage" class="mb-3" />
      <ManagementWorkspaceSurface variant="canvas">
      <UTabs v-model="activeEditorTab" data-testid="workflow-editor-tabs" :items="editorTabs" color="neutral" variant="link" size="sm" orientation="vertical" activation-mode="manual" class="h-full min-h-0 w-full" :ui="{ root: '!grid h-full min-h-0 w-full grid-cols-[176px_minmax(0,1fr)] items-stretch gap-0', list: '!flex h-full min-h-0 flex-col justify-start gap-1 overflow-y-auto rounded-none border-r border-[var(--border)] bg-[var(--surface-subtle)] p-3 [scrollbar-gutter:stable]', indicator: 'hidden', trigger: 'type-label relative h-9 w-full flex-none justify-start gap-2 rounded-[5px] px-2.5 py-0 text-[var(--text-secondary)] after:absolute after:inset-y-2 after:left-0 after:hidden after:w-0.5 after:rounded-full after:bg-[var(--accent)] hover:bg-[var(--surface-hover)] data-[state=active]:bg-[var(--selection)] data-[state=active]:font-semibold data-[state=active]:!text-[var(--text)] data-[state=active]:after:block', leadingIcon: 'size-4 shrink-0', label: 'min-w-0 flex-1 truncate text-left', trailingBadge: 'type-caption ml-auto min-w-4 justify-center px-1', content: 'min-h-0 overflow-y-auto rounded-none bg-[var(--app-bg)] px-5 py-5 focus:outline-none [scrollbar-gutter:stable]' }">
        <template #basic>
          <ManagementFormSection
            data-testid="workflow-basic-tab"
            :title="t('workflows.basicHeading')"
            :heading-level="2"
          >
            <ManagementFormRow :label="t('workflows.name')" required>
              <UInput v-model="name" :aria-label="t('workflows.name')" :maxlength="128" class="w-full" />
            </ManagementFormRow>
            <ManagementFormRow :label="t('workflows.workflowDescription')" multiline>
              <UTextarea v-model="description" :maxlength="512" :rows="3" autoresize :maxrows="6" class="w-full" />
            </ManagementFormRow>
            <ManagementFormRow :label="t('workflows.globalShortcut')" :help="t('workflows.shortcutHint')">
              <ShortcutRecorder v-model="globalShortcut" :disabled="busy" />
            </ManagementFormRow>
            <template #after>
              <UAlert v-if="messages[editing?.id ?? 'workflows']" color="error" variant="soft" :title="messages[editing?.id ?? 'workflows']" />
              <UAlert v-if="basicProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(basicProblems)" />
            </template>
          </ManagementFormSection>
        </template>

        <template #software>
          <section data-testid="workflow-software-tab" class="space-y-4" :aria-label="t('workflows.tabs.software')">
            <div class="flex items-end justify-between gap-3"><h2 class="type-body m-0 font-semibold">{{ t('workflows.softwareTargets') }}</h2><span class="type-metadata text-[var(--text-muted)]">{{ t('workflows.singleSoftwareHint') }}</span></div>
            <button type="button" data-testid="workflow-current-software" class="flex w-full items-center gap-3 rounded-[6px] border border-[var(--border)] px-4 py-3 text-left hover:bg-[var(--surface-hover)] focus-visible:outline-2 focus-visible:outline-[var(--accent)]" :aria-label="t('workflows.changeSoftware')" @click="openSoftwarePicker">
              <UIcon name="i-tabler-app-window" class="size-5 shrink-0 text-[var(--text-muted)]" />
              <span class="min-w-0 flex-1"><strong class="type-label block truncate">{{ activeSoftwareId ? softwareName(activeSoftwareId) : t('workflows.chooseTarget') }}</strong><span v-if="activeSoftwareId" class="type-metadata block truncate text-[var(--text-muted)]">{{ software.find(item => item.id === activeSoftwareId)?.executablePath }}</span></span>
              <span class="type-label text-[var(--accent)]">{{ t('workflows.changeSoftware') }}</span>
              <UIcon name="i-tabler-chevron-right" class="size-4 shrink-0" />
            </button>

            <section v-if="activeTarget" data-testid="workflow-adapter-config" class="space-y-3 border-t border-[var(--border)] pt-5">
              <div><h3 class="type-label m-0 font-semibold">{{ t('workflows.interceptionFor', { name: softwareName(activeTarget.softwareId) }) }}</h3><p class="type-metadata m-0 mt-1 text-[var(--text-muted)]">{{ t('workflows.adaptersHint') }}</p></div>
              <AdapterSelectionTable v-if="workflowAdapters.length" v-model="activeTarget.adapterPlan.adapterIds" :adapters="workflowAdapters" />
              <UAlert v-if="!workflowAdapters.length" color="warning" variant="soft" :title="t('workflows.noAdapters')" :description="t('workflows.noAdaptersDescription')" />
            </section>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.addSoftwareFirstDescription')" size="sm" />
            <UAlert v-if="softwareProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(softwareProblems)" />
          </section>
        </template>

        <template #dictionary>
          <section data-testid="workflow-dictionary-tab" class="space-y-4" :aria-label="t('workflows.tabs.dictionary')">
            <template v-if="activeTarget">
              <WorkflowDictionarySelection v-model:dictionary-ids="activeTarget.dictionaryIds" v-model:write-dictionary-id="activeTarget.writeDictionaryId" :dictionaries="dictionaries" :workflow-name="name" />
              <UAlert v-if="dictionaryProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(dictionaryProblems)" />
            </template>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.chooseTargetDescription')" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" :label="t('workflows.goSoftwareTab')" @click="activeEditorTab = 'software'" /></template></UEmpty>
          </section>
        </template>

        <template #font>
          <section data-testid="workflow-font-tab" class="space-y-4" :aria-label="t('workflows.tabs.font')">
            <template v-if="activeTarget">
              <div class="border-y border-[var(--border)] bg-[var(--surface-inset)] px-3 py-3"><strong class="type-label">{{ softwareName(activeTarget.softwareId) }}</strong><p class="type-metadata m-0 mt-2 leading-4 text-[var(--text-muted)]">{{ t('workflows.fontTargetHint', { name: softwareName(activeTarget.softwareId) }) }}</p></div>
              <UFormField :label="t('workflows.fontPolicy')" :hint="t('workflows.fontInheritanceHint')">
                <USelect :model-value="!activeTarget.fontPolicy ? 'original' : activeTarget.fontPolicy.preferDictionary ? 'dictionary' : 'workflow'" :items="fontModeOptions" :aria-label="t('workflows.fontPolicy')" class="w-full" @update:model-value="setFontMode(String($event))" />
              </UFormField>
              <div v-if="activeTarget.fontPolicy" class="space-y-3">
                <UFormField :label="t('workflows.fontScale')" :hint="t('workflows.fontScaleHint')">
                  <div class="flex items-center gap-2"><UInput :model-value="activeTarget.fontPolicy.scalePercent ?? 100" type="number" :min="50" :max="200" :step="5" :aria-label="t('workflows.fontScale')" class="w-28" @update:model-value="activeTarget.fontPolicy.scalePercent = Number($event)" /><span>%</span></div>
                </UFormField>
                <div class="flex items-start gap-4">
                  <UFormField :label="t('workflows.fontCoverageLabel')" class="w-56 shrink-0">
                    <USelect v-model="activeTarget.fontPolicy.coverage" :items="fontCoverageOptions" value-key="value" label-key="label" :aria-label="t('workflows.fontCoverageLabel')" class="w-full" />
                  </UFormField>
                  <p v-if="activeTarget.fontPolicy.coverage === 'dictionary_matches'" class="type-metadata m-0 max-w-[52ch] pt-5 leading-4 text-[var(--text-muted)]">{{ t('workflows.fontDictionaryMatchesHint') }}</p>
                </div>
                <UAlert v-if="activeTarget.fontPolicy.coverage === 'all_observations'" role="alert" color="warning" variant="soft" icon="i-tabler-alert-triangle" :aria-label="t('workflows.fontAllObservationsWarningTitle')" :title="t('workflows.fontAllObservationsWarningTitle')" :description="t('workflows.fontAllObservationsWarningDescription')" :ui="{ root: 'p-2', title: 'type-label', description: 'type-metadata leading-4' }" />
                <SelectedFontTags :families="activeTarget.fontPolicy.families" @move="moveFontFamily" @remove="toggleFontFamily" />
                <div class="flex items-center justify-between gap-3">
                  <div><h3 class="type-label m-0 font-semibold">{{ t('workflows.fontCatalog') }}</h3><p class="type-metadata m-0 mt-0.5 text-[var(--text-muted)]">{{ t('workflows.fontCacheCount', { count: installedFamilies.length }) }}</p></div>
                  <UButton color="neutral" variant="outline" size="xs" icon="i-tabler-refresh" :label="t('workflows.refreshFonts')" :aria-label="t('workflows.refreshFontsLabel')" :title="t('workflows.refreshFontsLabel')" :loading="fontRefreshing" :disabled="fontRefreshing" @click="emit('refreshFonts')" />
                </div>
                <div class="grid grid-cols-[minmax(0,1fr)_128px] gap-2"><UInput v-model="fontQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchFonts')" :aria-label="t('workflows.searchFonts')" /><USelect v-model="fontFilter" :items="catalogFilterOptions" value-key="value" label-key="label" :aria-label="t('workflows.fontFilter')" class="w-full" /></div>
                <div data-testid="workflow-font-catalog" class="max-h-64 overflow-y-auto rounded-[6px] border border-[var(--border)] [scrollbar-gutter:stable]">
                  <div v-for="family in visibleFontFamilies" :key="family" :data-font-family="family" class="flex min-h-10 cursor-pointer items-center gap-2 border-b border-[var(--border)] px-3 last:border-b-0 hover:bg-[var(--surface-hover)]" :class="activeTarget.fontPolicy.families.includes(family) ? 'bg-[var(--selection)]' : ''" @click="toggleFontFamilyFromRow($event, family)"><UCheckbox :model-value="activeTarget.fontPolicy.families.includes(family)" :aria-label="t('workflows.selectFontNamed', { name: family })" @update:model-value="toggleFontFamily(family)" /><span class="type-label min-w-0 flex-1 truncate">{{ family }}</span></div>
                  <div v-if="!visibleFontFamilies.length" class="type-metadata flex min-h-16 items-center justify-center px-4 py-4 text-center text-[var(--text-muted)]">{{ installedFamilies.length ? t('workflows.noFontMatch') : t('workflows.noInstalledFonts') }}</div>
                </div>
              </div>
              <div v-else class="type-metadata flex min-h-40 items-center justify-center rounded-[6px] border border-dashed border-[var(--border)] px-6 text-center leading-4 text-[var(--text-muted)]">{{ t('workflows.fontDisabledHint') }}</div>
              <UAlert v-if="fontProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(fontProblems)" />
            </template>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.chooseTargetDescription')" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" :label="t('workflows.goSoftwareTab')" @click="activeEditorTab = 'software'" /></template></UEmpty>
          </section>
        </template>
      </UTabs>
      </ManagementWorkspaceSurface>
    </template>

    <WorkflowSoftwarePicker v-model:open="softwarePickerOpen" :software="software" @select="selectWorkflowSoftware" />
    <ConfirmDialog :open="Boolean(pendingRemoval.length)" :title="t('workflows.deleteTitle')" :description="removalDescription" :confirm-label="t('workflows.deleteConfirm')" :busy="busy" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
    <ConfirmDialog :open="discardFormOpen" :title="t('common.discardTitle')" :description="t('common.discardDescription')" :cancel-label="t('common.continueEditing')" :confirm-label="t('common.discardChanges')" confirm-color="warning" @update:open="$event || (discardFormOpen = false)" @confirm="confirmDiscardForm" />

  </section>
</template>
