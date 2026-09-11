<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import {
  useAppSettings,
  type CloseBehavior,
  type LocalePreference,
  type ThemePreference,
} from '../appSettings'
import TextFilterSettings from './TextFilterSettings.vue'
import FontFallbackSettings from './FontFallbackSettings.vue'
import FavoriteFontSettings from './FavoriteFontSettings.vue'
import RecentSoftwareSettings from './RecentSoftwareSettings.vue'
import LanguageSettings from './LanguageSettings.vue'
import AiProfilesPanel from './AiProfilesPanel.vue'
import { useAppUpdate } from '../useAppUpdate'
import { version as appVersion } from '../../package.json'
import { displayShortcutToken, shortcutFromEvent } from '../shortcutKeys'

const section = ref('general')
const sections = ['general', 'software', 'fonts', 'languages', 'rules'] as const
const aiProfilesPanel = ref<InstanceType<typeof AiProfilesPanel>>()
const { t } = useI18n()
const appSettings = useAppSettings()
const appUpdate = useAppUpdate()
const openingDictionaryDirectory = ref(false)
const dictionaryDirectoryError = ref('')
async function openDictionaryDirectory() {
  if (openingDictionaryDirectory.value) return
  openingDictionaryDirectory.value = true
  dictionaryDirectoryError.value = ''
  try {
    await invoke('desktop_open_dictionary_directory')
  } catch {
    dictionaryDirectoryError.value = t('settings.data.openFailed')
  } finally {
    openingDictionaryDirectory.value = false
  }
}
async function updateAutoInterval(event: Event) {
  const input = event.target as HTMLInputElement
  if (input.value.trim()) await appSettings.setAutoCompleteIntervalSeconds(Number(input.value))
  input.value = String(appSettings.settings.value.autoCompleteIntervalSeconds)
}

type ShortcutTarget = 'softwareCapture'
type ShortcutStatus = 'idle' | 'checking' | 'conflict' | 'invalid' | 'failed' | 'saved'

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
  { value: 'tray' as const, label: t('settings.closeBehaviorOption.tray') },
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
    return ''
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
      : ''
  }
}

function shortcutStatusClass(target: ShortcutTarget) {
  if (shortcutStatusTarget.value !== target) return 'text-[var(--text-muted)]'
  if (shortcutStatus.value === 'conflict' || shortcutStatus.value === 'invalid') return 'text-amber-400'
  if (shortcutStatus.value === 'failed') return 'text-red-400'
  if (shortcutStatus.value === 'saved') return 'text-emerald-400'
  return 'text-[var(--text-muted)]'
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
  <UtilityPageShell
      title-id="settings-title"
      :title="t('settings.title')"
      icon="i-tabler-settings"
      content-test-id="settings-layout"
  >
    <div class="space-y-4">
      <UTabs v-model="section" :items="sections.map((value, index) => ({ value, label: t('settingsManager.' + value), icon: ['i-tabler-settings', 'i-tabler-app-window', 'i-tabler-typography', 'i-tabler-language', 'i-tabler-filter'][index] }))"
        :content="false" :aria-label="t('settingsManager.navigation')" data-testid="settings-tabs" color="neutral" variant="link" size="sm" activation-mode="manual"
        class="sticky top-0 z-20 w-full bg-[var(--app-bg)]"
        :ui="{ list: 'w-full justify-start gap-1 rounded-none border-b border-[var(--border)] bg-transparent p-0 overflow-x-auto', indicator: 'hidden', trigger: 'type-label relative h-10 flex-none gap-2 rounded-none px-3 text-[var(--text-secondary)] after:absolute after:inset-x-2 after:bottom-0 after:hidden after:h-0.5 after:bg-[var(--accent)] hover:bg-[var(--surface-hover)] data-[state=active]:font-semibold data-[state=active]:!text-[var(--text)] data-[state=active]:after:block', leadingIcon: 'size-4 shrink-0' }" />
      <RecentSoftwareSettings v-if="section === 'software'" />
      <div v-if="section === 'fonts'" class="space-y-4"><FavoriteFontSettings /><FontFallbackSettings /></div>
      <div v-show="section === 'rules'" class="space-y-4">
        <TextFilterSettings />
      </div>
      <LanguageSettings v-if="section === 'languages'" />
      <div v-show="section === 'general'" class="space-y-4">
        <UAlert
          v-if="appSettings.settingsError.value"
          role="alert"
          color="error"
          variant="soft"
          :title="t('settings.saveFailed')"
          :description="appSettings.settingsError.value"
          class="w-full"
        />

        <ManagementFormSection
          data-testid="settings-section-appearance"
          :title="t('settings.appearance')"
        >
          <ManagementFormRow
            :label="t('settings.language')"
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

        <ManagementFormSection :title="t('updates.title')">
          <ManagementFormRow :label="t('updates.autoCheck')" :description="t('updates.autoHint')" icon="i-tabler-refresh" control-width="compact">
            <USwitch :model-value="appSettings.settings.value.checkUpdatesOnStartup" :aria-label="t('updates.autoCheck')" :disabled="appSettings.settingsBusy.value" @update:model-value="appSettings.setCheckUpdatesOnStartup" />
          </ManagementFormRow>
          <ManagementFormRow :label="t('updates.current', { version: appVersion })" icon="i-tabler-info-circle" control-width="compact">
            <div class="space-y-2">
              <UButton color="neutral" variant="outline" :label="t('updates.check')" :loading="appUpdate.checking.value" @click="appUpdate.check(true)" />
              <p v-if="appUpdate.status.value !== 'idle'" role="status" class="type-metadata m-0">{{ t('updates.' + appUpdate.status.value + 'Status') }}</p>
            </div>
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection
          data-testid="settings-section-ai"
          :title="t('settings.aiTranslation.title')"
        >
          <template #actions>
            <UButton color="primary" variant="soft" size="sm" icon="i-tabler-plus" :label="t('ai.addProfile')" @click="aiProfilesPanel?.openCreate()" />
          </template>
          <AiProfilesPanel ref="aiProfilesPanel" :show-create="false" />
          <ManagementFormRow :label="t('ai.autoInterval')" :help="t('ai.autoIntervalHint')" icon="i-tabler-clock" control-width="compact">
            <UInput :model-value="appSettings.settings.value.autoCompleteIntervalSeconds" type="number" min="0" max="60" step="1" :aria-label="t('ai.autoInterval')" :disabled="appSettings.settingsBusy.value" class="w-full" @change="updateAutoInterval" />
          </ManagementFormRow>
        </ManagementFormSection>

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
              <UButton :title="shortcutRecording === row.target
                  ? t('settings.shortcutRecordingLabel', { name: row.label })
                  : t('settings.shortcutChangeLabel', { name: row.label, shortcut: currentShortcutDisplay(row.target) })"
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
                    <span v-if="index" class="type-caption text-[var(--text-muted)]">+</span>
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
              <p v-if="shortcutStatusMessage(row.target)" class="type-metadata m-0 leading-4" :class="shortcutStatusClass(row.target)" aria-live="polite">
                {{ shortcutStatusMessage(row.target) }}
              </p>
            </div>
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection data-testid="settings-section-data" :title="t('settings.data.title')">
          <ManagementFormRow :label="t('settings.data.dictionaries')" :description="t('settings.data.description')" icon="i-tabler-folder" control-width="compact">
            <UButton color="neutral" variant="outline" icon="i-tabler-folder-open" :label="t('settings.data.open')" :title="t('settings.data.open')" :loading="openingDictionaryDirectory" @click="openDictionaryDirectory" />
          </ManagementFormRow>
          <p v-if="dictionaryDirectoryError" role="alert" class="type-metadata px-4 pb-3 text-error">{{ dictionaryDirectoryError }}</p>
        </ManagementFormSection>

        <ManagementFormSection
          data-testid="settings-section-application"
          :title="t('settings.applicationAndPrivilege')"
        >
          <ManagementFormRow
            :label="t('settings.launchAtStartup')"
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

          <ManagementFormRow :label="t('settings.minimizeToTray')" :description="t('settings.minimizeToTrayHint')" icon="i-tabler-layout-bottombar" control-width="compact">
            <div class="flex justify-end"><USwitch :model-value="appSettings.minimizeToTray.value" :aria-label="t('settings.minimizeToTray')" :disabled="appSettings.settingsBusy.value" @update:model-value="appSettings.setMinimizeToTray($event).catch(() => undefined)" /></div>
          </ManagementFormRow>
          <ManagementFormRow :label="t('settings.alwaysOnTop')" icon="i-tabler-pin" control-width="compact">
            <div class="flex justify-end"><USwitch :model-value="appSettings.alwaysOnTop.value" :aria-label="t('settings.alwaysOnTop')" :disabled="appSettings.settingsBusy.value" @update:model-value="appSettings.setAlwaysOnTop($event).catch(() => undefined)" /></div>
          </ManagementFormRow>
          <ManagementFormRow
            :label="t('settings.closeBehavior')"
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
  </UtilityPageShell>
</template>
