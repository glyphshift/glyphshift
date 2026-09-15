import { defaultAiFilterPolicy, type AiFilterPolicy } from './textFilters'
import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'
import { setI18nLocale, type AppLocale } from './i18n'
import { translateCommandError } from './commandError'
import { normalizeFontFallbacks, type LanguageFallbackFont } from './fontFallbacks'
import { defaultTranslationLanguages, normalizeCatalog, normalizeTranslationLanguages } from './settingsCatalogs'

export type LocalePreference = 'system' | AppLocale
export type ThemePreference = 'system' | 'dark' | 'light'
export type CloseBehavior = 'minimize' | 'tray' | 'quit'
export type EffectiveTheme = 'dark' | 'light'

export interface GlobalShortcutProbe {
  shortcut: string
  available: boolean
}

export interface AppSettings {
  settingsSchemaVersion: 1
  safetyNoticeVersion: number
  onboardingVersion: number
  localePreference: LocalePreference
  themePreference: ThemePreference
  launchAtStartup: boolean
  launchElevated: boolean
  minimizeToTray: boolean
  alwaysOnTop: boolean
  closeBehavior: CloseBehavior
  softwareCaptureShortcut: string
  checkUpdatesOnStartup: boolean
  textFilterPolicy: AiFilterPolicy
  autoCompleteIntervalSeconds: number
  languageFallbackFonts: LanguageFallbackFont[]
  favoriteFonts: string[]
  translationLanguages: string[]
  recentSoftwareIds: string[] | null
}

interface AppSettingsUpdate {
  safetyNoticeVersion: number
  onboardingVersion: number
  localePreference: LocalePreference
  themePreference: ThemePreference
  launchAtStartup: boolean
  launchElevated: boolean
  minimizeToTray: boolean
  alwaysOnTop: boolean
  closeBehavior: CloseBehavior
  softwareCaptureShortcut: string
  checkUpdatesOnStartup: boolean
  textFilterPolicy: AiFilterPolicy
  autoCompleteIntervalSeconds: number
  languageFallbackFonts: LanguageFallbackFont[]
  favoriteFonts: string[]
  translationLanguages: string[]
  recentSoftwareIds: string[] | null
}

interface DesktopPrivilegeStatus {
  elevated: boolean
}

const BROWSER_STORAGE_KEY = 'glyphshift.app-settings.v1'
const fallbackSettings: AppSettings = {
  settingsSchemaVersion: 1,
  safetyNoticeVersion: 0,
  onboardingVersion: 0,
  localePreference: 'system',
  themePreference: 'dark',
  launchAtStartup: false,
  launchElevated: false,
  closeBehavior: 'quit',
  minimizeToTray: false,
  alwaysOnTop: false,
  softwareCaptureShortcut: 'Ctrl+Shift+F8',
  textFilterPolicy: defaultAiFilterPolicy(),
  autoCompleteIntervalSeconds: 10,
  checkUpdatesOnStartup: true,
  languageFallbackFonts: [],
  favoriteFonts: [],
  translationLanguages: defaultTranslationLanguages(),
  recentSoftwareIds: null,
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
  const localePreference = ['system', 'zh-CN', 'en-US'].includes(candidate.localePreference ?? '')
    ? candidate.localePreference as LocalePreference
    : fallbackSettings.localePreference
  const themePreference = ['system', 'dark', 'light'].includes(candidate.themePreference ?? '')
    ? candidate.themePreference as ThemePreference
    : fallbackSettings.themePreference
  const closeBehavior = ['minimize', 'tray', 'quit'].includes(candidate.closeBehavior ?? '')
    ? candidate.closeBehavior as CloseBehavior
    : fallbackSettings.closeBehavior
  const softwareCaptureShortcut = typeof candidate.softwareCaptureShortcut === 'string'
    && candidate.softwareCaptureShortcut.length > 0
    ? candidate.softwareCaptureShortcut
    : fallbackSettings.softwareCaptureShortcut
  return {
    textFilterPolicy: { ...defaultAiFilterPolicy(), ...(candidate.textFilterPolicy ?? {}) },
    languageFallbackFonts: normalizeFontFallbacks(candidate.languageFallbackFonts),
    favoriteFonts: normalizeCatalog(candidate.favoriteFonts, 64, 128),
    translationLanguages: normalizeTranslationLanguages(candidate.translationLanguages),
    recentSoftwareIds: Array.isArray(candidate.recentSoftwareIds) ? normalizeCatalog(candidate.recentSoftwareIds, 20, 128) : null,
    settingsSchemaVersion: 1,
    safetyNoticeVersion: Number.isInteger(candidate.safetyNoticeVersion) && candidate.safetyNoticeVersion! >= 0
      ? candidate.safetyNoticeVersion!
      : fallbackSettings.safetyNoticeVersion,
    onboardingVersion: Number.isInteger(candidate.onboardingVersion) && candidate.onboardingVersion! >= 0
      ? candidate.onboardingVersion!
      : fallbackSettings.onboardingVersion,
    localePreference,
    themePreference,
    launchAtStartup: typeof candidate.launchAtStartup === 'boolean'
      ? candidate.launchAtStartup
      : fallbackSettings.launchAtStartup,
    launchElevated: typeof candidate.launchElevated === 'boolean'
      ? candidate.launchElevated
      : fallbackSettings.launchElevated,
    closeBehavior,
    minimizeToTray: candidate.minimizeToTray === true,
    alwaysOnTop: candidate.alwaysOnTop === true,
    softwareCaptureShortcut,
    checkUpdatesOnStartup: typeof candidate.checkUpdatesOnStartup === 'boolean' ? candidate.checkUpdatesOnStartup : true,
    autoCompleteIntervalSeconds: Number.isInteger(candidate.autoCompleteIntervalSeconds) && candidate.autoCompleteIntervalSeconds! >= 0 && candidate.autoCompleteIntervalSeconds! <= 60 ? candidate.autoCompleteIntervalSeconds! : 10,  }
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

async function updateSoftwareCaptureShortcut(shortcut: string) {
  settingsBusy.value = true
  settingsError.value = ''
  try {
    const saved = hasDesktopRuntime()
      ? await invoke<AppSettings>('desktop_update_software_capture_shortcut', { shortcut })
      : { ...settings.value, softwareCaptureShortcut: shortcut }
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
      safetyNoticeVersion: settings.value.safetyNoticeVersion,
      onboardingVersion: settings.value.onboardingVersion,
      localePreference: settings.value.localePreference,
      themePreference: settings.value.themePreference,
      launchAtStartup: settings.value.launchAtStartup,
      launchElevated: settings.value.launchElevated,
      closeBehavior: settings.value.closeBehavior,
      minimizeToTray: settings.value.minimizeToTray,
      alwaysOnTop: settings.value.alwaysOnTop,
      softwareCaptureShortcut: settings.value.softwareCaptureShortcut,
      autoCompleteIntervalSeconds: settings.value.autoCompleteIntervalSeconds,
      checkUpdatesOnStartup: settings.value.checkUpdatesOnStartup,
      textFilterPolicy: settings.value.textFilterPolicy,
      languageFallbackFonts: settings.value.languageFallbackFonts,
      favoriteFonts: settings.value.favoriteFonts,
      translationLanguages: settings.value.translationLanguages,
      recentSoftwareIds: settings.value.recentSoftwareIds,
      ...patch,
    })
  }

  return {
    settings,
    safetyNoticeVersion: computed(() => settings.value.safetyNoticeVersion),
    onboardingVersion: computed(() => settings.value.onboardingVersion),
    async setSafetyNoticeVersion(value: number) {
      if (Number.isInteger(value) && value >= 0) await update({ safetyNoticeVersion: value })
    },
    async setOnboardingVersion(value: number) {
      if (Number.isInteger(value) && value >= 0) await update({ onboardingVersion: value })
    },
    async setLanguageFallbackFonts(value: LanguageFallbackFont[]) { await update({ languageFallbackFonts: value }) },
    async setFavoriteFonts(value: string[]) { await update({ favoriteFonts: value }) },
    async setTranslationLanguages(value: string[]) { await update({ translationLanguages: value }) },
    async setRecentSoftwareIds(value: string[]) { await update({ recentSoftwareIds: value }) },
    async setTextFilterPolicy(value: AiFilterPolicy) { await update({ textFilterPolicy: value }) },
    async setCheckUpdatesOnStartup(value: boolean) { await update({ checkUpdatesOnStartup: value }) },
    async setAutoCompleteIntervalSeconds(value: number) {
      if (Number.isInteger(value) && value >= 0 && value <= 60) await update({ autoCompleteIntervalSeconds: value })
    },    effectiveLocale,
    effectiveTheme,
    settingsError,
    settingsBusy,
    elevated,
    privilegeBusy,
    localePreference: computed(() => settings.value.localePreference),
    themePreference: computed(() => settings.value.themePreference),
    launchAtStartup: computed(() => settings.value.launchAtStartup),
    launchElevated: computed(() => settings.value.launchElevated),
    minimizeToTray: computed(() => settings.value.minimizeToTray),
    alwaysOnTop: computed(() => settings.value.alwaysOnTop),
    async setMinimizeToTray(value: boolean) { await update({ minimizeToTray: value }) },
    async setAlwaysOnTop(value: boolean) { await update({ alwaysOnTop: value }) },
    closeBehavior: computed(() => settings.value.closeBehavior),
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
    async probeSoftwareCaptureShortcut(shortcut: string) {
      return hasDesktopRuntime()
        ? invoke<GlobalShortcutProbe>('desktop_probe_software_capture_shortcut', { shortcut })
        : { shortcut, available: true }
    },
    async setSoftwareCaptureShortcut(shortcut: string) {
      await updateSoftwareCaptureShortcut(shortcut)
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
