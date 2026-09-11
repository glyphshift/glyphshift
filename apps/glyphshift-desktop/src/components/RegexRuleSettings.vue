<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import RegexRuleTester from './RegexRuleTester.vue'
import { propertyNumberRule, validateRegexRule, type RegexRule } from '../regexRules'

const { t } = useI18n()
const translationToken = '{{TR}}'
const rules = defineModel<RegexRule[]>({ default: () => [] })
const rows = computed(() => rules.value)
const editor = ref<{ index: number; rule: RegexRule } | null>(null)
const error = ref('')
const saved = ref(false)
const busy = ref(false)
const patternInput = ref<{ inputRef?: HTMLInputElement }>()

async function edit(index: number) {
  editor.value = { index, rule: { ...(rows.value[index] ?? propertyNumberRule) } }
  error.value = ''; saved.value = false
  await nextTick()
  patternInput.value?.inputRef?.focus()
}
function rulesModelUpdate(value: RegexRule[]) { rules.value = value }
async function persist(rules: RegexRule[]) {
  busy.value = true; error.value = ''; saved.value = false
  try { rulesModelUpdate(rules); saved.value = true; return true }
  catch { error.value = t('regexRules.saveFailed'); return false }
  finally { busy.value = false }
}
async function save() {
  if (!editor.value || busy.value) return
  busy.value = true; error.value = ''
  const { index, rule } = editor.value
  try { await validateRegexRule(rule) }
  catch (reason) { error.value = `${t('regexRules.invalid')} ${reason instanceof Error ? reason.message : String(reason)}`; busy.value = false; return }
  const next = rows.value.map(row => ({ ...row }))
  if (index === next.length) next.push({ ...rule })
  else next[index] = { ...rule }
  if (await persist(next)) editor.value = null
}
function move(index: number, offset: number) {
  const next = [...rows.value]
  const other = index + offset
  if (!next[index] || !next[other]) return
  ;[next[index], next[other]] = [next[other]!, next[index]!]
  void persist(next)
}
</script>

<template>
  <ManagementFormSection :title="t('regexRules.title')" :description="t('regexRules.description')" data-testid="dictionary-regex-rules">
    <template #actions>
      <UButton :label="t('regexRules.add')" icon="i-tabler-plus" size="sm" :disabled="!!editor || busy || rows.length >= 64" @click="edit(rows.length)" />
    </template>
    <p class="type-metadata my-4 text-[var(--text-muted)]">{{ t('regexRules.syntax', { translation: translationToken, capture: '$2' }) }}</p>
    <p v-if="!rows.length && !editor" class="type-body my-6 text-[var(--text-secondary)]">{{ t('regexRules.empty') }}</p>
    <ol class="m-0 list-none divide-y divide-[var(--border)] p-0">
      <li v-for="(row, index) in rows" :key="index" class="py-4" data-testid="regex-rule-row">
        <div class="flex items-start gap-3 @max-[620px]:flex-wrap">
          <USwitch :model-value="row.enabled" :aria-label="t('regexRules.enable', { number: index + 1 })" :disabled="!!editor || busy" class="mt-1" @update:model-value="persist(rows.map((item, i) => i === index ? { ...item, enabled: $event } : item))" />
          <div class="grid min-w-0 flex-1 grid-cols-2 gap-4 @max-[620px]:grid-cols-1" :class="!row.enabled && 'opacity-60'">
            <div><span class="type-metadata text-[var(--text-muted)]">{{ t('regexRules.pattern') }}</span><code class="mt-1 block whitespace-pre-wrap break-all text-sm text-[var(--text)]">{{ row.pattern }}</code></div>
            <div><span class="type-metadata text-[var(--text-muted)]">{{ t('regexRules.replacement') }}</span><code class="mt-1 block whitespace-pre-wrap break-all text-sm text-[var(--text)]">{{ row.replacement || t('regexRules.emptyReplacement') }}</code></div>
          </div>
          <div class="flex gap-1 @max-[620px]:ml-auto">
            <UButton icon="i-tabler-arrow-up" color="neutral" variant="ghost" size="sm" :aria-label="t('regexRules.up', { number: index + 1 })" :disabled="!!editor || busy || index === 0" @click="move(index, -1)" />
            <UButton icon="i-tabler-arrow-down" color="neutral" variant="ghost" size="sm" :aria-label="t('regexRules.down', { number: index + 1 })" :disabled="!!editor || busy || index === rows.length - 1" @click="move(index, 1)" />
            <UButton icon="i-tabler-pencil" color="neutral" variant="ghost" size="sm" :aria-label="t('regexRules.edit', { number: index + 1 })" :disabled="!!editor || busy" @click="edit(index)" />
            <UButton icon="i-tabler-trash" color="neutral" variant="ghost" size="sm" :aria-label="t('regexRules.remove', { number: index + 1 })" :disabled="!!editor || busy" @click="persist(rows.filter((_, i) => i !== index))" />
          </div>
        </div>
        <RegexRuleTester :rule="row" />
      </li>
    </ol>
    <div v-if="editor" class="space-y-4 border-t border-[var(--border)] py-4">
      <p class="type-body font-medium text-[var(--text)]">{{ editor.index === rows.length ? t('regexRules.add') : t('regexRules.edit', { number: editor.index + 1 }) }}</p>
      <div class="grid grid-cols-2 gap-4 @max-[620px]:grid-cols-1">
        <UFormField :label="t('regexRules.pattern')" required>
          <UInput ref="patternInput" v-model="editor.rule.pattern" :aria-label="t('regexRules.pattern')" :maxlength="1024" :disabled="busy" spellcheck="false" class="w-full" />
        </UFormField>
        <UFormField :label="t('regexRules.replacement')">
          <UInput v-model="editor.rule.replacement" :aria-label="t('regexRules.replacement')" :maxlength="4096" :disabled="busy" spellcheck="false" class="w-full" />
        </UFormField>
      </div>
      <p class="type-metadata text-[var(--text-muted)]">{{ t('regexRules.example') }}</p>
      <RegexRuleTester :rule="editor.rule" />
      <div class="flex gap-2">
        <UButton type="button" @click="save" :label="t('regexRules.save')" :loading="busy" :disabled="!editor.rule.pattern || busy" />
        <UButton :label="t('regexRules.cancel')" color="neutral" variant="ghost" :disabled="busy" @click="editor = null; error = ''" />
      </div>
    </div>
    <p v-if="error" role="alert" class="type-metadata my-4 whitespace-pre-wrap break-words text-error">{{ error }}</p>
    <p v-else-if="saved" role="status" class="type-metadata my-4 text-[var(--text-muted)]">{{ t('regexRules.saved') }}</p>
  </ManagementFormSection>
</template>
