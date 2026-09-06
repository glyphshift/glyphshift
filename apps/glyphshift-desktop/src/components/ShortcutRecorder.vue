<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { displayShortcutToken, shortcutFromEvent } from '../shortcutKeys'
import { translateCommandError } from '../commandError'

const props = defineProps<{ modelValue: string; disabled?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const { t } = useI18n()
const recording = ref(false)
const pending = ref('')
const error = ref('')
let generation = 0
const display = computed(() => props.modelValue.split('+').map(displayShortcutToken).join(' + '))
async function suspend(value: boolean) {
  if ('__TAURI_INTERNALS__' in window) await invoke('desktop_set_shortcut_recording', { recording: value })
}
function stop() {
  generation++
  recording.value = false
  pending.value = ''
  window.removeEventListener('keydown', keyDown, true)
  window.removeEventListener('keyup', keyUp, true)
  window.removeEventListener('blur', stop)
  window.removeEventListener('pagehide', stop)
  void suspend(false).catch(() => undefined)
}
async function start() {
  if (props.disabled || recording.value) return
  const attempt = ++generation
  error.value = ''
  try {
    await suspend(true)
    if (attempt !== generation) { await suspend(false); return }
    recording.value = true
    window.addEventListener('keydown', keyDown, true)
    window.addEventListener('keyup', keyUp, true)
    window.addEventListener('blur', stop)
    window.addEventListener('pagehide', stop)
  } catch (failure) { error.value = translateCommandError(failure) }
}
function keyDown(event: KeyboardEvent) {
  if (!recording.value) return
  if (event.code === 'Tab') { stop(); return }
  event.preventDefault()
  event.stopImmediatePropagation()
  if (event.repeat || event.isComposing) return
  if (event.code === 'Escape') { stop(); return }
  if (event.code === 'Backspace' || event.code === 'Delete') { emit('update:modelValue', ''); stop(); return }
  const candidate = shortcutFromEvent(event)
  if (candidate === null) return
  if (!candidate) { pending.value = ''; error.value = t('settings.shortcutInvalid'); return }
  pending.value = candidate
  error.value = ''
}
function keyUp(event: KeyboardEvent) {
  if (!recording.value || !pending.value || /^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code)) return
  if (event.code !== pending.value.split('+').at(-1)) return
  event.preventDefault()
  event.stopImmediatePropagation()
  emit('update:modelValue', pending.value)
  stop()
}
onBeforeUnmount(stop)
</script>

<template>
  <div class="w-full">
    <button type="button" :disabled="disabled" :aria-label="t('workflows.globalShortcut')" :aria-pressed="recording" :data-shortcut-recording="recording" :style="recording ? { borderColor: 'var(--success)', backgroundColor: 'var(--success-soft)', color: 'var(--success)' } : undefined" class="type-body min-h-9 w-full rounded-[6px] border px-3 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-[var(--accent)] disabled:opacity-50" :class="recording ? 'border-[var(--success)] bg-[var(--success-soft)] text-[var(--success)]' : 'border-[var(--border)] bg-[var(--field-bg)] text-[var(--text-secondary)] hover:border-[var(--border-strong)]'" @click="start" @blur="stop">
      {{ recording ? (pending ? pending.split('+').map(displayShortcutToken).join(' + ') : t('workflows.shortcutRecording')) : (modelValue ? display : t('workflows.shortcutPlaceholder')) }}
    </button>
    <p v-if="error" role="alert" class="type-metadata mb-0 mt-1 text-[var(--danger)]">{{ error }}</p>
    <p v-else class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">{{ t('workflows.shortcutHint') }}</p>
  </div>
</template>
