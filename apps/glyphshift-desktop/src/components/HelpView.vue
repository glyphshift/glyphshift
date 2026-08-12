<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AdapterOption } from '../model'

defineProps<{ adapters: AdapterOption[] }>()
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'software' | 'dictionaries' | 'capture' | 'settings']
}>()

const { t } = useI18n()
const openingDocumentationId = ref<string | null>(null)
const documentationError = ref<string | null>(null)
const expandedAdapterId = ref<string | null>(null)
const recoveryItems = computed(() => ([
  {
    id: 'software-not-running',
    icon: 'i-tabler-player-play',
    title: t('help.recovery.softwareNotRunning.title'),
    description: t('help.recovery.softwareNotRunning.description'),
    action: t('help.recovery.softwareNotRunning.action'),
    view: 'software' as const,
  },
  {
    id: 'privilege-mismatch',
    icon: 'i-tabler-shield-lock',
    title: t('help.recovery.privilegeMismatch.title'),
    description: t('help.recovery.privilegeMismatch.description'),
    action: t('help.recovery.privilegeMismatch.action'),
    view: 'settings' as const,
  },
  {
    id: 'no-observed-text',
    icon: 'i-tabler-radar-off',
    title: t('help.recovery.noObservedText.title'),
    description: t('help.recovery.noObservedText.description'),
    action: t('help.recovery.noObservedText.action'),
    view: 'capture' as const,
  },
]))

function platformLabel(value: string) {
  if (value === 'windows') return 'Windows'
  if (value === 'macos') return 'macOS'
  return value
}

function featureLabel(value: string) {
  return ['textObserve', 'textReplace', 'fontSubstitute', 'layoutAdjust', 'resourceReplace'].includes(value)
    ? t(`help.features.${value}`)
    : value
}

function toggleAdapterDetails(id: string) {
  expandedAdapterId.value = expandedAdapterId.value === id ? null : id
}

async function openDocumentation(adapter: AdapterOption) {
  if (!adapter.documentationUrl || openingDocumentationId.value) return
  documentationError.value = null
  openingDocumentationId.value = adapter.id
  try {
    if ('__TAURI_INTERNALS__' in window) await openUrl(adapter.documentationUrl)
    else window.open(adapter.documentationUrl, '_blank', 'noopener,noreferrer')
  }
  catch {
    documentationError.value = t('help.documentationOpenFailed')
  }
  finally {
    openingDocumentationId.value = null
  }
}
</script>

<template>
  <UtilityPageShell
      title-id="help-title"
      :title="t('help.title')"
      :description="t('help.description')"
      content-test-id="help-layout"
  >
        <section data-testid="help-section-recovery" class="@container" aria-labelledby="help-recovery-title">
          <h2 id="help-recovery-title" class="type-section-title m-0 font-semibold">{{ t('help.recoveryTitle') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[72ch] leading-4 text-[var(--text-muted)]">{{ t('help.recoveryDescription') }}</p>
          <ul class="m-0 mt-3 divide-y divide-[var(--border)] border-y border-[var(--border)] p-0" role="list">
            <li v-for="item in recoveryItems" :key="item.id" class="grid min-h-[68px] list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5 @max-[680px]:grid-cols-[32px_minmax(0,1fr)]">
              <span class="grid size-8 place-items-center rounded-[6px] bg-[var(--surface-subtle)] text-[var(--text-secondary)]" aria-hidden="true">
                <UIcon :name="item.icon" class="size-4" />
              </span>
              <div class="min-w-0">
                <h3 class="type-body m-0 font-semibold">{{ item.title }}</h3>
                <p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ item.description }}</p>
              </div>
              <UButton color="neutral" variant="outline" size="sm" trailing-icon="i-tabler-arrow-right" :label="item.action" class="@max-[680px]:col-start-2 @max-[680px]:justify-self-start" @click="emit('navigate', item.view)" />
            </li>
          </ul>
        </section>

        <section class="mt-6 grid grid-cols-[minmax(0,1fr)_auto] items-start gap-6 border-y border-[var(--border)] py-4" aria-labelledby="help-model-title">
          <div>
            <h2 id="help-model-title" class="type-section-title m-0 font-semibold">{{ t('help.modelTitle') }}</h2>
            <p class="type-metadata mb-0 mt-1 max-w-[75ch] leading-5 text-[var(--text-muted)]">{{ t('help.modelDescription') }}</p>
          </div>
          <div class="flex flex-wrap justify-end gap-2">
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-git-branch" :label="t('help.openWorkflows')" @click="emit('navigate', 'workflows')" />
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-book-2" :label="t('help.openDictionaries')" @click="emit('navigate', 'dictionaries')" />
          </div>
        </section>

        <section class="mt-6" aria-labelledby="adapter-help-title">
          <div class="mb-3 flex items-end justify-between gap-4">
            <div>
              <h2 id="adapter-help-title" class="type-section-title m-0 font-semibold">{{ t('help.adaptersTitle') }}</h2>
              <p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ t('help.adaptersDescription') }}</p>
            </div>
            <span class="type-caption shrink-0 tabular-nums text-[var(--text-muted)]">{{ t('help.availableCount', { count: adapters.length }) }}</span>
          </div>

          <p v-if="documentationError" class="type-metadata mb-2 mt-0 text-[var(--danger)]" role="alert">{{ documentationError }}</p>

          <div v-if="adapters.length" data-testid="help-adapter-list" class="@container overflow-hidden rounded-[7px] border border-[var(--border)] bg-[var(--surface)]">
            <ul class="m-0 p-0" role="list">
              <li v-for="(adapter, index) in adapters" :key="adapter.id" data-testid="help-adapter-item" class="list-none border-b border-[var(--border)] last:border-b-0">
                <div class="grid grid-cols-[minmax(0,1.35fr)_minmax(0,.85fr)_minmax(0,1fr)_auto] items-start gap-4 px-4 py-3 @max-[900px]:grid-cols-[minmax(0,1fr)_auto]">
                  <div class="min-w-0">
                    <h3 class="type-body m-0 font-semibold">{{ adapter.name }}</h3>
                    <p class="type-metadata mb-0 mt-1 max-w-[58ch] leading-4 text-[var(--text-muted)]">{{ adapter.summary }}</p>
                  </div>
                  <div class="min-w-0 @max-[900px]:col-start-1">
                    <div class="type-caption mb-1.5 text-[var(--text-muted)]">{{ t('help.appliesTo') }}</div>
                    <div class="flex flex-wrap gap-1">
                      <UBadge v-for="platform in adapter.platforms" :key="platform" color="neutral" variant="soft" size="sm" :label="platformLabel(platform)" />
                      <UBadge v-for="technology in adapter.technologies" :key="technology" color="neutral" variant="outline" size="sm" :label="technology" />
                    </div>
                  </div>
                  <div class="min-w-0 @max-[900px]:col-start-1">
                    <div class="type-caption mb-1.5 text-[var(--text-muted)]">{{ t('help.capabilities') }}</div>
                    <div class="flex flex-wrap gap-1">
                      <UBadge v-for="feature in adapter.features" :key="feature" color="neutral" variant="soft" size="sm" :label="featureLabel(feature)" />
                    </div>
                  </div>
                  <UButton
                    color="neutral"
                    variant="ghost"
                    size="xs"
                    :icon="expandedAdapterId === adapter.id ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'"
                    :label="expandedAdapterId === adapter.id ? t('help.hideDetails') : t('help.showDetails')"
                    :aria-label="expandedAdapterId === adapter.id ? t('help.hideDetailsFor', { name: adapter.name }) : t('help.showDetailsFor', { name: adapter.name })"
                    :aria-expanded="expandedAdapterId === adapter.id"
                    :aria-controls="`help-adapter-details-${index}`"
                    class="justify-self-end @max-[900px]:col-start-2 @max-[900px]:row-start-1"
                    @click="toggleAdapterDetails(adapter.id)"
                  />
                </div>

                <div
                  v-if="expandedAdapterId === adapter.id"
                  :id="`help-adapter-details-${index}`"
                  role="region"
                  :aria-label="t('help.detailsFor', { name: adapter.name })"
                  class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-6 border-t border-[var(--border)] bg-[var(--surface-subtle)] px-4 py-3 @max-[640px]:grid-cols-1"
                >
                  <dl class="m-0 grid grid-cols-2 gap-6">
                    <div>
                      <dt class="type-caption text-[var(--text-muted)]">{{ t('help.columns.version') }}</dt>
                      <dd class="type-label m-0 mt-1 tabular-nums">v{{ adapter.version }}</dd>
                    </div>
                    <div>
                      <dt class="type-caption text-[var(--text-muted)]">{{ t('help.columns.configuration') }}</dt>
                      <dd class="type-label m-0 mt-1">{{ adapter.configuration === 'none' ? t('help.noConfiguration') : adapter.configuration }}</dd>
                    </div>
                  </dl>
                  <UButton
                    v-if="adapter.documentationUrl"
                    color="neutral"
                    variant="outline"
                    size="sm"
                    icon="i-tabler-external-link"
                    :label="t('help.openDocumentation')"
                    :aria-label="t('help.openDocumentationFor', { name: adapter.name })"
                    :loading="openingDocumentationId === adapter.id"
                    @click="openDocumentation(adapter)"
                  />
                  <span v-else class="type-metadata text-[var(--text-muted)]">{{ t('help.documentationUnavailable') }}</span>
                </div>
              </li>
            </ul>
          </div>
          <UEmpty v-else icon="i-tabler-plug-off" :title="t('help.emptyTitle')" :description="t('help.emptyDescription')" />
        </section>
  </UtilityPageShell>
</template>
