import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'
import { setI18nLocale, type AppLocale } from './i18n'
import { translateCommandError } from './commandError'

export type LocalePreference = 'system' | AppLocale
export type ThemePreference = 'system' | 'dark' | 'light'
export type CloseBehavior = 'minimize' | 'quit'
export type EffectiveTheme = 'dark' | 'light'

export interface GlobalShortcutProbe {
  shortcut: string
  available: boolean
}

export interface AppSettings {
  settingsSchemaVersion: 1
  localePreference: LocalePreference
  themePreference: ThemePreference
  launchAtStartup: boolean
  launchElevated: boolean
  closeBehavior: CloseBehavior
  interactiveTranslationShortcut: string
  softwareCaptureShortcut: string
}

interface AppSettingsUpdate {
  localePreference: LocalePreference
  themePreference: ThemePreference
  launchAtStartup: boolean
  launchElevated: boolean
  closeBehavior: CloseBehavior
  interactiveTranslationShortcut: string
  softwareCaptureShortcut: string
}

interface DesktopPrivilegeStatus {
  elevated: boolean
}

const BROWSER_STORAGE_KEY = 'glyphshift.app-settings.v1'
const fallbackSettings: AppSettings = {
  settingsSchemaVersion: 1,
  localePreference: 'system',
  themePreference: 'dark',
  launchAtStartup: false,
  launchElevated: false,
  closeBehavior: 'quit',
  interactiveTranslationShortcut: 'Ctrl+Shift+F9',
  softwareCaptureShortcut: 'Ctrl+Shift+F8',
}

const settings = ref<AppSettings>({ ...fallbackSettings })
const effectiveLocale = ref<AppLocale>('zh-CN')
const effectiveTheme = ref<EffectiveTheme>('dark')
const settingsError = ref('')
const settingsBusy = ref(false)
const elevated = ref(false)
const privilegeBusy = ref(false)
let listenersInstalled = false

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function normalizeAppSettings(value: unknown): AppSettings | null {
  if (!value || typeof value !== 'object') return null
  const candidate = value as Partial<AppSettings>
  if (candidate.settingsSchemaVersion !== 1
    || !['system', 'zh-CN', 'en-US'].includes(candidate.localePreference ?? '')
    || !['system', 'dark', 'light'].includes(candidate.themePreference ?? '')
    || (candidate.launchAtStartup !== undefined && typeof candidate.launchAtStartup !== 'boolean')
    || (candidate.launchElevated !== undefined && typeof candidate.launchElevated !== 'boolean')
    || (candidate.closeBehavior !== undefined && !['minimize', 'quit'].includes(candidate.closeBehavior))
    || (candidate.interactiveTranslationShortcut !== undefined
      && (typeof candidate.interactiveTranslationShortcut !== 'string'
        || candidate.interactiveTranslationShortcut.length === 0))
    || (candidate.softwareCaptureShortcut !== undefined
      && (typeof candidate.softwareCaptureShortcut !== 'string'
        || candidate.softwareCaptureShortcut.length === 0))) return null
  return {
    settingsSchemaVersion: 1,
    localePreference: candidate.localePreference as LocalePreference,
    themePreference: candidate.themePreference as ThemePreference,
    launchAtStartup: candidate.launchAtStartup ?? false,
    launchElevated: candidate.launchElevated ?? false,
    closeBehavior: candidate.closeBehavior ?? 'quit',
    interactiveTranslationShortcut: candidate.interactiveTranslationShortcut ?? 'Ctrl+Shift+F9',
    softwareCaptureShortcut: candidate.softwareCaptureShortcut ?? 'Ctrl+Shift+F8',
  }
}

function readBrowserSettings(): AppSettings {
  try {
    const value = JSON.parse(localStorage.getItem(BROWSER_STORAGE_KEY) ?? 'null')
    return normalizeAppSettings(value) ?? { ...fallbackSettings }
  }
  catch {
    return { ...fallbackSettings }
  }
}

function localeFromSystem(): AppLocale {
  const candidates = navigator.languages?.length ? navigator.languages : [navigator.language]
  for (const candidate of candidates) {
    const locale = candidate.replaceAll('_', '-')
    if (/^zh-(?:Hans|CN|SG)(?:-|$)/i.test(locale)) return 'zh-CN'
    if (/^en(?:-|$)/i.test(locale)) return 'en-US'
  }
  return 'zh-CN'
}

function themeFromSystem(): EffectiveTheme {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function applySettings(next: AppSettings) {
  settings.value = next
  effectiveLocale.value = next.localePreference === 'system' ? localeFromSystem() : next.localePreference
  effectiveTheme.value = next.themePreference === 'system' ? themeFromSystem() : next.themePreference
  setI18nLocale(effectiveLocale.value)
  document.documentElement.lang = effectiveLocale.value
  document.documentElement.classList.toggle('dark', effectiveTheme.value === 'dark')
  document.documentElement.classList.toggle('light', effectiveTheme.value === 'light')
  document.documentElement.dataset.theme = effectiveTheme.value
}

function refreshSystemPreferences() {
  if (settings.value.localePreference === 'system' || settings.value.themePreference === 'system') {
    applySettings(settings.value)
  }
}

function installSystemPreferenceListeners() {
  if (listenersInstalled) return
  listenersInstalled = true
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', refreshSystemPreferences)
  window.addEventListener('languagechange', refreshSystemPreferences)
}

export async function initializeAppSettings() {
  try {
    const initial = hasDesktopRuntime()
      ? await invoke<AppSettings>('desktop_settings')
      : readBrowserSettings()
    applySettings(normalizeAppSettings(initial) ?? { ...fallbackSettings })
  }
  catch (error) {
    applySettings({ ...fallbackSettings })
    settingsError.value = translateCommandError(error)
  }
  installSystemPreferenceListeners()
}

export function reapplyAppSettings() {
  applySettings(settings.value)
}

async function updateAppSettings(update: AppSettingsUpdate) {
  settingsBusy.value = true
  settingsError.value = ''
  try {
    const saved = hasDesktopRuntime()
      ? await invoke<AppSettings>('desktop_update_settings', { update })
      : { settingsSchemaVersion: 1 as const, ...update }
    if (!hasDesktopRuntime()) localStorage.setItem(BROWSER_STORAGE_KEY, JSON.stringify(saved))
    const normalized = normalizeAppSettings(saved)
    if (!normalized) {
      throw { schemaVersion: 1, code: 'settings.invalid_data', args: {} }
    }
    applySettings(normalized)
  }
  catch (error) {
    settingsError.value = translateCommandError(error)
    throw error
  }
  finally {
    settingsBusy.value = false
  }
}

async function updateGlobalShortcut(
  command: 'desktop_update_interactive_translation_shortcut' | 'desktop_update_software_capture_shortcut',
  key: 'interactiveTranslationShortcut' | 'softwareCaptureShortcut',
  shortcut: string,
) {
  settingsBusy.value = true
  settingsError.value = ''
  try {
    const saved = hasDesktopRuntime()
      ? await invoke<AppSettings>(command, { shortcut })
      : { ...settings.value, [key]: shortcut }
    if (!hasDesktopRuntime()) localStorage.setItem(BROWSER_STORAGE_KEY, JSON.stringify(saved))
    const normalized = normalizeAppSettings(saved)
    if (!normalized) {
      throw { schemaVersion: 1, code: 'settings.invalid_data', args: {} }
    }
    applySettings(normalized)
  }
  catch (error) {
    settingsError.value = translateCommandError(error)
    throw error
  }
  finally {
    settingsBusy.value = false
  }
}

export function useAppSettings() {
  function update(patch: Partial<AppSettingsUpdate>) {
    return updateAppSettings({
      localePreference: settings.value.localePreference,
      themePreference: settings.value.themePreference,
      launchAtStartup: settings.value.launchAtStartup,
      launchElevated: settings.value.launchElevated,
      closeBehavior: settings.value.closeBehavior,
      interactiveTranslationShortcut: settings.value.interactiveTranslationShortcut,
      softwareCaptureShortcut: settings.value.softwareCaptureShortcut,
      ...patch,
    })
  }

  return {
    settings,
    effectiveLocale,
    effectiveTheme,
    settingsError,
    settingsBusy,
    elevated,
    privilegeBusy,
    localePreference: computed(() => settings.value.localePreference),
    themePreference: computed(() => settings.value.themePreference),
    launchAtStartup: computed(() => settings.value.launchAtStartup),
    launchElevated: computed(() => settings.value.launchElevated),
    closeBehavior: computed(() => settings.value.closeBehavior),
    interactiveTranslationShortcut: computed(() => settings.value.interactiveTranslationShortcut),
    softwareCaptureShortcut: computed(() => settings.value.softwareCaptureShortcut),
    async setLocalePreference(localePreference: LocalePreference) {
      await update({ localePreference })
    },
    async setThemePreference(themePreference: ThemePreference) {
      await update({ themePreference })
    },
    async setLaunchAtStartup(launchAtStartup: boolean) {
      await update({ launchAtStartup })
    },
    async setLaunchElevated(launchElevated: boolean) {
      await update({ launchElevated })
      if (!launchElevated || !hasDesktopRuntime() || elevated.value) return
      privilegeBusy.value = true
      settingsError.value = ''
      try {
        await invoke('desktop_restart_elevated')
      }
      catch (error) {
        settingsError.value = translateCommandError(error)
        throw error
      }
      finally {
        privilegeBusy.value = false
      }
    },
    async setCloseBehavior(closeBehavior: CloseBehavior) {
      await update({ closeBehavior })
    },
    async probeInteractiveTranslationShortcut(shortcut: string) {
      return hasDesktopRuntime()
        ? invoke<GlobalShortcutProbe>('desktop_probe_interactive_translation_shortcut', { shortcut })
        : { shortcut, available: shortcut !== settings.value.softwareCaptureShortcut }
    },
    async setInteractiveTranslationShortcut(shortcut: string) {
      await updateGlobalShortcut(
        'desktop_update_interactive_translation_shortcut',
        'interactiveTranslationShortcut',
        shortcut,
      )
    },
    async probeSoftwareCaptureShortcut(shortcut: string) {
      return hasDesktopRuntime()
        ? invoke<GlobalShortcutProbe>('desktop_probe_software_capture_shortcut', { shortcut })
        : { shortcut, available: shortcut !== settings.value.interactiveTranslationShortcut }
    },
    async setSoftwareCaptureShortcut(shortcut: string) {
      await updateGlobalShortcut(
        'desktop_update_software_capture_shortcut',
        'softwareCaptureShortcut',
        shortcut,
      )
    },
    async refreshPrivilegeStatus() {
      privilegeBusy.value = true
      settingsError.value = ''
      try {
        elevated.value = hasDesktopRuntime()
          ? (await invoke<DesktopPrivilegeStatus>('desktop_privilege_status')).elevated
          : false
      }
      catch (error) {
        settingsError.value = translateCommandError(error)
      }
      finally {
        privilegeBusy.value = false
      }
    },
  }
}
