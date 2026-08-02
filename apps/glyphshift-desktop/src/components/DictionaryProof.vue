<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { DictionaryDetail, DictionaryRule, HookTypeOption } from '../model'

type RuleRow = { entry: DictionaryRule; index: number }

const unchangedFontValue = '__glyphshift_unchanged_font__'
const props = defineProps<{
  detail: DictionaryDetail
  busy: boolean
  hookTypes: HookTypeOption[]
  fontFamilies: string[]
}>()
const emit = defineEmits<{ back: []; save: [detail: DictionaryDetail] }>()
const copyDetail = (value: DictionaryDetail) => JSON.parse(JSON.stringify(value)) as DictionaryDetail
const draft = ref(copyDetail(props.detail))
const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<number>())
const ruleFilter = ref('all')
const columns = ref({ textBehavior: true, translation: true, fontBehavior: true, fontFamily: true })

const ruleFilterOptions = [
  { value: 'all', label: '全部规则' },
  { value: 'text', label: '替换文字' },
  { value: 'font', label: '替换字体' },
  { value: 'combined', label: '文字 + 字体' },
  { value: 'keep', label: '保持原文' },
]
const textBehaviorItems = [
  { value: 'keep', label: '保持原文' },
  { value: 'replace', label: '替换文字' },
]
const fontBehaviorItems = computed(() => [
  { value: 'inherit', label: '跟随词典默认' },
  { value: 'unchanged', label: '保持宿主字体' },
  { value: 'substitute', label: '替换为指定字体', disabled: !availableFonts.value.length },
])
const hookTypeLabel = computed(() => {
  if (!draft.value.hookTypeId) return '全部 Hook'
  return props.hookTypes.find(option => option.id === draft.value.hookTypeId)?.label ?? '未识别 Hook'
})
const availableFonts = computed(() => {
  const families = new Set(props.fontFamilies)
  if (draft.value.defaultFont.kind === 'substitute') families.add(draft.value.defaultFont.family)
  draft.value.entries.forEach((entry) => {
    if (entry.font.kind === 'substitute') families.add(entry.font.family)
  })
  return [...families].filter(Boolean).sort((left, right) => left.localeCompare(right))
})
const defaultFontItems = computed(() => [
  { value: unchangedFontValue, label: '保持宿主字体' },
  ...availableFonts.value.map(family => ({ value: family, label: family })),
])
const filteredRows = computed<RuleRow[]>(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return draft.value.entries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => {
      const replacesText = entry.translation !== null
      const replacesFont = entry.font.kind === 'substitute'
      const matchesBehavior = ruleFilter.value === 'all'
        || (ruleFilter.value === 'text' && replacesText)
        || (ruleFilter.value === 'font' && replacesFont)
        || (ruleFilter.value === 'combined' && replacesText && replacesFont)
        || (ruleFilter.value === 'keep' && !replacesText)
      return matchesBehavior
        && (!needle || `${entry.source} ${entry.translation ?? ''} ${entry.font.kind === 'substitute' ? entry.font.family : ''}`.toLocaleLowerCase().includes(needle))
    })
})
const pageCount = computed(() => Math.max(1, Math.ceil(filteredRows.value.length / pageSize.value)))
const pageRows = computed(() => filteredRows.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageRows.value.length) && pageRows.value.every(row => selected.value.has(row.index)))
const filterLabel = computed(() => ruleFilterOptions.find(option => option.value === ruleFilter.value)?.label ?? '全部规则')
const columnOptions = computed(() => [
  { key: 'translation', label: '译文', visible: columns.value.translation },
  { key: 'textBehavior', label: '文字处理', visible: columns.value.textBehavior },
  { key: 'fontBehavior', label: '字体处理', visible: columns.value.fontBehavior },
  { key: 'fontFamily', label: '字体', visible: columns.value.fontFamily },
])
const tableColumns = computed<TableColumn<RuleRow>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'source', header: '原文', meta: { class: { th: 'w-44', td: 'w-44 p-1.5' } } },
  ...(columns.value.translation ? [{ id: 'translation', header: '译文', meta: { class: { th: 'w-44', td: 'w-44 p-1.5' } } } satisfies TableColumn<RuleRow>] : []),
  ...(columns.value.textBehavior ? [{ id: 'textBehavior', header: '文字处理', meta: { class: { th: 'w-28', td: 'w-28 p-1.5' } } } satisfies TableColumn<RuleRow>] : []),
  ...(columns.value.fontBehavior ? [{ id: 'fontBehavior', header: '字体处理', meta: { class: { th: 'w-36', td: 'w-36 p-1.5' } } } satisfies TableColumn<RuleRow>] : []),
  ...(columns.value.fontFamily ? [{ id: 'fontFamily', header: '字体', meta: { class: { th: 'w-44', td: 'w-44 p-1.5' } } } satisfies TableColumn<RuleRow>] : []),
  { id: 'actions', header: '', meta: { class: { th: 'w-14', td: 'w-14 text-center' } } },
])

watch(() => props.detail, (value) => {
  draft.value = copyDetail(value)
  selected.value = new Set()
  page.value = 1
})
watch([query, ruleFilter, pageSize], () => { page.value = 1 })
watch(() => filteredRows.value.length, () => {
  if (page.value > pageCount.value) page.value = pageCount.value
})

function addRule() {
  const defaultLocation = draft.value.entries.find(entry => entry.location.trim())?.location ?? 'main-ui'
  draft.value.entries.push({
    location: defaultLocation,
    context: null,
    source: '',
    translation: '',
    font: { kind: 'inherit' },
    adapterIds: draft.value.hookTypeId ? [draft.value.hookTypeId] : [],
  })
  query.value = ''
  ruleFilter.value = 'all'
  page.value = Math.max(1, Math.ceil(draft.value.entries.length / pageSize.value))
}

function setDefaultFont(value: unknown) {
  const family = String(value ?? '')
  draft.value.defaultFont = !family || family === unchangedFontValue ? { kind: 'unchanged' } : { kind: 'substitute', family }
}

function setTextBehavior(entry: DictionaryRule, value: unknown) {
  entry.translation = value === 'replace' ? (entry.translation ?? '') : null
}

function setTranslation(entry: DictionaryRule, value: unknown) {
  if (entry.translation !== null) entry.translation = String(value ?? '')
}

function setFontBehavior(entry: DictionaryRule, value: unknown) {
  const behavior = String(value ?? '')
  if (behavior === 'inherit') entry.font = { kind: 'inherit' }
  else if (behavior === 'unchanged') entry.font = { kind: 'unchanged' }
  else {
    const family = entry.font.kind === 'substitute' ? entry.font.family : availableFonts.value[0]
    if (family) entry.font = { kind: 'substitute', family }
  }
}

function setEntryFont(entry: DictionaryRule, value: unknown) {
  const family = String(value ?? '')
  if (family) entry.font = { kind: 'substitute', family }
}

function toggleColumn(key: string, visible: boolean) {
  if (key === 'textBehavior' || key === 'translation' || key === 'fontBehavior' || key === 'fontFamily') columns.value[key] = visible
}

function toggleSelection(index: number) {
  const next = new Set(selected.value)
  next.has(index) ? next.delete(index) : next.add(index)
  selected.value = next
}

function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) pageRows.value.forEach(row => next.delete(row.index))
  else pageRows.value.forEach(row => next.add(row.index))
  selected.value = next
}

function deleteRule(index: number) {
  draft.value.entries.splice(index, 1)
  selected.value = new Set()
}

function deleteSelectedRules() {
  ;[...selected.value].sort((left, right) => right - left).forEach(index => draft.value.entries.splice(index, 1))
  selected.value = new Set()
}

function save() {
  const adapterIds = draft.value.hookTypeId ? [draft.value.hookTypeId] : []
  const validEntries = draft.value.entries
    .filter(entry => entry.location.trim() && entry.source.trim())
    .map(entry => ({ ...entry, adapterIds }))
  emit('save', { ...draft.value, entries: validEntries })
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-title">
    <header class="mb-4 flex min-h-10 items-center gap-3">
      <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-arrow-left" aria-label="返回词典" @click="emit('back')" />
      <div class="min-w-0">
        <h1 id="dictionary-title" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">{{ draft.name }}</h1>
        <p class="mb-0 mt-1 flex flex-wrap items-center gap-x-2 text-[10px] text-[var(--text-muted)]">
          <span>{{ draft.locale }}</span>
          <span>适用 Hook：{{ hookTypeLabel }}</span>
          <span>修订 {{ draft.revision }} · {{ draft.entries.length }} 条规则</span>
        </p>
      </div>
      <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" label="保存词典" class="ml-auto" :disabled="busy" @click="save" />
    </header>

    <div class="mb-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-[minmax(0,0.75fr)_minmax(0,1.1fr)_240px]">
      <UFormField label="词典名称">
        <UInput v-model="draft.name" :maxlength="128" size="sm" class="w-full" aria-label="词典名称" />
      </UFormField>
      <UFormField label="词典描述">
        <UInput v-model="draft.description" :maxlength="512" size="sm" class="w-full" placeholder="说明这份词典适用的场景" aria-label="词典描述" />
      </UFormField>
      <UFormField label="默认字体" :hint="`${fontFamilies.length} 款本机字体`">
        <USelectMenu
          :model-value="draft.defaultFont.kind === 'substitute' ? draft.defaultFont.family : unchangedFontValue"
          :items="defaultFontItems"
          value-key="value"
          label-key="label"
          :search-input="{ placeholder: '搜索本机字体' }"
          :virtualize="{ estimateSize: 32 }"
          size="sm"
          class="w-full"
          aria-label="默认字体"
          @update:model-value="setDefaultFont"
        />
      </UFormField>
    </div>

    <ManagementTableFrame
      v-model:query="query"
      v-model:filter-value="ruleFilter"
      v-model:page="page"
      v-model:page-size="pageSize"
      search-placeholder="搜索原文、译文或字体"
      search-label="搜索词典规则"
      :filter-label="filterLabel"
      filter-aria-label="筛选词典规则"
      :filter-options="ruleFilterOptions"
      :column-options="columnOptions"
      columns-label="显示规则列"
      :selected-count="selected.size"
      selected-label="条规则"
      :total="filteredRows.length"
      item-label="条规则"
      @toggle-column="toggleColumn"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" label="批量删除规则" :disabled="busy" @click="deleteSelectedRules" />
      </template>

      <div class="flex h-full min-h-0 flex-col">
        <UTable :data="pageRows" :columns="tableColumns" sticky class="min-h-0 flex-1" :ui="{ base: 'min-w-[900px]' }">
          <template #select-header>
            <UCheckbox :model-value="pageSelected" aria-label="选择本页规则" @update:model-value="togglePageSelection" />
          </template>
          <template #select-cell="{ row }">
            <UCheckbox :model-value="selected.has(row.original.index)" :aria-label="`选择 ${row.original.entry.source || '新规则'}`" @update:model-value="toggleSelection(row.original.index)" />
          </template>
          <template #source-cell="{ row }">
            <UInput
              v-model="row.original.entry.source"
              size="xs"
              variant="outline"
              class="w-full"
              :ui="{ base: 'ring-[var(--border)] focus:ring-[var(--border-strong)]' }"
              :aria-label="`${row.original.entry.source || '新规则'} 的原文`"
            />
          </template>
          <template #textBehavior-cell="{ row }">
            <USelect
              :model-value="row.original.entry.translation === null ? 'keep' : 'replace'"
              :items="textBehaviorItems"
              value-key="value"
              label-key="label"
              size="xs"
              variant="none"
              class="w-full"
              :aria-label="`${row.original.entry.source || '新规则'} 的文字处理`"
              @update:model-value="setTextBehavior(row.original.entry, $event)"
            />
          </template>
          <template #translation-cell="{ row }">
            <UInput
              :model-value="row.original.entry.translation ?? ''"
              :disabled="row.original.entry.translation === null"
              size="xs"
              variant="outline"
              class="w-full"
              :ui="{ base: 'ring-[var(--border)] focus:ring-[var(--border-strong)]' }"
              :placeholder="row.original.entry.translation === null ? '保持原文' : '输入替换文字'"
              :aria-label="`${row.original.entry.source || '新规则'} 的译文`"
              @update:model-value="setTranslation(row.original.entry, $event)"
            />
          </template>
          <template #fontBehavior-cell="{ row }">
            <USelect
              :model-value="row.original.entry.font.kind"
              :items="fontBehaviorItems"
              value-key="value"
              label-key="label"
              size="xs"
              variant="none"
              class="w-full"
              :aria-label="`${row.original.entry.source || '新规则'} 的字体处理`"
              @update:model-value="setFontBehavior(row.original.entry, $event)"
            />
          </template>
          <template #fontFamily-cell="{ row }">
            <USelectMenu
              v-if="row.original.entry.font.kind === 'substitute'"
              :model-value="row.original.entry.font.family"
              :items="availableFonts"
              :search-input="{ placeholder: '搜索本机字体' }"
              :virtualize="{ estimateSize: 32 }"
              size="xs"
              variant="none"
              class="w-full"
              :aria-label="`${row.original.entry.source || '新规则'} 的字体`"
              @update:model-value="setEntryFont(row.original.entry, $event)"
            />
            <span v-else class="block truncate px-2 text-[10px] text-[var(--text-muted)]">
              {{ row.original.entry.font.kind === 'inherit' ? '跟随词典默认' : '保持宿主字体' }}
            </span>
          </template>
          <template #actions-cell="{ row }">
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="`删除 ${row.original.entry.source || '新规则'}`" @click="deleteRule(row.original.index)" />
          </template>
          <template #empty>
            <UEmpty
              icon="i-tabler-language"
              :title="draft.entries.length ? '没有符合当前筛选的规则' : '还没有替换规则'"
              :description="draft.entries.length ? '调整搜索或行为筛选后再试。' : '新增规则后，可设置文字与字体处理；规则会继承整份词典固定的适用 Hook。'"
            >
              <template #actions>
                <UButton color="primary" variant="ghost" size="sm" icon="i-tabler-plus" label="新增规则" @click="addRule" />
              </template>
            </UEmpty>
          </template>
        </UTable>
        <div class="shrink-0 border-t border-[var(--border)] bg-[var(--surface)] p-2">
          <UButton color="primary" variant="ghost" size="sm" icon="i-tabler-plus" label="新增规则" @click="addRule" />
        </div>
      </div>
    </ManagementTableFrame>
  </section>
</template>
