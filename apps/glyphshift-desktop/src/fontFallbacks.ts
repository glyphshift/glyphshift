export interface LanguageFallbackFont {
  language: string
  fontFamily: string
}

export const commonFontLanguages = ['zh-CN', 'zh-TW', 'en', 'ja', 'ko', 'fr', 'de', 'es', 'pt', 'ru', 'it', 'ar', 'th', 'vi', 'id', 'tr', 'pl', 'uk', 'hi', 'nl']

export function normalizeFontLanguage(value: string): string | null {
  const language = value.trim().toLowerCase()
  return language.length <= 63 && /^(?:[a-z]{2,8}|x)(?:-[a-z0-9]{1,8})*$/.test(language) && language !== 'auto' && language !== 'x' ? language : null
}

export function normalizeFontFallbacks(value: unknown): LanguageFallbackFont[] {
  if (!Array.isArray(value)) return []
  const result: LanguageFallbackFont[] = []
  for (const row of value.slice(0, 64)) {
    if (!row || typeof row.language !== 'string' || typeof row.fontFamily !== 'string') continue
    const language = normalizeFontLanguage(row.language)
    const fontFamily = row.fontFamily.trim()
    if (!language || !fontFamily || [...fontFamily].length > 128 || /[\u0000-\u001f\u007f-\u009f]/.test(fontFamily) || result.some(item => item.language === language)) continue
    result.push({ language, fontFamily })
  }
  return result
}
