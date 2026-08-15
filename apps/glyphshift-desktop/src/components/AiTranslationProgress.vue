<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AiTranslationBatchStatus, AiTranslationJob } from '../useAiTranslation'

const props = defineProps<{
  job: AiTranslationJob
  elapsed: string
}>()

const emit = defineEmits<{
  cancel: []
  dismiss: []
}>()
const { t } = useI18n()

const batches = computed(() => props.job.batches ?? [])
const active = computed(() => !['completed', 'completed_with_failures', 'cancelled'].includes(props.job.status))
const orderedBatches = computed(() => {
  const priority: Record<AiTranslationBatchStatus, number> = {
    retrying: 0,
    running: 1,
    failed: 2,
    queued: 3,
    completed: 4,
    cancelled: 5,
  }
  return [...batches.value].sort((left, right) => {
    const statusOrder = priority[left.status] - priority[right.status]
    if (statusOrder !== 0) return statusOrder
    if (['completed', 'cancelled'].includes(left.status)) return right.batchNumber - left.batchNumber
    return left.batchNumber - right.batchNumber
  })
})
const statusCounts = computed(() => batches.value.reduce((counts, batch) => {
  counts[batch.status] += 1
  return counts
}, {
  queued: 0,
  running: 0,
  retrying: 0,
  completed: 0,
  failed: 0,
  cancelled: 0,
} as Record<AiTranslationBatchStatus, number>))

const finishedCount = computed(() => statusCounts.value.completed + statusCounts.value.failed)
const peakConcurrency = computed(() => Math.max(
  props.job.peakConcurrency ?? 0,
  statusCounts.value.running,
))

function formatDuration(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)
  const trailing = `${minutes}:${seconds.toString().padStart(2, '0')}`
  return hours ? `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}` : trailing
}

function statusTone(status: AiTranslationBatchStatus) {
  if (status === 'running') return 'primary'
  if (status === 'retrying') return 'warning'
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'error'
  return 'neutral'
}
</script>

<template>
  <section
    data-testid="ai-translation-progress"
    role="region"
    :aria-label="t('ai.progressRegion')"
    class="mb-3 overflow-hidden rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface)]"
  >
    <header class="flex items-start justify-between gap-3 px-3 py-2.5">
      <div role="status" aria-live="polite" aria-atomic="true" class="min-w-0">
        <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
          <strong class="type-section-title text-[var(--text)]">{{ t(active ? 'ai.translating' : 'ai.batchReportTitle') }}</strong>
          <span class="type-metadata tabular-nums text-[var(--text-muted)]">
            {{ t('ai.progressSummary', { completed: job.completedCount, total: job.totalCount, elapsed }) }}
          </span>
        </div>
        <p class="type-metadata m-0 mt-1 text-[var(--text-muted)]">
          {{ t('ai.concurrencyEvidence', { running: statusCounts.running, limit: job.maxConcurrency, peak: peakConcurrency }) }}
        </p>
      </div>
      <UButton v-if="active" color="neutral" variant="ghost" size="xs" :label="t('ai.cancelJob')" @click="emit('cancel')" />
      <UButton v-else color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :label="t('ai.dismissBatchReport')" @click="emit('dismiss')" />
    </header>

    <dl class="m-0 grid grid-cols-2 border-y border-[var(--border)] bg-[var(--surface-subtle)] sm:grid-cols-4">
      <div class="px-3 py-2">
        <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.runningBatches') }}</dt>
        <dd class="m-0 mt-0.5 text-xs font-semibold tabular-nums text-[var(--text)]">{{ statusCounts.running }}</dd>
      </div>
      <div class="border-l border-[var(--border)] px-3 py-2">
        <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.retryingBatches') }}</dt>
        <dd class="m-0 mt-0.5 text-xs font-semibold tabular-nums text-[var(--text)]">{{ statusCounts.retrying }}</dd>
      </div>
      <div class="border-t border-[var(--border)] px-3 py-2 sm:border-l sm:border-t-0">
        <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.queuedBatches') }}</dt>
        <dd class="m-0 mt-0.5 text-xs font-semibold tabular-nums text-[var(--text)]">{{ statusCounts.queued }}</dd>
      </div>
      <div class="border-l border-t border-[var(--border)] px-3 py-2 sm:border-t-0">
        <dt class="type-metadata text-[var(--text-muted)]">{{ t('ai.finishedBatches') }}</dt>
        <dd class="m-0 mt-0.5 text-xs font-semibold tabular-nums text-[var(--text)]">{{ finishedCount }}/{{ job.totalBatches }}</dd>
      </div>
    </dl>

    <ol v-if="orderedBatches.length" class="m-0 max-h-56 list-none divide-y divide-[var(--border)] overflow-y-auto p-0" :aria-label="t('ai.batchDetails')">
      <li
        v-for="batch in orderedBatches"
        :key="batch.batchNumber"
        class="grid min-h-9 grid-cols-[3.5rem_5.5rem_minmax(3rem,1fr)_auto] items-center gap-2 px-3 py-1.5"
        :data-testid="`ai-batch-${batch.batchNumber}`"
      >
        <span class="type-label font-semibold tabular-nums text-[var(--text)]">{{ t('ai.batchNumber', { number: batch.batchNumber }) }}</span>
        <UBadge :color="statusTone(batch.status)" variant="subtle" size="sm" :label="t(`ai.batchStatus.${batch.status}`)" class="justify-self-start" />
        <span class="type-metadata min-w-0 truncate text-[var(--text-muted)]">
          {{ t('ai.batchItems', { count: batch.itemCount }) }}
          <template v-if="batch.startedAfterMs !== null"> · {{ t('ai.batchStartedAfter', { elapsed: formatDuration(batch.startedAfterMs) }) }}</template>
          <template v-if="batch.attemptCount > 0"> · {{ t(batch.status === 'retrying' ? 'ai.batchRetryAttempt' : 'ai.batchAttempt', { attempt: batch.attemptCount }) }}</template>
        </span>
        <time class="type-metadata tabular-nums text-[var(--text-muted)]">{{ formatDuration(batch.elapsedMs) }}</time>
        <p v-if="batch.lastError && (batch.attemptCount > 1 || ['retrying', 'failed'].includes(batch.status))" class="type-metadata col-start-2 col-end-5 m-0 truncate pb-1 text-[var(--text-muted)]" :title="batch.lastError.safeMessage">
          {{ batch.lastError.safeMessage }}
        </p>
      </li>
    </ol>
  </section>
</template>
