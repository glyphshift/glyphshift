import type { AiTranslationItemInput, AiSkipReason } from './useAiTranslation'
export interface AiFilterPolicy {
  skipPureNumbersOrSymbols: boolean
  skipNumericMeasurements: boolean
  skipSingleCharacter: boolean
  skipTextContainingDigits: boolean
  skipUrls: boolean
  skipEmails: boolean
  skipFilePaths: boolean
  skipShortcuts: boolean
  maxSourceChars: number | null
  excludedPatterns: string[]
}

export const defaultAiFilterPolicy = (): AiFilterPolicy => ({
  skipPureNumbersOrSymbols: true,
  skipNumericMeasurements: true,
  skipSingleCharacter: true,
  skipTextContainingDigits: false,
  skipUrls: true,
  skipEmails: true,
  skipFilePaths: true,
  skipShortcuts: true,
  maxSourceChars: null,
  excludedPatterns: [],
})

export function skipReason(item: AiTranslationItemInput, policy: AiFilterPolicy): AiSkipReason | null {
  const source = item.source.trim()
  if (item.translation?.trim()) return 'already_translated'
  if (item.ignored) return 'ignored'
  if (!source) return 'empty_source'
  if (policy.skipSingleCharacter && [...source].length === 1) return 'single_character'
  if (policy.skipUrls && /^(?:https?|ftp):\/\//i.test(source)) return 'url'
  if (policy.skipEmails && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(source)) return 'email'
  if (policy.skipFilePaths && /^(?:[a-z]:[\\/]|\\\\|\/)[^\n]+/i.test(source)) return 'file_path'
  if (policy.skipShortcuts && /^(?:(?:ctrl|alt|shift|cmd|command|win|super)\s*\+\s*)+[\w\d]+$/i.test(source)) return 'shortcut'
  if (policy.skipNumericMeasurements && /^\d+(?:[.,]\d+)?\s*(?:[x×]\s*\d+(?:[.,]\d+)?|fps|hz|px|%|ms|s|kb|mb|gb|°c)$/i.test(source)) return 'numeric_measurement'
  if (policy.skipPureNumbersOrSymbols && !/[\p{L}]/u.test(source)) return 'pure_number_or_symbols'
  if (policy.skipTextContainingDigits && /\d/.test(source)) return 'contains_digit'
  if (policy.maxSourceChars && [...source].length > policy.maxSourceChars) return 'too_long'
  if (policy.excludedPatterns.some(pattern => {
    try { return new RegExp(pattern).test(source) }
    catch { return false }
  })) return 'custom_pattern'
  return null
}
