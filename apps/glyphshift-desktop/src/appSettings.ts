import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'
import { setI18nLocale, type AppLocale } from './i18n'
import { translateCommandError } from './commandError'

export type LocalePreference = 'system' | AppLocale
export type ThemePreference = 'system' | 'dark' | 'light'
export type EffectiveTheme = 'dark' | 'light'

export interface AppSettings {
  settingsSchemaVersion: 1
  localePreference: LocalePreference
  themePreference: ThemePreference
}

interface AppSettingsUpdate {
  localePreference: LocalePreference
  themePreference: ThemePreference
}

const BROWSER_STORAGE_KEY = 'glyphshift.app-settings.v1'
const fallbackSettings: AppSettings = {
  settingsSchemaVersion: 1,
  localePreference: 'system',
  themePreference: 'dark',
}

const settings = ref<AppSettings>({ ...fallbackSettings })
const effectiveLocale = ref<AppLocale>('zh-CN')
const effectiveTheme = ref<EffectiveTheme>('dark')
const settingsError = ref('')
const settingsBusy = ref(false)
let listenersInstalled = false

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function isAppSettings(value: unknown): value is AppSettings {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Partial<AppSettings>
  return candidate.settingsSchemaVersion === 1
    && ['system', 'zh-CN', 'en-US'].includes(candidate.localePreference ?? '')
    && ['system', 'dark', 'light'].includes(candidate.themePreference ?? '')
}

function readBrowserSettings(): AppSettings {
  try {
    const value = JSON.parse(localStorage.getItem(BROWSER_STORAGE_KEY) ?? 'null')
    return isAppSettings(value) ? value : { ...fallbackSettings }
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
    applySettings(isAppSettings(initial) ? initial : { ...fallbackSettings })
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
    if (!isAppSettings(saved)) {
      throw { schemaVersion: 1, code: 'settings.invalid_data', args: {} }
    }
    applySettings(saved)
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
  return {
    settings,
    effectiveLocale,
    effectiveTheme,
    settingsError,
    settingsBusy,
    localePreference: computed(() => settings.value.localePreference),
    themePreference: computed(() => settings.value.themePreference),
    async setLocalePreference(localePreference: LocalePreference) {
      await updateAppSettings({
        localePreference,
        themePreference: settings.value.themePreference,
      })
    },
    async setThemePreference(themePreference: ThemePreference) {
      await updateAppSettings({
        localePreference: settings.value.localePreference,
        themePreference,
      })
    },
  }
}
