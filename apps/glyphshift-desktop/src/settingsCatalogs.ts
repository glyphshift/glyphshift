import { commonFontLanguages, normalizeFontLanguage } from './fontFallbacks'
import defaultFavoriteFontGroups from './defaultFavoriteFonts.json'

export function recommendedFavoriteFonts(installed: string[], favorites: string[] = []): string[] {
  const result = [...favorites]
  for (const aliases of defaultFavoriteFontGroups) {
    if (result.some(font => aliases.some(alias => alias.toLowerCase() === font.toLowerCase()))) continue
    const match = aliases.map(alias => installed.find(font => font.toLowerCase() === alias.toLowerCase())).find(Boolean)
    if (match && result.length < 64) result.push(match)
  }
  return result
}

export const defaultTranslationLanguages = () => commonFontLanguages.map(code => code.toLowerCase())

// Suggest languages recognized by the host's locale data; custom tags remain supported.
export const suggestedLanguageCodes = (() => {
  const names = new Intl.DisplayNames(['en'], { type: 'language', fallback: 'none' })
  const codes = new Set(commonFontLanguages.map(code => code.toLowerCase()))
  for (const first of 'abcdefghijklmnopqrstuvwxyz') {
    for (const second of 'abcdefghijklmnopqrstuvwxyz') {
      const code = first + second
      if (names.of(code)) codes.add(canonicalLanguageCode(code).toLowerCase())
    }
  }
  return [...codes]
})()

export function normalizeCatalog(value: unknown, limit: number, maxLength: number): string[] {
  if (!Array.isArray(value)) return []
  const result: string[] = []
  const seen = new Set<string>()
  for (const item of value.slice(0, limit)) {
    if (typeof item !== 'string') continue
    const text = item.trim()
    if (!text || [...text].length > maxLength || /[\u0000-\u001f\u007f-\u009f]/.test(text) || seen.has(text.toLowerCase())) continue
    seen.add(text.toLowerCase()); result.push(text)
  }
  return result
}

export function normalizeTranslationLanguages(value: unknown): string[] {
  return Array.isArray(value)
    ? normalizeCatalog(value, 128, 63).map(normalizeFontLanguage).filter((code): code is string => Boolean(code))
    : defaultTranslationLanguages()
}

export function preferredFonts(installed: string[], favorites: string[], selected: string[] = []): string[] {
  return [...new Set([...favorites.filter(font => installed.includes(font) || selected.includes(font)), ...selected, ...installed])]
}

export function translationLanguageLabel(code: string, locale: string): string {
  try { return `${new Intl.DisplayNames([locale], { type: 'language' }).of(code)} · ${code}` }
  catch { return code }
}

export function canonicalLanguageCode(code: string): string {
  try { return Intl.getCanonicalLocales(code)[0] ?? code }
  catch { return code }
}
