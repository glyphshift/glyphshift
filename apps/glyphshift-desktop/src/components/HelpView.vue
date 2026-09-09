<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { version as appVersion } from '../../package.json'
import type { AdapterOption } from '../model'
import { adapterSummary } from '../adapterPresentation'

type HelpTab = 'guide' | 'ai' | 'recovery' | 'compatibility' | 'about'
type HelpTarget = 'workflows' | 'software' | 'dictionaries' | 'capture' | 'translation-tasks' | 'settings'
type AboutLinkId = 'github' | 'bilibili'

defineProps<{ adapters: AdapterOption[] }>()
const emit = defineEmits<{ navigate: [view: HelpTarget] }>()

const { t } = useI18n()
const activeTab = ref<HelpTab>('guide')
const openingDocumentationId = ref<string | null>(null)
const documentationError = ref<string | null>(null)
const expandedAdapterId = ref<string | null>(null)
const openingAboutLinkId = ref<AboutLinkId | null>(null)
const aboutLinkError = ref<string | null>(null)

const helpTabs = computed(() => [
  { value: 'guide' as const, slot: 'guide', label: t('help.tabs.guide'), icon: 'i-tabler-route' },
  { value: 'ai' as const, slot: 'ai', label: t('help.tabs.ai'), icon: 'i-tabler-sparkles' },
  { value: 'recovery' as const, slot: 'recovery', label: t('help.tabs.recovery'), icon: 'i-tabler-lifebuoy' },
  { value: 'compatibility' as const, slot: 'compatibility', label: t('help.tabs.compatibility'), icon: 'i-tabler-plug-connected' },
  { value: 'about' as const, slot: 'about', label: t('help.tabs.about'), icon: 'i-tabler-info-circle' },
])
const aboutLinks = computed(() => [
  { id: 'github' as const, label: t('help.about.github'), icon: 'i-tabler-brand-github', url: 'https://github.com/Yuelioi/glyphshift' },
  { id: 'bilibili' as const, label: t('help.about.bilibili'), icon: 'i-tabler-brand-bilibili', url: 'https://space.bilibili.com/4279370' },
])
const gettingStartedItems = computed(() => ([
  { id: 'add-software', title: t('help.guide.steps.addSoftware.title'), description: t('help.guide.steps.addSoftware.description'), action: t('help.guide.steps.addSoftware.action'), view: 'workflows' as const },
  { id: 'create-probe', title: t('help.guide.steps.createProbe.title'), description: t('help.guide.steps.createProbe.description'), action: t('help.guide.steps.createProbe.action'), view: 'workflows' as const },
  { id: 'collect-text', title: t('help.guide.steps.collectText.title'), description: t('help.guide.steps.collectText.description'), action: t('help.guide.steps.collectText.action'), view: 'workflows' as const },
  { id: 'translate', title: t('help.guide.steps.translate.title'), description: t('help.guide.steps.translate.description'), action: t('help.guide.steps.translate.action'), view: 'dictionaries' as const },
  { id: 'verify', title: t('help.guide.steps.verify.title'), description: t('help.guide.steps.verify.description'), action: t('help.guide.steps.verify.action'), view: 'workflows' as const },
  { id: 'enable-workflow', title: t('help.guide.steps.enableWorkflow.title'), description: t('help.guide.steps.enableWorkflow.description'), action: t('help.guide.steps.enableWorkflow.action'), view: 'workflows' as const },
]))
const maintenanceItems = computed(() => ([
  { id: 'dictionary', icon: 'i-tabler-language', title: t('help.guide.maintenance.dictionary.title'), description: t('help.guide.maintenance.dictionary.description'), action: t('help.guide.maintenance.dictionary.action'), view: 'dictionaries' as const },
  { id: 'recheck', icon: 'i-tabler-radar', title: t('help.guide.maintenance.recheck.title'), description: t('help.guide.maintenance.recheck.description'), action: t('help.guide.maintenance.recheck.action'), view: 'workflows' as const },
  { id: 'tasks', icon: 'i-tabler-list-check', title: t('help.guide.maintenance.tasks.title'), description: t('help.guide.maintenance.tasks.description'), action: t('help.guide.maintenance.tasks.action'), view: 'translation-tasks' as const },
]))
const aiSteps = computed(() => ([
  { id: 'profile', title: t('help.aiGuide.steps.profile.title'), description: t('help.aiGuide.steps.profile.description'), action: t('help.aiGuide.steps.profile.action'), view: 'settings' as const },
  { id: 'prepare', title: t('help.aiGuide.steps.prepare.title'), description: t('help.aiGuide.steps.prepare.description'), action: t('help.aiGuide.steps.prepare.action'), view: 'dictionaries' as const },
  { id: 'run', title: t('help.aiGuide.steps.run.title'), description: t('help.aiGuide.steps.run.description'), action: t('help.aiGuide.steps.run.action'), view: 'dictionaries' as const },
  { id: 'review', title: t('help.aiGuide.steps.review.title'), description: t('help.aiGuide.steps.review.description'), action: t('help.aiGuide.steps.review.action'), view: 'translation-tasks' as const },
]))
const usageTerms = computed(() => [
  { id: 'input', term: t('help.aiGuide.usage.input.term'), description: t('help.aiGuide.usage.input.description') },
  { id: 'cached', term: t('help.aiGuide.usage.cached.term'), description: t('help.aiGuide.usage.cached.description') },
  { id: 'output', term: t('help.aiGuide.usage.output.term'), description: t('help.aiGuide.usage.output.description') },
  { id: 'reasoning', term: t('help.aiGuide.usage.reasoning.term'), description: t('help.aiGuide.usage.reasoning.description') },
  { id: 'total', term: t('help.aiGuide.usage.total.term'), description: t('help.aiGuide.usage.total.description') },
])
const recoveryItems = computed(() => ([
  { id: 'software-not-running', icon: 'i-tabler-library', title: t('help.recovery.softwareNotRunning.title'), description: t('help.recovery.softwareNotRunning.description'), action: t('help.recovery.softwareNotRunning.action'), view: 'workflows' as const },
  { id: 'privilege-mismatch', icon: 'i-tabler-shield-lock', title: t('help.recovery.privilegeMismatch.title'), description: t('help.recovery.privilegeMismatch.description'), action: t('help.recovery.privilegeMismatch.action'), view: 'settings' as const },
  { id: 'no-observed-text', icon: 'i-tabler-radar-off', title: t('help.recovery.noObservedText.title'), description: t('help.recovery.noObservedText.description'), action: t('help.recovery.noObservedText.action'), view: 'workflows' as const },
  { id: 'ai-unavailable', icon: 'i-tabler-language-off', title: t('help.recovery.aiUnavailable.title'), description: t('help.recovery.aiUnavailable.description'), action: t('help.recovery.aiUnavailable.action'), view: 'settings' as const },
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

async function openAboutLink(link: { id: AboutLinkId; url: string }) {
  if (openingAboutLinkId.value) return
  aboutLinkError.value = null
  openingAboutLinkId.value = link.id
  try {
    if ('__TAURI_INTERNALS__' in window) await openUrl(link.url)
    else window.open(link.url, '_blank', 'noopener,noreferrer')
  }
  catch {
    aboutLinkError.value = t('help.about.openFailed')
  }
  finally {
    openingAboutLinkId.value = null
  }
}
</script>

<template>
  <UtilityPageShell
    title-id="help-title"
    :title="t('help.title')"
    icon="i-tabler-help-circle"
    content-test-id="help-layout"
  >
    <UTabs
      v-model="activeTab"
      data-testid="help-tabs"
      :items="helpTabs"
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
        content: 'pt-5 focus:outline-none',
      }"
    >
      <template #guide>
        <section data-testid="help-section-getting-started" class="help-card @container" aria-labelledby="help-getting-started-title">
          <h2 id="help-getting-started-title" class="type-section-title m-0 font-semibold">{{ t('help.guide.title') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[74ch] leading-4 text-[var(--text-muted)]">{{ t('help.guide.description') }}</p>
          <ol class="m-0 mt-3 divide-y divide-[var(--border)] border-t border-[var(--border)] p-0">
            <li v-for="(item, index) in gettingStartedItems" :key="item.id" class="grid min-h-[72px] list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-3 @max-[680px]:grid-cols-[32px_minmax(0,1fr)]">
              <span class="grid size-8 place-items-center rounded-[6px] bg-[var(--accent-soft)] type-label font-semibold tabular-nums text-[var(--accent-strong)]" aria-hidden="true">{{ index + 1 }}</span>
              <div class="min-w-0">
                <h3 class="type-body m-0 font-semibold">{{ item.title }}</h3>
                <p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ item.description }}</p>
              </div>
              <UButton color="primary" variant="soft" size="sm" trailing-icon="i-tabler-arrow-right" :label="item.action" class="@max-[680px]:col-start-2 @max-[680px]:justify-self-start" @click="emit('navigate', item.view)" />
            </li>
          </ol>
        </section>

        <section class="help-card @container mt-4" aria-labelledby="help-maintenance-title">
          <h2 id="help-maintenance-title" class="type-section-title m-0 font-semibold">{{ t('help.guide.maintenanceTitle') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[74ch] leading-4 text-[var(--text-muted)]">{{ t('help.guide.maintenanceDescription') }}</p>
          <ul class="m-0 mt-3 divide-y divide-[var(--border)] border-t border-[var(--border)] p-0" role="list">
            <li v-for="item in maintenanceItems" :key="item.id" class="grid min-h-[64px] list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5 @max-[680px]:grid-cols-[32px_minmax(0,1fr)]">
              <UIcon :name="item.icon" class="size-4 justify-self-center text-[var(--text-secondary)]" aria-hidden="true" />
              <div class="min-w-0"><h3 class="type-body m-0 font-semibold">{{ item.title }}</h3><p class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">{{ item.description }}</p></div>
              <UButton color="neutral" variant="outline" size="sm" trailing-icon="i-tabler-arrow-right" :label="item.action" class="@max-[680px]:col-start-2 @max-[680px]:justify-self-start" @click="emit('navigate', item.view)" />
            </li>
          </ul>
        </section>

        <div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <section v-for="topic in ['dictionary', 'probe']" :key="topic" class="help-card" :aria-labelledby="`help-${topic}-title`">
            <h2 :id="`help-${topic}-title`" class="type-section-title m-0 font-semibold">{{ t(`help.updates.${topic}.title`) }}</h2>
            <p class="type-body mb-0 mt-3 leading-6 text-[var(--text-secondary)]">{{ t(`help.updates.${topic}.description`) }}</p>
          </section>
        </div>
      </template>

      <template #ai>
        <section data-testid="help-section-ai" class="help-card @container" aria-labelledby="help-ai-title">
          <h2 id="help-ai-title" class="type-section-title m-0 font-semibold">{{ t('help.aiGuide.title') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[74ch] leading-4 text-[var(--text-muted)]">{{ t('help.aiGuide.description') }}</p>
          <ol class="m-0 mt-3 divide-y divide-[var(--border)] border-t border-[var(--border)] p-0">
            <li v-for="(item, index) in aiSteps" :key="item.id" class="grid min-h-[72px] list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-3 @max-[680px]:grid-cols-[32px_minmax(0,1fr)]">
              <span class="grid size-8 place-items-center rounded-[6px] bg-[var(--accent-soft)] type-label font-semibold tabular-nums text-[var(--accent-strong)]" aria-hidden="true">{{ index + 1 }}</span>
              <div class="min-w-0"><h3 class="type-body m-0 font-semibold">{{ item.title }}</h3><p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ item.description }}</p></div>
              <UButton color="primary" variant="soft" size="sm" trailing-icon="i-tabler-arrow-right" :label="item.action" class="@max-[680px]:col-start-2 @max-[680px]:justify-self-start" @click="emit('navigate', item.view)" />
            </li>
          </ol>
        </section>


        <div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <section v-for="topic in ['automatic', 'profiles']" :key="topic" class="help-card" :aria-labelledby="`help-${topic}-title`">
            <h2 :id="`help-${topic}-title`" class="type-section-title m-0 font-semibold">{{ t(`help.updates.${topic}.title`) }}</h2>
            <p class="type-body mb-0 mt-3 leading-6 text-[var(--text-secondary)]">{{ t(`help.updates.${topic}.description`) }}</p>
          </section>
        </div>
        <section class="help-card mt-4" aria-labelledby="help-ai-usage-title">
          <h2 id="help-ai-usage-title" class="type-section-title m-0 font-semibold">{{ t('help.aiGuide.usageTitle') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[74ch] leading-4 text-[var(--text-muted)]">{{ t('help.aiGuide.usageDescription') }}</p>
          <dl class="m-0 mt-3 divide-y divide-[var(--border)] border-t border-[var(--border)]">
            <div v-for="item in usageTerms" :key="item.id" class="grid grid-cols-[128px_minmax(0,1fr)] gap-4 px-3 py-2.5 max-[560px]:grid-cols-1 max-[560px]:gap-1">
              <dt class="type-label font-semibold text-[var(--text)]">{{ item.term }}</dt>
              <dd class="type-metadata m-0 text-[var(--text-muted)]">{{ item.description }}</dd>
            </div>
          </dl>
        </section>
      </template>

      <template #recovery>
        <section data-testid="help-section-recovery" class="help-card @container" aria-labelledby="help-recovery-title">
          <h2 id="help-recovery-title" class="type-section-title m-0 font-semibold">{{ t('help.recoveryTitle') }}</h2>
          <p class="type-metadata mb-0 mt-1 max-w-[74ch] leading-4 text-[var(--text-muted)]">{{ t('help.recoveryDescription') }}</p>
          <ul class="m-0 mt-3 divide-y divide-[var(--border)] border-t border-[var(--border)] p-0" role="list">
            <li v-for="item in recoveryItems" :key="item.id" class="grid min-h-[68px] list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5 @max-[680px]:grid-cols-[32px_minmax(0,1fr)]">
              <span class="grid size-8 place-items-center rounded-[6px] bg-[var(--surface-subtle)] text-[var(--text-secondary)]" aria-hidden="true"><UIcon :name="item.icon" class="size-4" /></span>
              <div class="min-w-0"><h3 class="type-body m-0 font-semibold">{{ item.title }}</h3><p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ item.description }}</p></div>
              <UButton color="neutral" variant="outline" size="sm" trailing-icon="i-tabler-arrow-right" :label="item.action" class="@max-[680px]:col-start-2 @max-[680px]:justify-self-start" @click="emit('navigate', item.view)" />
            </li>
          </ul>
        </section>

        <div class="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <section v-for="topic in ['refresh', 'experimental']" :key="topic" class="help-card" :aria-labelledby="`help-${topic}-title`">
            <h2 :id="`help-${topic}-title`" class="type-section-title m-0 font-semibold">{{ t(`help.updates.${topic}.title`) }}</h2>
            <p class="type-body mb-0 mt-3 leading-6 text-[var(--text-secondary)]">{{ t(`help.updates.${topic}.description`) }}</p>
          </section>
        </div>
      </template>

      <template #compatibility>
        <section data-testid="help-section-compatibility" aria-labelledby="adapter-help-title">
          <div class="mb-3 flex items-end justify-between gap-4">
            <div><h2 id="adapter-help-title" class="type-section-title m-0 font-semibold">{{ t('help.adaptersTitle') }}</h2><p class="type-metadata mb-0 mt-1 max-w-[76ch] leading-4 text-[var(--text-muted)]">{{ t('help.adaptersDescription') }}</p></div>
            <span class="type-caption shrink-0 tabular-nums text-[var(--text-muted)]">{{ t('help.availableCount', { count: adapters.length }) }}</span>
          </div>

          <p v-if="documentationError" class="type-metadata mb-2 mt-0 text-[var(--danger)]" role="alert">{{ documentationError }}</p>
          <div v-if="adapters.length" data-testid="help-adapter-list" class="@container overflow-hidden rounded-[7px] border border-[var(--border)] bg-[var(--surface)]">
            <ul class="m-0 p-0" role="list">
              <li v-for="(adapter, index) in adapters" :key="adapter.id" data-testid="help-adapter-item" class="list-none border-b border-[var(--border)] last:border-b-0">
                <div class="grid grid-cols-[minmax(0,1.35fr)_minmax(0,.85fr)_minmax(0,1fr)_auto] items-start gap-4 px-4 py-3 @max-[900px]:grid-cols-[minmax(0,1fr)_auto]">
                  <div class="min-w-0"><h3 class="type-body m-0 font-semibold">{{ adapter.name }}</h3><p class="type-metadata mb-0 mt-1 max-w-[58ch] leading-4 text-[var(--text-muted)]">{{ adapterSummary(adapter, t) }}</p></div>
                  <div class="min-w-0 @max-[900px]:col-start-1"><div class="type-caption mb-1.5 text-[var(--text-muted)]">{{ t('help.appliesTo') }}</div><div class="flex flex-wrap gap-1"><UBadge v-for="platform in adapter.platforms" :key="platform" color="neutral" variant="soft" size="sm" :label="platformLabel(platform)" /><UBadge v-for="technology in adapter.technologies" :key="technology" color="neutral" variant="outline" size="sm" :label="technology" /></div></div>
                  <div class="min-w-0 @max-[900px]:col-start-1"><div class="type-caption mb-1.5 text-[var(--text-muted)]">{{ t('help.capabilities') }}</div><div class="flex flex-wrap gap-1"><UBadge v-for="feature in adapter.features" :key="feature" color="neutral" variant="soft" size="sm" :label="featureLabel(feature)" /></div></div>
                  <UButton :title="expandedAdapterId === adapter.id ? t('help.hideDetailsFor', { name: adapter.name }) : t('help.showDetailsFor', { name: adapter.name })" color="neutral" variant="ghost" size="xs" :icon="expandedAdapterId === adapter.id ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'" :label="expandedAdapterId === adapter.id ? t('help.hideDetails') : t('help.showDetails')" :aria-label="expandedAdapterId === adapter.id ? t('help.hideDetailsFor', { name: adapter.name }) : t('help.showDetailsFor', { name: adapter.name })" :aria-expanded="expandedAdapterId === adapter.id" :aria-controls="`help-adapter-details-${index}`" class="justify-self-end @max-[900px]:col-start-2 @max-[900px]:row-start-1" @click="toggleAdapterDetails(adapter.id)" />
                </div>
                <div v-if="expandedAdapterId === adapter.id" :id="`help-adapter-details-${index}`" role="region" :aria-label="t('help.detailsFor', { name: adapter.name })" class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-6 border-t border-[var(--border)] bg-[var(--surface-subtle)] px-4 py-3 @max-[640px]:grid-cols-1">
                  <dl class="m-0 grid grid-cols-2 gap-6"><div><dt class="type-caption text-[var(--text-muted)]">{{ t('help.columns.version') }}</dt><dd class="type-label m-0 mt-1 tabular-nums">v{{ adapter.version }}</dd></div><div><dt class="type-caption text-[var(--text-muted)]">{{ t('help.columns.configuration') }}</dt><dd class="type-label m-0 mt-1">{{ adapter.configuration === 'none' ? t('help.noConfiguration') : adapter.configuration }}</dd></div></dl>
                  <UButton :title="t('help.openDocumentationFor', { name: adapter.name })" v-if="adapter.documentationUrl" color="neutral" variant="outline" size="sm" icon="i-tabler-external-link" :label="t('help.openDocumentation')" :aria-label="t('help.openDocumentationFor', { name: adapter.name })" :loading="openingDocumentationId === adapter.id" @click="openDocumentation(adapter)" />
                  <span v-else class="type-metadata text-[var(--text-muted)]">{{ t('help.documentationUnavailable') }}</span>
                </div>
              </li>
            </ul>
          </div>
          <UEmpty v-else icon="i-tabler-plug-off" :title="t('help.emptyTitle')" :description="t('help.emptyDescription')" />
        </section>
      </template>

      <template #about>
        <section data-testid="help-section-about" class="help-card @container" aria-labelledby="help-about-title">
          <div class="flex items-center gap-3 border-b border-[var(--border)] pb-5">
            <span class="grid size-10 shrink-0 place-items-center rounded-[7px] border border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--accent-strong)]" aria-hidden="true">
              <UIcon name="i-tabler-info-circle" class="size-5" />
            </span>
            <div class="min-w-0">
              <h2 id="help-about-title" class="type-section-title m-0 font-semibold">Glyphshift</h2>
              <p class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">{{ t('help.about.version', { version: appVersion }) }}</p>
            </div>
          </div>

          <p class="type-body mt-4 text-[var(--text-secondary)]">{{ t('help.updates.license') }}</p>
          <p v-if="aboutLinkError" class="type-metadata mb-2 mt-4 text-[var(--danger)]" role="alert">{{ aboutLinkError }}</p>
          <ul class="m-0 divide-y divide-[var(--border)] border-b border-[var(--border)] p-0" role="list">
            <li v-for="link in aboutLinks" :key="link.id" class="grid min-h-16 list-none grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 px-3 py-3 @max-[640px]:grid-cols-[32px_minmax(0,1fr)]">
              <UIcon :name="link.icon" class="size-5 justify-self-center text-[var(--text-secondary)]" aria-hidden="true" />
              <div class="min-w-0">
                <h3 class="type-body m-0 font-semibold">{{ link.label }}</h3>
                <p class="type-metadata mb-0 mt-1 truncate text-[var(--text-muted)]" :title="link.url">{{ link.url }}</p>
              </div>
              <UButton :title="t('help.about.openNamed', { name: link.label })" color="neutral" variant="outline" size="sm" icon="i-tabler-external-link" :label="t('help.about.open')" :aria-label="t('help.about.openNamed', { name: link.label })" :loading="openingAboutLinkId === link.id" class="@max-[640px]:col-start-2 @max-[640px]:justify-self-start" @click="openAboutLink(link)" />
            </li>
          </ul>
        </section>
      </template>
    </UTabs>
  </UtilityPageShell>
</template>

<style scoped>
.help-card {
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  padding: 20px;
  min-width: 0;
}
</style>
