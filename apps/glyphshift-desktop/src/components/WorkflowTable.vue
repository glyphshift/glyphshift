<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
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
import { editableRowIndex } from '../tableInteraction'
import { usePageEscape } from '../usePageEscape'
import { useTableColumns } from '../useTableColumns'

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
  create: [name: string, description: string, targets: WorkflowTarget[]]
  save: [detail: WorkflowDetail]
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
const targets = ref<WorkflowTarget[]>([])
const activeSoftwareId = ref<string | null>(null)
const softwareQuery = ref('')
const dictionaryQuery = ref('')
const fontQuery = ref('')
const dictionaryFilter = ref('all')
const fontFilter = ref('all')
const activeEditorTab = ref('basic')
const pendingRemoval = ref<WorkflowSummary[]>([])
const diagnosticWorkflow = ref<WorkflowSummary | null>(null)
const formBaseline = ref('')
const discardFormOpen = ref(false)

function serializeForm() {
  return JSON.stringify({ name: name.value, description: description.value, targets: targets.value })
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
const visibleSoftware = computed(() => {
  const needle = softwareQuery.value.trim().toLocaleLowerCase()
  return props.software.filter(item => !needle || `${item.name} ${item.vendor} ${item.executableName}`.toLocaleLowerCase().includes(needle))
})
const activeTarget = computed(() => targets.value.find(target => target.softwareId === activeSoftwareId.value) ?? null)
const visibleDictionaries = computed(() => {
  const needle = dictionaryQuery.value.trim().toLocaleLowerCase()
  const selectedIds = activeTarget.value?.dictionaryIds ?? []
  return props.dictionaries
    .filter(item => (!needle || `${item.metadata.name} ${item.metadata.sourceLocale} ${item.metadata.targetLocale}`.toLocaleLowerCase().includes(needle))
      && (dictionaryFilter.value !== 'selected' || selectedIds.includes(item.metadata.id)))
    .sort((left, right) => {
      const leftIndex = selectedIds.indexOf(left.metadata.id)
      const rightIndex = selectedIds.indexOf(right.metadata.id)
      if (leftIndex >= 0 && rightIndex >= 0) return leftIndex - rightIndex
      if (leftIndex >= 0) return -1
      if (rightIndex >= 0) return 1
      return 0
    })
})
const visibleFontFamilies = computed(() => {
  const needle = fontQuery.value.trim().toLocaleLowerCase()
  const selectedFamilies = activeTarget.value?.fontPolicy?.families ?? []
  return props.installedFamilies
    .filter(family => (!needle || family.toLocaleLowerCase().includes(needle))
      && (fontFilter.value !== 'selected' || selectedFamilies.includes(family)))
    .sort((left, right) => {
      const leftIndex = selectedFamilies.indexOf(left)
      const rightIndex = selectedFamilies.indexOf(right)
      if (leftIndex >= 0 && rightIndex >= 0) return leftIndex - rightIndex
      if (leftIndex >= 0) return -1
      if (rightIndex >= 0) return 1
      return 0
    })
})
const targetOptions = computed(() => targets.value.map(target => ({
  value: target.softwareId,
  label: softwareName(target.softwareId),
  description: t('workflows.targetAssetSummary', {
    adapters: target.adapterPlan.adapterIds.length,
    dictionaries: target.dictionaryIds.length,
  }),
})))
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
    if (!target.fontPolicy.families.length) problems.push(t('workflows.problems.fontFamily', { name: label }))
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
  { value: 'basic', slot: 'basic', label: t('workflows.tabs.basic'), icon: 'i-tabler-adjustments-horizontal', badge: problemBadge(basicProblems.value) },
  { value: 'software', slot: 'software', label: t('workflows.tabs.software'), icon: 'i-tabler-app-window', badge: problemBadge(softwareProblems.value) },
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
const adapterGroups = computed(() => {
  const groups = new Map<string, AdapterOption[]>()
  for (const adapter of workflowAdapters.value) {
    const platform = adapter.platforms.join(' / ') || t('workflows.crossPlatform')
    const technology = adapter.technologies.join(' / ') || t('workflows.otherTechnology')
    const key = `${platform} · ${technology}`
    groups.set(key, [...(groups.get(key) ?? []), adapter])
  }
  return [...groups.entries()]
})
const columnOptions = computed(() => [
  { key: 'software', label: t('workflows.columns.software'), visible: visibleColumns.value.software },
  { key: 'adapters', label: t('workflows.columns.adapters'), visible: visibleColumns.value.adapters },
  { key: 'assets', label: t('workflows.columns.assets'), visible: visibleColumns.value.assets },
  { key: 'status', label: t('workflows.columns.status'), visible: visibleColumns.value.status },
  { key: 'enabled', label: t('workflows.columns.enabled'), visible: visibleColumns.value.enabled },
])
const tableColumns = computed<TableColumn<WorkflowSummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'workflow', header: t('workflows.columns.workflow'), meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  ...(visibleColumns.value.software ? [{ id: 'software', header: t('workflows.columns.software'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.adapters ? [{ id: 'adapters', header: t('workflows.columns.adapters'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.assets ? [{ id: 'assets', header: t('workflows.columns.assets') } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.status ? [{ id: 'status', header: t('workflows.columns.status'), meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(visibleColumns.value.enabled ? [{ id: 'enabled', header: t('workflows.columns.enabled'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  { id: 'actions', header: t('workflows.columns.actions'), meta: { class: { th: 'w-32 text-center', td: 'w-32 text-center' } } },
])
const removalDescription = computed(() => pendingRemoval.value.length === 1
  ? t('workflows.deleteOne', { name: pendingRemoval.value[0]?.name ?? '' })
  : t('workflows.deleteMany', { count: pendingRemoval.value.length }))

watch([query, statusFilter, pageSize], () => { page.value = 1 })
watch(activeSoftwareId, () => {
  dictionaryQuery.value = ''
  fontQuery.value = ''
  dictionaryFilter.value = 'all'
  fontFilter.value = 'all'
})
watch(() => props.editing, (detail) => {
  if (!detail) return
  name.value = detail.name
  description.value = detail.description
  targets.value = clone(detail.targets)
  activeSoftwareId.value = targets.value[0]?.softwareId ?? null
  activeEditorTab.value = 'basic'
  formBaseline.value = serializeForm()
}, { immediate: true })

function softwareName(id: string) {
  return props.software.find(item => item.id === id)?.name ?? id
}
function targetForSoftware(id: string) {
  return targets.value.find(target => target.softwareId === id) ?? null
}
function softwareTargetIsConfigured(id: string) {
  const target = targetForSoftware(id)
  return target ? targetIsConfigured(target) : false
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
  return ids.map(id => props.adapters.find(adapter => adapter.id === id)?.name ?? id).join(locale.value === 'zh-CN' ? '、' : ', ')
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
  if (!target.fontPolicy.families.length) return false
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
  name.value = ''
  description.value = ''
  targets.value = []
  activeSoftwareId.value = null
  softwareQuery.value = ''
  dictionaryQuery.value = ''
  fontQuery.value = ''
  dictionaryFilter.value = 'all'
  fontFilter.value = 'all'
  activeEditorTab.value = 'basic'
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
  targets.value.push({
    softwareId: id,
    adapterPlan: { strategy: 'parallel', adapterIds: [] },
    dictionaryIds: [],
    fontPolicy: null,
  })
  activeSoftwareId.value = id
}
function toggleAdapter(id: string) {
  if (!activeTarget.value) return
  activeTarget.value.adapterPlan.adapterIds = activeTarget.value.adapterPlan.adapterIds.includes(id)
    ? activeTarget.value.adapterPlan.adapterIds.filter(candidate => candidate !== id)
    : [...activeTarget.value.adapterPlan.adapterIds, id]
}
function toggleDictionary(id: string) {
  if (!activeTarget.value) return
  activeTarget.value.dictionaryIds = activeTarget.value.dictionaryIds.includes(id)
    ? activeTarget.value.dictionaryIds.filter(candidate => candidate !== id)
    : [...activeTarget.value.dictionaryIds, id]
}
function moveDictionary(id: string, offset: number) {
  if (!activeTarget.value) return
  const current = activeTarget.value.dictionaryIds.indexOf(id)
  const destination = current + offset
  if (current < 0 || destination < 0 || destination >= activeTarget.value.dictionaryIds.length) return
  const next = [...activeTarget.value.dictionaryIds]
  ;[next[current], next[destination]] = [next[destination], next[current]]
  activeTarget.value.dictionaryIds = next
}
function setFontPolicyEnabled(enabled: boolean) {
  if (!activeTarget.value) return
  activeTarget.value.fontPolicy = enabled
    ? { families: [], coverage: 'dictionary_matches' }
    : null
}
function toggleFontFamily(family: string) {
  const policy = activeTarget.value?.fontPolicy
  if (!policy) return
  policy.families = policy.families.includes(family)
    ? policy.families.filter(candidate => candidate !== family)
    : [...policy.families, family]
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
  const nextTargets = clone(targets.value)
  if (props.editing) emit('save', { ...props.editing, name: name.value.trim(), description: description.value.trim(), targets: nextTargets })
  else emit('create', name.value.trim(), description.value.trim(), nextTargets)
  if (!props.editing) creating.value = false
  resetForm()
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
  if (item) emit('open', item.id)
}

usePageEscape(() => formOpen.value, requestCloseForm)
</script>

<template>
  <section :data-testid="formOpen ? 'workflow-editor' : undefined" class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" :aria-labelledby="formOpen ? 'workflow-editor-title' : 'workflow-title'">
    <template v-if="!formOpen">
    <ManagementPageHeader title-id="workflow-title" :title="t('workflows.title')" :description="t('workflows.description')" icon="i-tabler-git-branch">
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
      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[1080px]' }" @dblclick="openOnDoubleClick">
        <template #select-header><UCheckbox :model-value="pageSelected" :aria-label="t('workflows.selectPage')" @update:model-value="togglePageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleSelection(row.original.id)" /></template>
        <template #workflow-cell="{ row }"><div class="truncate font-semibold">{{ row.original.name }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]">{{ row.original.description || t('workflows.revision', { revision: row.original.revision }) }}</div></template>
        <template #software-cell="{ row }"><div class="truncate" :title="softwareNames(row.original)">{{ softwareNames(row.original) }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.targetCount', { count: row.original.softwareIds.length }) }}</div></template>
        <template #adapters-cell="{ row }"><div class="truncate" :title="adapterNames(row.original)">{{ adapterNames(row.original) || t('workflows.notConfigured') }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.parallel') }}</div></template>
        <template #assets-cell="{ row }"><div class="truncate" :title="dictionaryNames(row.original)">{{ t('workflows.dictionaries', { names: dictionaryNames(row.original) || t('workflows.none') }) }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]" :title="fontNames(row.original)">{{ t('workflows.fonts', { names: fontNames(row.original) || t('workflows.none') }) }}</div></template>
        <template #status-cell="{ row }">
          <UPopover v-if="runtimeIssues(row.original).length" :ui="{ content: 'z-[80]' }">
            <UButton :color="actualColor(row.original)" variant="soft" size="xs" :label="actualLabel(row.original)" />
            <template #content>
              <section data-testid="workflow-runtime-issues" class="w-80 p-3" :aria-label="t('workflows.runtimeIssue.title')">
                <h3 class="m-0 text-[11px] font-semibold">{{ t('workflows.runtimeIssue.title') }}</h3>
                <p class="m-0 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.runtimeIssue.description') }}</p>
                <div class="mt-3 divide-y divide-[var(--border)] border-y border-[var(--border)]">
                  <div v-for="issue in runtimeIssues(row.original)" :key="issue.softwareId" class="py-2.5">
                    <div class="text-[10px] font-semibold">{{ issue.softwareName }}</div>
                    <p class="m-0 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ translateCommandError(issue.error) }}</p>
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
        <template #actions-cell="{ row }"><div class="flex justify-center gap-0.5"><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-activity-heartbeat" :disabled="actualStatus(row.original) !== 'running'" :aria-label="t('workflows.diagnostics.openNamed', { name: row.original.name })" @click="diagnosticWorkflow = row.original" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('common.editNamed', { name: row.original.name })" @click="emit('open', row.original.id)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-copy" :aria-label="t('workflows.copyNamed', { name: row.original.name })" @click="emit('copy', row.original.id)" /><UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original]" /></div></template>
        <template #empty><UEmpty icon="i-tabler-git-branch" :title="items.length ? t('workflows.noMatch') : t('workflows.empty')" :description="t('workflows.emptyDescription')" /></template>
      </UTable>
    </ManagementTableFrame>

    <RuntimeDiagnosticsModal :open="Boolean(diagnosticWorkflow)" :workflow="diagnosticWorkflow" @update:open="$event || (diagnosticWorkflow = null)" />
    </template>

    <template v-else>
      <ManagementDetailHeader
        title-id="workflow-editor-title"
        :title="editorTitle"
        :description="t('workflows.formDescription')"
        :back-label="t('workflows.backToList')"
        @back="requestCloseForm"
      >
        <template #status><UBadge v-if="formDirty" color="warning" variant="subtle" size="sm" :label="t('common.unsaved')" /></template>
        <template #actions>
          <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="editingWorkflow ? t('workflows.save') : t('workflows.createConfirm')" :loading="busy" :disabled="busy || !formValid" @click="submitForm" />
        </template>
      </ManagementDetailHeader>
      <ManagementWorkspaceSurface variant="canvas">
      <UTabs v-model="activeEditorTab" data-testid="workflow-editor-tabs" :items="editorTabs" color="neutral" variant="link" size="sm" orientation="vertical" activation-mode="manual" class="h-full min-h-0 w-full" :ui="{ root: '!grid h-full min-h-0 w-full grid-cols-[176px_minmax(0,1fr)] items-stretch gap-0', list: '!flex h-full min-h-0 flex-col justify-start gap-1 overflow-y-auto rounded-none border-r border-[var(--border)] bg-[var(--surface-subtle)] p-3 [scrollbar-gutter:stable]', indicator: 'hidden', trigger: 'relative h-9 w-full flex-none justify-start gap-2 rounded-[5px] px-2.5 py-0 text-[10px] text-[var(--text-secondary)] after:absolute after:inset-y-2 after:left-0 after:hidden after:w-0.5 after:rounded-full after:bg-[var(--accent)] hover:bg-[var(--surface-hover)] data-[state=active]:bg-[var(--selection)] data-[state=active]:font-semibold data-[state=active]:!text-[var(--text)] data-[state=active]:after:block', leadingIcon: 'size-4 shrink-0', label: 'min-w-0 flex-1 truncate text-left', trailingBadge: 'ml-auto min-w-4 justify-center px-1 text-[8px]', content: 'min-h-0 overflow-y-auto rounded-none bg-[var(--app-bg)] px-5 py-5 focus:outline-none [scrollbar-gutter:stable]' }">
        <template #basic>
          <ManagementFormSection
            data-testid="workflow-basic-tab"
            :title="t('workflows.basicHeading')"
            :description="t('workflows.basicHint')"
            :heading-level="3"
          >
            <ManagementFormRow :label="t('workflows.name')" required>
              <UInput v-model="name" :maxlength="128" class="w-full" />
            </ManagementFormRow>
            <ManagementFormRow :label="t('workflows.workflowDescription')" multiline>
              <UTextarea v-model="description" :maxlength="512" :rows="3" autoresize :maxrows="6" class="w-full" />
            </ManagementFormRow>
            <template #after>
              <UAlert v-if="basicProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(basicProblems)" />
            </template>
          </ManagementFormSection>
        </template>

        <template #software>
          <section data-testid="workflow-software-tab" class="space-y-4" :aria-label="t('workflows.tabs.software')">
            <div class="flex items-end justify-between gap-3"><div><h3 class="m-0 text-[12px] font-semibold">{{ t('workflows.softwareTargets') }}</h3><p class="m-0 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.softwareInterceptionHint') }}</p></div><span class="shrink-0 text-[9px] text-[var(--text-muted)]">{{ t('workflows.targetCount', { count: targets.length }) }}</span></div>
            <UInput v-model="softwareQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchSoftware')" :aria-label="t('workflows.searchSoftware')" />
            <div data-testid="workflow-software-catalog" class="max-h-48 overflow-y-auto rounded-[6px] border border-[var(--border)] [scrollbar-gutter:stable]">
              <div v-for="item in visibleSoftware" :key="item.id" class="flex min-h-12 items-center border-b border-[var(--border)] last:border-b-0" :class="activeSoftwareId === item.id ? 'bg-[var(--selection)]' : ''">
                <UButton color="neutral" variant="ghost" class="flex min-w-0 flex-1 justify-start gap-2 rounded-none px-3 py-2 text-left" :aria-label="targetForSoftware(item.id) ? t('workflows.selectSoftwareTargetNamed', { name: item.name }) : t('workflows.addSoftwareNamed', { name: item.name })" :aria-pressed="Boolean(targetForSoftware(item.id))" @click="targetForSoftware(item.id) ? (activeSoftwareId = item.id) : toggleSoftware(item.id)">
                  <UIcon :name="targetForSoftware(item.id) ? 'i-tabler-square-check-filled' : 'i-tabler-square'" class="size-4 shrink-0" :class="targetForSoftware(item.id) ? 'text-[var(--accent)]' : 'text-[var(--text-muted)]'" />
                  <span class="min-w-0 flex-1"><strong class="block truncate text-[10px]">{{ item.name }}</strong><span class="block truncate text-[9px] text-[var(--text-muted)]">{{ targetForSoftware(item.id) ? t('workflows.adapterCount', { count: targetForSoftware(item.id)?.adapterPlan.adapterIds.length ?? 0 }) : item.executableName }}</span></span>
                  <UBadge v-if="targetForSoftware(item.id)" :color="softwareTargetIsConfigured(item.id) ? 'success' : 'warning'" variant="soft" size="sm" :label="softwareTargetIsConfigured(item.id) ? t('workflows.targetConfigured') : t('workflows.targetNeedsConfiguration')" />
                </UButton>
                <UButton v-if="targetForSoftware(item.id)" color="error" variant="ghost" size="xs" icon="i-tabler-x" :aria-label="t('workflows.removeTargetNamed', { name: item.name })" class="mr-2 shrink-0" @click="toggleSoftware(item.id)" />
              </div>
              <div v-if="!visibleSoftware.length" class="flex min-h-20 flex-col items-center justify-center gap-2 px-4 py-5 text-center text-[9px] text-[var(--text-muted)]"><span>{{ software.length ? t('workflows.noSoftwareMatch') : t('workflows.addSoftwareFirstDescription') }}</span><UButton v-if="!software.length" color="neutral" variant="outline" size="xs" :label="t('workflows.goAddSoftware')" @click="closeForm(); emit('navigate', 'software')" /></div>
            </div>

            <section v-if="activeTarget" data-testid="workflow-adapter-config" class="space-y-3 border-t border-[var(--border)] pt-5">
              <div><h4 class="m-0 text-[11px] font-semibold">{{ t('workflows.interceptionFor', { name: softwareName(activeTarget.softwareId) }) }}</h4><p class="m-0 mt-1 text-[9px] text-[var(--text-muted)]">{{ t('workflows.adaptersHint') }}</p></div>
              <div v-for="[group, options] in adapterGroups" :key="group" class="space-y-1"><div class="text-[9px] text-[var(--text-muted)]">{{ group }}</div><div class="overflow-hidden rounded-[5px] border border-[var(--border)]"><label v-for="adapter in options" :key="adapter.id" class="flex cursor-pointer items-start gap-2 border-b border-[var(--border)] p-2.5 last:border-b-0 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.adapterPlan.adapterIds.includes(adapter.id)" class="mt-0.5" @update:model-value="toggleAdapter(adapter.id)" /><span class="min-w-0"><strong class="block text-[10px]">{{ adapter.name }}</strong><span class="block text-[9px] leading-4 text-[var(--text-muted)]">{{ adapter.summary }}</span></span></label></div></div>
              <UAlert v-if="!workflowAdapters.length" color="warning" variant="soft" :title="t('workflows.noAdapters')" :description="t('workflows.noAdaptersDescription')" />
            </section>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.addSoftwareFirstDescription')" size="sm" />
            <UAlert v-if="softwareProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(softwareProblems)" />
          </section>
        </template>

        <template #dictionary>
          <section data-testid="workflow-dictionary-tab" class="space-y-4" :aria-label="t('workflows.tabs.dictionary')">
            <template v-if="activeTarget">
              <div class="border-y border-[var(--border)] bg-[var(--surface-inset)] px-3 py-3"><UFormField :label="t('workflows.currentTarget')"><USelectMenu v-model="activeSoftwareId" :items="targetOptions" value-key="value" label-key="label" description-key="description" :search-input="{ placeholder: t('workflows.searchSelectedTargets') }" :aria-label="t('workflows.currentTarget')" class="w-full" :ui="{ content: 'z-[90]' }" /></UFormField><p class="m-0 mt-2 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.dictionaryTargetHint', { name: softwareName(activeTarget.softwareId) }) }}</p></div>
              <div class="flex items-end justify-between gap-3"><div><h3 class="m-0 text-[12px] font-semibold">{{ t('workflows.orderedDictionaries', { count: activeTarget.dictionaryIds.length }) }}</h3><p class="m-0 mt-1 text-[9px] text-[var(--text-muted)]">{{ t('workflows.dictionarySingleListHint') }}</p></div><UButton v-if="!dictionaries.length" color="neutral" variant="ghost" size="xs" :label="t('workflows.goCreate')" @click="closeForm(); emit('navigate', 'dictionaries')" /></div>
              <div class="grid grid-cols-[minmax(0,1fr)_128px] gap-2"><UInput v-model="dictionaryQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchDictionaries')" :aria-label="t('workflows.searchDictionaries')" /><USelect v-model="dictionaryFilter" :items="catalogFilterOptions" value-key="value" label-key="label" :aria-label="t('workflows.dictionaryFilter')" class="w-full" /></div>
              <div data-testid="workflow-dictionary-catalog" class="max-h-80 overflow-y-auto rounded-[6px] border border-[var(--border)] [scrollbar-gutter:stable]">
                <div v-for="item in visibleDictionaries" :key="item.metadata.id" :data-dictionary-id="item.metadata.id" class="flex min-h-11 items-center gap-2 border-b border-[var(--border)] px-3 py-1.5 last:border-b-0 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.dictionaryIds.includes(item.metadata.id)" :aria-label="t('workflows.selectDictionaryNamed', { name: item.metadata.name })" @update:model-value="toggleDictionary(item.metadata.id)" /><span class="min-w-0 flex-1"><strong class="block truncate text-[10px]">{{ item.metadata.name }}</strong><span class="block text-[9px] text-[var(--text-muted)]">{{ item.metadata.sourceLocale }} → {{ item.metadata.targetLocale }}</span></span><template v-if="activeTarget.dictionaryIds.includes(item.metadata.id)"><UBadge color="neutral" variant="soft" size="sm" :label="t('workflows.priorityNumber', { number: activeTarget.dictionaryIds.indexOf(item.metadata.id) + 1 })" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="activeTarget.dictionaryIds.indexOf(item.metadata.id) === 0" :aria-label="t('workflows.raiseDictionary', { name: item.metadata.name })" @click="moveDictionary(item.metadata.id, -1)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="activeTarget.dictionaryIds.indexOf(item.metadata.id) === activeTarget.dictionaryIds.length - 1" :aria-label="t('workflows.lowerDictionary', { name: item.metadata.name })" @click="moveDictionary(item.metadata.id, 1)" /></template></div>
                <div v-if="!visibleDictionaries.length" class="flex min-h-20 items-center justify-center px-4 py-5 text-center text-[9px] text-[var(--text-muted)]">{{ dictionaries.length ? t('workflows.noDictionaryMatch') : t('workflows.noDictionaries') }}</div>
              </div>
              <UAlert v-if="dictionaryProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(dictionaryProblems)" />
            </template>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.chooseTargetDescription')" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" :label="t('workflows.goSoftwareTab')" @click="activeEditorTab = 'software'" /></template></UEmpty>
          </section>
        </template>

        <template #font>
          <section data-testid="workflow-font-tab" class="space-y-4" :aria-label="t('workflows.tabs.font')">
            <template v-if="activeTarget">
              <div class="border-y border-[var(--border)] bg-[var(--surface-inset)] px-3 py-3"><UFormField :label="t('workflows.currentTarget')"><USelectMenu v-model="activeSoftwareId" :items="targetOptions" value-key="value" label-key="label" description-key="description" :search-input="{ placeholder: t('workflows.searchSelectedTargets') }" :aria-label="t('workflows.currentTarget')" class="w-full" :ui="{ content: 'z-[90]' }" /></UFormField><p class="m-0 mt-2 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.fontTargetHint', { name: softwareName(activeTarget.softwareId) }) }}</p></div>
              <div class="flex items-start justify-between gap-3"><div><h3 class="m-0 text-[12px] font-semibold">{{ t('workflows.fontPolicy') }}</h3><p class="m-0 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.fontPolicyHint') }}</p></div><USwitch :model-value="Boolean(activeTarget.fontPolicy)" :aria-label="t('workflows.fontPolicyToggle')" @update:model-value="setFontPolicyEnabled(Boolean($event))" /></div>
              <div v-if="activeTarget.fontPolicy" class="space-y-3">
                <div class="flex items-start gap-4">
                  <UFormField :label="t('workflows.fontCoverageLabel')" class="w-56 shrink-0">
                    <USelect v-model="activeTarget.fontPolicy.coverage" :items="fontCoverageOptions" value-key="value" label-key="label" :aria-label="t('workflows.fontCoverageLabel')" class="w-full" />
                  </UFormField>
                  <p v-if="activeTarget.fontPolicy.coverage === 'dictionary_matches'" class="m-0 max-w-[52ch] pt-5 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.fontDictionaryMatchesHint') }}</p>
                </div>
                <UAlert v-if="activeTarget.fontPolicy.coverage === 'all_observations'" role="alert" color="warning" variant="soft" icon="i-tabler-alert-triangle" :aria-label="t('workflows.fontAllObservationsWarningTitle')" :title="t('workflows.fontAllObservationsWarningTitle')" :description="t('workflows.fontAllObservationsWarningDescription')" :ui="{ root: 'p-2', title: 'text-[9px]', description: 'text-[9px] leading-4' }" />
                <div class="flex items-center justify-between gap-3">
                  <div><h4 class="m-0 text-[11px] font-semibold">{{ t('workflows.fontCatalog') }}</h4><p class="m-0 mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.fontCacheCount', { count: installedFamilies.length }) }}</p></div>
                  <UButton color="neutral" variant="outline" size="xs" icon="i-tabler-refresh" :label="t('workflows.refreshFonts')" :aria-label="t('workflows.refreshFontsLabel')" :title="t('workflows.refreshFontsLabel')" :loading="fontRefreshing" :disabled="fontRefreshing" @click="emit('refreshFonts')" />
                </div>
                <div class="grid grid-cols-[minmax(0,1fr)_128px] gap-2"><UInput v-model="fontQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchFonts')" :aria-label="t('workflows.searchFonts')" /><USelect v-model="fontFilter" :items="catalogFilterOptions" value-key="value" label-key="label" :aria-label="t('workflows.fontFilter')" class="w-full" /></div>
                <div data-testid="workflow-font-catalog" class="max-h-64 overflow-y-auto rounded-[6px] border border-[var(--border)] [scrollbar-gutter:stable]">
                  <div v-for="family in visibleFontFamilies" :key="family" :data-font-family="family" class="flex min-h-10 items-center gap-2 border-b border-[var(--border)] px-3 last:border-b-0 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.fontPolicy.families.includes(family)" :aria-label="t('workflows.selectFontNamed', { name: family })" @update:model-value="toggleFontFamily(family)" /><span class="min-w-0 flex-1 truncate text-[9px]">{{ family }}</span><template v-if="activeTarget.fontPolicy.families.includes(family)"><UBadge color="neutral" variant="soft" size="sm" :label="t('workflows.priorityNumber', { number: activeTarget.fontPolicy.families.indexOf(family) + 1 })" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="activeTarget.fontPolicy.families.indexOf(family) === 0" :aria-label="t('workflows.raiseFont', { name: family })" @click="moveFontFamily(family, -1)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="activeTarget.fontPolicy.families.indexOf(family) === activeTarget.fontPolicy.families.length - 1" :aria-label="t('workflows.lowerFont', { name: family })" @click="moveFontFamily(family, 1)" /></template></div>
                  <div v-if="!visibleFontFamilies.length" class="flex min-h-16 items-center justify-center px-4 py-4 text-center text-[9px] text-[var(--text-muted)]">{{ installedFamilies.length ? t('workflows.noFontMatch') : t('workflows.noInstalledFonts') }}</div>
                </div>
              </div>
              <div v-else class="flex min-h-40 items-center justify-center rounded-[6px] border border-dashed border-[var(--border)] px-6 text-center text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.fontDisabledHint') }}</div>
              <UAlert v-if="fontProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="describeProblems(fontProblems)" />
            </template>
            <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.chooseTargetDescription')" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" :label="t('workflows.goSoftwareTab')" @click="activeEditorTab = 'software'" /></template></UEmpty>
          </section>
        </template>
      </UTabs>
      </ManagementWorkspaceSurface>
    </template>

    <ConfirmDialog :open="Boolean(pendingRemoval.length)" :title="t('workflows.deleteTitle')" :description="removalDescription" :busy="busy" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
    <ConfirmDialog :open="discardFormOpen" :title="t('common.discardTitle')" :description="t('common.discardDescription')" :cancel-label="t('common.continueEditing')" :confirm-label="t('common.discardChanges')" confirm-color="warning" @update:open="$event || (discardFormOpen = false)" @confirm="confirmDiscardForm" />
  </section>
</template>
