<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import type { InteractiveTranslationBubblePresentation } from '../model'

const { t } = useI18n()
const presentation = ref<InteractiveTranslationBubblePresentation | null>(null)
const currentIndex = ref(0)
const copied = ref(false)
let unlistenNative: UnlistenFn | null = null
let copyReset: ReturnType<typeof setTimeout> | undefined

const blocks = computed(() => presentation.value?.result.blocks ?? [])
const currentBlock = computed(() => blocks.value[currentIndex.value])
const visualResult = computed(() => currentBlock.value?.provenance === 'visual')
const positionLabel = computed(() => t('interactiveTranslation.bubblePosition', {
  current: currentIndex.value + 1,
  total: blocks.value.length,
}))

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function receive(next: InteractiveTranslationBubblePresentation | null) {
  presentation.value = next
  currentIndex.value = Math.min(
    Math.max(next?.focusBlockIndex ?? 0, 0),
    Math.max((next?.result.blocks.length ?? 1) - 1, 0),
  )
  copied.value = false
}

function receiveBrowserEvent(event: Event) {
  receive((event as CustomEvent<InteractiveTranslationBubblePresentation>).detail)
}

function previous() {
  if (!blocks.value.length) return
  currentIndex.value = (currentIndex.value - 1 + blocks.value.length) % blocks.value.length
  copied.value = false
}

function next() {
  if (!blocks.value.length) return
  currentIndex.value = (currentIndex.value + 1) % blocks.value.length
  copied.value = false
}

async function copyCurrent() {
  const block = currentBlock.value
  if (!block) return
  const value = block.translation ? `${block.source}\n${block.translation}` : block.source
  try {
    await navigator.clipboard.writeText(value)
    copied.value = true
    if (copyReset) clearTimeout(copyReset)
    copyReset = setTimeout(() => { copied.value = false }, 1800)
  }
  catch {
    copied.value = false
  }
}

async function dismiss() {
  presentation.value = null
  if (hasDesktopRuntime()) {
    await invoke('desktop_dismiss_interactive_translation_bubble').catch(() => undefined)
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') void dismiss()
  if (event.key === 'ArrowLeft') previous()
  if (event.key === 'ArrowRight') next()
}

watch(presentation, next => {
  document.body.dataset.bubbleVisible = next ? 'true' : 'false'
})

onMounted(async () => {
  document.body.dataset.surface = 'translation-bubble'
  window.addEventListener('keydown', handleKeydown)
  window.addEventListener('glyphshift:interactive-translation-bubble', receiveBrowserEvent)
  if (!hasDesktopRuntime()) return
  const existing = await invoke<InteractiveTranslationBubblePresentation | null>(
    'desktop_interactive_translation_bubble',
  ).catch(() => null)
  if (existing) receive(existing)
  unlistenNative = await listen<InteractiveTranslationBubblePresentation>(
    'interactive-translation-bubble',
    event => receive(event.payload),
  )
})

onBeforeUnmount(() => {
  if (copyReset) clearTimeout(copyReset)
  unlistenNative?.()
  window.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('glyphshift:interactive-translation-bubble', receiveBrowserEvent)
  delete document.body.dataset.surface
  delete document.body.dataset.bubbleVisible
})
</script>

<template>
  <main
    v-if="presentation && currentBlock"
    class="translation-bubble"
    data-testid="interactive-translation-bubble"
    aria-live="polite"
  >
    <header class="bubble-header" data-tauri-drag-region>
      <span class="source-badge" :class="{ visual: visualResult }" data-tauri-drag-region>
        {{ t(visualResult ? 'interactiveTranslation.ocrSourceShort' : 'interactiveTranslation.uiaSourceShort') }}
      </span>
      <span class="position" data-tauri-drag-region>{{ positionLabel }}</span>
      <nav class="bubble-actions" :aria-label="t('interactiveTranslation.bubbleActions')">
        <button type="button" :aria-label="t('interactiveTranslation.bubblePrevious')" :disabled="blocks.length < 2" @click="previous">‹</button>
        <button type="button" :aria-label="t('interactiveTranslation.bubbleNext')" :disabled="blocks.length < 2" @click="next">›</button>
        <button type="button" class="copy-action" :aria-label="t('interactiveTranslation.copy')" @click="copyCurrent">
          {{ copied ? t('interactiveTranslation.copied') : t('interactiveTranslation.copy') }}
        </button>
        <button type="button" class="close-action" :aria-label="t('interactiveTranslation.bubbleClose')" @click="dismiss">×</button>
      </nav>
    </header>

    <section class="source-block">
      <span class="eyebrow">{{ t('interactiveTranslation.original') }}</span>
      <p>{{ currentBlock.source }}</p>
    </section>
    <section class="translation-block" :class="{ missing: currentBlock.translationState === 'missing' }">
      <span class="eyebrow">{{ t('interactiveTranslation.translation') }}</span>
      <p>{{ currentBlock.translation ?? t('interactiveTranslation.missing') }}</p>
    </section>
    <footer v-if="presentation.result.partial" class="partial-note">
      {{ t('interactiveTranslation.bubblePartial') }}
    </footer>
  </main>
</template>

<style scoped>
.translation-bubble {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  background: var(--surface);
  box-shadow: 0 18px 46px color-mix(in oklab, #07101f 24%, transparent);
}

.bubble-header {
  display: flex;
  height: 42px;
  align-items: center;
  gap: 8px;
  padding: 0 7px 0 12px;
  border-bottom: 1px solid var(--border);
  background: var(--frame-bg);
  user-select: none;
}

.source-badge {
  border: 1px solid var(--accent-border);
  border-radius: 999px;
  padding: 3px 7px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: .08em;
}

.source-badge.visual {
  border-color: color-mix(in oklab, var(--warning) 42%, var(--border));
  background: var(--warning-soft);
  color: var(--warning);
}

.position {
  color: var(--text-muted);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.bubble-actions {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-left: auto;
}

button {
  display: inline-grid;
  min-width: 27px;
  height: 27px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font: inherit;
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
}

button:hover:not(:disabled),
button:focus-visible {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  color: var(--text);
  outline: none;
}

button:disabled {
  color: var(--text-muted);
  cursor: default;
  opacity: .45;
}

.copy-action {
  min-width: 45px;
  padding: 0 7px;
  font-size: 10px;
  font-weight: 650;
}

.close-action {
  font-size: 16px;
}

.source-block,
.translation-block {
  padding: 13px 15px 12px;
}

.source-block {
  min-height: 74px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-subtle);
}

.translation-block {
  flex: 1;
  min-height: 94px;
  background: color-mix(in oklab, var(--accent-soft) 35%, var(--surface));
  box-shadow: inset 0 2px 0 var(--accent-border);
}

.translation-block.missing {
  background: var(--surface);
  box-shadow: inset 0 1px 0 var(--border);
}

.eyebrow {
  color: var(--text-muted);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: .1em;
  text-transform: uppercase;
}

p {
  display: -webkit-box;
  overflow: hidden;
  margin: 5px 0 0;
  color: var(--text);
  font-size: 14px;
  line-height: 1.45;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.translation-block p {
  font-size: 15px;
  font-weight: 650;
}

.translation-block.missing p {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 500;
}

.partial-note {
  position: absolute;
  right: 13px;
  bottom: 7px;
  color: var(--text-muted);
  font-size: 9px;
}
</style>
