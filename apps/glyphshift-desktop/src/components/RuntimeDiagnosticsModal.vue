<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import { translateCommandError } from '../commandError'
import type {
  WorkflowRuntimeDiagnostics,
  WorkflowRuntimeTrace,
  WorkflowSummary,
} from '../model'

const props = defineProps<{
  open: boolean
  workflow: WorkflowSummary | null
}>()
const emit = defineEmits<{ 'update:open': [value: boolean] }>()
const { t } = useI18n()

const records = ref<WorkflowRuntimeTrace[]>([])
const dropped = ref(0)
const query = ref('')
const loading = ref(false)
const error = ref('')
const diagnosticsEnabled = ref(false)
let timer: ReturnType<typeof setInterval> | undefined
let session = 0

const title = computed(() => t('workflows.diagnostics.title', { name: props.workflow?.name ?? '' }))
const filteredRecords = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  if (!needle) return records.value
  return records.value.filter(record => (
    `${record.sourceText} ${record.softwareName} ${record.adapterName} ${record.status}`
      .toLocaleLowerCase()
      .includes(needle)
  ))
})
const columns = computed<TableColumn<WorkflowRuntimeTrace>[]>(() => [
  { id: 'source', header: t('workflows.diagnostics.columns.source'), meta: { class: { th: 'w-[31%]', td: 'w-[31%]' } } },
  { id: 'origin', header: t('workflows.diagnostics.columns.origin'), meta: { class: { th: 'w-[24%]', td: 'w-[24%]' } } },
  { id: 'decision', header: t('workflows.diagnostics.columns.decision'), meta: { class: { th: 'w-[25%]', td: 'w-[25%]' } } },
  { id: 'publication', header: t('workflows.diagnostics.columns.publication'), meta: { class: { th: 'w-[20%]', td: 'w-[20%]' } } },
])

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function shortIdentity(identity: string) {
  return identity.slice(0, 8)
}

function statusLabel(record: WorkflowRuntimeTrace) {
  return t(`workflows.diagnostics.status.${record.status}`)
}

function statusColor(record: WorkflowRuntimeTrace): 'primary' | 'error' | 'neutral' {
  if (record.status === 'matched') return 'primary'
  if (record.status === 'no_match' || record.status === 'context_recorded') return 'neutral'
  return 'error'
}

async function refresh() {
  if (!props.open || !props.workflow || loading.value) return
  loading.value = true
  try {
    const batch = hasDesktopRuntime()
      ? await invoke<WorkflowRuntimeDiagnostics>('desktop_workflow_diagnostics', { workflowId: props.workflow.id })
      : { workflowId: props.workflow.id, records: [], dropped: 0 }
    records.value = [...records.value, ...batch.records].slice(-256)
    dropped.value += batch.dropped
    error.value = ''
  }
  catch (cause) {
    error.value = translateCommandError(cause)
    if (timer) clearInterval(timer)
    timer = undefined
  }
  finally {
    loading.value = false
  }
}

async function start() {
  const currentSession = ++session
  records.value = []
  dropped.value = 0
  query.value = ''
  error.value = ''
  if (!props.workflow) return
  try {
    if (hasDesktopRuntime()) {
      await invoke('desktop_control_workflow_diagnostics', {
        workflowId: props.workflow.id,
        enabled: true,
      })
    }
    if (currentSession !== session || !props.open) {
      if (hasDesktopRuntime()) {
        await invoke('desktop_control_workflow_diagnostics', {
          workflowId: props.workflow.id,
          enabled: false,
        })
      }
      return
    }
    diagnosticsEnabled.value = true
    await refresh()
    if (currentSession === session && props.open) timer = setInterval(refresh, 1000)
  }
  catch (cause) {
    error.value = translateCommandError(cause)
  }
}

async function stop() {
  ++session
  if (timer) clearInterval(timer)
  timer = undefined
  const workflowId = props.workflow?.id
  const shouldDisable = diagnosticsEnabled.value
  diagnosticsEnabled.value = false
  if (shouldDisable && workflowId && hasDesktopRuntime()) {
    try {
      await invoke('desktop_control_workflow_diagnostics', { workflowId, enabled: false })
    }
    catch (cause) {
      error.value = translateCommandError(cause)
      return
    }
  }
  emit('update:open', false)
}

watch(() => props.open, (open) => {
  if (open) void start()
  else if (diagnosticsEnabled.value) void stop()
})
onBeforeUnmount(() => {
  ++session
  if (timer) clearInterval(timer)
  if (diagnosticsEnabled.value && props.workflow && hasDesktopRuntime()) {
    void invoke('desktop_control_workflow_diagnostics', {
      workflowId: props.workflow.id,
      enabled: false,
    })
  }
})
</script>

<template>
  <UModal
    :open="open"
    :title="title"
    :description="t('workflows.diagnostics.description')"
    :ui="{
      content: 'flex max-h-[calc(100dvh-32px)] max-w-[900px] flex-col',
      header: 'min-h-0 shrink-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'mt-1 text-[10px] leading-4',
      body: 'flex min-h-0 flex-1 flex-col gap-3 px-5 py-4',
      footer: 'shrink-0 px-5 py-4',
    }"
    @update:open="$event || stop()"
  >
    <template #body>
      <UAlert v-if="error" role="alert" color="error" variant="soft" :title="t('workflows.diagnostics.error')" :description="error">
        <template #actions>
          <UButton color="error" variant="outline" size="xs" :label="t('workflows.diagnostics.retry')" @click="refresh" />
        </template>
      </UAlert>
      <UAlert v-if="dropped" color="warning" variant="soft" icon="i-tabler-alert-triangle" :description="t('workflows.diagnostics.dropped', { count: dropped })" />

      <div class="flex items-center gap-2">
        <UInput v-model="query" icon="i-tabler-search" size="sm" class="min-w-0 flex-1" :placeholder="t('workflows.diagnostics.search')" :aria-label="t('workflows.diagnostics.search')" />
        <span class="shrink-0 text-[9px] text-[var(--text-muted)]" aria-live="polite">{{ t('workflows.diagnostics.recentCount', { count: records.length }) }}</span>
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="t('workflows.diagnostics.refresh')" :loading="loading" @click="refresh" />
      </div>

      <div class="min-h-[260px] flex-1 overflow-auto rounded-[7px] border border-[var(--border)] bg-[var(--surface)] [scrollbar-gutter:stable]">
        <UTable :data="filteredRecords" :columns="columns" sticky :ui="{ base: 'min-w-[760px]' }">
          <template #source-cell="{ row }">
            <div class="line-clamp-2 break-words font-medium" :title="row.original.sourceText">{{ row.original.sourceText }}</div>
          </template>
          <template #origin-cell="{ row }">
            <div class="truncate font-medium" :title="row.original.softwareName">{{ row.original.softwareName }}</div>
            <div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]" :title="row.original.adapterName">{{ row.original.adapterName }}</div>
          </template>
          <template #decision-cell="{ row }">
            <div class="flex flex-wrap gap-1">
              <UBadge :color="statusColor(row.original)" variant="soft" size="sm" :label="statusLabel(row.original)" />
              <UBadge v-if="row.original.text === 'replaced'" color="primary" variant="outline" size="sm" :label="t('workflows.diagnostics.text.replaced')" />
              <UBadge v-if="row.original.font !== 'unmatched'" :color="row.original.font === 'substituted' ? 'primary' : 'neutral'" variant="outline" size="sm" :label="t(`workflows.diagnostics.font.${row.original.font}`)" />
            </div>
          </template>
          <template #publication-cell="{ row }">
            <div class="font-medium tabular-nums" :title="row.original.publicationIdentity">G{{ row.original.generation }} · {{ shortIdentity(row.original.publicationIdentity) }}</div>
            <div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('workflows.diagnostics.publicationIdentity') }}</div>
          </template>
          <template #empty>
            <UEmpty icon="i-tabler-activity-heartbeat" :title="query ? t('workflows.diagnostics.noMatch') : t('workflows.diagnostics.waiting')" :description="query ? t('workflows.diagnostics.noMatchDescription') : t('workflows.diagnostics.waitingDescription')" />
          </template>
        </UTable>
      </div>
    </template>
    <template #footer>
      <span class="text-[9px] text-[var(--text-muted)]">{{ t('workflows.diagnostics.autoRefresh') }}</span>
      <UButton class="ml-auto" color="neutral" variant="outline" size="sm" :label="t('workflows.diagnostics.close')" @click="stop" />
    </template>
  </UModal>
</template>
