<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toValue, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useGuidedTour, type GuidedTourStep } from '../useGuidedTour'

const props = withDefaults(defineProps<{
  open: boolean
  busy?: boolean
  error?: string
}>(), {
  busy: false,
  error: '',
})

const emit = defineEmits<{
  skip: []
  complete: []
  navigate: [section: string]
}>()
const { t } = useI18n()

type FirstRunStep = GuidedTourStep & {
  key: 'intro' | 'permissions' | 'ai' | 'rules' | 'recent' | 'fonts' | 'languages' | 'create' | 'software' | 'dictionary' | 'dictionaryPicker' | 'save' | 'start'
  section?: string
  title: string
  description: string
  interaction: 'button' | 'click' | 'observe'
  clickSelector?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}

const introReference = {
  getBoundingClientRect() {
    const card = document.querySelector<HTMLElement>('[data-testid="first-run-guide"]')?.closest<HTMLElement>('[data-slot="content"]')
    const height = card?.offsetHeight ?? 0
    return new DOMRect(window.innerWidth / 2, window.innerHeight * 0.4 - height / 2, 0, 0)
  },
}

const steps = computed<FirstRunStep[]>(() => [
  {
    key: 'intro',
    target: introReference,
    title: t('firstRun.guide.title'),
    description: t('firstRun.guide.description'),
    interaction: 'button',
  },
  ...[
    { key: 'permissions', section: 'general', target: '[data-tour="settings-admin"]' },
    { key: 'ai', section: 'ai', target: '.tour-settings-ai' },
    { key: 'rules', section: 'rules', target: '.tour-settings-rules' },
    { key: 'recent', section: 'software', target: '.tour-settings-software' },
    { key: 'fonts', section: 'fonts', target: '.tour-settings-fonts' },
    { key: 'languages', section: 'languages', target: '.tour-settings-languages' },
  ].map(step => ({
    ...step,
    key: step.key as FirstRunStep['key'],
    title: t(`firstRun.guide.steps.${step.key}.title`),
    description: t(`firstRun.guide.steps.${step.key}.description`),
    interaction: 'button' as const,
    side: 'bottom' as const,
  })),
  {
    key: 'create',
    target: '[data-tour="workflow-create"]',
    clickSelector: '[data-tour="workflow-create"]',
    title: t('firstRun.guide.steps.create.title'),
    description: t('firstRun.guide.steps.create.description'),
    interaction: 'click',
    side: 'bottom',
  },
  {
    key: 'software',
    target: '[data-tour="workflow-software-picker"]',
    title: t('firstRun.guide.steps.software.title'),
    description: t('firstRun.guide.steps.software.description'),
    interaction: 'observe',
    side: 'top',
  },
  {
    key: 'dictionary',
    target: '.tour-workflow-dictionary-tab',
    clickSelector: '.tour-workflow-dictionary-tab',
    title: t('firstRun.guide.steps.dictionary.title'),
    description: t('firstRun.guide.steps.dictionary.description'),
    interaction: 'click',
    side: 'right',
  },
  {
    key: 'dictionaryPicker',
    target: '[data-tour="workflow-dictionary-picker"]',
    title: t('firstRun.guide.steps.dictionaryPicker.title'),
    description: t('firstRun.guide.steps.dictionaryPicker.description'),
    interaction: 'observe',
    side: 'top',
  },
  {
    key: 'save',
    target: '[data-tour="workflow-save"]',
    clickSelector: '[data-tour="workflow-save"]',
    title: t('firstRun.guide.steps.save.title'),
    description: t('firstRun.guide.steps.save.description'),
    interaction: 'click',
    side: 'bottom',
  },
  {
    key: 'start',
    target: '[data-tour="workflow-start"]',
    clickSelector: '[data-tour="workflow-start"]',
    title: t('firstRun.guide.steps.start.title'),
    description: t('firstRun.guide.steps.start.description'),
    interaction: 'click',
    side: 'bottom',
  },
])

const tour = useGuidedTour(steps)
const rect = ref<DOMRectReadOnly | null>(null)
let mutationObserver: MutationObserver | null = null
let pendingIndex: number | null = null
let animationFrame = 0

const current = computed(() => tour.current.value as FirstRunStep | undefined)
const targetStep = computed(() => current.value?.key !== 'intro')
const clickStep = computed(() => current.value?.interaction === 'click')
const spotlightStyle = computed(() => rect.value ? {
  top: `${Math.max(2, rect.value.top - 8)}px`,
  left: `${Math.max(2, rect.value.left - 8)}px`,
  width: `${Math.min(window.innerWidth - 2, rect.value.right + 8) - Math.max(2, rect.value.left - 8)}px`,
  height: `${Math.min(window.innerHeight - 2, rect.value.bottom + 8) - Math.max(2, rect.value.top - 8)}px`,
} : {})
const cursorStyle = computed(() => rect.value ? {
  top: `${Math.min(window.innerHeight - 28, rect.value.bottom - 6)}px`,
  left: `${Math.min(window.innerWidth - 28, rect.value.right - 6)}px`,
} : {})

function resolveTarget(index: number) {
  const target = toValue(steps.value[index]?.target)
  if (target == null) return null
  if (typeof target === 'string') return document.querySelector(target)
  return target
}

function targetReady(index: number) {
  const target = resolveTarget(index)
  return index === 0 || Boolean(target && (!(target instanceof Element) || target.getBoundingClientRect().width > 0))
}

function refreshRect() {
  const reference = resolveTarget(tour.index.value)
  if (!targetStep.value || !reference) {
    rect.value = null
    return
  }
  const boundsTarget = reference instanceof Element
    && (current.value?.key === 'software' || current.value?.key === 'dictionaryPicker')
    ? reference.closest('[role="dialog"]') ?? reference
    : reference
  const next = boundsTarget.getBoundingClientRect()
  const previous = rect.value
  if (!previous || previous.x !== next.x || previous.y !== next.y
    || previous.width !== next.width || previous.height !== next.height) rect.value = next
}

// Track modal transitions and layout changes that do not emit resize or DOM mutations.
function trackLayout() {
  if (!props.open || !tour.open.value) {
    animationFrame = 0
    return
  }
  refreshRect()
  animationFrame = window.requestAnimationFrame(trackLayout)
}

function goTo(index: number) {
  pendingIndex = null
  tour.goTo(index)
  void nextTick(refreshRect)
}

function queueAdvance(index: number) {
  pendingIndex = index
  const step = steps.value[index]
  if (step?.section) emit('navigate', step.section)
  else if (step?.key === 'create') emit('navigate', 'workflows')
  void nextTick(checkPendingAdvance)
  checkPendingAdvance()
}

function checkPendingAdvance() {
  if (pendingIndex == null || !targetReady(pendingIndex)) return
  goTo(pendingIndex)
}

function checkObservedProgress() {
  if (!props.open || !tour.open.value) return
  if (current.value?.key === 'software') {
    const configured = document.querySelector('[data-testid="workflow-adapter-config"]')
    const pickerOpen = document.querySelector('[data-tour="workflow-software-picker"]')
    if (configured && !pickerOpen) queueAdvance(steps.value.findIndex(step => step.key === 'dictionary'))
  }
  if (current.value?.key === 'dictionaryPicker') {
    const save = document.querySelector('[data-tour="workflow-save"]:not([disabled])')
    const pickerOpen = document.querySelector('[data-tour="workflow-dictionary-picker"]')
    if (save && !pickerOpen) queueAdvance(steps.value.findIndex(step => step.key === 'save'))
  }
}

function onDocumentClick(event: MouseEvent) {
  if (!props.open || !tour.open.value || current.value?.interaction !== 'click') return
  const selector = current.value.clickSelector
  const target = event.target instanceof Element && selector ? event.target.closest(selector) : null
  if (!target) return
  if (current.value.key === 'start') {
    window.setTimeout(() => {
      tour.finish()
      emit('complete')
    }, 0)
    return
  }
  queueAdvance(tour.index.value + 1)
}

function beginInteractiveSteps() {
  queueAdvance(tour.index.value + 1)
}

function skipGuide() {
  tour.finish()
  emit('skip')
}

function startWhenReady() {
  if (!props.open) return
  tour.start(0)
  if (!animationFrame) animationFrame = window.requestAnimationFrame(trackLayout)
  void nextTick(refreshRect)
}

watch(() => props.open, value => {
  if (value) startWhenReady()
  else {
    pendingIndex = null
    tour.finish()
    rect.value = null
  }
}, { immediate: true })
watch(() => tour.index.value, () => void nextTick(refreshRect))

onMounted(() => {
  if (!animationFrame && props.open) animationFrame = window.requestAnimationFrame(trackLayout)
  document.addEventListener('click', onDocumentClick, true)
  window.addEventListener('resize', refreshRect)
  window.addEventListener('scroll', refreshRect, true)
  mutationObserver = new MutationObserver(() => {
    checkPendingAdvance()
    checkObservedProgress()
    refreshRect()
  })
  mutationObserver.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['style', 'data-state'] })
})

onBeforeUnmount(() => {
  window.cancelAnimationFrame(animationFrame)
  document.removeEventListener('click', onDocumentClick, true)
  window.removeEventListener('resize', refreshRect)
  window.removeEventListener('scroll', refreshRect, true)
  mutationObserver?.disconnect()
})
</script>

<template>
  <Teleport to="body">
  <div
    v-if="props.open && tour.open.value && targetStep && rect"
    data-testid="first-run-tour-spotlight"
    class="first-run-tour-spotlight pointer-events-none fixed z-[110] rounded-[8px] border-2 border-[var(--accent)]"
    :style="spotlightStyle"
    aria-hidden="true"
  />
  <div
    v-if="props.open && tour.open.value && clickStep && rect"
    class="first-run-tour-cursor pointer-events-none fixed z-[150] grid size-7 place-items-center rounded-full bg-[var(--accent)] text-white shadow-lg"
    :style="cursorStyle"
    aria-hidden="true"
  >
    <UIcon name="i-tabler-pointer" class="size-4" />
  </div>
  </Teleport>

  <UPopover
    :key="current?.key"
    :open="props.open && tour.open.value"
    :reference="tour.reference.value"
    :portal="true"
    :dismissible="false"
    :arrow="targetStep"
    :content="{
      side: current?.side ?? 'bottom',
      align: 'center',
      sideOffset: targetStep ? 12 : 0,
      collisionPadding: 16,
      prioritizePosition: true,
      updatePositionStrategy: current?.key === 'intro' ? 'always' : 'optimized',
    }"
    :ui="{ content: 'pointer-events-auto z-[140] w-[340px] max-w-[calc(100vw-32px)] p-0 shadow-xl' }"
  >
    <template #content>
      <section data-testid="first-run-guide" class="p-4" :aria-label="t('firstRun.guide.title')">
        <div class="flex items-start gap-3">
          <div class="grid size-8 shrink-0 place-items-center rounded-[7px] bg-[var(--accent-soft)] text-[var(--accent-strong)]">
            <UIcon :name="current?.key === 'intro' ? 'i-tabler-route' : 'i-tabler-pointer'" class="size-4.5" aria-hidden="true" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex items-center justify-between gap-3">
              <h2 class="m-0 type-section-title font-semibold text-[var(--text)]">{{ current?.title }}</h2>
              <span class="type-caption shrink-0 tabular-nums text-[var(--text-muted)]">{{ tour.index.value + 1 }} / {{ tour.total.value }}</span>
            </div>
            <p class="mb-0 mt-1.5 type-label leading-5 text-[var(--text-muted)]">{{ current?.description }}</p>
          </div>
        </div>

        <p v-if="props.error" role="alert" class="mb-0 mt-3 type-metadata leading-5 text-[var(--danger)]">{{ props.error }}</p>

        <div class="mt-4 flex items-center gap-2 border-t border-[var(--border)] pt-3">
          <UButton color="neutral" variant="ghost" size="xs" :label="t('firstRun.guide.skip')" :disabled="busy" @click="skipGuide" />
          <div class="ml-auto flex items-center gap-1.5 type-metadata text-[var(--text-muted)]">
            <template v-if="current?.interaction === 'click'">
              <UIcon name="i-tabler-pointer" class="size-3.5" aria-hidden="true" />
              <span>{{ t('firstRun.guide.clickToContinue') }}</span>
            </template>
            <template v-else-if="current?.interaction === 'observe'">
              <UIcon name="i-tabler-loader-2" class="size-3.5 animate-spin" aria-hidden="true" />
              <span>{{ t('firstRun.guide.completeToContinue') }}</span>
            </template>
            <UButton v-else color="primary" variant="solid" size="xs" :label="t('firstRun.guide.next')" @click="beginInteractiveSteps" />
          </div>
        </div>
      </section>
    </template>
  </UPopover>
</template>

<style scoped>
.first-run-tour-spotlight {
  box-shadow: 0 0 0 9999px rgb(4 8 15 / 48%);
}

.first-run-tour-cursor {
  animation: first-run-tour-cursor 900ms ease-in-out infinite alternate;
}

@keyframes first-run-tour-cursor {
  from { transform: translate(2px, 2px); }
  to { transform: translate(-3px, -3px); }
}

@media (prefers-reduced-motion: reduce) {
  .first-run-tour-cursor { animation: none; }
}
</style>
