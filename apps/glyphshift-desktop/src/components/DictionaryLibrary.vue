<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { DictionarySummary, HookTypeOption } from '../model'

const allHooksValue = '__glyphshift_all_hooks__'

const props = defineProps<{
  items: DictionarySummary[]
  hookTypes: HookTypeOption[]
  busy: boolean
  messages: Record<string, string>
}>()
const emit = defineEmits<{
  open: [id: string]
  create: [name: string, description: string, locale: string, hookTypeId: string | null]
  remove: [ids: string[]]
}>()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const localeFilter = ref('all')
const columns = ref({ description: true, hook: true, locale: true, rules: true, revision: true })
const creating = ref(false)
const createName = ref('')
const createDescription = ref('')
const createLocale = ref('zh-CN')
const createHookTypeId = ref(allHooksValue)
const createSubmitting = ref(false)
const createStartCount = ref(0)
const pendingRemoval = ref<DictionarySummary[]>([])

const locales = computed(() => [...new Set(props.items.map(item => item.locale))].sort())
const localeFilterOptions = computed(() => [
  { value: 'all', label: '全部语言' },
  ...locales.value.map(locale => ({ value: locale, label: locale })),
])
const hookTypeOptions = computed(() => [
  { value: allHooksValue, label: '全部 Hook' },
  ...props.hookTypes.map(option => ({ value: option.id, label: option.label })),
])
const filterLabel = computed(() => localeFilter.value === 'all' ? '全部语言' : localeFilter.value)
const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter(item => (localeFilter.value === 'all' || item.locale === localeFilter.value)
    && (!needle || `${item.name} ${item.description} ${item.locale} ${hookTypeLabel(item.hookTypeId)}`.toLocaleLowerCase().includes(needle)))
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.id)))
const columnOptions = computed(() => [
  { key: 'description', label: '描述', visible: columns.value.description },
  { key: 'hook', label: '适用 Hook', visible: columns.value.hook },
  { key: 'locale', label: '语言', visible: columns.value.locale },
  { key: 'rules', label: '替换规则', visible: columns.value.rules },
  { key: 'revision', label: '修订', visible: columns.value.revision },
])
const tableColumns = computed<TableColumn<DictionarySummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'dictionary', header: '词典', meta: { class: { th: 'w-[24%]', td: 'w-[24%]' } } },
  ...(columns.value.description ? [{ id: 'description', header: '描述', meta: { class: { th: 'w-[34%]', td: 'w-[34%]' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(columns.value.hook ? [{ id: 'hook', header: '适用 Hook', meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(columns.value.locale ? [{ id: 'locale', header: '语言', meta: { class: { th: 'w-32', td: 'w-32' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(columns.value.rules ? [{ id: 'rules', header: '替换规则', meta: { class: { th: 'w-28 text-center', td: 'w-28 text-center' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(columns.value.revision ? [{ id: 'revision', header: '修订', meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } } satisfies TableColumn<DictionarySummary>] : []),
  { id: 'actions', header: '操作', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
])

watch([query, localeFilter, pageSize], () => { page.value = 1 })
watch(() => filtered.value.length, () => {
  if (page.value > pageCount.value) page.value = pageCount.value
})
watch(() => props.items.length, (count) => {
  if (creating.value && createSubmitting.value && count > createStartCount.value) {
    creating.value = false
    createSubmitting.value = false
    createName.value = ''
    createDescription.value = ''
    createLocale.value = 'zh-CN'
    createHookTypeId.value = allHooksValue
  }
})
watch(() => props.messages.dictionaries, (message) => {
  if (message) createSubmitting.value = false
})

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
  if (key === 'description' || key === 'hook' || key === 'locale' || key === 'rules' || key === 'revision') columns.value[key] = visible
}

function submitCreate() {
  if (!createName.value.trim() || !createLocale.value.trim()) return
  createStartCount.value = props.items.length
  createSubmitting.value = true
  emit(
    'create',
    createName.value.trim(),
    createDescription.value.trim(),
    createLocale.value.trim(),
    createHookTypeId.value === allHooksValue ? null : createHookTypeId.value,
  )
}

function hookTypeLabel(hookTypeId: string | null) {
  if (!hookTypeId) return '全部 Hook'
  return props.hookTypes.find(option => option.id === hookTypeId)?.label ?? '未识别 Hook'
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
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-library-title">
    <ManagementPageHeader
      title-id="dictionary-library-title"
      title="词典"
      description="独立管理可复用的文字与字体规则；适用 Hook 在整份词典上设置一次。"
      icon="i-tabler-book-2"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新建词典" :disabled="busy" @click="creating = true" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="messages.dictionaries" color="error" variant="soft" :description="messages.dictionaries" class="mb-3" />

    <ManagementTableFrame
      v-model:query="query"
      v-model:filter-value="localeFilter"
      v-model:page="page"
      v-model:page-size="pageSize"
      search-placeholder="搜索词典名称、语言或 Hook"
      search-label="搜索词典"
      :filter-label="filterLabel"
      filter-aria-label="筛选词典"
      :filter-options="localeFilterOptions"
      :column-options="columnOptions"
      columns-label="显示列"
      :selected-count="selected.size"
      selected-label="份词典"
      :total="filtered.length"
      item-label="份词典"
      @toggle-column="toggleColumn"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除" :disabled="busy" @click="startBulkRemoval" />
      </template>

      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[820px]' }">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" aria-label="选择本页词典" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.id)" :aria-label="`选择 ${row.original.name}`" @update:model-value="toggleSelection(row.original.id)" />
        </template>
        <template #dictionary-cell="{ row }">
          <div class="truncate font-semibold" :title="row.original.name">{{ row.original.name }}</div>
        </template>
        <template #description-cell="{ row }">
          <div class="truncate text-[var(--text-muted)]" :title="row.original.description || undefined">
            {{ row.original.description || '未填写描述' }}
          </div>
        </template>
        <template #locale-cell="{ row }">{{ row.original.locale }}</template>
        <template #hook-cell="{ row }">{{ hookTypeLabel(row.original.hookTypeId) }}</template>
        <template #rules-cell="{ row }"><span class="tabular-nums">{{ row.original.entryCount }}</span></template>
        <template #revision-cell="{ row }"><span class="tabular-nums">{{ row.original.revision }}</span></template>
        <template #actions-cell="{ row }">
          <UButton color="primary" variant="ghost" size="xs" label="编辑" :aria-label="`编辑 ${row.original.name}`" @click="emit('open', row.original.id)" />
        </template>
        <template #empty>
          <UEmpty
            icon="i-tabler-book-2"
            :title="items.length ? '没有匹配的词典' : '还没有词典'"
            :description="items.length ? '调整搜索或语言筛选后再试。' : '创建词典后，可由多个工作流按顺序复用。'"
          >
            <template v-if="!items.length" #actions>
              <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新建词典" @click="creating = true" />
            </template>
          </UEmpty>
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      v-model:open="creating"
      title="新建词典"
      description="先说明适用场景，再进入编辑器添加文字和字体规则。"
      confirm-label="创建词典"
      :confirm-disabled="busy || !createName.trim() || !createLocale.trim()"
      :busy="createSubmitting"
      @confirm="submitCreate"
    >
      <div class="space-y-3">
        <UFormField label="词典名称" required>
          <UInput v-model="createName" autofocus size="sm" class="w-full" aria-label="词典名称" />
        </UFormField>
        <UFormField label="词典描述">
          <UTextarea v-model="createDescription" :maxlength="512" :rows="2" autoresize class="w-full" placeholder="例如：用于合成软件菜单汉化" aria-label="词典描述" />
        </UFormField>
        <UFormField label="语言" required>
          <UInput v-model="createLocale" size="sm" class="w-full" aria-label="词典语言" />
        </UFormField>
        <UFormField label="适用 Hook" description="整份词典的文字和字体规则都会使用这里选择的 Hook。">
          <USelect
            v-model="createHookTypeId"
            :items="hookTypeOptions"
            value-key="value"
            label-key="label"
            size="sm"
            class="w-full"
            :content="{ side: 'top', align: 'start', sideOffset: 4, position: 'popper' }"
            :ui="{ content: 'relative z-[100]' }"
            aria-label="适用 Hook"
          />
        </UFormField>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      title="删除词典"
      :description="`确认删除 ${pendingRemoval.length} 份词典？被工作流引用的词典会保留并提示原因。`"
      :busy="busy"
      @update:open="$event || closeRemoval()"
      @confirm="confirmRemoval"
    />
  </section>
</template>
