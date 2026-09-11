import { invoke } from '@tauri-apps/api/core'

export interface RegexRule { pattern: string; replacement: string; enabled: boolean }
export const propertyNumberRule: RegexRule = {
  pattern: '^(.+?)(:[ \\t]*[0-9]+)$',
  replacement: '{{TR}}$2',
  enabled: true,
}

export async function validateRegexRule(rule: RegexRule): Promise<void> {
  if ('__TAURI_INTERNALS__' in window) {
    await invoke('desktop_validate_regex_rule', { rule })
    return
  }
  // Browser development only; the desktop always validates with the Runtime's Rust engine.
  const expression = new RegExp(`(?:${rule.pattern})|`, 'u')
  const groups = expression.exec('')!.length
  if (rule.replacement.includes('{{TR}}') && groups < 2) throw new Error('unknown_capture')
  for (const token of rule.replacement.matchAll(/\$\$|\$(\d+)/g)) {
    if (token[1] !== undefined && Number(token[1]) >= groups) throw new Error('unknown_capture')
  }
}


export interface RegexRuleTestResult { matched: boolean; captures: (string | null)[]; output: string; missingTranslation: boolean }
export async function testRegexRule(rule: RegexRule, source: string, mockTranslation: string): Promise<RegexRuleTestResult> {
  if ('__TAURI_INTERNALS__' in window) return invoke('desktop_test_regex_rule', { rule, source, mockTranslation })
  // Browser fixtures only. The desktop uses the same Rust engine as live replacement.
  await validateRegexRule(rule)
  const match = new RegExp(rule.pattern, 'u').exec(source)
  if (!match) return { matched: false, captures: [], output: source, missingTranslation: false }
  let missingTranslation = false
  const body = rule.replacement.replace(/\{\{TR\}\}|\$\$|\$(\d+)/g, (token, group) => {
    if (token === '$$') return '$'
    if (group !== undefined) return match[Number(group)] ?? ''
    if (!mockTranslation || match[1] === undefined) missingTranslation = true
    return mockTranslation
  })
  return { matched: true, captures: Array.from(match, value => value ?? null), missingTranslation,
    output: missingTranslation ? source : source.slice(0, match.index) + body + source.slice(match.index + match[0].length) }
}
