const interactiveSelector = 'button, a, input, select, textarea, [role="button"], [role="checkbox"], [role="switch"]'

type ManagementColumnMeta = {
  class: {
    th: string
    td: string
  }
}

const pinnedHeader = 'sticky z-40 bg-[var(--surface-subtle)]'
const pinnedCell = 'sticky z-20 bg-[var(--surface-inset)] group-hover:bg-[var(--surface-subtle)]'

export function managementSelectionColumnMeta(): ManagementColumnMeta {
  return {
    class: {
      th: `management-table-selection-cell left-0 w-11 min-w-11 max-w-11 ${pinnedHeader}`,
      td: `management-table-selection-cell left-0 w-11 min-w-11 max-w-11 ${pinnedCell}`,
    },
  }
}

export function managementIdentityColumnMeta(width: 'w-52' | 'w-60' | 'w-64', withSelection = true): ManagementColumnMeta {
  const edge = 'after:pointer-events-none after:absolute after:inset-y-0 after:right-0 after:w-px after:bg-[var(--border)]'
  const left = withSelection ? 'left-11' : 'left-0'
  return {
    class: {
      th: `management-table-identity-cell ${left} ${width} min-w-52 max-w-64 ${pinnedHeader} ${edge}`,
      td: `management-table-identity-cell ${left} ${width} min-w-52 max-w-64 ${pinnedCell} ${edge}`,
    },
  }
}

export function managementActionsColumnMeta(width: 'w-20' | 'w-24' | 'w-28' | 'w-48' | 'w-60'): ManagementColumnMeta {
  const bounds = width === 'w-60' ? 'min-w-60 max-w-60' : width === 'w-48' ? 'min-w-48 max-w-48' : 'min-w-20 max-w-28'
  const edge = 'before:pointer-events-none before:absolute before:inset-y-0 before:left-0 before:w-px before:bg-[var(--border)]'
  return {
    class: {
      th: `management-table-actions-cell right-0 ${width} ${bounds} text-center ${pinnedHeader} ${edge}`,
      td: `management-table-actions-cell right-0 ${width} ${bounds} text-center ${pinnedCell} ${edge}`,
    },
  }
}

export function editableRowIndex(event: MouseEvent): number | null {
  const target = event.target
  if (!(target instanceof Element) || target.closest(interactiveSelector)) return null
  const row = target.closest('tbody tr')
  const body = row?.parentElement
  if (!row || !body) return null
  const index = Array.from(body.children).indexOf(row)
  return index >= 0 ? index : null
}
