<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  AI_TRANSLATION_BATCH_LIMITS,
  useAppSettings,
  type CloseBehavior,
  type LocalePreference,
  type ThemePreference,
} from '../appSettings'
import AiProfilesPanel from './AiProfilesPanel.vue'

const { t } = useI18n()
const appSettings = useAppSettings()

type ShortcutTarget = 'softwareCapture'
type ShortcutStatus = 'idle' | 'checking' | 'conflict' | 'invalid' | 'failed' | 'saved'

const modifierCodes = new Set([
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'ShiftLeft',
  'ShiftRight',
  'MetaLeft',
  'MetaRight',
])
const shortcutRecording = ref<ShortcutTarget | null>(null)
const shortcutStatusTarget = ref<ShortcutTarget | null>(null)
const shortcutPreview = ref('')
const shortcutStatus = ref<ShortcutStatus>('idle')
let shortcutAttempt = 0

const localeItems = computed(() => [
  { value: 'system' as const, label: t('settings.locale.system') },
  { value: 'zh-CN' as const, label: t('settings.locale.zhCN') },
  { value: 'en-US' as const, label: t('settings.locale.enUS') },
])
const themeItems = computed(() => [
  { value: 'system' as const, label: t('settings.themeOption.system') },
  { value: 'dark' as const, label: t('settings.themeOption.dark') },
  { value: 'light' as const, label: t('settings.themeOption.light') },
])
const closeBehaviorItems = computed(() => [
  { value: 'minimize' as const, label: t('settings.closeBehaviorOption.minimize') },
  { value: 'quit' as const, label: t('settings.closeBehaviorOption.quit') },
])
const shortcutRows = computed(() => [
  {
    target: 'softwareCapture' as const,
    label: t('settings.softwareCaptureShortcut'),
    description: t('settings.softwareCaptureShortcutDescription'),
    icon: 'i-tabler-crosshair',
  },
])

function displayShortcutToken(token: string) {
  if (token === 'Super') return 'Win'
  if (token.startsWith('Key') && token.length === 4) return token.slice(3)
  if (token.startsWith('Digit') && token.length === 6) return token.slice(5)
  if (token.startsWith('Numpad')) return `Num ${token.slice(6)}`
  if (token === 'ArrowUp') return '↑'
  if (token === 'ArrowDown') return '↓'
  if (token === 'ArrowLeft') return '←'
  if (token === 'ArrowRight') return '→'
  return token
}

function displayShortcut(shortcut: string) {
  return shortcut.split('+').map(displayShortcutToken).join(' + ')
}

function configuredShortcut(_target: ShortcutTarget) {
  return appSettings.softwareCaptureShortcut.value
}

function currentShortcutDisplay(target: ShortcutTarget) {
  return displayShortcut(configuredShortcut(target))
}

function visibleShortcutParts(target: ShortcutTarget) {
  const shortcut = shortcutRecording.value === target ? shortcutPreview.value : configuredShortcut(target)
  return shortcut ? shortcut.split('+').map(displayShortcutToken) : [t('settings.shortcutWaiting')]
}

function shortcutStatusMessage(target: ShortcutTarget) {
  if (shortcutStatusTarget.value !== target) {
    return t('settings.shortcutCurrent', { shortcut: currentShortcutDisplay(target) })
  }
  const candidate = displayShortcut(shortcutPreview.value)
  switch (shortcutStatus.value) {
    case 'checking': return t('settings.shortcutChecking', { shortcut: candidate })
    case 'conflict': return t('settings.shortcutConflict', { shortcut: candidate })
    case 'invalid': return t('settings.shortcutInvalid')
    case 'failed': return t('settings.shortcutSaveFailed')
    case 'saved': return t('settings.shortcutSaved', { shortcut: currentShortcutDisplay(target) })
    default: return shortcutRecording.value === target
      ? t('settings.shortcutCancelHint')
      : t('settings.shortcutCurrent', { shortcut: currentShortcutDisplay(target) })
  }
}

function shortcutStatusClass(target: ShortcutTarget) {
  if (shortcutStatusTarget.value !== target) return 'text-[var(--text-muted)]'
  if (shortcutStatus.value === 'conflict' || shortcutStatus.value === 'invalid') return 'text-amber-400'
  if (shortcutStatus.value === 'failed') return 'text-red-400'
  if (shortcutStatus.value === 'saved') return 'text-emerald-400'
  return 'text-[var(--text-muted)]'
}

function shortcutFromEvent(event: KeyboardEvent) {
  if (modifierCodes.has(event.code)) return null
  const tokens: string[] = []
  if (event.ctrlKey) tokens.push('Ctrl')
  if (event.altKey) tokens.push('Alt')
  if (event.shiftKey) tokens.push('Shift')
  if (event.metaKey) tokens.push('Super')
  if (!event.ctrlKey && !event.altKey && !event.metaKey) return ''
  tokens.push(event.code)
  return tokens.join('+')
}

function stopShortcutRecording(resetStatus = true) {
  shortcutRecording.value = null
  shortcutPreview.value = ''
  if (resetStatus) {
    shortcutStatus.value = 'idle'
    shortcutStatusTarget.value = null
  }
  window.removeEventListener('keydown', recordShortcut, true)
}

function startShortcutRecording(target: ShortcutTarget) {
  if (appSettings.settingsBusy.value || shortcutRecording.value) return
  shortcutAttempt += 1
  shortcutRecording.value = target
  shortcutStatusTarget.value = target
  shortcutPreview.value = ''
  shortcutStatus.value = 'idle'
  window.addEventListener('keydown', recordShortcut, true)
}

async function recordShortcut(event: KeyboardEvent) {
  if (!shortcutRecording.value || event.repeat) return
  event.preventDefault()
  event.stopImmediatePropagation()
  if (event.code === 'Escape') {
    shortcutAttempt += 1
    stopShortcutRecording()
    return
  }
  if (shortcutStatus.value === 'checking' || appSettings.settingsBusy.value) return
  const shortcut = shortcutFromEvent(event)
  if (shortcut === null) {
    shortcutStatus.value = 'idle'
    shortcutPreview.value = [
      event.ctrlKey && 'Ctrl',
      event.altKey && 'Alt',
      event.shiftKey && 'Shift',
      event.metaKey && 'Super',
    ].filter(Boolean).join('+')
    return
  }
  if (!shortcut) {
    shortcutPreview.value = event.code
    shortcutStatus.value = 'invalid'
    return
  }

  const attempt = ++shortcutAttempt
  const target = shortcutRecording.value
  shortcutPreview.value = shortcut
  shortcutStatus.value = 'checking'
  try {
    const probe = await appSettings.probeSoftwareCaptureShortcut(shortcut)
    if (attempt !== shortcutAttempt || shortcutRecording.value !== target) return
    shortcutPreview.value = probe.shortcut
    if (!probe.available) {
      shortcutStatus.value = 'conflict'
      return
    }
    await appSettings.setSoftwareCaptureShortcut(probe.shortcut)
    if (attempt !== shortcutAttempt || shortcutRecording.value !== target) return
    stopShortcutRecording(false)
    shortcutStatus.value = 'saved'
  }
  catch {
    if (attempt === shortcutAttempt && shortcutRecording.value === target) shortcutStatus.value = 'failed'
  }
}
function updateLocale(value: unknown) {
  void appSettings.setLocalePreference(value as LocalePreference).catch(() => undefined)
}

function updateTheme(value: unknown) {
  void appSettings.setThemePreference(value as ThemePreference).catch(() => undefined)
}

function updateLaunchAtStartup(value: boolean) {
  void appSettings.setLaunchAtStartup(value).catch(() => undefined)
}

function updateCloseBehavior(value: unknown) {
  void appSettings.setCloseBehavior(value as CloseBehavior).catch(() => undefined)
}

function updateLaunchElevated(value: boolean) {
  void appSettings.setLaunchElevated(value).catch(() => undefined)
}

function updateAiBatchItems(value: number | null | undefined) {
  if (!Number.isInteger(value)
    || (value ?? 0) < AI_TRANSLATION_BATCH_LIMITS.items.min
    || (value ?? 0) > AI_TRANSLATION_BATCH_LIMITS.items.max) return
  void appSettings.setAiTranslationBatch({
    ...appSettings.aiTranslationBatch.value,
    maxItemsPerRequest: value as number,
  }).catch(() => undefined)
}

function updateAiBatchInputTokens(value: number | null | undefined) {
  if (!Number.isInteger(value)
    || (value ?? 0) < AI_TRANSLATION_BATCH_LIMITS.inputTokens.min
    || (value ?? 0) > AI_TRANSLATION_BATCH_LIMITS.inputTokens.max) return
  void appSettings.setAiTranslationBatch({
    ...appSettings.aiTranslationBatch.value,
    maxInputTokensPerRequest: value as number,
  }).catch(() => undefined)
}

onMounted(() => void appSettings.refreshPrivilegeStatus())
onBeforeUnmount(() => {
  shortcutAttempt += 1
  stopShortcutRecording()
})
</script>

<template>
  <!--
    THESIS: 设置只呈现立即生效的应用级偏好，不用说明文字冒充功能。
    OWN-WORLD: 继承高密度 Windows 管理器、薄分隔线与克制 cobalt 焦点。
    STORY: 用户扫描外观与快捷键分组，点击当前组合键后直接按键录制，并在可用性确认后立即生效。
    FIRST VIEWPORT: 紧凑页头下优先呈现外观和快捷键；标签说明在左，真实控件与内联状态在右。
    FORM: established Operate surface；现有管理器结构的局部扩展。
    FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
  -->
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--app-bg)] p-4" aria-labelledby="settings-title">
    <ManagementDetailHeader
      title-id="settings-title"
      :title="t('settings.title')"
      :description="t('settings.description')"
    />

    <UAlert
      v-if="appSettings.settingsError.value"
      role="alert"
      color="error"
      variant="soft"
      :title="t('settings.saveFailed')"
      :description="appSettings.settingsError.value"
      class="mx-auto mb-4 w-full max-w-[980px]"
    />

    <ManagementWorkspaceSurface variant="canvas">
      <div class="h-full overflow-y-auto p-5 [scrollbar-gutter:stable]">
        <div class="space-y-4">
        <ManagementFormSection :title="t('settings.appearance')" :description="t('settings.appearanceDescription')">
          <ManagementFormRow
            :label="t('settings.language')"
            :description="t('settings.languageDescription')"
            icon="i-tabler-language"
            control-width="compact"
          >
              <USelect
                :model-value="appSettings.localePreference.value"
                :items="localeItems"
                value-key="value"
                label-key="label"
                :aria-label="t('settings.language')"
                :disabled="appSettings.settingsBusy.value"
                class="w-full"
                @update:model-value="updateLocale"
              />
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.theme')"
            :description="t('settings.themeDescription')"
            icon="i-tabler-sun-moon"
            control-width="compact"
          >
              <USelect
                :model-value="appSettings.themePreference.value"
                :items="themeItems"
                value-key="value"
                label-key="label"
                :aria-label="t('settings.theme')"
                :disabled="appSettings.settingsBusy.value"
                class="w-full"
                @update:model-value="updateTheme"
              />
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection :title="t('settings.aiBatch.title')" :description="t('settings.aiBatch.description')">
          <ManagementFormRow
            :label="t('settings.aiBatch.items')"
            :description="t('settings.aiBatch.itemsDescription')"
            icon="i-tabler-list-numbers"
            control-width="compact"
          >
            <UInputNumber
              :model-value="appSettings.aiTranslationBatch.value.maxItemsPerRequest"
              :min="AI_TRANSLATION_BATCH_LIMITS.items.min"
              :max="AI_TRANSLATION_BATCH_LIMITS.items.max"
              :step="1"
              :aria-label="t('settings.aiBatch.items')"
              :disabled="appSettings.settingsBusy.value"
              class="w-full"
              @update:model-value="updateAiBatchItems"
            />
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.aiBatch.inputTokens')"
            :description="t('settings.aiBatch.inputTokensDescription')"
            icon="i-tabler-braces"
            control-width="compact"
          >
            <UInputNumber
              :model-value="appSettings.aiTranslationBatch.value.maxInputTokensPerRequest"
              :min="AI_TRANSLATION_BATCH_LIMITS.inputTokens.min"
              :max="AI_TRANSLATION_BATCH_LIMITS.inputTokens.max"
              :step="1000"
              :aria-label="t('settings.aiBatch.inputTokens')"
              :disabled="appSettings.settingsBusy.value"
              class="w-full"
              @update:model-value="updateAiBatchInputTokens"
            />
          </ManagementFormRow>
        </ManagementFormSection>

        <AiProfilesPanel />

        <ManagementFormSection :title="t('settings.shortcuts')" :description="t('settings.shortcutsDescription')">
          <ManagementFormRow
            v-for="row in shortcutRows"
            :key="row.target"
            :label="row.label"
            :description="row.description"
            :icon="row.icon"
            control-width="compact"
          >
            <div class="space-y-2">
              <UButton
                color="neutral"
                variant="outline"
                class="flex min-h-9 w-full items-center justify-between gap-3 rounded-md border px-3 py-1.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-strong)] disabled:cursor-not-allowed disabled:opacity-60"
                :class="shortcutRecording === row.target
                  ? 'border-[var(--accent-strong)] bg-[var(--accent-soft)]'
                  : 'border-[var(--border)] bg-[var(--surface)] hover:border-[var(--border-strong)]'"
                :aria-label="shortcutRecording === row.target
                  ? t('settings.shortcutRecordingLabel', { name: row.label })
                  : t('settings.shortcutChangeLabel', { name: row.label, shortcut: currentShortcutDisplay(row.target) })"
                :aria-pressed="shortcutRecording === row.target"
                :disabled="appSettings.settingsBusy.value
                  || Boolean(shortcutRecording && shortcutRecording !== row.target)"
                @click="startShortcutRecording(row.target)"
              >
                <span class="flex min-w-0 flex-wrap items-center gap-1" aria-hidden="true">
                  <template v-for="(part, index) in visibleShortcutParts(row.target)" :key="`${part}-${index}`">
                    <span v-if="index" class="text-[10px] text-[var(--text-muted)]">+</span>
                    <kbd class="min-w-6 rounded border border-[var(--border-strong)] bg-[var(--surface-inset)] px-1.5 py-0.5 text-center font-mono text-[11px] font-semibold text-[var(--text)] shadow-sm">
                      {{ part }}
                    </kbd>
                  </template>
                </span>
                <UIcon
                  :name="shortcutRecording === row.target ? 'i-tabler-keyboard' : 'i-tabler-edit'"
                  class="size-4 shrink-0 text-[var(--text-muted)]"
                  aria-hidden="true"
                />
              </UButton>
              <p class="m-0 text-[10px] leading-4" :class="shortcutStatusClass(row.target)" aria-live="polite">
                {{ shortcutStatusMessage(row.target) }}
              </p>
            </div>
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection :title="t('settings.behavior')" :description="t('settings.behaviorDescription')">
          <ManagementFormRow
            :label="t('settings.launchAtStartup')"
            :description="t('settings.launchAtStartupDescription')"
            icon="i-tabler-rocket"
            control-width="compact"
          >
            <div class="flex justify-end">
              <USwitch
                :model-value="appSettings.launchAtStartup.value"
                :aria-label="t('settings.launchAtStartup')"
                :disabled="appSettings.settingsBusy.value"
                @update:model-value="updateLaunchAtStartup"
              />
            </div>
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.closeBehavior')"
            :description="t('settings.closeBehaviorDescription')"
            icon="i-tabler-door-exit"
            control-width="compact"
          >
            <USelect
              :model-value="appSettings.closeBehavior.value"
              :items="closeBehaviorItems"
              value-key="value"
              label-key="label"
              :aria-label="t('settings.closeBehavior')"
              :disabled="appSettings.settingsBusy.value"
              class="w-full"
              @update:model-value="updateCloseBehavior"
            />
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection :title="t('settings.privilege')" :description="t('settings.privilegeDescription')">
          <ManagementFormRow
            :label="t('settings.launchElevated')"
            :description="t('settings.launchElevatedDescription')"
            icon="i-tabler-shield-up"
            control-width="compact"
          >
            <div class="flex justify-end">
              <USwitch
                :model-value="appSettings.launchElevated.value"
                :aria-label="t('settings.launchElevated')"
                :disabled="appSettings.settingsBusy.value || appSettings.privilegeBusy.value"
                @update:model-value="updateLaunchElevated"
              />
            </div>
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.currentPrivilege')"
            :description="appSettings.elevated.value ? t('settings.elevatedDescription') : t('settings.standardDescription')"
            icon="i-tabler-shield-lock"
            control-width="compact"
          >
            <div class="flex justify-end">
              <UBadge
                :color="appSettings.elevated.value ? 'warning' : 'neutral'"
                variant="soft"
                :label="appSettings.elevated.value ? t('settings.elevated') : t('settings.standard')"
              />
            </div>
          </ManagementFormRow>
        </ManagementFormSection>
        </div>
      </div>
    </ManagementWorkspaceSurface>
  </section>
</template>
