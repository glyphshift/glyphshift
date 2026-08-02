<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { DictionarySummary, SoftwareRecord, WorkflowDetail, WorkflowRuntimeStatus, WorkflowSummary } from '../model'

const props = defineProps<{
  items: WorkflowSummary[]
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  activationIds: Set<string>
  runtimeStatus: Record<string, WorkflowRuntimeStatus>
  busy: boolean
  refreshing: boolean
  messages: Record<string, string>
  editing: WorkflowDetail | null
}>()
const emit = defineEmits<{
  toggle: [id: string, enabled: boolean]
  toggleMany: [ids: string[], enabled: boolean]
  refresh: []
  open: [id: string]
  create: [name: string, description: string, softwareIds: string[], dictionaryIds: string[]]
  save: [detail: WorkflowDetail]
  closeEdit: []
  copy: [id: string]
  remove: [ids: string[]]
  navigate: [view: 'software' | 'dictionaries']
}>()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const statusFilter = ref('all')
const columns = ref({ description: true, software: true, dictionaries: true, status: true, enabled: true })
const creating = ref(false)
const createName = ref('')
const createDescription = ref('')
const softwareQuery = ref('')
const dictionaryQuery = ref('')
const selectedSoftwareIds = ref(new Set<string>())
const selectedDictionaryIds = ref<string[]>([])
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
    const software = item.softwareIds.map(id => props.software.find(candidate => candidate.id === id)?.name ?? id)
    const dictionaries = item.dictionaryIds.map(id => props.dictionaries.find(candidate => candidate.id === id)?.name ?? id)
    return !needle || `${item.name} ${item.description} ${software.join(' ')} ${dictionaries.join(' ')}`.toLocaleLowerCase().includes(needle)
  })
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.id)))
const statusFilterLabel = computed(() => statusFilterOptions.find(option => option.value === statusFilter.value)?.label ?? '全部状态')
const columnOptions = computed(() => [
  { key: 'description', label: '描述', visible: columns.value.description },
  { key: 'software', label: '软件', visible: columns.value.software },
  { key: 'dictionaries', label: '词典', visible: columns.value.dictionaries },
  { key: 'status', label: '实际状态', visible: columns.value.status },
  { key: 'enabled', label: '启用', visible: columns.value.enabled },
])
const tableColumns = computed<TableColumn<WorkflowSummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'workflow', header: '工作流', meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } },
  ...(columns.value.description ? [{ id: 'description', header: '描述', meta: { class: { th: 'w-[20%]', td: 'w-[20%]' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(columns.value.software ? [{ id: 'software', header: '软件', meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(columns.value.dictionaries ? [{ id: 'dictionaries', header: '词典', meta: { class: { th: 'w-[18%]', td: 'w-[18%]' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(columns.value.status ? [{ id: 'status', header: '实际状态', meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  ...(columns.value.enabled ? [{ id: 'enabled', header: '启用', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } } satisfies TableColumn<WorkflowSummary>] : []),
  { id: 'actions', header: '操作', meta: { class: { th: 'w-28 text-center', td: 'w-28 text-center' } } },
])
const visibleSoftware = computed(() => {
  const needle = softwareQuery.value.trim().toLocaleLowerCase()
  return props.software.filter(item => !needle || `${item.name} ${item.vendor} ${item.executableName}`.toLocaleLowerCase().includes(needle))
})
const visibleDictionaries = computed(() => {
  const needle = dictionaryQuery.value.trim().toLocaleLowerCase()
  return props.dictionaries.filter(item => !needle || `${item.name} ${item.locale}`.toLocaleLowerCase().includes(needle))
})
const selectedDictionaryNames = computed(() => selectedDictionaryIds.value.map(id => props.dictionaries.find(item => item.id === id)?.name ?? id))
const formOpen = computed(() => creating.value || Boolean(props.editing))
const editingWorkflow = computed(() => Boolean(props.editing))
const workflowMessage = computed(() => props.messages.workflows || props.items
  .map(item => props.messages[item.id])
  .find(Boolean) || '')

watch([query, statusFilter, pageSize], () => { page.value = 1 })
watch(() => filtered.value.length, () => {
  if (page.value > pageCount.value) page.value = pageCount.value
})
watch(() => props.editing, (detail) => {
  if (!detail) return
  createName.value = detail.name
  createDescription.value = detail.description
  selectedSoftwareIds.value = new Set(detail.targets.map(target => target.softwareId))
  selectedDictionaryIds.value = [...(detail.targets[0]?.dictionaryIds ?? [])]
  softwareQuery.value = ''
  dictionaryQuery.value = ''
}, { immediate: true })

function softwareNames(item: WorkflowSummary) {
  return item.softwareIds.map(id => props.software.find(candidate => candidate.id === id)?.name ?? id).join('、')
}

function dictionaryNames(item: WorkflowSummary) {
  return item.dictionaryIds.map(id => props.dictionaries.find(candidate => candidate.id === id)?.name ?? id).join('、')
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

function toggleColumn(key: string, visible: boolean) {
  if (key === 'description' || key === 'software' || key === 'dictionaries' || key === 'status' || key === 'enabled') columns.value[key] = visible
}

function toggleSoftware(id: string) {
  const next = new Set(selectedSoftwareIds.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selectedSoftwareIds.value = next
}

function toggleDictionary(id: string) {
  selectedDictionaryIds.value = selectedDictionaryIds.value.includes(id)
    ? selectedDictionaryIds.value.filter(candidate => candidate !== id)
    : [...selectedDictionaryIds.value, id]
}

function resetForm() {
  createName.value = ''
  createDescription.value = ''
  softwareQuery.value = ''
  dictionaryQuery.value = ''
  selectedSoftwareIds.value = new Set()
  selectedDictionaryIds.value = []
}

function startCreate() {
  resetForm()
  creating.value = true
}

function closeForm() {
  if (editingWorkflow.value) emit('closeEdit')
  else creating.value = false
}

function submitForm() {
  if (!createName.value.trim() || !selectedSoftwareIds.value.size || !selectedDictionaryIds.value.length) return
  const targets = [...selectedSoftwareIds.value].map(softwareId => ({ softwareId, dictionaryIds: [...selectedDictionaryIds.value] }))
  if (props.editing) {
    emit('save', {
      ...props.editing,
      name: createName.value.trim(),
      description: createDescription.value.trim(),
      targets,
    })
    return
  }
  emit('create', createName.value.trim(), createDescription.value.trim(), [...selectedSoftwareIds.value], [...selectedDictionaryIds.value])
  creating.value = false
  resetForm()
}

function startBulkRemoval() {
  pendingRemoval.value = props.items.filter(item => selected.value.has(item.id))
}

function closeRemoval() {
  pendingRemoval.value = []
}

function confirmRemoval() {
  const ids = pendingRemoval.value.map(item => item.id)
  emit('remove', ids)
  selected.value = new Set([...selected.value].filter(id => !ids.includes(id)))
  pendingRemoval.value = []
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
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="workflow-title">
    <ManagementPageHeader
      title-id="workflow-title"
      title="工作流"
      description="组合软件与有序词典；启用状态会在软件重启后继续维持。"
      icon="i-tabler-git-branch"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新建工作流" :disabled="busy" @click="startCreate" />
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-tabler-refresh"
          :label="refreshing ? '正在刷新' : '刷新状态'"
          :loading="refreshing"
          :disabled="refreshing"
          @click="emit('refresh')"
        />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="workflowMessage" role="alert" color="error" variant="soft" title="工作流操作失败" :description="workflowMessage" class="mb-3" />

    <ManagementTableFrame
      v-model:query="query"
      v-model:filter-value="statusFilter"
      v-model:page="page"
      v-model:page-size="pageSize"
      search-placeholder="搜索工作流、软件或词典"
      search-label="搜索工作流"
      :filter-label="statusFilterLabel"
      filter-aria-label="筛选工作流"
      :filter-options="statusFilterOptions"
      :column-options="columnOptions"
      columns-label="显示列"
      :selected-count="selected.size"
      selected-label="个工作流"
      :total="filtered.length"
      item-label="个工作流"
      @toggle-column="toggleColumn"
    >
      <template #bulk-actions>
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-play" label="批量启用" :disabled="busy" @click="emit('toggleMany', [...selected], true)" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-player-stop" label="批量停用" :disabled="busy" @click="emit('toggleMany', [...selected], false)" />
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除" :disabled="busy" @click="startBulkRemoval" />
      </template>

      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[980px]' }">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" aria-label="选择本页工作流" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.id)" :aria-label="`选择 ${row.original.name}`" @update:model-value="toggleSelection(row.original.id)" />
        </template>
        <template #workflow-cell="{ row }">
          <div class="truncate font-semibold" :title="row.original.name">{{ row.original.name }}</div>
          <div class="mt-0.5 text-[9px] text-[var(--text-muted)]">修订 {{ row.original.revision }}</div>
        </template>
        <template #description-cell="{ row }">
          <div class="truncate text-[var(--text-muted)]" :title="row.original.description || undefined">
            {{ row.original.description || '未填写描述' }}
          </div>
        </template>
        <template #software-cell="{ row }">
          <div class="truncate" :title="softwareNames(row.original)">{{ softwareNames(row.original) }}</div>
          <div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ row.original.softwareIds.length }} 个目标</div>
        </template>
        <template #dictionaries-cell="{ row }">
          <div class="truncate" :title="dictionaryNames(row.original)">{{ dictionaryNames(row.original) }}</div>
          <div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ row.original.dictionaryIds.length }} 份词典</div>
        </template>
        <template #status-cell="{ row }">
          <UBadge :color="actualColor(row.original)" variant="soft" size="sm" :label="actualLabel(row.original)" />
        </template>
        <template #enabled-cell="{ row }">
          <USwitch
            :model-value="activationIds.has(row.original.id)"
            :disabled="busy"
            :aria-label="`启用 ${row.original.name}`"
            @update:model-value="emit('toggle', row.original.id, Boolean($event))"
          />
        </template>
        <template #actions-cell="{ row }">
          <div class="flex items-center justify-center gap-0.5">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="`编辑 ${row.original.name}`" @click="emit('open', row.original.id)" />
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-copy" :aria-label="`复制 ${row.original.name}`" :disabled="busy" @click="emit('copy', row.original.id)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="`删除 ${row.original.name}`" :disabled="busy" @click="pendingRemoval = [row.original]" />
          </div>
        </template>
        <template #empty>
          <UEmpty
            icon="i-tabler-git-branch"
            :title="items.length ? '没有匹配的工作流' : '还没有工作流'"
            :description="items.length ? '调整搜索或状态筛选后再试。' : '工作流把软件和可复用词典组合成持续运行的期望。'"
          />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="formOpen"
      :title="editingWorkflow ? '编辑工作流' : '新建工作流'"
      :description="editingWorkflow ? '修改名称、描述、软件与词典组合。' : '选择多个软件，并按勾选顺序建立共享的词典优先级。'"
      :confirm-label="editingWorkflow ? '保存工作流' : '创建工作流'"
      :confirm-disabled="busy || !createName.trim() || !selectedSoftwareIds.size || !selectedDictionaryIds.length"
      :busy="busy"
      width="xl"
      @update:open="$event || closeForm()"
      @confirm="submitForm"
    >
      <div class="space-y-3">
        <UFormField label="工作流名称" required>
          <UInput v-model="createName" :maxlength="128" size="sm" class="w-full" aria-label="工作流名称" />
        </UFormField>
        <UFormField label="工作流描述">
          <UTextarea v-model="createDescription" :maxlength="512" :rows="2" autoresize class="w-full" placeholder="说明这套组合适用的场景" aria-label="工作流描述" />
        </UFormField>

        <div class="grid min-h-0 grid-cols-2 gap-3">
          <section class="flex min-h-0 flex-col overflow-hidden rounded-[6px] border border-[var(--border)]" aria-labelledby="workflow-software-title">
            <div class="border-b border-[var(--border)] p-3">
              <h3 id="workflow-software-title" class="m-0 text-[11px] font-semibold">软件 <span class="font-normal text-[var(--text-muted)]">{{ selectedSoftwareIds.size }} 已选</span></h3>
              <UInput v-model="softwareQuery" icon="i-tabler-search" size="sm" class="mt-2 w-full" placeholder="搜索软件" aria-label="搜索并选择软件" />
            </div>
            <div class="min-h-40 flex-1 overflow-auto p-1">
              <div v-for="item in visibleSoftware" :key="item.id" class="flex min-h-9 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]">
                <UCheckbox :model-value="selectedSoftwareIds.has(item.id)" :aria-label="`将 ${item.name} 添加到工作流`" @update:model-value="toggleSoftware(item.id)" />
                <span class="min-w-0"><strong class="block truncate text-[10px] font-medium">{{ item.name }}</strong><span class="block truncate text-[9px] text-[var(--text-muted)]">{{ item.vendor }} · {{ item.version }}</span></span>
              </div>
              <UEmpty v-if="!software.length" title="先添加软件" description="工作流至少需要一个程序目标。" size="sm">
                <template #actions>
                  <UButton color="neutral" variant="outline" size="sm" label="去添加软件" @click="closeForm(); emit('navigate', 'software')" />
                </template>
              </UEmpty>
            </div>
          </section>

          <section class="flex min-h-0 flex-col overflow-hidden rounded-[6px] border border-[var(--border)]" aria-labelledby="workflow-dictionary-title">
            <div class="border-b border-[var(--border)] p-3">
              <h3 id="workflow-dictionary-title" class="m-0 text-[11px] font-semibold">有序词典 <span class="font-normal text-[var(--text-muted)]">{{ selectedDictionaryIds.length }} 已选</span></h3>
              <UInput v-model="dictionaryQuery" icon="i-tabler-search" size="sm" class="mt-2 w-full" placeholder="搜索词典" aria-label="搜索并选择词典" />
            </div>
            <div class="min-h-40 flex-1 overflow-auto p-1">
              <div v-for="item in visibleDictionaries" :key="item.id" class="flex min-h-9 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]">
                <UCheckbox :model-value="selectedDictionaryIds.includes(item.id)" :aria-label="`使用 ${item.name}`" @update:model-value="toggleDictionary(item.id)" />
                <span class="min-w-0"><strong class="block truncate text-[10px] font-medium">{{ item.name }}</strong><span class="block text-[9px] text-[var(--text-muted)]">{{ item.locale }} · {{ item.entryCount }} 条规则</span></span>
              </div>
              <UEmpty v-if="!dictionaries.length" title="先创建词典" description="工作流至少需要一份替换规则集。" size="sm">
                <template #actions>
                  <UButton color="neutral" variant="outline" size="sm" label="去创建词典" @click="closeForm(); emit('navigate', 'dictionaries')" />
                </template>
              </UEmpty>
            </div>
          </section>
        </div>

        <UAlert
          color="neutral"
          variant="soft"
          title="词典优先级"
          :description="selectedDictionaryNames.length ? selectedDictionaryNames.join(' → ') : '尚未选择'"
        />
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      title="删除工作流"
      :description="`确认删除 ${pendingRemoval.length === 1 ? `“${pendingRemoval[0]?.name}”` : `${pendingRemoval.length} 个工作流`}？软件和词典资产不会被删除；已启用的工作流必须先停用。`"
      :busy="busy"
      @update:open="$event || closeRemoval()"
      @confirm="confirmRemoval"
    />
  </section>
</template>
