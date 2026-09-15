<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import LanguageSelect from './LanguageSelect.vue'
import FontFamilySelect from './FontFamilySelect.vue'
import { useI18n } from 'vue-i18n'
import type { DictionarySummary, WorkflowFontPolicy, WorkflowDictionaryFont } from '../model'
import { useWorkspace } from '../useWorkspace'

const props = defineProps<{ dictionaries: DictionarySummary[]; workflowName: string }>()
const ids = defineModel<string[]>('dictionaryIds', { required: true })
const writer = defineModel<string | null>('writeDictionaryId')
const fontPolicy = defineModel<WorkflowFontPolicy | null>('fontPolicy', { default: null })
function updateFont(id: string, patch: Partial<WorkflowDictionaryFont>) {
  const policy = fontPolicy.value ?? { families: [], scalePercent: 100, coverage: 'dictionary_matches' as const }
  const overrides = { ...policy.dictionaryOverrides }
  const font = { ...(overrides[id] ?? { families: [] }), ...patch }
  if (!font.families.length && font.scalePercent == null) delete overrides[id]
  else overrides[id] = font
  fontPolicy.value = { ...policy, dictionaryOverrides: overrides }
}
const { t } = useI18n()
const workspace = useWorkspace()
const open = ref(false)
const mode = ref('new')
const query = ref('')
const pending = ref<string[]>([])
const dictionaryName = ref('')
const sourceLocale = ref('en-US')
const targetLocale = ref('zh-CN')
const creating = ref(false)
const error = ref('')
const selected = computed(() => ids.value.map(id => ({ id, dictionary: props.dictionaries.find(item => item.metadata.id === id) })))
const available = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.dictionaries.filter(item => !ids.value.includes(item.metadata.id)
    && (!needle || `${item.metadata.name} ${item.metadata.sourceLocale} ${item.metadata.targetLocale}`.toLocaleLowerCase().includes(needle)))
})
watch(() => [...ids.value], current => {
  if (!writer.value || !current.includes(writer.value)) writer.value = current[0] ?? null
}, { immediate: true })
function showPicker() {
  mode.value = ids.value.length ? 'existing' : 'new'
  pending.value = []
  query.value = ''
  error.value = ''
  dictionaryName.value = props.workflowName ? `${props.workflowName} ${t('titleBar.dictionaries')}` : ''
  open.value = true
}
function togglePending(id: string) {
  pending.value = pending.value.includes(id) ? pending.value.filter(item => item !== id) : [...pending.value, id]
}
function move(id: string, offset: number) {
  const index = ids.value.indexOf(id)
  const next = index + offset
  if (index < 0 || next < 0 || next >= ids.value.length) return
  const reordered = [...ids.value]
  ;[reordered[index], reordered[next]] = [reordered[next], reordered[index]]
  ids.value = reordered
}
function remove(id: string) {
  if (fontPolicy.value?.dictionaryOverrides?.[id]) {
    const overrides = { ...fontPolicy.value.dictionaryOverrides }
    delete overrides[id]
    fontPolicy.value = { ...fontPolicy.value, dictionaryOverrides: overrides }
  }
  ids.value = ids.value.filter(item => item !== id)
}
async function confirm() {
  if (creating.value) return
  if (mode.value === 'existing') {
    const known = new Set(props.dictionaries.map(item => item.metadata.id))
    ids.value = [...new Set([...ids.value, ...pending.value.filter(id => known.has(id))])]
    open.value = false
    return
  }
  if (!dictionaryName.value.trim() || !sourceLocale.value.trim() || !targetLocale.value.trim()) return
  creating.value = true
  error.value = ''
  try {
    const id = await workspace.createDictionaryWithId({ name: dictionaryName.value.trim(), description: '',
      sourceLocale: sourceLocale.value.trim(), targetLocale: targetLocale.value.trim(), releaseVersion: '0.1.0',
      authors: [], license: null, homepage: null, tags: [] })
    if (!id) { error.value = workspace.messages.value.dictionaries || t('workflows.dictionaryCreateFailed'); return }
    ids.value = [...ids.value, id]
    writer.value = id
    open.value = false
  } finally { creating.value = false }
}
defineExpose({ showPicker })
</script>

<template>
  <div class="@container space-y-4">
    <div class="flex items-center justify-between gap-4">
      <h2 class="type-body m-0 flex items-center gap-1 font-semibold">{{ t('workflows.selectedDictionaries', { count: ids.length }) }}<UPopover mode="hover"><UButton class="shrink-0" icon="i-tabler-help-circle" color="neutral" variant="ghost" size="xs" :aria-label="t('workflows.dictionaryHelpTitle')" /><template #content><div class="max-w-80 space-y-2 p-3 text-xs leading-5"><p>{{ t('workflows.dictionaryWriteHelp') }}</p><p>{{ t('workflows.dictionaryOtherHelp') }}</p><p>{{ t('workflows.dictionaryPriorityHelp') }}</p></div></template></UPopover></h2>
      <UButton color="primary" variant="soft" size="sm" icon="i-tabler-plus" :label="t('workflows.addDictionaries')" @click="showPicker" />
    </div>
    <div data-testid="workflow-selected-dictionaries" class="overflow-hidden rounded-md border border-[var(--border)]">
      <div v-for="(item, index) in selected" :key="item.id" :data-dictionary-id="item.id" class="grid min-h-16 grid-cols-[20px_minmax(0,1fr)_auto] items-center gap-3 border-b @min-[800px]:grid-cols-[20px_minmax(120px,1fr)_minmax(0,360px)_auto] border-[var(--border)] px-3 py-2 last:border-b-0">
        <span class="type-metadata w-5 shrink-0 text-center tabular-nums text-[var(--text-muted)]">{{ index + 1 }}</span>
        <div class="min-w-0">
          <strong class="type-label block truncate" :title="item.dictionary?.metadata.name ?? item.id">{{ item.dictionary?.metadata.name ?? item.id }}</strong>
          <span v-if="item.dictionary" class="type-metadata text-[var(--text-muted)]">{{ item.dictionary.metadata.sourceLocale }} → {{ item.dictionary.metadata.targetLocale }}</span>
        </div>
        <div class="col-span-2 col-start-2 row-start-2 flex min-w-0 flex-wrap items-center gap-3 @min-[800px]:col-span-1 @min-[800px]:col-start-3 @min-[800px]:row-start-1">
          <FontFamilySelect :model-value="fontPolicy?.dictionaryOverrides?.[item.id]?.families ?? []" :aria-label="t('workflowTypography.fontFor', { name: item.dictionary?.metadata.name ?? item.id })" class="w-52" @update:model-value="updateFont(item.id, { families: $event })" />
          <div class="flex items-center gap-1">
            <UInput :model-value="fontPolicy?.dictionaryOverrides?.[item.id]?.scalePercent ?? ''" type="number" :min="50" :max="200" :step="5" :placeholder="t('workflowTypography.inheritScale')" :aria-label="t('workflowTypography.scaleFor', { name: item.dictionary?.metadata.name ?? item.id })" class="w-28" @update:model-value="updateFont(item.id, { scalePercent: $event === '' ? null : Number($event) })" />
            <span class="type-metadata">%</span>
          </div>
        </div>
        <div class="col-start-3 row-start-1 flex items-center gap-3 @min-[800px]:col-start-4">
        <label class="type-label flex shrink-0 cursor-pointer items-center gap-2" :title="t('workflows.writerRadioHint')">
          <input type="radio" name="workflow-write-dictionary" class="size-4 accent-[var(--accent)]" :checked="writer === item.id" :aria-label="t('workflows.writeToDictionary', { name: item.dictionary?.metadata.name ?? item.id })" @change="writer = item.id">
          <span>{{ t('workflows.writerLabel') }}</span>
        </label>
        <div class="flex shrink-0 items-center">
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-up" :disabled="index === 0" :aria-label="t('workflows.raiseDictionary', { name: item.dictionary?.metadata.name ?? item.id })" :title="t('workflows.raiseDictionary', { name: item.dictionary?.metadata.name ?? item.id })" @click="move(item.id, -1)" />
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-down" :disabled="index === ids.length - 1" :aria-label="t('workflows.lowerDictionary', { name: item.dictionary?.metadata.name ?? item.id })" :title="t('workflows.lowerDictionary', { name: item.dictionary?.metadata.name ?? item.id })" @click="move(item.id, 1)" />
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :aria-label="t('workflows.removeDictionary', { name: item.dictionary?.metadata.name ?? item.id })" :title="t('workflows.removeDictionary', { name: item.dictionary?.metadata.name ?? item.id })" @click="remove(item.id)" />
        </div>
        </div>
      </div>
      <p v-if="!selected.length" class="type-metadata m-0 px-4 py-8 text-center text-[var(--text-muted)]">{{ t('workflows.addDictionaryHint') }}</p>
    </div>
    <ManagementFormModal :open="open" :title="t('workflows.addDictionaries')" width="lg" :busy="creating"
      :confirm-label="mode === 'new' ? t('workflows.createAndAddDictionary') : t('workflows.addSelectedDictionaries')"
      :confirm-disabled="mode === 'existing' ? !pending.length : !dictionaryName.trim() || !sourceLocale.trim() || !targetLocale.trim()"
      @update:open="open = $event" @confirm="confirm">
      <div data-tour="workflow-dictionary-picker" class="space-y-4">
        <UTabs v-model="mode" :content="false" :items="[{ value: 'new', label: t('workflows.createWriteDictionary') }, { value: 'existing', label: t('workflows.existingDictionaries') }]" />
        <template v-if="mode === 'existing'">
          <UInput v-model="query" icon="i-tabler-search" class="w-full" :placeholder="t('workflows.searchDictionaries')" :aria-label="t('workflows.searchDictionaries')" />
          <div class="max-h-80 overflow-y-auto rounded-md border border-[var(--border)]">
            <label v-for="item in available" :key="item.metadata.id" class="flex min-h-14 cursor-pointer items-center gap-3 border-b border-[var(--border)] px-3 py-2 last:border-b-0 hover:bg-[var(--surface-hover)]">
              <UCheckbox :model-value="pending.includes(item.metadata.id)" :aria-label="t('workflows.selectDictionaryNamed', { name: item.metadata.name })" @update:model-value="togglePending(item.metadata.id)" />
              <span class="min-w-0"><strong class="type-label block truncate">{{ item.metadata.name }}</strong><span class="type-metadata text-[var(--text-muted)]">{{ item.metadata.sourceLocale }} → {{ item.metadata.targetLocale }}</span></span>
            </label>
            <p v-if="!available.length" class="type-metadata m-0 px-4 py-8 text-center text-[var(--text-muted)]">{{ t('workflows.noDictionaryMatch') }}</p>
          </div>
        </template>
        <template v-else>
          <UFormField :label="t('workflows.dictionaryName')"><UInput v-model="dictionaryName" class="w-full" :aria-label="t('workflows.dictionaryName')" /></UFormField>
          <div class="grid grid-cols-2 gap-3">
            <UFormField :label="t('workflows.sourceLocale')"><LanguageSelect v-model="sourceLocale" allow-auto class="w-full" :aria-label="t('workflows.sourceLocale')" /></UFormField>
            <UFormField :label="t('workflows.targetLocale')"><LanguageSelect v-model="targetLocale" class="w-full" :aria-label="t('workflows.targetLocale')" /></UFormField>
          </div>
        </template>
        <p v-if="error" role="alert" class="type-metadata text-error">{{ error }}</p>
      </div>
    </ManagementFormModal>
  </div>
</template>
