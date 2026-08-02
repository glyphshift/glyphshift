<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { DictionaryDetail, DictionaryRule } from '../model'

const props = defineProps<{ detail: DictionaryDetail; busy: boolean }>()
const emit = defineEmits<{ back: []; save: [detail: DictionaryDetail] }>()

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

const draft = ref<DictionaryDetail>(clone(props.detail))
const query = ref('')
const editingIndex = ref<number | null>(null)
const editorOpen = ref(false)
const rule = ref<DictionaryRule>(emptyRule())
const pendingRemoval = ref<number[]>([])
const selected = ref(new Set<number>())

watch(() => props.detail, value => { draft.value = clone(value) }, { deep: true })

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return draft.value.entries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => !needle || `${entry.location} ${entry.source} ${entry.translation ?? ''} ${entry.context?.kind ?? ''} ${entry.context?.key ?? ''}`.toLocaleLowerCase().includes(needle))
})
const columns: TableColumn<{ entry: DictionaryRule; index: number }>[] = [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'location', header: '位置', meta: { class: { th: 'w-32', td: 'w-32' } } },
  { id: 'source', header: '原文' },
  { id: 'translation', header: '译文' },
  { id: 'context', header: '语境', meta: { class: { th: 'w-44', td: 'w-44' } } },
  { id: 'actions', header: '操作', meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
]

function emptyRule(): DictionaryRule {
  return { location: 'main-ui', context: null, source: '', translation: '' }
}

function startCreate() {
  editingIndex.value = null
  rule.value = emptyRule()
  editorOpen.value = true
}

function startEdit(index: number) {
  editingIndex.value = index
  rule.value = clone(draft.value.entries[index]!)
  editorOpen.value = true
}

function saveRule() {
  const next = clone(rule.value)
  next.location = next.location.trim()
  next.source = next.source.trim()
  next.translation = next.translation?.trim() || null
  if (!next.location || !next.source) return
  if (next.context && (!next.context.kind.trim() || !next.context.key.trim())) next.context = null
  if (editingIndex.value === null) draft.value.entries.push(next)
  else draft.value.entries[editingIndex.value] = next
  editorOpen.value = false
}

function setContextEnabled(enabled: boolean) {
  rule.value.context = enabled ? (rule.value.context ?? { kind: 'control', key: '' }) : null
}

function confirmRemoval() {
  const removed = new Set(pendingRemoval.value)
  draft.value.entries = draft.value.entries.filter((_, index) => !removed.has(index))
  selected.value = new Set()
  pendingRemoval.value = []
}

function saveDraft() {
  emit('save', clone(draft.value))
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-title">
    <div class="mb-3 flex items-start justify-between gap-4">
      <div class="flex min-w-0 items-start gap-2">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-arrow-left" aria-label="返回词典列表" @click="emit('back')" />
        <div class="min-w-0">
          <h1 id="dictionary-title" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">{{ draft.metadata.name }}</h1>
          <p class="m-0 mt-1 text-[10px] text-[var(--text-muted)]">{{ draft.metadata.sourceLocale }} → {{ draft.metadata.targetLocale }} · 发布 v{{ draft.metadata.releaseVersion }} · 本地修订 {{ draft.revision }}</p>
        </div>
      </div>
      <div class="flex shrink-0 gap-2">
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-plus" label="添加规则" @click="startCreate" />
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" label="保存词典" :loading="busy" :disabled="busy || !draft.metadata.name.trim() || !draft.metadata.sourceLocale.trim() || !draft.metadata.targetLocale.trim() || !draft.metadata.releaseVersion.trim()" @click="saveDraft" />
      </div>
    </div>

    <div class="mb-3 grid grid-cols-12 gap-3 rounded-[8px] border border-[var(--border)] bg-[var(--surface)] p-3">
      <UFormField label="名称" class="col-span-3"><UInput v-model="draft.metadata.name" :maxlength="128" class="w-full" /></UFormField>
      <UFormField label="说明" class="col-span-5"><UInput v-model="draft.metadata.description" :maxlength="512" class="w-full" /></UFormField>
      <UFormField label="发布版本" class="col-span-2"><UInput v-model="draft.metadata.releaseVersion" class="w-full" /></UFormField>
      <UFormField label="标签" class="col-span-2" hint="逗号分隔">
        <UInput :model-value="draft.metadata.tags.join(', ')" class="w-full" @update:model-value="draft.metadata.tags = String($event).split(',').map(value => value.trim()).filter(Boolean)" />
      </UFormField>
      <UFormField label="源语言" class="col-span-2"><UInput v-model="draft.metadata.sourceLocale" class="w-full" /></UFormField>
      <UFormField label="目标语言" class="col-span-2"><UInput v-model="draft.metadata.targetLocale" class="w-full" /></UFormField>
      <UFormField label="作者" class="col-span-3" hint="逗号分隔"><UInput :model-value="draft.metadata.authors.join(', ')" class="w-full" @update:model-value="draft.metadata.authors = String($event).split(',').map(value => value.trim()).filter(Boolean)" /></UFormField>
      <UFormField label="许可证" class="col-span-2"><UInput :model-value="draft.metadata.license ?? ''" class="w-full" @update:model-value="draft.metadata.license = String($event).trim() || null" /></UFormField>
      <UFormField label="主页" class="col-span-3"><UInput :model-value="draft.metadata.homepage ?? ''" class="w-full" @update:model-value="draft.metadata.homepage = String($event).trim() || null" /></UFormField>
    </div>

    <ManagementTableFrame
      v-model:query="query"
      :page="1"
      :page-size="Math.max(20, filtered.length)"
      search-placeholder="搜索位置、原文、译文或语境"
      search-label="搜索词典规则"
      :selected-count="selected.size"
      selected-label="条规则"
      :total="filtered.length"
      item-label="条规则"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="删除所选" @click="pendingRemoval = [...selected]" />
      </template>
      <UTable :data="filtered" :columns="columns" sticky :ui="{ base: 'min-w-[820px]' }">
        <template #select-header></template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.index)" :aria-label="`选择规则 ${row.original.entry.source}`" @update:model-value="selected.has(row.original.index) ? selected.delete(row.original.index) : selected.add(row.original.index); selected = new Set(selected)" />
        </template>
        <template #location-cell="{ row }"><code class="text-[10px]">{{ row.original.entry.location }}</code></template>
        <template #source-cell="{ row }"><div class="truncate" :title="row.original.entry.source">{{ row.original.entry.source }}</div></template>
        <template #translation-cell="{ row }"><div class="truncate" :title="row.original.entry.translation ?? undefined">{{ row.original.entry.translation ?? '保持原文' }}</div></template>
        <template #context-cell="{ row }"><span class="text-[var(--text-muted)]">{{ row.original.entry.context ? `${row.original.entry.context.kind}:${row.original.entry.context.key}` : '通用' }}</span></template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-0.5">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="`编辑 ${row.original.entry.source}`" @click="startEdit(row.original.index)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="`删除 ${row.original.entry.source}`" @click="pendingRemoval = [row.original.index]" />
          </div>
        </template>
        <template #empty>
          <UEmpty icon="i-tabler-text-plus" :title="draft.entries.length ? '没有匹配的规则' : '还没有文字规则'" description="词典规则只决定文字保持或替换；字体和拦截方式在工作流中组合。" />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="editorOpen"
      :title="editingIndex === null ? '添加文字规则' : '编辑文字规则'"
      description="位置和可选语境共同标识规则的使用范围。"
      confirm-label="保存规则"
      :confirm-disabled="!rule.location.trim() || !rule.source.trim()"
      width="lg"
      @update:open="$event || (editorOpen = false)"
      @confirm="saveRule"
    >
      <div class="space-y-3">
        <UFormField label="语义位置" required><UInput v-model="rule.location" class="w-full" placeholder="例如 main-ui" /></UFormField>
        <div class="grid grid-cols-2 gap-3">
          <UFormField label="原文" required><UTextarea v-model="rule.source" :rows="3" class="w-full" /></UFormField>
          <UFormField label="译文" hint="留空表示保持原文"><UTextarea v-model="rule.translation" :rows="3" class="w-full" /></UFormField>
        </div>
        <UCheckbox :model-value="Boolean(rule.context)" label="限定语境" @update:model-value="setContextEnabled(Boolean($event))" />
        <div v-if="rule.context" class="grid grid-cols-2 gap-3">
          <UFormField label="语境类型"><UInput v-model="rule.context.kind" class="w-full" /></UFormField>
          <UFormField label="语境键"><UInput v-model="rule.context.key" class="w-full" /></UFormField>
        </div>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      title="删除文字规则"
      :description="`确认删除 ${pendingRemoval.length} 条规则？保存词典后生效。`"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
  </section>
</template>
