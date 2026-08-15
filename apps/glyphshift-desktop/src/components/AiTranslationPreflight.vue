<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { estimateAiTranslationInput, type AiProfile, type AiTranslationPlan } from '../useAiTranslation'

const props = defineProps<{
  open: boolean
  plan: AiTranslationPlan | null
  profile: AiProfile | null
  maxItemsPerRequest: number
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  proceed: []
}>()

const { n, t } = useI18n()
const estimate = computed(() => props.plan
  ? estimateAiTranslationInput(props.plan, props.maxItemsPerRequest)
  : { estimatedInputTokens: 0, totalBatches: 0 })
const formattedTokens = computed(() => n(estimate.value.estimatedInputTokens))

function proceed() {
  emit('update:open', false)
  emit('proceed')
}
</script>

<template>
  <UModal
    :open="open"
    :title="t('ai.preflightTitle')"
    :description="t('ai.preflightDescription')"
    :ui="{
      content: 'max-w-[440px]',
      header: 'min-h-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'type-metadata mt-1 leading-4',
      body: 'px-5 py-4',
      footer: 'px-5 py-4',
    }"
    @update:open="emit('update:open', $event)"
  >
    <template #body>
      <dl class="m-0 divide-y divide-[var(--border)] border-y border-[var(--border)]">
        <div class="flex items-center justify-between gap-4 py-2.5">
          <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.estimatedInput') }}</dt>
          <dd class="m-0 text-[12px] font-semibold text-[var(--text)]">{{ t('ai.estimatedTokens', { tokens: formattedTokens }) }}</dd>
        </div>
        <div class="flex items-center justify-between gap-4 py-2.5">
          <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.preflightWorkload') }}</dt>
          <dd class="m-0 text-[12px] font-semibold text-[var(--text)]">{{ t('ai.preflightWorkloadValue', { items: plan?.candidates.length ?? 0, batches: estimate.totalBatches }) }}</dd>
        </div>
        <div class="flex items-center justify-between gap-4 py-2.5">
          <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.preflightBatchPolicy') }}</dt>
          <dd class="m-0 text-[12px] font-semibold text-[var(--text)]">{{ t('ai.preflightBatchPolicyValue', { batchSize: maxItemsPerRequest, concurrency: profile?.maxConcurrency ?? 1 }) }}</dd>
        </div>
        <div class="flex items-center justify-between gap-4 py-2.5">
          <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.preflightFailurePolicy') }}</dt>
          <dd class="m-0 text-[12px] font-semibold text-[var(--text)]">{{ t('ai.preflightFailurePolicyValue', { timeout: (profile?.timeoutMs ?? 300_000) / 1_000, retries: profile?.maxRetries ?? 2 }) }}</dd>
        </div>
      </dl>
      <p class="type-metadata mb-0 mt-3 leading-4 text-[var(--text-muted)]">{{ t('ai.estimateDisclaimer') }}</p>
    </template>

    <template #footer>
      <div class="ml-auto flex items-center gap-2">
        <UButton color="neutral" variant="outline" size="sm" :label="t('common.cancel')" @click="emit('update:open', false)" />
        <UButton color="primary" size="sm" :label="t('ai.startTranslation')" @click="proceed" />
      </div>
    </template>
  </UModal>
</template>
