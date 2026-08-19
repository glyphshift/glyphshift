<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui'
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
import { managementActionsColumnMeta, managementIdentityColumnMeta } from '../tableInteraction'
import { useTableColumns } from '../useTableColumns'
import { useWorkspace } from '../useWorkspace'

type TaskTab = 'current' | 'statistics' | 'list'

const { t, locale } = useI18n()
const ai = useAiTranslation()
const workspace = useWorkspace()
const activeTab = ref<TaskTab>('current')
const batchDetailsOpen = ref(false)
const statisticsQuery = ref('')
const statisticsProfile = ref('all')
const statisticsPage = ref(1)
const statisticsPageSize = ref(20)
const historyQuery = ref('')
const historyStatus = ref('all')
const historyPage = ref(1)
const historyPageSize = ref(20)
const expandedHistory = ref<Record<string, boolean>>({})
const { columns: statisticsVisibleColumns, toggleColumn: toggleStatisticsColumn } = useTableColumns('glyphshift.table-columns.ai-task-statistics.v2', {
  protocol: false,
  reasoning: true,
  tasks: true,
  texts: true,
  time: true,
  tokens: true,
})
const { columns: historyVisibleColumns, toggleColumn: toggleHistoryColumn } = useTableColumns('glyphshift.table-columns.ai-task-history', {
  started: true,
  source: true,
  status: true,
  texts: true,
  time: true,
  tokens: true,
})

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
const historyPageItems = computed(() => filteredHistory.value.slice(
  (historyPage.value - 1) * historyPageSize.value,
  historyPage.value * historyPageSize.value,
))
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
const statisticsProfileItems = computed(() => [
  { value: 'all', label: t('ai.tasks.profileFilter.all') },
  ...[...new Set(modelStats.value.map(group => group.profileName))]
    .sort((left, right) => left.localeCompare(right, locale.value))
    .map(profileName => ({ value: profileName, label: profileName })),
])
const filteredModelStats = computed(() => {
  const needle = statisticsQuery.value.trim().toLocaleLowerCase()
  return modelStats.value.filter(group => (
    (statisticsProfile.value === 'all' || group.profileName === statisticsProfile.value)
    && (!needle || `${group.profileName} ${group.modelId} ${reasoningLabel(group.reasoningEffort)}`.toLocaleLowerCase().includes(needle))
  ))
})
const statisticsPageItems = computed(() => filteredModelStats.value.slice(
  (statisticsPage.value - 1) * statisticsPageSize.value,
  statisticsPage.value * statisticsPageSize.value,
))
const statisticsColumnOptions = computed(() => [
  { key: 'protocol', label: t('ai.tasks.columns.protocol'), visible: statisticsVisibleColumns.value.protocol, defaultVisible: false },
  { key: 'reasoning', label: t('ai.tasks.columns.reasoning'), visible: statisticsVisibleColumns.value.reasoning },
  { key: 'tasks', label: t('ai.tasks.columns.tasks'), visible: statisticsVisibleColumns.value.tasks },
  { key: 'texts', label: t('ai.tasks.columns.texts'), visible: statisticsVisibleColumns.value.texts },
  { key: 'time', label: t('ai.tasks.columns.time'), visible: statisticsVisibleColumns.value.time },
  { key: 'tokens', label: t('ai.tasks.columns.tokens'), visible: statisticsVisibleColumns.value.tokens },
])
const historyColumnOptions = computed(() => [
  { key: 'started', label: t('ai.tasks.columns.started'), visible: historyVisibleColumns.value.started },
  { key: 'source', label: t('ai.tasks.columns.source'), visible: historyVisibleColumns.value.source },
  { key: 'status', label: t('ai.tasks.columns.status'), visible: historyVisibleColumns.value.status },
  { key: 'texts', label: t('ai.tasks.columns.texts'), visible: historyVisibleColumns.value.texts },
  { key: 'time', label: t('ai.tasks.columns.time'), visible: historyVisibleColumns.value.time },
  { key: 'tokens', label: t('ai.tasks.columns.tokens'), visible: historyVisibleColumns.value.tokens },
])
const statisticsColumns = computed<TableColumn<ModelAggregate>[]>(() => [
  { id: 'model', header: t('ai.tasks.columns.model'), meta: managementIdentityColumnMeta('w-52', false) },
  ...(statisticsVisibleColumns.value.protocol ? [{ id: 'protocol', header: t('ai.tasks.columns.protocol'), meta: { class: { th: 'w-36', td: 'w-36' } } } satisfies TableColumn<ModelAggregate>] : []),
  ...(statisticsVisibleColumns.value.reasoning ? [{ id: 'reasoning', header: t('ai.tasks.columns.reasoning'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<ModelAggregate>] : []),
  ...(statisticsVisibleColumns.value.tasks ? [{ accessorKey: 'tasks', header: t('ai.tasks.columns.tasks'), meta: { class: { th: 'w-20 text-right', td: 'w-20 text-right' } } } satisfies TableColumn<ModelAggregate>] : []),
  ...(statisticsVisibleColumns.value.texts ? [{ accessorKey: 'texts', header: t('ai.tasks.columns.texts'), meta: { class: { th: 'w-24 text-right', td: 'w-24 text-right' } } } satisfies TableColumn<ModelAggregate>] : []),
  ...(statisticsVisibleColumns.value.time ? [{ accessorKey: 'elapsedMs', header: t('ai.tasks.columns.time'), meta: { class: { th: 'w-24 text-right', td: 'w-24 text-right' } } } satisfies TableColumn<ModelAggregate>] : []),
  ...(statisticsVisibleColumns.value.tokens ? [{ id: 'tokens', header: t('ai.tasks.columns.tokens'), meta: { class: { th: 'w-40 text-right', td: 'w-40 text-right' } } } satisfies TableColumn<ModelAggregate>] : []),
])
const historyColumns = computed<TableColumn<AiTranslationRunRecord>[]>(() => [
  { id: 'model', header: t('ai.tasks.columns.model'), meta: managementIdentityColumnMeta('w-52', false) },
  ...(historyVisibleColumns.value.started ? [{ accessorKey: 'startedAtMs', header: t('ai.tasks.columns.started'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  ...(historyVisibleColumns.value.source ? [{ id: 'source', header: t('ai.tasks.columns.source'), meta: { class: { th: 'w-24', td: 'w-24' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  ...(historyVisibleColumns.value.status ? [{ accessorKey: 'status', header: t('ai.tasks.columns.status'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  ...(historyVisibleColumns.value.texts ? [{ id: 'texts', header: t('ai.tasks.columns.texts'), meta: { class: { th: 'w-24 text-right', td: 'w-24 text-right' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  ...(historyVisibleColumns.value.time ? [{ accessorKey: 'elapsedMs', header: t('ai.tasks.columns.time'), meta: { class: { th: 'w-24 text-right', td: 'w-24 text-right' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  ...(historyVisibleColumns.value.tokens ? [{ id: 'tokens', header: t('ai.tasks.columns.tokens'), meta: { class: { th: 'w-40 text-right', td: 'w-40 text-right' } } } satisfies TableColumn<AiTranslationRunRecord>] : []),
  { id: 'actions', header: t('ai.tasks.columns.actions'), meta: managementActionsColumnMeta('w-20') },
])

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

function protocolLabel(protocol: AiProviderProtocol) {
  const labels: Record<AiProviderProtocol, string> = {
    codex_subscription: 'codexSubscription',
    open_ai_responses: 'openAiResponses',
    open_ai_chat_completions: 'openAiChat',
    open_ai_compatible: 'openAiCompatible',
    anthropic_messages: 'anthropic',
    gemini_generate_content: 'gemini',
    ollama_chat: 'ollama',
  }
  return t(`ai.protocol.${labels[protocol]}`)
}

function historyRowId(record: AiTranslationRunRecord) {
  return record.recordId
}

watch(() => current.value?.jobId, () => {
  batchDetailsOpen.value = false
})
watch(() => current.value?.batches.map(batch => batch.status).join(','), (statuses) => {
  if (statuses?.includes('retrying') || statuses?.includes('failed')) batchDetailsOpen.value = true
}, { immediate: true })
watch([statisticsQuery, statisticsProfile, statisticsPageSize], () => { statisticsPage.value = 1 })
watch([historyQuery, historyStatus, historyPageSize], () => {
  historyPage.value = 1
  expandedHistory.value = {}
})

onMounted(() => void ai.connectTaskMonitor().catch(() => undefined))
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="translation-tasks-title">
    <ManagementPageHeader
      title-id="translation-tasks-title"
      :title="t('ai.tasks.title')"
      icon="i-tabler-list-check"
    />
    <div data-testid="translation-tasks-content" class="min-h-0 flex-1">
    <UTabs
      v-model="activeTab"
      data-testid="translation-task-tabs"
      :items="taskTabs"
      color="neutral"
      variant="link"
      size="sm"
      activation-mode="manual"
      class="flex h-full min-h-0 w-full flex-col"
      :ui="{
        list: 'w-full justify-start gap-1 rounded-none border-b border-[var(--border)] bg-transparent p-0',
        indicator: 'hidden',
        trigger: 'type-label relative h-10 flex-none gap-2 rounded-none px-3 text-[var(--text-secondary)] after:absolute after:inset-x-2 after:bottom-0 after:hidden after:h-0.5 after:bg-[var(--accent)] hover:bg-[var(--surface-hover)] data-[state=active]:font-semibold data-[state=active]:!text-[var(--text)] data-[state=active]:after:block',
        leadingIcon: 'size-4 shrink-0',
        trailingBadge: 'type-caption ml-1 min-w-4 justify-center px-1',
        content: 'min-h-0 flex-1 pt-3 focus:outline-none',
      }"
    >
      <template #current>
        <section data-testid="translation-task-current" class="h-full min-h-0 overflow-y-auto [scrollbar-gutter:stable]" :aria-label="t('ai.tasks.tabs.current')">
          <ManagementWorkspaceSurface v-if="current">
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
          </ManagementWorkspaceSurface>

          <ManagementWorkspaceSurface v-else>
            <UEmpty
              icon="i-tabler-list-check"
              :title="t('ai.tasks.emptyTitle')"
              :description="t('ai.tasks.emptyDescription')"
              class="h-full min-h-80 bg-[var(--surface-inset)]"
            />
          </ManagementWorkspaceSurface>
        </section>
      </template>

      <template #statistics>
        <section data-testid="translation-task-statistics" class="flex h-full min-h-0 flex-col" :aria-label="t('ai.tasks.tabs.statistics')">
          <ManagementTableFrame
            v-model:query="statisticsQuery"
            v-model:filter-value="statisticsProfile"
            v-model:page="statisticsPage"
            v-model:page-size="statisticsPageSize"
            :search-placeholder="t('ai.tasks.searchStatistics')"
            :search-label="t('ai.tasks.searchStatistics')"
            :filter-label="statisticsProfileItems.find(item => item.value === statisticsProfile)?.label ?? t('ai.tasks.profileFilter.all')"
            :filter-aria-label="t('ai.tasks.filterProfile')"
            :filter-options="statisticsProfileItems"
            :column-options="statisticsColumnOptions"
            :columns-label="t('table.columns')"
            :total="filteredModelStats.length"
            :item-label="t('ai.tasks.statisticsItemLabel')"
            @toggle-column="toggleStatisticsColumn"
          >
            <UTable
              data-testid="translation-statistics-table"
              role="region"
              tabindex="0"
              aria-labelledby="translation-tasks-title"
              :data="statisticsPageItems"
              :columns="statisticsColumns"
              sticky
              class="management-table-scroll"
              :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[760px]' }"
            >
              <template #model-cell="{ row }">
                <div class="min-w-0"><strong class="block truncate text-[var(--text)]">{{ row.original.modelId }}</strong><span class="type-metadata mt-0.5 block truncate text-[var(--text-muted)]">{{ row.original.profileName }}</span></div>
              </template>
              <template #protocol-cell="{ row }"><span class="text-[var(--text-secondary)]">{{ protocolLabel(row.original.protocol) }}</span></template>
              <template #reasoning-cell="{ row }"><span class="text-[var(--text-secondary)]">{{ reasoningLabel(row.original.reasoningEffort) }}</span></template>
              <template #tasks-cell="{ row }"><span class="tabular-nums">{{ formatNumber(row.original.tasks) }}</span></template>
              <template #texts-cell="{ row }"><span class="tabular-nums">{{ formatNumber(row.original.texts) }}</span></template>
              <template #elapsedMs-cell="{ row }"><span class="tabular-nums">{{ formatDuration(row.original.elapsedMs) }}</span></template>
              <template #tokens-cell="{ row }">
                <div class="min-w-0 text-right tabular-nums"><strong class="block font-semibold text-[var(--text)]">{{ row.original.usageAvailable ? formatNumber(row.original.usage.totalTokens) : t('ai.tasks.notReported') }}</strong><span v-if="row.original.usageAvailable" class="type-caption block truncate text-[var(--text-muted)]" :title="t('ai.batchUsage', { input: formatNumber(row.original.usage.inputTokens), cached: formatNumber(row.original.usage.cachedInputTokens), output: formatNumber(row.original.usage.outputTokens), reasoning: formatNumber(row.original.usage.reasoningTokens), total: formatNumber(row.original.usage.totalTokens) })">{{ t('ai.tasks.usageBreakdown', { input: formatNumber(row.original.usage.inputTokens), cached: formatNumber(row.original.usage.cachedInputTokens), output: formatNumber(row.original.usage.outputTokens), reasoning: formatNumber(row.original.usage.reasoningTokens) }) }}</span></div>
              </template>
              <template #empty>
                <UEmpty icon="i-tabler-chart-bar" :title="modelStats.length ? t('ai.tasks.noStatisticsMatch') : t('ai.tasks.noModelStats')" :description="modelStats.length ? t('ai.tasks.adjustStatisticsFilters') : undefined" />
              </template>
            </UTable>
          </ManagementTableFrame>
        </section>
      </template>

      <template #list>
        <section data-testid="translation-task-list" class="flex h-full min-h-0 flex-col" :aria-label="t('ai.tasks.tabs.list')">
          <ManagementTableFrame
            v-model:query="historyQuery"
            v-model:filter-value="historyStatus"
            v-model:page="historyPage"
            v-model:page-size="historyPageSize"
            :search-placeholder="t('ai.tasks.searchTasks')"
            :search-label="t('ai.tasks.searchTasks')"
            :filter-label="historyStatusItems.find(item => item.value === historyStatus)?.label ?? t('ai.tasks.statusFilter.all')"
            :filter-aria-label="t('ai.tasks.filterStatus')"
            :filter-options="historyStatusItems"
            :column-options="historyColumnOptions"
            :columns-label="t('table.columns')"
            :total="filteredHistory.length"
            :item-label="t('ai.tasks.taskItemLabel')"
            @toggle-column="toggleHistoryColumn"
          >
            <UTable
              v-model:expanded="expandedHistory"
              data-testid="translation-history-table"
              role="region"
              tabindex="0"
              aria-labelledby="translation-tasks-title"
              :data="historyPageItems"
              :columns="historyColumns"
              :get-row-id="historyRowId"
              sticky
              class="management-table-scroll"
              :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[900px]' }"
            >
              <template #model-cell="{ row }">
                <div class="min-w-0"><strong class="block truncate text-[var(--text)]">{{ row.original.modelId }}</strong><span class="type-metadata mt-0.5 block truncate text-[var(--text-muted)]">{{ row.original.profileName }} · {{ reasoningLabel(row.original.reasoningEffort) }}</span></div>
              </template>
              <template #startedAtMs-cell="{ row }"><time class="type-metadata whitespace-nowrap text-[var(--text-muted)]">{{ formatDate(row.original.startedAtMs) }}</time></template>
              <template #source-cell="{ row }"><span class="text-[var(--text-secondary)]">{{ scopeLabel(row.original) }}</span></template>
              <template #status-cell="{ row }"><UBadge :color="statusTone(row.original.status)" variant="subtle" size="sm" :label="t(`ai.tasks.status.${row.original.status}`)" /></template>
              <template #texts-cell="{ row }"><span class="tabular-nums">{{ formatNumber(row.original.appliedCount) }}/{{ formatNumber(row.original.totalCount) }}</span></template>
              <template #elapsedMs-cell="{ row }"><span class="tabular-nums">{{ formatDuration(row.original.elapsedMs) }}</span></template>
              <template #tokens-cell="{ row }">
                <div class="min-w-0 text-right tabular-nums"><strong class="block font-semibold text-[var(--text)]">{{ row.original.usage ? formatNumber(row.original.usage.totalTokens) : t('ai.tasks.notReported') }}</strong><span v-if="row.original.usage" class="type-caption block truncate text-[var(--text-muted)]" :title="t('ai.batchUsage', { input: formatNumber(row.original.usage.inputTokens), cached: formatNumber(row.original.usage.cachedInputTokens), output: formatNumber(row.original.usage.outputTokens), reasoning: formatNumber(row.original.usage.reasoningTokens), total: formatNumber(row.original.usage.totalTokens) })">{{ t('ai.tasks.usageBreakdown', { input: formatNumber(row.original.usage.inputTokens), cached: formatNumber(row.original.usage.cachedInputTokens), output: formatNumber(row.original.usage.outputTokens), reasoning: formatNumber(row.original.usage.reasoningTokens) }) }}</span></div>
              </template>
              <template #actions-cell="{ row }">
                <UButton color="neutral" variant="ghost" size="xs" :icon="row.getIsExpanded() ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'" :aria-label="row.getIsExpanded() ? t('ai.tasks.hideTaskDetails', { model: row.original.modelId }) : t('ai.tasks.viewTaskDetails', { model: row.original.modelId })" :aria-expanded="row.getIsExpanded()" @click="row.toggleExpanded()" />
              </template>
              <template #expanded="{ row }">
                <div class="bg-[var(--surface-subtle)] px-4 py-3" role="region" :aria-label="t('ai.tasks.taskDetailsFor', { model: row.original.modelId })">
                  <p class="type-metadata m-0 text-[var(--text-muted)]">{{ t('ai.tasks.requestSummary', { attempts: row.original.requestAttempts, retries: row.original.retryAttempts, peak: row.original.peakConcurrency, written: row.original.appliedCount, skipped: row.original.skippedCount }) }}</p>
                  <p v-if="row.original.usage" class="type-metadata m-0 mt-1 tabular-nums text-[var(--text-muted)]">{{ t('ai.batchUsage', { input: formatNumber(row.original.usage.inputTokens), cached: formatNumber(row.original.usage.cachedInputTokens), output: formatNumber(row.original.usage.outputTokens), reasoning: formatNumber(row.original.usage.reasoningTokens), total: formatNumber(row.original.usage.totalTokens) }) }}</p>
                  <p v-else class="type-metadata m-0 mt-1 text-[var(--text-muted)]">{{ t('ai.tasks.usageNotReported') }}</p>
                  <ol class="m-0 mt-3 max-h-48 list-none divide-y divide-[var(--border)] overflow-y-auto border-y border-[var(--border)] p-0 [scrollbar-gutter:stable]">
                    <li v-for="batch in orderedBatches(row.original.batches)" :key="batch.batchNumber" class="grid min-h-9 grid-cols-[3.5rem_5.5rem_minmax(0,1fr)_auto] items-center gap-2 px-2 py-1.5">
                      <strong class="type-label tabular-nums text-[var(--text)]">{{ t('ai.batchNumber', { number: batch.batchNumber }) }}</strong>
                      <UBadge :color="batchTone(batch.status)" variant="subtle" size="sm" :label="t(`ai.batchStatus.${batch.status}`)" class="justify-self-start" />
                      <span class="type-metadata min-w-0 truncate text-[var(--text-muted)]">{{ t('ai.batchItems', { count: batch.itemCount }) }} · {{ t('ai.batchAttempt', { attempt: batch.attemptCount }) }}</span>
                      <time class="type-metadata tabular-nums text-[var(--text-muted)]">{{ formatDuration(batch.elapsedMs) }}</time>
                      <p v-if="batch.lastError" class="type-metadata col-start-2 col-end-5 m-0 truncate text-[var(--danger)]" :title="batch.lastError.safeMessage">{{ batch.lastError.safeMessage }}</p>
                    </li>
                  </ol>
                </div>
              </template>
              <template #empty>
                <UEmpty icon="i-tabler-list-details" :title="history.length ? t('ai.tasks.noTaskMatch') : t('ai.tasks.noHistory')" :description="history.length ? t('ai.tasks.adjustTaskFilters') : undefined" />
              </template>
            </UTable>
          </ManagementTableFrame>
        </section>
      </template>
    </UTabs>
    </div>
  </section>
</template>
