<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { testRegexRule, type RegexRule, type RegexRuleTestResult } from '../regexRules'
const props = defineProps<{ rule: RegexRule }>()
const { t } = useI18n()
const translationToken = '{{TR}}'
const source = ref('')
const mockTranslation = ref('')
const result = ref<RegexRuleTestResult | null>(null)
const error = ref('')
const busy = ref(false)
let revision = 0
watch(() => [source.value, mockTranslation.value, props.rule.pattern, props.rule.replacement], () => {
  revision++; result.value = null; error.value = ''
}, { flush: 'sync' })
async function run() {
  if (busy.value) return
  const current = revision
  busy.value = true; error.value = ''; result.value = null
  try {
    const response = await testRegexRule({ ...props.rule }, source.value, mockTranslation.value)
    if (revision === current) result.value = response
  } catch {
    if (revision === current) error.value = t('regexRules.testError')
  } finally { busy.value = false }
}
</script>

<template>
  <div class="mt-4 space-y-3" data-testid="regex-rule-tester">
    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-end gap-3 @max-[620px]:grid-cols-1">
      <UFormField :label="t('regexRules.testSource')">
        <UInput v-model="source" :aria-label="t('regexRules.testSource')" placeholder="Total:18" :maxlength="4096" class="w-full" @keydown.enter.prevent="run" />
      </UFormField>
      <UFormField :label="t('regexRules.mockTranslation')">
        <UInput v-model="mockTranslation" :aria-label="t('regexRules.mockTranslation')" :placeholder="t('regexRules.mockPlaceholder')" :maxlength="4096" class="w-full" @keydown.enter.prevent="run" />
      </UFormField>
      <UButton type="button" :label="t('regexRules.test')" :loading="busy" class="justify-center" @click="run" />
    </div>
    <p class="type-metadata text-[var(--text-muted)]">{{ t('regexRules.testHint', { translation: translationToken }) }}</p>
    <p v-if="error" role="alert" class="type-metadata text-error">{{ error }}</p>
    <div v-if="result" role="status" class="space-y-2 text-sm" data-testid="regex-test-result">
      <p>{{ t(result.matched ? 'regexRules.matched' : 'regexRules.notMatched') }}</p>
      <dl v-if="result.matched" class="flex flex-wrap gap-x-5 gap-y-2">
        <div v-for="(value, index) in result.captures" :key="index" class="flex min-w-0 gap-2">
          <dt class="text-[var(--text-muted)]">{{ index === 0 ? t('regexRules.wholeMatch') : `$${index}` }}</dt>
          <dd class="whitespace-pre-wrap break-all">{{ value === null ? t('regexRules.unmatchedGroup') : value === '' ? t('regexRules.emptyReplacement') : value }}</dd>
        </div>
      </dl>
      <p v-if="result.missingTranslation" class="text-[var(--text-muted)]">{{ t('regexRules.missingMock') }}</p>
      <p class="text-[var(--text-muted)]">{{ t('regexRules.testOutput') }}</p>
      <pre class="m-0 whitespace-pre-wrap break-all font-sans text-[var(--text)]">{{ result.output === '' ? t('regexRules.emptyReplacement') : result.output }}</pre>
    </div>
  </div>
</template>
