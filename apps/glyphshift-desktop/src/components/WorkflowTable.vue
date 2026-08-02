<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type {
  AdapterOption,
  DictionarySummary,
  FontProfileSummary,
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
  fontProfiles: FontProfileSummary[]
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
  navigate: [view: 'software' | 'dictionaries' | 'fonts']
}>()

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
const fontProfileToAdd = ref('__none__')
const pendingRemoval = ref<WorkflowSummary[]>([])

const statusFilterOptions = [
  { value: 'all', label: '全部状态' },
  { value: 'enabled', label: '已启用' },
  { value: 'disabled', label: '已停用' },
]
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
const activeTarget = computed(() => targets.value.find(target => target.softwareId === activeSoftwareId.value) ?? null)
const activeSoftware = computed(() => props.software.find(item => item.id === activeSoftwareId.value) ?? null)
const formOpen = computed(() => creating.value || Boolean(props.editing))
const editingWorkflow = computed(() => Boolean(props.editing))
const formProblems = computed(() => {
  const problems: string[] = []
  if (!name.value.trim()) problems.push('填写工作流名称')
  if (!targets.value.length) problems.push('至少添加一个软件目标')
  for (const target of targets.value) {
    const label = softwareName(target.softwareId)
    if (!target.adapterPlan.adapterIds.length) problems.push(`${label}：至少选择一种拦截方式`)
    if (!target.dictionaryIds.length && !target.fontBindings.length) problems.push(`${label}：至少选择词典或字体方案`)
    const software = props.software.find(item => item.id === target.softwareId)
    const occupied = new Set<string>()
    for (const binding of target.fontBindings) {
      const locations = binding.scope.kind === 'all'
        ? (software?.locations.map(location => location.id) ?? [])
        : binding.scope.locationIds
      if (!locations.length) problems.push(`${label}：字体绑定需要至少一个位置`)
      for (const location of locations) {
        if (occupied.has(location)) problems.push(`${label}：字体位置 ${location} 被重复绑定`)
        occupied.add(location)
      }
    }
  }
  return [...new Set(problems)]
})
const formValid = computed(() => formProblems.value.length === 0)
const formProblemDescription = computed(() => {
  const visible = formProblems.value.slice(0, 3).join('；')
  const remaining = formProblems.value.length - 3
  return remaining > 0 ? `${visible}；另有 ${remaining} 项` : visible
})
const workflowMessage = computed(() => props.messages.workflows || props.items.map(item => props.messages[item.id]).find(Boolean) || '')
const adapterGroups = computed(() => {
  const groups = new Map<string, AdapterOption[]>()
  for (const adapter of props.adapters) {
    const platform = adapter.platforms.join(' / ') || '跨平台'
    const technology = adapter.technologies.join(' / ') || '其他技术'
    const key = `${platform} · ${technology}`
    groups.set(key, [...(groups.get(key) ?? []), adapter])
  }
  return [...groups.entries()]
})
const columns: TableColumn<WorkflowSummary>[] = [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'workflow', header: '工作流', meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  { id: 'software', header: '软件', meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { id: 'adapters', header: '拦截方式', meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  { id: 'assets', header: '语言 / 字体资产' },
  { id: 'status', header: '实际状态', meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } },
  { id: 'enabled', header: '启用', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
  { id: 'actions', header: '操作', meta: { class: { th: 'w-28 text-center', td: 'w-28 text-center' } } },
]

watch([query, statusFilter, pageSize], () => { page.value = 1 })
watch(activeSoftwareId, () => { fontProfileToAdd.value = '__none__' })
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
  return item.softwareIds.map(softwareName).join('、')
}
function dictionaryName(id: string) {
  return props.dictionaries.find(candidate => candidate.metadata.id === id)?.metadata.name ?? id
}
function dictionaryNames(item: WorkflowSummary) {
  return item.dictionaryIds.map(dictionaryName).join('、')
}
function adapterNames(item: WorkflowSummary) {
  const ids = [...new Set(item.targets.flatMap(target => target.adapterPlan.adapterIds))]
  return ids.map(id => props.adapters.find(adapter => adapter.id === id)?.name ?? id).join('、')
}
function fontNames(item: WorkflowSummary) {
  const ids = [...new Set(item.targets.flatMap(target => target.fontBindings.map(binding => binding.fontProfileId)))]
  return ids.map(id => props.fontProfiles.find(profile => profile.metadata.id === id)?.metadata.name ?? id).join('、')
}
function actualLabel(item: WorkflowSummary) {
  const status = props.runtimeStatus[item.id]
  if (!props.activationIds.has(item.id)) return '已停用'
  if (status && status.targets.length > 0 && status.targets.every(target => target.active)) return '运行中'
  if (status && Object.keys(status.errors).length) return '需要处理'
  return '等待目标'
}
function actualColor(item: WorkflowSummary): 'success' | 'error' | 'neutral' {
  const label = actualLabel(item)
  if (label === '运行中') return 'success'
  if (label === '需要处理') return 'error'
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
  fontProfileToAdd.value = '__none__'
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
    fontBindings: [],
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
function fontProfileName(id: string) {
  return props.fontProfiles.find(profile => profile.metadata.id === id)?.metadata.name ?? id
}
function addFontBinding() {
  if (!activeTarget.value || fontProfileToAdd.value === '__none__') return
  activeTarget.value.fontBindings.push({
    fontProfileId: fontProfileToAdd.value,
    scope: activeTarget.value.fontBindings.length ? { kind: 'locations', locationIds: [] } : { kind: 'all' },
  })
  fontProfileToAdd.value = '__none__'
}
function removeFontBinding(index: number) {
  activeTarget.value?.fontBindings.splice(index, 1)
}
function setFontScope(binding: WorkflowTarget['fontBindings'][number], kind: string) {
  binding.scope = kind === 'locations' ? { kind: 'locations', locationIds: [] } : { kind: 'all' }
}
function toggleFontLocation(binding: WorkflowTarget['fontBindings'][number], id: string) {
  if (binding.scope.kind !== 'locations') return
  binding.scope.locationIds = binding.scope.locationIds.includes(id)
    ? binding.scope.locationIds.filter(candidate => candidate !== id)
    : [...binding.scope.locationIds, id]
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
    <ManagementPageHeader title-id="workflow-title" title="工作流" description="按软件目标组合拦截方式、有序词典与字体方案，并持续维持运行期望。" icon="i-tabler-git-branch">
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新建工作流" :disabled="busy" @click="startCreate" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="refreshing ? '正在刷新' : '刷新状态'" :loading="refreshing" :disabled="refreshing" @click="emit('refresh')" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="workflowMessage" role="alert" color="error" variant="soft" title="工作流操作失败" :description="workflowMessage" class="mb-3" />
    <ManagementTableFrame v-model:query="query" v-model:filter-value="statusFilter" v-model:page="page" v-model:page-size="pageSize" search-placeholder="搜索工作流、软件或拦截方式" search-label="搜索工作流" :filter-label="statusFilterOptions.find(option => option.value === statusFilter)?.label" filter-aria-label="筛选工作流" :filter-options="statusFilterOptions" :selected-count="selected.size" selected-label="个工作流" :total="filtered.length" item-label="个工作流">
      <template #bulk-actions>
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-play" label="批量启用" :disabled="busy" @click="emit('toggleMany', [...selected], true)" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-stop" label="批量停用" :disabled="busy" @click="emit('toggleMany', [...selected], false)" />
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.id))" />
      </template>
      <UTable :data="pageItems" :columns="columns" sticky :ui="{ base: 'min-w-[1080px]' }">
        <template #select-header><UCheckbox :model-value="pageSelected" aria-label="选择本页工作流" @update:model-value="togglePageSelection" /></template>
        <template #select-cell="{ row }"><UCheckbox :model-value="selected.has(row.original.id)" :aria-label="`选择 ${row.original.name}`" @update:model-value="toggleSelection(row.original.id)" /></template>
        <template #workflow-cell="{ row }"><div class="truncate font-semibold">{{ row.original.name }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]">{{ row.original.description || `修订 ${row.original.revision}` }}</div></template>
        <template #software-cell="{ row }"><div class="truncate" :title="softwareNames(row.original)">{{ softwareNames(row.original) }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ row.original.softwareIds.length }} 个目标</div></template>
        <template #adapters-cell="{ row }"><div class="truncate" :title="adapterNames(row.original)">{{ adapterNames(row.original) || '未配置' }}</div><div class="mt-0.5 text-[9px] text-[var(--text-muted)]">并行执行</div></template>
        <template #assets-cell="{ row }"><div class="truncate" :title="dictionaryNames(row.original)">词典：{{ dictionaryNames(row.original) || '无' }}</div><div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]" :title="fontNames(row.original)">字体：{{ fontNames(row.original) || '无' }}</div></template>
        <template #status-cell="{ row }"><UBadge :color="actualColor(row.original)" variant="soft" size="sm" :label="actualLabel(row.original)" /></template>
        <template #enabled-cell="{ row }"><USwitch :model-value="activationIds.has(row.original.id)" :disabled="busy" :aria-label="`启用 ${row.original.name}`" @update:model-value="emit('toggle', row.original.id, Boolean($event))" /></template>
        <template #actions-cell="{ row }"><div class="flex justify-center gap-0.5"><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="`编辑 ${row.original.name}`" @click="emit('open', row.original.id)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-copy" :aria-label="`复制 ${row.original.name}`" @click="emit('copy', row.original.id)" /><UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="`删除 ${row.original.name}`" @click="pendingRemoval = [row.original]" /></div></template>
        <template #empty><UEmpty icon="i-tabler-git-branch" :title="items.length ? '没有匹配的工作流' : '还没有工作流'" description="每个目标可以独立组合 Adapter、词典和字体方案。" /></template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal :open="formOpen" :title="editingWorkflow ? '编辑工作流' : '新建工作流'" description="先选择软件，再逐个配置目标；不同目标不会共享隐式选择。" :confirm-label="editingWorkflow ? '保存工作流' : '创建工作流'" :confirm-disabled="busy || !formValid" :busy="busy" width="xl" @update:open="$event || closeForm()" @confirm="submitForm">
      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-3"><UFormField label="工作流名称" required><UInput v-model="name" :maxlength="128" class="w-full" /></UFormField><UFormField label="工作流描述"><UInput v-model="description" :maxlength="512" class="w-full" /></UFormField></div>
        <div class="grid h-[430px] grid-cols-[220px_1fr] overflow-hidden rounded-[7px] border border-[var(--border)]">
          <aside class="flex min-h-0 flex-col border-r border-[var(--border)] bg-[var(--surface-subtle)]">
            <div class="border-b border-[var(--border)] p-2"><UInput v-model="softwareQuery" icon="i-tabler-search" size="sm" class="w-full" placeholder="搜索软件" aria-label="搜索软件" /></div>
            <div class="min-h-0 flex-1 overflow-auto p-1">
              <UButton v-for="item in visibleSoftware" :key="item.id" color="neutral" variant="ghost" class="flex w-full justify-start gap-2 rounded-[4px] px-2 py-2 text-left" :class="activeSoftwareId === item.id ? 'bg-[var(--surface-hover)]' : ''" :icon="targets.some(target => target.softwareId === item.id) ? 'i-tabler-square-check' : 'i-tabler-square'" :aria-label="`${targets.some(target => target.softwareId === item.id) ? '切换到' : '添加'} ${item.name}`" :aria-pressed="activeSoftwareId === item.id" @click="targets.some(target => target.softwareId === item.id) ? (activeSoftwareId = item.id) : toggleSoftware(item.id)">
                <span class="min-w-0"><strong class="block truncate text-[10px]">{{ item.name }}</strong><span class="block truncate text-[9px] text-[var(--text-muted)]">{{ item.executableName }}</span></span>
              </UButton>
              <UEmpty v-if="!software.length" title="先添加软件" description="工作流至少需要一个软件目标。" size="sm"><template #actions><UButton color="neutral" variant="outline" size="sm" label="去添加软件" @click="closeForm(); emit('navigate', 'software')" /></template></UEmpty>
            </div>
          </aside>

          <div v-if="activeTarget" class="min-h-0 overflow-auto p-3 [scrollbar-gutter:stable] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-[var(--accent)]" aria-label="目标配置，可向下滚动" tabindex="0">
            <div class="mb-3 flex items-center justify-between"><div><h3 class="m-0 text-[12px] font-semibold">{{ activeSoftware?.name }}</h3><p class="m-0 mt-0.5 text-[9px] text-[var(--text-muted)]">此处配置仅属于这个软件目标 · 向下滚动查看全部组合项</p></div><UButton color="error" variant="ghost" size="xs" icon="i-tabler-x" label="移除目标" @click="toggleSoftware(activeTarget.softwareId)" /></div>

            <section class="mb-3"><h4 class="m-0 mb-2 text-[10px] font-semibold">拦截方式 <span class="font-normal text-[var(--text-muted)]">· 可多选，并行执行</span></h4>
              <div v-for="[group, options] in adapterGroups" :key="group" class="mb-2"><div class="mb-1 text-[9px] text-[var(--text-muted)]">{{ group }}</div><label v-for="adapter in options" :key="adapter.id" class="mb-1 flex cursor-pointer items-start gap-2 rounded-[5px] border border-[var(--border)] p-2 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.adapterPlan.adapterIds.includes(adapter.id)" class="mt-0.5" @update:model-value="toggleAdapter(adapter.id)" /><span class="min-w-0"><strong class="block text-[10px]">{{ adapter.name }}</strong><span class="block text-[9px] leading-4 text-[var(--text-muted)]">{{ adapter.summary }}</span></span></label></div>
              <UAlert v-if="!adapters.length" color="warning" variant="soft" title="当前没有可用 Adapter" description="请先安装或重新构建 Runtime Bundle。" />
            </section>

            <div class="grid grid-cols-2 gap-3">
              <section><div class="mb-2 flex items-center justify-between"><h4 class="m-0 text-[10px] font-semibold">有序词典 · {{ activeTarget.dictionaryIds.length }}</h4><UButton v-if="!dictionaries.length" color="neutral" variant="ghost" size="xs" label="去创建" @click="closeForm(); emit('navigate', 'dictionaries')" /></div><UInput v-model="dictionaryQuery" icon="i-tabler-search" size="sm" class="mb-1 w-full" placeholder="搜索词典" aria-label="搜索词典" /><div class="max-h-28 overflow-auto rounded-[5px] border border-[var(--border)] p-1"><label v-for="item in visibleDictionaries" :key="item.metadata.id" class="flex min-h-8 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]"><UCheckbox :model-value="activeTarget.dictionaryIds.includes(item.metadata.id)" @update:model-value="toggleDictionary(item.metadata.id)" /><span class="min-w-0"><strong class="block truncate text-[10px]">{{ item.metadata.name }}</strong><span class="block text-[9px] text-[var(--text-muted)]">{{ item.metadata.sourceLocale }} → {{ item.metadata.targetLocale }}</span></span></label></div><div v-if="activeTarget.dictionaryIds.length" class="mt-2"><div class="mb-1 text-[9px] text-[var(--text-muted)]">优先级从上到下；较早词典覆盖重复规则</div><ol class="space-y-1"><li v-for="(id, index) in activeTarget.dictionaryIds" :key="id" class="flex min-h-7 items-center gap-1 rounded-[4px] bg-[var(--surface-subtle)] px-2"><span class="w-4 text-[9px] tabular-nums text-[var(--text-muted)]">{{ index + 1 }}</span><span class="min-w-0 flex-1 truncate text-[9px]">{{ dictionaryName(id) }}</span><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="index === 0" :aria-label="`提高 ${dictionaryName(id)} 的优先级`" @click="moveDictionary(id, -1)" /><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="index === activeTarget.dictionaryIds.length - 1" :aria-label="`降低 ${dictionaryName(id)} 的优先级`" @click="moveDictionary(id, 1)" /></li></ol></div></section>
              <section><div class="mb-2 flex items-center justify-between"><h4 class="m-0 text-[10px] font-semibold">字体绑定 · {{ activeTarget.fontBindings.length }}</h4><UButton v-if="!fontProfiles.length" color="neutral" variant="ghost" size="xs" label="去创建" @click="closeForm(); emit('navigate', 'fonts')" /></div><div class="flex gap-1"><USelect v-model="fontProfileToAdd" :items="[{ value: '__none__', label: '选择字体方案' }, ...fontProfiles.map(profile => ({ value: profile.metadata.id, label: profile.metadata.name }))]" value-key="value" label-key="label" class="min-w-0 flex-1" aria-label="选择要添加的字体方案" /><UButton color="neutral" variant="outline" size="sm" icon="i-tabler-plus" label="添加" :disabled="fontProfileToAdd === '__none__'" @click="addFontBinding" /></div><div v-for="(binding, bindingIndex) in activeTarget.fontBindings" :key="`${binding.fontProfileId}-${bindingIndex}`" class="mt-2 rounded-[5px] border border-[var(--border)] p-2" role="group" :aria-label="`字体绑定 ${fontProfileName(binding.fontProfileId)}`"><div class="mb-2 flex items-center justify-between gap-2"><strong class="min-w-0 truncate text-[10px]">{{ fontProfileName(binding.fontProfileId) }}</strong><UButton color="error" variant="ghost" size="xs" icon="i-tabler-x" :aria-label="`移除字体绑定 ${fontProfileName(binding.fontProfileId)}`" @click="removeFontBinding(bindingIndex)" /></div><div class="mb-2 flex gap-2"><UButton size="xs" :variant="binding.scope.kind === 'all' ? 'solid' : 'outline'" label="全部位置" :aria-pressed="binding.scope.kind === 'all'" @click="setFontScope(binding, 'all')" /><UButton size="xs" :variant="binding.scope.kind === 'locations' ? 'solid' : 'outline'" label="指定位置" :aria-pressed="binding.scope.kind === 'locations'" @click="setFontScope(binding, 'locations')" /></div><div v-if="binding.scope.kind === 'locations'" class="space-y-1"><label v-for="location in activeSoftware?.locations ?? []" :key="location.id" class="flex items-center gap-2"><UCheckbox :model-value="binding.scope.kind === 'locations' && binding.scope.locationIds.includes(location.id)" @update:model-value="toggleFontLocation(binding, location.id)" /><span class="text-[9px]">{{ location.label }}</span></label></div></div></section>
            </div>
            <UAlert v-if="formProblems.length" color="warning" variant="soft" title="还不能保存" :description="formProblemDescription" class="mt-3" />
          </div>
          <UEmpty v-else icon="i-tabler-app-window" title="选择一个软件目标" description="添加目标后，再为它配置拦截方式、词典和字体。" />
        </div>
      </div>
    </ManagementFormModal>

    <ConfirmDialog :open="Boolean(pendingRemoval.length)" title="删除工作流" :description="`确认删除 ${pendingRemoval.length === 1 ? `“${pendingRemoval[0]?.name}”` : `${pendingRemoval.length} 个工作流`}？引用的软件、词典与字体方案不会被删除。`" :busy="busy" @update:open="$event || (pendingRemoval = [])" @confirm="confirmRemoval" />
  </section>
</template>
