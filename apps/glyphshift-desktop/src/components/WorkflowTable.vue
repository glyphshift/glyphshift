<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type {
  AdapterOption,
  DictionarySummary,
  SoftwareRecord,
  WorkflowDetail,
  WorkflowRuntimeStatus,
  WorkflowSummary,
  WorkflowTarget,
} from '../model'

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
  open: [id: string]
  create: [name: string, description: string, targets: WorkflowTarget[]]
  save: [detail: WorkflowDetail]
  closeEdit: []
  copy: [id: string]
  remove: [ids: string[]]
  navigate: [view: 'software' | 'dictionaries']
}>()
const { t, locale } = useI18n()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const statusFilter = ref('all')
const creating = ref(false)
const name = ref('')
const description = ref('')
const targets = ref<WorkflowTarget[]>([])
const activeSoftwareId = ref<string | null>(null)
const softwareQuery = ref('')
const dictionaryQuery = ref('')
const fontQuery = ref('')
const pendingRemoval = ref<WorkflowSummary[]>([])

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
const visibleDictionaries = computed(() => {
  const needle = dictionaryQuery.value.trim().toLocaleLowerCase()
  return props.dictionaries.filter(item => !needle || `${item.metadata.name} ${item.metadata.sourceLocale} ${item.metadata.targetLocale}`.toLocaleLowerCase().includes(needle))
})
const visibleFontFamilies = computed(() => {
  const needle = fontQuery.value.trim().toLocaleLowerCase()
  return props.installedFamilies.filter(family => !needle || family.toLocaleLowerCase().includes(needle))
})
const activeTarget = computed(() => targets.value.find(target => target.softwareId === activeSoftwareId.value) ?? null)
const activeSoftware = computed(() => props.software.find(item => item.id === activeSoftwareId.value) ?? null)
const formOpen = computed(() => creating.value || Boolean(props.editing))
const editingWorkflow = computed(() => Boolean(props.editing))
const formProblems = computed(() => {
  const problems: string[] = []
  if (!name.value.trim()) problems.push(t('workflows.problems.name'))
  if (!targets.value.length) problems.push(t('workflows.problems.target'))
  for (const target of targets.value) {
    const label = softwareName(target.softwareId)
    if (!target.adapterPlan.adapterIds.length) problems.push(t('workflows.problems.adapter', { name: label }))
    if (!target.dictionaryIds.length && !target.fontPolicy) problems.push(t('workflows.problems.asset', { name: label }))
    if (target.fontPolicy) {
      if (!target.fontPolicy.families.length) problems.push(t('workflows.problems.fontFamily', { name: label }))
      if (target.fontPolicy.coverage === 'dictionary_matches' && !target.dictionaryIds.length) {
        problems.push(t('workflows.problems.fontDictionary', { name: label }))
      }
    }
  }
  return [...new Set(problems)]
})
const formValid = computed(() => formProblems.value.length === 0)
const formProblemDescription = computed(() => {
  const visible = formProblems.value.slice(0, 3).join('；')
  const remaining = formProblems.value.length - 3
  return remaining > 0 ? t('workflows.moreProblems', { visible, count: remaining }) : visible
})
const workflowMessage = computed(() => props.messages.workflows || props.items.map(item => props.messages[item.id]).find(Boolean) || '')
const adapterGroups = computed(() => {
  const groups = new Map<string, AdapterOption[]>()
  for (const adapter of props.adapters) {
    const platform = adapter.platforms.join(' / ') || t('workflows.crossPlatform')
    const technology = adapter.technologies.join(' / ') || t('workflows.otherTechnology')
    const key = `${platform} · ${technology}`
    groups.set(key, [...(groups.get(key) ?? []), adapter])
  }
  return [...groups.entries()]
})
const columns = computed<TableColumn<WorkflowSummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'workflow', header: t('workflows.columns.workflow'), meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  { id: 'software', header: t('workflows.columns.software'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { id: 'adapters', header: t('workflows.columns.adapters'), meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { id: 'assets', header: t('workflows.columns.assets') },
  { id: 'status', header: t('workflows.columns.status'), meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } },
  { id: 'enabled', header: t('workflows.columns.enabled'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
  { id: 'actions', header: t('workflows.columns.actions'), meta: { class: { th: 'w-28 text-center', td: 'w-28 text-center' } } },
])
const removalDescription = computed(() => pendingRemoval.value.length === 1
  ? t('workflows.deleteOne', { name: pendingRemoval.value[0]?.name ?? '' })
  : t('workflows.deleteMany', { count: pendingRemoval.value.length }))

watch([query, statusFilter, pageSize], () => { page.value = 1 })
watch(activeSoftwareId, () => { fontQuery.value = '' })
watch(() => props.editing, (detail) => {
  if (!detail) return
  name.value = detail.name
  description.value = detail.description
  targets.value = clone(detail.targets)
  activeSoftwareId.value = targets.value[0]?.softwareId ?? null
}, { immediate: true })

function softwareName(id: string) {
  return props.software.find(item => item.id === id)?.name ?? id
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
function actualStatus(item: WorkflowSummary): 'disabled' | 'running' | 'attention' | 'waiting' {
  const status = props.runtimeStatus[item.id]
  if (!props.activationIds.has(item.id)) return 'disabled'
  if (status && status.targets.length > 0 && status.targets.every(target => target.active)) return 'running'
  if (status && Object.keys(status.errors).length) return 'attention'
  return 'waiting'
}
function actualLabel(item: WorkflowSummary) {
  return t(`workflows.status.${actualStatus(item)}`)
}
function actualColor(item: WorkflowSummary): 'success' | 'error' | 'neutral' {
  const status = actualStatus(item)
  if (status === 'running') return 'success'
  if (status === 'attention') return 'error'
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
}
function startCreate() {
  resetForm()
  creating.value = true
}
function closeForm() {
  if (props.editing) emit('closeEdit')
  else creating.value = false
  resetForm()
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
function setFontCoverage(coverage: 'dictionary_matches' | 'all_observations') {
  if (activeTarget.value?.fontPolicy) activeTarget.value.fontPolicy.coverage = coverage
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
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="workflow-title">
    <ManagementPageHeader title-id="workflow-title" :title="t('workflows.title')" :description="t('workflows.description')" icon="i-tabler-git-branch">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('workflows.create')" :disabled="busy" @click="startCreate" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="refreshing ? t('workflows.refreshing') : t('workflows.refresh')" :loading="refreshing" :disabled="refreshing" @click="emit('refresh')" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="workflowMessage" role="alert" color="error" variant="soft" :title="t('workflows.error')" :description="workflowMessage" class="mb-3" />
    <ManagementTableFrame v-model:query="query" v-model:filter-value="statusFilter" v-model:page="page" v-model:page-size="pageSize" :search-placeholder="t('workflows.searchPlaceholder')" :search-label="t('workflows.searchLabel')" :filter-label="statusFilterOptions.find(option => option.value === statusFilter)?.label" :filter-aria-label="t('workflows.filterLabel')" :filter-options="statusFilterOptions" :selected-count="selected.size" :selected-label="t('workflows.itemLabel')" :total="filtered.length" :item-label="t('workflows.itemLabel')">
      <template #bulk-actions>
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-play" :label="t('workflows.bulkEnable')" :disabled="busy" @click="emit('toggleMany', [...selected], true)" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-stop" :label="t('workflows.bulkDisable')" :disabled="busy" @click="emit('toggleMany', [...selected], false)" />
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('workflows.bulkDelete')" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.id))" />
      </template>
      <UTable :data="pageItems" :columns="columns" sticky :ui="{ base: 'min-w-[1080px]' }">
        <template #select-header><UCheckbox :model-value="pageSelected" :aria-label="t('workflows.selectPage')" @update:model-value="togglePageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggleSelection(row.original.id)" /></template>
        <template #workflow-cell="{ row }"><div class="truncate font-semibold">{{ row.original.name }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]">{{ row.original.description || t('workflows.revision', { revision: row.original.revision }) }}</div></template>
        <template #software-cell="{ row }"><div class="truncate" :title="softwareNames(row.original)">{{ softwareNames(row.original) }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.targetCount', { count: row.original.softwareIds.length }) }}</div></template>
        <template #adapters-cell="{ row }"><div class="truncate" :title="adapterNames(row.original)">{{ adapterNames(row.original) || t('workflows.notConfigured') }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.parallel') }}</div></template>
        <template #assets-cell="{ row }"><div class="truncate" :title="dictionaryNames(row.original)">{{ t('workflows.dictionaries', { names: dictionaryNames(row.original) || t('workflows.none') }) }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]" :title="fontNames(row.original)">{{ t('workflows.fonts', { names: fontNames(row.original) || t('workflows.none') }) }}</div></template>
        <template #status-cell="{ row }"><UBadge :color="actualColor(row.original)" variant="soft" size="sm" :label="actualLabel(row.original)" /></template>
        <template #enabled-cell="{ row }"><USwitch :model-value="activationIds.has(row.original.id)" :disabled="busy" :aria-label="t('workflows.enableNamed', { name: row.original.name })" @update:model-value="emit('toggle', row.original.id, Boolean($event))" /></template>
        <template #actions-cell="{ row }"><div class="flex justify-center gap-0.5"><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('common.editNamed', { name: row.original.name })" @click="emit('open', row.original.id)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-copy" :aria-label="t('workflows.copyNamed', { name: row.original.name })" @click="emit('copy', row.original.id)" /><UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original]" /></div></template>
        <template #empty><UEmpty icon="i-tabler-git-branch" :title="items.length ? t('workflows.noMatch') : t('workflows.empty')" :description="t('workflows.emptyDescription')" /></template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal :open="formOpen" :title="editingWorkflow ? t('workflows.edit') : t('workflows.create')" :description="t('workflows.formDescription')" :confirm-label="editingWorkflow ? t('workflows.save') : t('workflows.createConfirm')" :confirm-disabled="busy || !formValid" :busy="busy" width="xl" @update:open="$event || closeForm()" @confirm="submitForm">
      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-3"><UFormField :label="t('workflows.name')" required><UInput v-model="name" :maxlength="128" class="w-full" /></UFormField><UFormField :label="t('workflows.workflowDescription')"><UInput v-model="description" :maxlength="512" class="w-full" /></UFormField></div>
        <div class="grid h-[430px] grid-cols-[220px_1fr] overflow-hidden rounded-[7px] border border-[var(--border)]">
          <aside class="flex min-h-0 flex-col border-r border-[var(--border)] bg-[var(--surface-subtle)]">
            <div class="border-b border-[var(--border)] p-2"><UInput v-model="softwareQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchSoftware')" :aria-label="t('workflows.searchSoftware')" /></div>
            <div class="min-h-0 flex-1 overflow-auto p-1">
              <UButton v-for="item in visibleSoftware" :key="item.id" color="neutral" variant="ghost" class="flex w-full justify-start gap-2 rounded-[4px] px-2 py-2 text-left" :class="activeSoftwareId === item.id ? 'bg-[var(--surface-hover)]' : ''" :icon="targets.some(target => target.softwareId === item.id) ? 'i-tabler-square-check' : 'i-tabler-square'" :aria-label="targets.some(target => target.softwareId === item.id) ? t('workflows.switchToSoftware', { name: item.name }) : t('workflows.addSoftwareNamed', { name: item.name })" :aria-pressed="activeSoftwareId === item.id" @click="targets.some(target => target.softwareId === item.id) ? (activeSoftwareId = item.id) : toggleSoftware(item.id)">
                <span class="min-w-0"><strong class="block truncate text-[10px]">{{ item.name }}</strong><span class="block truncate text-[9px] text-[var(--text-muted)]">{{ item.executableName }}</span></span>
              </UButton>
              <UEmpty v-if="!software.length" :title="t('workflows.addSoftwareFirst')" :description="t('workflows.addSoftwareFirstDescription')" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" :label="t('workflows.goAddSoftware')" @click="closeForm(); emit('navigate', 'software')" /></template></UEmpty>
            </div>
          </aside>

          <div v-if="activeTarget" class="min-h-0 overflow-auto p-3 [scrollbar-gutter:stable] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-[var(--accent)]" :aria-label="t('workflows.targetConfigLabel')" tabindex="0">
            <div class="mb-3 flex items-center justify-between"><div><h3 class="m-0 text-[12px] font-semibold">{{ activeSoftware?.name }}</h3><p class="m-0 mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.targetDescription') }}</p></div><UButton color="error" variant="ghost" size="xs" icon="i-tabler-x" :label="t('workflows.removeTarget')" @click="toggleSoftware(activeTarget.softwareId)" /></div>

            <section class="mb-3"><h4 class="m-0 mb-2 text-[10px] font-semibold">{{ t('workflows.adapters') }} <span class="font-normal text-[var(--text-muted)]">{{ t('workflows.adaptersHint') }}</span></h4>
              <div v-for="[group, options] in adapterGroups" :key="group" class="mb-2"><div class="mb-1 text-[9px] text-[var(--text-muted)]">{{ group }}</div><label v-for="adapter in options" :key="adapter.id" class="mb-1 flex cursor-pointer items-start gap-2 rounded-[5px] border border-[var(--border)] p-2 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.adapterPlan.adapterIds.includes(adapter.id)" class="mt-0.5" @update:model-value="toggleAdapter(adapter.id)" /><span class="min-w-0"><strong class="block text-[10px]">{{ adapter.name }}</strong><span class="block text-[9px] leading-4 text-[var(--text-muted)]">{{ adapter.summary }}</span></span></label></div>
              <UAlert v-if="!adapters.length" color="warning" variant="soft" :title="t('workflows.noAdapters')" :description="t('workflows.noAdaptersDescription')" />
            </section>

            <div class="grid grid-cols-2 gap-3">
              <section><div class="mb-2 flex items-center justify-between"><h4 class="m-0 text-[10px] font-semibold">{{ t('workflows.orderedDictionaries', { count: activeTarget.dictionaryIds.length }) }}</h4><UButton v-if="!dictionaries.length" color="neutral" variant="ghost" size="xs" :label="t('workflows.goCreate')" @click="closeForm(); emit('navigate', 'dictionaries')" /></div><UInput v-model="dictionaryQuery" icon="i-tabler-search" size="sm" class="mb-1 w-full" :placeholder="t('workflows.searchDictionaries')" :aria-label="t('workflows.searchDictionaries')" /><div class="max-h-28 overflow-auto rounded-[5px] border border-[var(--border)] p-1"><label v-for="item in visibleDictionaries" :key="item.metadata.id" class="flex min-h-8 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.dictionaryIds.includes(item.metadata.id)" @update:model-value="toggleDictionary(item.metadata.id)" /><span class="min-w-0"><strong class="block truncate text-[10px]">{{ item.metadata.name }}</strong><span class="block text-[9px] text-[var(--text-muted)]">{{ item.metadata.sourceLocale }} → {{ item.metadata.targetLocale }}</span></span></label></div><div v-if="activeTarget.dictionaryIds.length" class="mt-2"><div class="mb-1 text-[9px] text-[var(--text-muted)]">{{ t('workflows.dictionaryPriorityHint') }}</div><ol class="space-y-1"><li v-for="(id, index) in activeTarget.dictionaryIds" :key="id" class="flex min-h-7 items-center gap-1 rounded-[4px] bg-[var(--surface-subtle)] px-2"><span class="w-4 text-[9px] tabular-nums text-[var(--text-muted)]">{{ index + 1 }}</span><span class="min-w-0 flex-1 truncate text-[9px]">{{ dictionaryName(id) }}</span><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="index === 0" :aria-label="t('workflows.raiseDictionary', { name: dictionaryName(id) })" @click="moveDictionary(id, -1)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="index === activeTarget.dictionaryIds.length - 1" :aria-label="t('workflows.lowerDictionary', { name: dictionaryName(id) })" @click="moveDictionary(id, 1)" /></li></ol></div></section>
              <section>
                <div class="mb-2 flex items-start justify-between gap-3">
                  <div><h4 class="m-0 text-[10px] font-semibold">{{ t('workflows.fontPolicy') }}</h4><p class="m-0 mt-0.5 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('workflows.fontPolicyHint') }}</p></div>
                  <USwitch :model-value="Boolean(activeTarget.fontPolicy)" :aria-label="t('workflows.fontPolicyToggle')" @update:model-value="setFontPolicyEnabled(Boolean($event))" />
                </div>
                <div v-if="activeTarget.fontPolicy" class="space-y-2">
                  <div class="grid gap-1" role="group" :aria-label="t('workflows.fontCoverageLabel')">
                    <UButton size="xs" :variant="activeTarget.fontPolicy.coverage === 'dictionary_matches' ? 'solid' : 'outline'" :label="t('workflows.fontDictionaryMatches')" :aria-pressed="activeTarget.fontPolicy.coverage === 'dictionary_matches'" @click="setFontCoverage('dictionary_matches')" />
                    <UButton size="xs" :variant="activeTarget.fontPolicy.coverage === 'all_observations' ? 'solid' : 'outline'" :label="t('workflows.fontAllObservations')" :aria-pressed="activeTarget.fontPolicy.coverage === 'all_observations'" @click="setFontCoverage('all_observations')" />
                  </div>
                  <p class="m-0 text-[9px] leading-4 text-[var(--text-muted)]">{{ activeTarget.fontPolicy.coverage === 'dictionary_matches' ? t('workflows.fontDictionaryMatchesHint') : t('workflows.fontAllObservationsHint') }}</p>
                  <UInput v-model="fontQuery" icon="i-tabler-search" size="sm" class="w-full" :placeholder="t('workflows.searchFonts')" :aria-label="t('workflows.searchFonts')" />
                  <div class="max-h-24 overflow-auto rounded-[5px] border border-[var(--border)] p-1">
                    <label v-for="family in visibleFontFamilies" :key="family" class="flex min-h-7 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.fontPolicy.families.includes(family)" @update:model-value="toggleFontFamily(family)" /><span class="min-w-0 truncate text-[9px]">{{ family }}</span></label>
                    <div v-if="!installedFamilies.length" class="px-2 py-3 text-center text-[9px] text-[var(--text-muted)]">{{ t('workflows.noInstalledFonts') }}</div>
                  </div>
                  <ol v-if="activeTarget.fontPolicy.families.length" class="space-y-1" :aria-label="t('workflows.fontPriority')">
                    <li v-for="(family, index) in activeTarget.fontPolicy.families" :key="family" class="flex min-h-7 items-center gap-1 rounded-[4px] bg-[var(--surface-subtle)] px-2"><span class="w-4 text-[9px] tabular-nums text-[var(--text-muted)]">{{ index + 1 }}</span><span class="min-w-0 flex-1 truncate text-[9px]">{{ family }}</span><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="index === 0" :aria-label="t('workflows.raiseFont', { name: family })" @click="moveFontFamily(family, -1)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="index === activeTarget.fontPolicy.families.length - 1" :aria-label="t('workflows.lowerFont', { name: family })" @click="moveFontFamily(family, 1)" /></li>
                  </ol>
                </div>
              </section>
            </div>
            <UAlert v-if="formProblems.length" color="warning" variant="soft" :title="t('workflows.cannotSave')" :description="formProblemDescription" class="mt-3" />
          </div>
          <UEmpty v-else icon="i-tabler-app-window" :title="t('workflows.chooseTarget')" :description="t('workflows.chooseTargetDescription')" />
        </div>
      </div>
    </ManagementFormModal>

    <ConfirmDialog :open="Boolean(pendingRemoval.length)" :title="t('workflows.deleteTitle')" :description="removalDescription" :busy="busy" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
  </section>
</template>
