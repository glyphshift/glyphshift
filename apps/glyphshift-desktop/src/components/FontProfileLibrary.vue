<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { FontProfileDetail, FontProfileSummary, WorkflowSummary } from '../model'

const props = defineProps<{
  items: FontProfileSummary[]
  installedFamilies: string[]
  workflows: WorkflowSummary[]
  editing: FontProfileDetail | null
  busy: boolean
  messages: Record<string, string>
}>()
const emit = defineEmits<{
  open: [id: string]
  closeEdit: []
  create: [name: string, description: string, families: string[]]
  save: [detail: FontProfileDetail]
  remove: [ids: string[]]
}>()

const query = ref('')
const familyQuery = ref('')
const creating = ref(false)
const name = ref('')
const description = ref('')
const families = ref<string[]>([])
const pendingRemoval = ref<FontProfileSummary[]>([])
const selected = ref(new Set<string>())

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter(item => !needle || `${item.metadata.name} ${item.metadata.description} ${item.families.join(' ')}`.toLocaleLowerCase().includes(needle))
})
const visibleFamilies = computed(() => {
  const needle = familyQuery.value.trim().toLocaleLowerCase()
  return props.installedFamilies.filter(family => !needle || family.toLocaleLowerCase().includes(needle))
})
const formOpen = computed(() => creating.value || Boolean(props.editing))
const columns: TableColumn<FontProfileSummary>[] = [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'profile', header: '字体方案', meta: { class: { th: 'w-[30%]', td: 'w-[30%]' } } },
  { id: 'candidates', header: '有序候选' },
  { id: 'resolved', header: '本机命中', meta: { class: { th: 'w-44', td: 'w-44' } } },
  { id: 'references', header: '引用状态', meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'revision', header: '修订', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
  { id: 'actions', header: '操作', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
]

watch(() => props.editing, (detail) => {
  if (!detail) return
  name.value = detail.metadata.name
  description.value = detail.metadata.description
  families.value = [...detail.families]
  familyQuery.value = ''
}, { immediate: true })

function resetForm() {
  name.value = ''
  description.value = ''
  families.value = []
  familyQuery.value = ''
}

function referenceCount(id: string) {
  return props.workflows.filter(workflow => workflow.targets.some(target => target.fontBindings.some(binding => binding.fontProfileId === id))).length
}

function closeForm() {
  if (props.editing) emit('closeEdit')
  else creating.value = false
  resetForm()
}

function toggleFamily(family: string) {
  families.value = families.value.includes(family)
    ? families.value.filter(candidate => candidate !== family)
    : [...families.value, family]
}

function moveFamily(index: number, offset: number) {
  const target = index + offset
  if (target < 0 || target >= families.value.length) return
  const next = [...families.value]
  const [value] = next.splice(index, 1)
  next.splice(target, 0, value!)
  families.value = next
}

function submit() {
  if (!name.value.trim() || !families.value.length) return
  if (props.editing) {
    emit('save', {
      ...props.editing,
      metadata: { ...props.editing.metadata, name: name.value.trim(), description: description.value.trim() },
      families: [...families.value],
    })
  }
  else {
    emit('create', name.value.trim(), description.value.trim(), [...families.value])
    creating.value = false
  }
  resetForm()
}

function confirmRemoval() {
  const ids = pendingRemoval.value.map(item => item.metadata.id)
  emit('remove', ids)
  selected.value = new Set([...selected.value].filter(id => !ids.includes(id)))
  pendingRemoval.value = []
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="font-profile-title">
    <ManagementPageHeader
      title-id="font-profile-title"
      title="字体"
      description="维护可复用的有序字体候选；工作流决定它应用到哪个软件和哪些界面位置。"
      icon="i-tabler-typography"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" label="新建字体方案" :disabled="busy" @click="resetForm(); creating = true" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="messages.fontProfiles" role="alert" color="error" variant="soft" title="字体方案操作失败" :description="messages.fontProfiles" class="mb-3" />

    <ManagementTableFrame
      v-model:query="query"
      :page="1"
      :page-size="Math.max(20, filtered.length)"
      search-placeholder="搜索名称或候选字体"
      search-label="搜索字体方案"
      :selected-count="selected.size"
      selected-label="个字体方案"
      :total="filtered.length"
      item-label="个字体方案"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.metadata.id))" />
      </template>
      <UTable :data="filtered" :columns="columns" sticky :ui="{ base: 'min-w-[820px]' }">
        <template #select-header></template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.metadata.id)" :disabled="referenceCount(row.original.metadata.id) > 0" :aria-label="`选择 ${row.original.metadata.name}`" @update:model-value="selected.has(row.original.metadata.id) ? selected.delete(row.original.metadata.id) : selected.add(row.original.metadata.id); selected = new Set(selected)" />
        </template>
        <template #profile-cell="{ row }">
          <UButton color="neutral" variant="link" class="block max-w-full justify-start p-0 text-left" @click="emit('open', row.original.metadata.id)">
            <span class="block truncate font-semibold">{{ row.original.metadata.name }}</span>
            <span class="mt-0.5 block truncate text-[9px] text-[var(--text-muted)]">{{ row.original.metadata.description || '未填写描述' }}</span>
          </UButton>
        </template>
        <template #candidates-cell="{ row }">
          <div class="truncate" :title="row.original.families.join(' → ')">{{ row.original.families.join(' → ') }}</div>
        </template>
        <template #resolved-cell="{ row }">
          <UBadge :color="row.original.resolvedFamily ? 'neutral' : 'warning'" variant="soft" size="sm" :label="row.original.resolvedFamily || '本机未命中'" />
        </template>
        <template #references-cell="{ row }">
          <UBadge :color="referenceCount(row.original.metadata.id) ? 'warning' : 'neutral'" variant="soft" size="sm" :label="referenceCount(row.original.metadata.id) ? `被 ${referenceCount(row.original.metadata.id)} 个工作流引用` : '未引用'" />
        </template>
        <template #revision-cell="{ row }">{{ row.original.revision }}</template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-0.5">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="`编辑 ${row.original.metadata.name}`" @click="emit('open', row.original.metadata.id)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="`删除 ${row.original.metadata.name}`" :disabled="referenceCount(row.original.metadata.id) > 0" @click="pendingRemoval = [row.original]" />
          </div>
        </template>
        <template #empty>
          <UEmpty icon="i-tabler-typography" :title="items.length ? '没有匹配的字体方案' : '还没有字体方案'" description="按优先顺序添加候选字体，Glyphshift 会选择本机第一个可用项。" />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="formOpen"
      :title="editing ? '编辑字体方案' : '新建字体方案'"
      description="候选顺序从上到下；未命中的候选会自动回退到下一项。"
      :confirm-label="editing ? '保存字体方案' : '创建字体方案'"
      :confirm-disabled="busy || !name.trim() || !families.length"
      :busy="busy"
      width="lg"
      @update:open="$event || closeForm()"
      @confirm="submit"
    >
      <div class="space-y-3">
        <UFormField label="方案名称" required><UInput v-model="name" :maxlength="128" class="w-full" /></UFormField>
        <UFormField label="说明"><UTextarea v-model="description" :maxlength="512" :rows="2" class="w-full" /></UFormField>
        <div class="grid min-h-64 grid-cols-2 gap-3">
          <section class="overflow-hidden rounded-[6px] border border-[var(--border)]">
            <div class="border-b border-[var(--border)] p-2"><UInput v-model="familyQuery" icon="i-tabler-search" size="sm" class="w-full" placeholder="搜索本机字体" aria-label="搜索本机字体" /></div>
            <div class="h-56 overflow-auto p-1">
              <label v-for="family in visibleFamilies" :key="family" class="flex min-h-8 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]">
                <UCheckbox :model-value="families.includes(family)" @update:model-value="toggleFamily(family)" />
                <span class="truncate text-[10px]" :style="{ fontFamily: family }">{{ family }}</span>
              </label>
              <UEmpty v-if="!installedFamilies.length" title="未读取到本机字体" description="桌面端会在字体目录可用后显示候选。" size="sm" />
            </div>
          </section>
          <section class="overflow-hidden rounded-[6px] border border-[var(--border)]">
            <h3 class="m-0 border-b border-[var(--border)] px-3 py-2 text-[10px] font-semibold">候选优先级 · {{ families.length }}</h3>
            <div class="h-56 overflow-auto p-1">
              <div v-for="(family, index) in families" :key="family" class="flex min-h-9 items-center gap-2 rounded-[4px] px-2 hover:bg-[var(--surface-hover)]">
                <span class="w-5 text-center text-[9px] text-[var(--text-muted)]">{{ index + 1 }}</span>
                <span class="min-w-0 flex-1 truncate text-[10px]">{{ family }}</span>
                <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="index === 0" :aria-label="`上移 ${family}`" @click="moveFamily(index, -1)" />
                <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="index === families.length - 1" :aria-label="`下移 ${family}`" @click="moveFamily(index, 1)" />
              </div>
            </div>
          </section>
        </div>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      title="删除字体方案"
      :description="`确认删除 ${pendingRemoval.length === 1 ? `“${pendingRemoval[0]?.metadata.name}”` : `${pendingRemoval.length} 个字体方案`}？被工作流引用的方案不能删除。`"
      :busy="busy"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
  </section>
</template>
