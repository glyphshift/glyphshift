const interactiveSelector = 'button, a, input, select, textarea, [role="button"], [role="checkbox"], [role="switch"]'

export function editableRowIndex(event: MouseEvent): number | null {
  const target = event.target
  if (!(target instanceof Element) || target.closest(interactiveSelector)) return null
  const row = target.closest('tbody tr')
  const body = row?.parentElement
  if (!row || !body) return null
  const index = Array.from(body.children).indexOf(row)
  return index >= 0 ? index : null
}
