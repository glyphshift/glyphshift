<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { SoftwareRecord } from '../model'

const props = defineProps<{ items: SoftwareRecord[]; busy: boolean; messages: Record<string, string> }>()
const emit = defineEmits<{
  add: [name: string, description: string, path: string]
  updateSoftware: [id: string, name: string, description: string, path: string]
  remove: [ids: string[]]
}>()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const columns = ref({ description: true, binding: true })
const editing = ref<SoftwareRecord | null>(null)
const editName = ref('')
const editDescription = ref('')
const editPath = ref('')
const pendingRemoval = ref<string[]>([])
const adding = ref(false)
const addName = ref('')
const addDescription = ref('')
const addPath = ref('')
const addSubmitting = ref(false)
const addStartCount = ref(0)

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter(item => !needle
    || `${item.name} ${item.description} ${item.vendor} ${item.version} ${item.executablePath ?? item.executableName}`
      .toLocaleLowerCase()
      .includes(needle))
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.id)))
const columnOptions = computed(() => [
  { key: 'description', label: '描述', visible: columns.value.description },
  { key: 'binding', label: '程序位置', visible: columns.value.binding },
])
const tableColumns = computed<TableColumn<SoftwareRecord>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'software', header: '软件', meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  ...(columns.value.description ? [{ id: 'description', header: '描述', meta: { class: { th: 'w-[28%]', td: 'w-[28%]' } } } satisfies TableColumn<SoftwareRecord>] : []),
  ...(columns.value.binding ? [{ id: 'binding', header: '程序位置' } satisfies TableColumn<SoftwareRecord>] : []),
  { id: 'actions', header: '操作', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
])

watch([query, pageSize], () => { page.value = 1 })
watch(() => filtered.value.length, () => {
  if (page.value > pageCount.value) page.value = pageCount.value
})
watch(() => props.items.length, (count) => {
  if (adding.value && addSubmitting.value && count > addStartCount.value) {
    adding.value = false
    addSubmitting.value = false
  }
})
watch(() => props.messages.software, (message) => {
  if (message) addSubmitting.value = false
})

function toggle(id: string) {
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
  if (key === 'description' || key === 'binding') columns.value[key] = visible
}

function startEdit(item: SoftwareRecord) {
  editing.value = item
  editName.value = item.name
  editDescription.value = item.description
  editPath.value = item.executablePath ?? ''
}

function closeEdit() {
  editing.value = null
}

function saveEdit() {
  if (!editing.value || !editName.value.trim() || !editPath.value.trim()) return
  emit('updateSoftware', editing.value.id, editName.value.trim(), editDescription.value.trim(), editPath.value.trim())
  editing.value = null
}

function closeRemoval() {
  pendingRemoval.value = []
}

function confirmRemoval() {
  emit('remove', pendingRemoval.value)
  selected.value = new Set([...selected.value].filter(id => !pendingRemoval.value.includes(id)))
  pendingRemoval.value = []
}

function startAdd() {
  addName.value = ''
  addDescription.value = ''
  addPath.value = ''
  addSubmitting.value = false
  addStartCount.value = props.items.length
  adding.value = true
}

async function browseExecutable() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: '选择 Windows 应用程序',
      filters: [{ name: 'Windows 应用程序', extensions: ['exe'] }],
    })
    if (typeof selected !== 'string') return
    addPath.value = selected
    if (!addName.value.trim()) addName.value = (selected.split(/[\\/]/).pop() ?? '').replace(/\.exe$/i, '')
  }
  catch {
    // 浏览器预览仍允许手动输入合成路径；桌面壳会提供真实文件选择器。
  }
}

function submitAdd() {
  if (!addName.value.trim() || !addPath.value.trim()) return
  addSubmitting.value = true
  emit('add', addName.value.trim(), addDescription.value.trim(), addPath.value.trim())
}

</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="software-title">
    <ManagementPageHeader
      title-id="software-title"
      title="软件"
      description="管理软件名称、用途说明和程序位置。"
      icon="i-tabler-apps"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新增软件" :disabled="busy" @click="startAdd" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="messages.software" color="error" variant="soft" :description="messages.software" class="mb-3" />

    <ManagementTableFrame
      v-model:query="query"
      v-model:page="page"
      v-model:page-size="pageSize"
      search-placeholder="搜索软件、描述、开发者或程序路径"
      search-label="搜索软件"
      :column-options="columnOptions"
      columns-label="显示列"
      :selected-count="selected.size"
      selected-label="个软件"
      :total="filtered.length"
      item-label="个软件"
      @toggle-column="toggleColumn"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除" :disabled="busy" @click="pendingRemoval = [...selected]" />
      </template>

      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[760px]' }">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" aria-label="选择本页软件" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.id)" :aria-label="`选择 ${row.original.name}`" @update:model-value="toggle(row.original.id)" />
        </template>
        <template #software-cell="{ row }">
          <div class="truncate font-semibold" :title="row.original.name">{{ row.original.name }}</div>
        </template>
        <template #description-cell="{ row }">
          <div class="truncate text-[var(--text-muted)]" :title="row.original.description || undefined">
            {{ row.original.description || '未填写描述' }}
          </div>
        </template>
        <template #binding-cell="{ row }">
          <div class="truncate" :title="row.original.executablePath ?? row.original.executableName">
            {{ row.original.executablePath ?? row.original.executableName }}
          </div>
          <div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]">{{ row.original.vendor }} · {{ row.original.version }}</div>
        </template>
        <template #actions-cell="{ row }">
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="`编辑 ${row.original.name}`" @click="startEdit(row.original)" />
        </template>
        <template #empty>
          <UEmpty
            icon="i-tabler-apps"
            :title="items.length ? '没有匹配的软件' : '还没有添加软件'"
            :description="items.length ? '调整搜索条件后再试。' : '填写名称并选择程序文件，之后可在工作流中使用。'"
          >
            <template v-if="!items.length" #actions>
              <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新增软件" @click="startAdd" />
            </template>
          </UEmpty>
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      v-model:open="adding"
      title="新增软件"
      description="填写软件名称、用途说明和本机程序位置。"
      confirm-label="添加软件"
      :confirm-disabled="busy || !addName.trim() || !addPath.trim()"
      :busy="addSubmitting"
      @confirm="submitAdd"
    >
      <div class="space-y-3">
        <UFormField label="软件名称" required>
          <UInput v-model="addName" autofocus size="sm" class="w-full" aria-label="软件名称" />
        </UFormField>
        <UFormField label="软件描述">
          <UTextarea v-model="addDescription" :maxlength="512" :rows="2" autoresize class="w-full" placeholder="例如：用于合成项目的界面汉化" aria-label="软件描述" />
        </UFormField>
        <UFormField label="程序路径" required>
          <div class="flex gap-2">
            <UInput v-model="addPath" size="sm" class="min-w-0 flex-1" placeholder="选择 .exe 文件或输入完整路径" aria-label="程序路径" />
            <UButton color="neutral" variant="outline" size="sm" label="浏览…" @click="browseExecutable" />
          </div>
        </UFormField>
      </div>
    </ManagementFormModal>

    <ManagementFormModal
      :open="Boolean(editing)"
      title="编辑软件"
      description="修改显示信息或重新绑定 Windows 程序。"
      confirm-label="保存修改"
      :confirm-disabled="!editName.trim() || !editPath.trim()"
      @update:open="$event || closeEdit()"
      @confirm="saveEdit"
    >
      <div class="space-y-3">
        <UFormField label="显示名称" required>
          <UInput v-model="editName" size="sm" class="w-full" aria-label="显示名称" />
        </UFormField>
        <UFormField label="软件描述">
          <UTextarea v-model="editDescription" :maxlength="512" :rows="2" autoresize class="w-full" aria-label="软件描述" />
        </UFormField>
        <UFormField label="程序路径" required>
          <UInput v-model="editPath" size="sm" class="w-full" aria-label="程序路径" />
        </UFormField>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      title="删除软件"
      :description="`确认从 Glyphshift 删除 ${pendingRemoval.length} 个软件？原始程序文件不会被删除；被工作流引用的软件会保留并提示原因。`"
      :busy="busy"
      @update:open="$event || closeRemoval()"
      @confirm="confirmRemoval"
    />
  </section>
</template>
