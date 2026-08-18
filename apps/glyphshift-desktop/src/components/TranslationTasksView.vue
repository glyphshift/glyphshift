<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  useAiTranslation,
  type AiProviderUsage,
  type AiProviderProtocol,
  type AiReasoningEffort,
  type AiTranslationBatch,
  type AiTranslationBatchStatus,
  type AiTranslationRunRecord,
} from '../useAiTranslation'
import { useWorkspace } from '../useWorkspace'

type TaskTab = 'current' | 'statistics' | 'list'

const { t, locale } = useI18n()
const ai = useAiTranslation()
const workspace = useWorkspace()
const activeTab = ref<TaskTab>('current')
const batchDetailsOpen = ref(false)
const historyQuery = ref('')
const historyStatus = ref('all')
const expandedRecordId = ref<string | null>(null)

const current = computed(() => ai.taskCenter.value.current)
const history = computed(() => ai.taskCenter.value.history)
const taskTabs = computed(() => [
  {
    value: 'current' as const,
    slot: 'current',
    label: t('ai.tasks.tabs.current'),
    icon: 'i-tabler-activity',
    badge: ai.taskRunning.value
      ? { label: t('ai.tasks.status.running'), color: 'primary' as const, variant: 'soft' as const }
      : undefined,
  },
  { value: 'statistics' as const, slot: 'statistics', label: t('ai.tasks.tabs.statistics'), icon: 'i-tabler-chart-bar' },
  {
    value: 'list' as const,
    slot: 'list',
    label: t('ai.tasks.tabs.list'),
    icon: 'i-tabler-list-details',
    badge: history.value.length
      ? { label: formatNumber(history.value.length), color: 'neutral' as const, variant: 'soft' as const }
      : undefined,
  },
])
const historyStatusItems = computed(() => [
  { value: 'all', label: t('ai.tasks.statusFilter.all') },
  { value: 'completed', label: t('ai.tasks.status.completed') },
  { value: 'completed_with_failures', label: t('ai.tasks.status.completed_with_failures') },
  { value: 'cancelled', label: t('ai.tasks.status.cancelled') },
  { value: 'interrupted', label: t('ai.tasks.status.interrupted') },
])
const filteredHistory = computed(() => {
  const needle = historyQuery.value.trim().toLocaleLowerCase()
  return history.value.filter(record => (
    (historyStatus.value === 'all' || record.status === historyStatus.value)
    && (!needle || `${record.profileName} ${record.modelId} ${scopeLabel(record)} ${t(`ai.tasks.status.${record.status}`)}`.toLocaleLowerCase().includes(needle))
  ))
})
const currentDictionaryName = computed(() => {
  const dictionaryId = current.value?.targetDictionaryId
  if (!dictionaryId) return t('ai.tasks.connectionCheck')
  return workspace.model.value.dictionaries.find(item => item.metadata.id === dictionaryId)?.metadata.name
    ?? t('ai.tasks.unknownDictionary')
})
const currentBatches = computed(() => orderedBatches(current.value?.batches ?? []))
const currentActiveRequests = computed(() => current.value?.batches.filter(batch => (
  batch.status === 'running' || batch.status === 'retrying'
)).length ?? 0)

interface ModelAggregate {
  key: string
  profileName: string
  modelId: string
  protocol: AiProviderProtocol
  reasoningEffort: AiReasoningEffort
  tasks: number
  texts: number
  elapsedMs: number
  usage: AiProviderUsage
  usageAvailable: boolean
}

const modelStats = computed(() => {
  const groups = new Map<string, ModelAggregate>()
  for (const record of history.value) {
    const reasoningEffort = record.reasoningEffort ?? 'automatic'
    const key = `${record.protocol}\u0000${record.modelId}\u0000${record.profileName}\u0000${reasoningEffort}`
    const group = groups.get(key) ?? {
      key,
      profileName: record.profileName,
      modelId: record.modelId,
      protocol: record.protocol,
      reasoningEffort,
      tasks: 0,
      texts: 0,
      elapsedMs: 0,
      usage: { inputTokens: 0, cachedInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 0 },
      usageAvailable: false,
    }
    group.tasks += 1
    group.texts += record.appliedCount
    group.elapsedMs += record.elapsedMs
    if (record.usage) {
      group.usageAvailable = true
      group.usage.inputTokens += record.usage.inputTokens
      group.usage.cachedInputTokens += record.usage.cachedInputTokens
      group.usage.outputTokens += record.usage.outputTokens
      group.usage.reasoningTokens += record.usage.reasoningTokens
      group.usage.totalTokens += record.usage.totalTokens
    }
    groups.set(key, group)
  }
  return [...groups.values()].sort((left, right) => right.tasks - left.tasks || right.texts - left.texts)
})

function formatNumber(value: number) {
  return new Intl.NumberFormat(locale.value).format(value)
}

function formatDuration(milliseconds: number) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000))
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const remainder = seconds % 60
  return hours
    ? `${hours}:${minutes.toString().padStart(2, '0')}:${remainder.toString().padStart(2, '0')}`
    : `${minutes}:${remainder.toString().padStart(2, '0')}`
}

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat(locale.value, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestamp))
}

function statusTone(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'running' || status === 'queued') return 'primary'
  if (status === 'completed_with_failures' || status === 'interrupted') return 'warning'
  if (status === 'cancelled') return 'neutral'
  return 'error'
}

function progressTone(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'completed_with_failures' || status === 'interrupted') return 'warning'
  return 'primary'
}

function batchTone(status: AiTranslationBatchStatus) {
  if (status === 'running') return 'primary'
  if (status === 'retrying') return 'warning'
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'error'
  return 'neutral'
}

function orderedBatches(batches: AiTranslationBatch[]) {
  const priority: Record<AiTranslationBatchStatus, number> = {
    retrying: 0,
    running: 1,
    failed: 2,
    queued: 3,
    completed: 4,
    cancelled: 5,
  }
  return [...batches].sort((left, right) => {
    const statusOrder = priority[left.status] - priority[right.status]
    return statusOrder || left.batchNumber - right.batchNumber
  })
}

function scopeLabel(record: AiTranslationRunRecord) {
  return t(`ai.tasks.origin.${record.scopeKind === 'probe' ? 'probe' : record.scopeKind === 'connection' ? 'connection' : 'dictionary'}`)
}

function reasoningLabel(effort: AiReasoningEffort | undefined) {
  const value = effort ?? 'automatic'
  return t(`ai.reasoningOption.${value}`)
}

function toggleRecord(recordId: string) {
  expandedRecordId.value = expandedRecordId.value === recordId ? null : recordId
}

watch(() => current.value?.jobId, () => {
  batchDetailsOpen.value = false
})
watch(() => current.value?.batches.map(batch => batch.status).join(','), (statuses) => {
  if (statuses?.includes('retrying') || statuses?.includes('failed')) batchDetailsOpen.value = true
}, { immediate: true })

onMounted(() => void ai.connectTaskMonitor().catch(() => undefined))
</script>

<template>
  <UtilityPageShell
    title-id="translation-tasks-title"
    :title="t('ai.tasks.title')"
    :description="t('ai.tasks.description')"
    content-test-id="translation-tasks-content"
  >
    <UTabs
      v-model="activeTab"
      data-testid="translation-task-tabs"
      :items="taskTabs"
      color="neutral"
      variant="link"
      size="sm"
      activation-mode="manual"
      class="w-full"
      :ui="{
        list: 'w-full justify-start gap-1 rounded-none border-b border-[var(--border)] bg-transparent p-0',
        indicator: 'hidden',
        trigger: 'type-label relative h-10 flex-none gap-2 rounded-none px-3 text-[var(--text-secondary)] after:absolute after:inset-x-2 after:bottom-0 after:hidden after:h-0.5 after:bg-[var(--accent)] hover:bg-[var(--surface-hover)] data-[state=active]:font-semibold data-[state=active]:!text-[var(--text)] data-[state=active]:after:block',
        leadingIcon: 'size-4 shrink-0',
        trailingBadge: 'type-caption ml-1 min-w-4 justify-center px-1',
        content: 'pt-5 focus:outline-none',
      }"
    >
      <template #current>
        <section data-testid="translation-task-current" :aria-label="t('ai.tasks.tabs.current')">
          <div v-if="current" class="overflow-hidden rounded-[var(--radius-surface)] border border-[var(--border)] bg-[var(--surface)]">
            <header class="flex flex-wrap items-start justify-between gap-4 px-4 py-4">
              <div class="min-w-0">
                <div class="flex flex-wrap items-center gap-2">
                  <strong class="truncate text-[13px] font-semibold text-[var(--text)]">{{ currentDictionaryName }}</strong>
                  <UBadge :color="statusTone(current.status)" variant="soft" size="sm" :label="t(`ai.tasks.status.${current.status}`)" />
                  <UBadge v-if="current.dictionaryLocked" color="warning" variant="subtle" size="sm" icon="i-tabler-lock" :label="t('ai.tasks.dictionaryLocked')" />
                </div>
                <p class="type-metadata m-0 mt-1 truncate text-[var(--text-muted)]">
                  {{ t(`ai.tasks.origin.${current.origin}`) }} · {{ current.profileName }} · {{ current.modelId }} · {{ t('ai.tasks.reasoningUsed', { effort: reasoningLabel(current.reasoningEffort) }) }}
                </p>
              </div>
              <UButton
                v-if="ai.taskRunning.value"
                color="error"
                variant="soft"
                size="sm"
                icon="i-tabler-player-stop"
                :label="t('ai.tasks.stopTranslation')"
                class="shrink-0"
                @click="ai.cancelCurrentJob"
              />
            </header>

            <div class="border-t border-[var(--border)] bg-[var(--surface-subtle)] px-4 py-4">
              <div class="mb-2 flex items-end justify-between gap-4">
                <div>
                  <strong class="type-body font-semibold text-[var(--text)]">{{ t('ai.tasks.overallProgress') }}</strong>
                  <p class="type-metadata m-0 mt-0.5 text-[var(--text-muted)]">{{ t('ai.tasks.modelProgress', { completed: current.completedCount, total: current.totalCount }) }}</p>
                </div>
                <strong class="text-[18px] font-semibold tabular-nums tracking-[-0.02em] text-[var(--text)]">{{ current.completedCount }}/{{ current.totalCount }}</strong>
              </div>
              <UProgress :model-value="current.completedCount" :max="Math.max(1, current.totalCount)" :color="progressTone(current.status)" size="sm" />
              <dl class="mt-4 grid grid-cols-2 gap-x-6 gap-y-3 sm:grid-cols-4">
                <div><dt class="type-caption text-[var(--text-muted)]">{{ t('ai.tasks.written') }}</dt><dd class="type-label m-0 mt-0.5 font-semibold tabular-nums text-[var(--text)]">{{ formatNumber(current.appliedCount) }}</dd></div>
                <div><dt class="type-caption text-[var(--text-muted)]">{{ t('ai.tasks.finishedBatchProgress') }}</dt><dd class="type-label m-0 mt-0.5 font-semibold tabular-nums text-[var(--text)]">{{ current.finishedBatches }}/{{ current.totalBatches }}</dd></div>
                <div><dt class="type-caption text-[var(--text-muted)]">{{ t('ai.tasks.activeRequests') }}</dt><dd class="type-label m-0 mt-0.5 font-semibold tabular-nums text-[var(--text)]">{{ currentActiveRequests }}/{{ current.maxConcurrency }}</dd></div>
                <div><dt class="type-caption text-[var(--text-muted)]">{{ t('ai.tasks.elapsed') }}</dt><dd class="type-label m-0 mt-0.5 font-semibold tabular-nums text-[var(--text)]">{{ formatDuration(current.elapsedMs) }}</dd></div>
              </dl>
            </div>

            <div class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] px-4 py-3">
              <div class="min-w-0">
                <strong class="type-label block font-semibold text-[var(--text)]">{{ t('ai.tasks.reportedUsage') }}</strong>
                <p v-if="current.usage" class="type-metadata m-0 mt-0.5 truncate tabular-nums text-[var(--text-muted)]" :title="t('ai.batchUsage', { input: formatNumber(current.usage.inputTokens), cached: formatNumber(current.usage.cachedInputTokens), output: formatNumber(current.usage.outputTokens), reasoning: formatNumber(current.usage.reasoningTokens), total: formatNumber(current.usage.totalTokens) })">
                  {{ t('ai.tasks.usageBreakdown', { input: formatNumber(current.usage.inputTokens), cached: formatNumber(current.usage.cachedInputTokens), output: formatNumber(current.usage.outputTokens), reasoning: formatNumber(current.usage.reasoningTokens) }) }}
                </p>
                <p v-else class="type-metadata m-0 mt-0.5 text-[var(--text-muted)]">{{ t('ai.tasks.usagePending') }}</p>
              </div>
              <strong v-if="current.usage" class="shrink-0 text-[16px] font-semibold tabular-nums text-[var(--text)]">{{ formatNumber(current.usage.totalTokens) }} <small class="type-caption font-normal text-[var(--text-muted)]">Token</small></strong>
            </div>

            <UAlert v-if="current.writebackError" color="error" variant="soft" icon="i-tabler-alert-circle" :title="t('ai.tasks.writebackFailed')" :description="t('ai.tasks.writebackFailedDescription')" class="m-3" />

            <UCollapsible v-model:open="batchDetailsOpen" class="border-t border-[var(--border)]">
              <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                class="w-full justify-between rounded-none px-4 py-2.5"
                :label="t('ai.tasks.batchDetailsToggle', { finished: current.finishedBatches, total: current.totalBatches })"
                :trailing-icon="batchDetailsOpen ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'"
              />
              <template #content>
                <ol data-testid="translation-task-batches" class="m-0 max-h-64 list-none divide-y divide-[var(--border)] overflow-y-auto border-t border-[var(--border)] p-0 [scrollbar-gutter:stable]">
                  <li v-for="batch in currentBatches" :key="batch.batchNumber" :data-testid="`ai-batch-${batch.batchNumber}`" class="grid min-h-10 grid-cols-[3.5rem_5.5rem_minmax(0,1fr)_auto] items-center gap-2 px-4 py-2">
                    <strong class="type-label tabular-nums text-[var(--text)]">{{ t('ai.batchNumber', { number: batch.batchNumber }) }}</strong>
                    <UBadge :color="batchTone(batch.status)" variant="subtle" size="sm" :label="t(`ai.batchStatus.${batch.status}`)" class="justify-self-start" />
                    <span class="type-metadata min-w-0 truncate text-[var(--text-muted)]">{{ t('ai.batchItems', { count: batch.itemCount }) }} · {{ t(batch.status === 'retrying' ? 'ai.batchRetryAttempt' : 'ai.batchAttempt', { attempt: batch.attemptCount }) }}</span>
                    <time class="type-metadata tabular-nums text-[var(--text-muted)]">{{ formatDuration(batch.elapsedMs) }}</time>
                    <p v-if="batch.lastError" class="type-metadata col-start-2 col-end-5 m-0 truncate text-[var(--danger)]" :title="batch.lastError.safeMessage">{{ batch.lastError.safeMessage }}</p>
                    <p v-if="batch.usage" class="type-caption col-start-2 col-end-5 m-0 truncate tabular-nums text-[var(--text-muted)]" :title="t('ai.batchUsage', { input: formatNumber(batch.usage.inputTokens), cached: formatNumber(batch.usage.cachedInputTokens), output: formatNumber(batch.usage.outputTokens), reasoning: formatNumber(batch.usage.reasoningTokens), total: formatNumber(batch.usage.totalTokens) })">{{ t('ai.batchUsage', { input: formatNumber(batch.usage.inputTokens), cached: formatNumber(batch.usage.cachedInputTokens), output: formatNumber(batch.usage.outputTokens), reasoning: formatNumber(batch.usage.reasoningTokens), total: formatNumber(batch.usage.totalTokens) }) }}</p>
                  </li>
                </ol>
              </template>
            </UCollapsible>
          </div>

          <div v-else class="flex min-h-44 items-center gap-3 border-y border-[var(--border)] py-6 text-[var(--text-muted)]">
            <UIcon name="i-tabler-list-check" class="size-7 shrink-0" aria-hidden="true" />
            <div><strong class="type-body text-[var(--text)]">{{ t('ai.tasks.emptyTitle') }}</strong><p class="type-metadata mb-0 mt-1">{{ t('ai.tasks.emptyDescription') }}</p></div>
          </div>
        </section>
      </template>

      <template #statistics>
        <section data-testid="translation-task-statistics" :aria-label="t('ai.tasks.tabs.statistics')">
          <div class="mb-3"><h2 class="type-section-title m-0 text-[var(--text)]">{{ t('ai.tasks.modelStatsTitle') }}</h2><p class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">{{ t('ai.tasks.modelStatsDescription') }}</p></div>
          <div v-if="modelStats.length" class="overflow-hidden rounded-[var(--radius-surface)] border border-[var(--border)] bg-[var(--surface)]">
            <div class="grid grid-cols-[minmax(11rem,1fr)_5rem_6rem_6rem_10rem] gap-3 border-b border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2 type-label text-[var(--text-muted)]">
              <span>{{ t('ai.tasks.model') }}</span><span>{{ t('ai.tasks.tasks') }}</span><span>{{ t('ai.tasks.texts') }}</span><span>{{ t('ai.tasks.time') }}</span><span>{{ t('ai.tasks.totalTokens') }}</span>
            </div>
            <div v-for="group in modelStats" :key="group.key" class="grid min-h-12 grid-cols-[minmax(11rem,1fr)_5rem_6rem_6rem_10rem] items-center gap-3 border-b border-[var(--border)] px-3 py-2 last:border-b-0">
              <div class="min-w-0"><strong class="block truncate text-[11px] text-[var(--text)]">{{ group.modelId }}</strong><span class="type-caption block truncate text-[var(--text-muted)]">{{ group.profileName }} · {{ reasoningLabel(group.reasoningEffort) }}</span></div>
              <span class="type-metadata tabular-nums text-[var(--text-secondary)]">{{ formatNumber(group.tasks) }}</span>
              <span class="type-metadata tabular-nums text-[var(--text-secondary)]">{{ formatNumber(group.texts) }}</span>
              <span class="type-metadata tabular-nums text-[var(--text-secondary)]">{{ formatDuration(group.elapsedMs) }}</span>
              <span class="type-metadata tabular-nums text-[var(--text-secondary)]"><strong class="block font-semibold text-[var(--text)]">{{ group.usageAvailable ? formatNumber(group.usage.totalTokens) : t('ai.tasks.notReported') }}</strong><small v-if="group.usageAvailable" class="type-caption block truncate text-[var(--text-muted)]">{{ t('ai.tasks.usageBreakdown', { input: formatNumber(group.usage.inputTokens), cached: formatNumber(group.usage.cachedInputTokens), output: formatNumber(group.usage.outputTokens), reasoning: formatNumber(group.usage.reasoningTokens) }) }}</small></span>
            </div>
          </div>
          <p v-else class="type-metadata border-y border-[var(--border)] py-5 text-[var(--text-muted)]">{{ t('ai.tasks.noModelStats') }}</p>
        </section>
      </template>

      <template #list>
        <section data-testid="translation-task-list" :aria-label="t('ai.tasks.tabs.list')">
          <div class="mb-3 flex flex-wrap items-end justify-between gap-3">
            <div><h2 class="type-section-title m-0 text-[var(--text)]">{{ t('ai.tasks.historyTitle') }}</h2><p class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">{{ t('ai.tasks.historyDescription') }}</p></div>
            <span class="type-caption tabular-nums text-[var(--text-muted)]">{{ t('ai.tasks.taskCount', { visible: filteredHistory.length, total: history.length }) }}</span>
          </div>
          <div class="mb-3 grid grid-cols-[minmax(0,1fr)_160px] gap-2 max-[620px]:grid-cols-1">
            <UInput v-model="historyQuery" icon="i-tabler-search" size="sm" :placeholder="t('ai.tasks.searchTasks')" :aria-label="t('ai.tasks.searchTasks')" class="w-full" />
            <USelect v-model="historyStatus" :items="historyStatusItems" value-key="value" label-key="label" :aria-label="t('ai.tasks.filterStatus')" class="w-full" />
          </div>
          <div v-if="filteredHistory.length" class="overflow-hidden rounded-[var(--radius-surface)] border border-[var(--border)] bg-[var(--surface)]">
            <div class="grid grid-cols-[7rem_6rem_minmax(10rem,1fr)_6rem_6rem_10rem_2.5rem] gap-3 border-b border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2 type-label text-[var(--text-muted)]">
              <span>{{ t('ai.tasks.started') }}</span><span>{{ t('ai.tasks.source') }}</span><span>{{ t('ai.tasks.model') }}</span><span>{{ t('ai.tasks.texts') }}</span><span>{{ t('ai.tasks.time') }}</span><span>{{ t('ai.tasks.totalTokens') }}</span><span></span>
            </div>
            <div v-for="record in filteredHistory" :key="record.recordId" class="border-b border-[var(--border)] last:border-b-0">
              <div class="grid min-h-12 grid-cols-[7rem_6rem_minmax(10rem,1fr)_6rem_6rem_10rem_2.5rem] items-center gap-3 px-3 py-2">
                <time class="type-caption text-[var(--text-muted)]">{{ formatDate(record.startedAtMs) }}</time>
                <UBadge :color="statusTone(record.status)" variant="subtle" size="sm" :label="scopeLabel(record)" class="justify-self-start" />
                <div class="min-w-0"><strong class="block truncate text-[11px] text-[var(--text)]">{{ record.modelId }}</strong><span class="type-caption block truncate text-[var(--text-muted)]">{{ record.profileName }} · {{ reasoningLabel(record.reasoningEffort) }} · {{ t(`ai.tasks.status.${record.status}`) }}</span></div>
                <span class="type-metadata tabular-nums text-[var(--text-secondary)]">{{ formatNumber(record.appliedCount) }}/{{ formatNumber(record.totalCount) }}</span>
                <span class="type-metadata tabular-nums text-[var(--text-secondary)]">{{ formatDuration(record.elapsedMs) }}</span>
                <span class="type-metadata tabular-nums text-[var(--text-secondary)]"><strong class="block font-semibold text-[var(--text)]">{{ record.usage ? formatNumber(record.usage.totalTokens) : t('ai.tasks.notReported') }}</strong><small v-if="record.usage" class="type-caption block truncate text-[var(--text-muted)]">{{ t('ai.tasks.usageBreakdown', { input: formatNumber(record.usage.inputTokens), cached: formatNumber(record.usage.cachedInputTokens), output: formatNumber(record.usage.outputTokens), reasoning: formatNumber(record.usage.reasoningTokens) }) }}</small></span>
                <UButton color="neutral" variant="ghost" size="xs" :icon="expandedRecordId === record.recordId ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'" :aria-label="expandedRecordId === record.recordId ? t('ai.tasks.hideTaskDetails', { model: record.modelId }) : t('ai.tasks.viewTaskDetails', { model: record.modelId })" :aria-expanded="expandedRecordId === record.recordId" @click="toggleRecord(record.recordId)" />
              </div>
              <div v-if="expandedRecordId === record.recordId" class="border-t border-[var(--border)] bg-[var(--surface-subtle)] px-4 py-3" role="region" :aria-label="t('ai.tasks.taskDetailsFor', { model: record.modelId })">
                <p class="type-metadata m-0 text-[var(--text-muted)]">{{ t('ai.tasks.requestSummary', { attempts: record.requestAttempts, retries: record.retryAttempts, peak: record.peakConcurrency, written: record.appliedCount, skipped: record.skippedCount }) }}</p>
                <p v-if="record.usage" class="type-metadata m-0 mt-1 tabular-nums text-[var(--text-muted)]">{{ t('ai.batchUsage', { input: formatNumber(record.usage.inputTokens), cached: formatNumber(record.usage.cachedInputTokens), output: formatNumber(record.usage.outputTokens), reasoning: formatNumber(record.usage.reasoningTokens), total: formatNumber(record.usage.totalTokens) }) }}</p>
                <p v-else class="type-metadata m-0 mt-1 text-[var(--text-muted)]">{{ t('ai.tasks.usageNotReported') }}</p>
                <ol class="m-0 mt-3 max-h-48 list-none divide-y divide-[var(--border)] overflow-y-auto border-y border-[var(--border)] p-0 [scrollbar-gutter:stable]">
                  <li v-for="batch in orderedBatches(record.batches)" :key="batch.batchNumber" class="grid min-h-9 grid-cols-[3.5rem_5.5rem_minmax(0,1fr)_auto] items-center gap-2 px-2 py-1.5">
                    <strong class="type-label tabular-nums text-[var(--text)]">{{ t('ai.batchNumber', { number: batch.batchNumber }) }}</strong>
                    <UBadge :color="batchTone(batch.status)" variant="subtle" size="sm" :label="t(`ai.batchStatus.${batch.status}`)" class="justify-self-start" />
                    <span class="type-metadata min-w-0 truncate text-[var(--text-muted)]">{{ t('ai.batchItems', { count: batch.itemCount }) }} · {{ t('ai.batchAttempt', { attempt: batch.attemptCount }) }}</span>
                    <time class="type-metadata tabular-nums text-[var(--text-muted)]">{{ formatDuration(batch.elapsedMs) }}</time>
                    <p v-if="batch.lastError" class="type-metadata col-start-2 col-end-5 m-0 truncate text-[var(--danger)]" :title="batch.lastError.safeMessage">{{ batch.lastError.safeMessage }}</p>
                  </li>
                </ol>
              </div>
            </div>
          </div>
          <p v-else class="type-metadata border-y border-[var(--border)] py-5 text-[var(--text-muted)]">{{ history.length ? t('ai.tasks.noTaskMatch') : t('ai.tasks.noHistory') }}</p>
        </section>
      </template>
    </UTabs>
  </UtilityPageShell>
</template>
