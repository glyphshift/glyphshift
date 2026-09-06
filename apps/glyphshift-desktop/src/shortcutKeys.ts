export function shortcutFromEvent(event: KeyboardEvent): string | null {
  if (/^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code)) return null
  if (!event.code) return ''
  if (!event.ctrlKey && !event.altKey && !event.metaKey) return ''
  return [event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Super', event.code].filter(Boolean).join('+')
}

export function displayShortcutToken(token: string) {
  if (token === 'Super') return 'Win'
  if (/^Key[A-Z]$/.test(token)) return token.slice(3)
  if (/^Digit\d$/.test(token)) return token.slice(5)
  if (token.startsWith('Numpad')) return `Num ${token.slice(6)}`
  return ({ ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→' } as Record<string, string>)[token] ?? token
}
