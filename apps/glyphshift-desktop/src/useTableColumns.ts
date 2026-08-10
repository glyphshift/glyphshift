import { ref, watch } from 'vue'

function loadColumnVisibility(storageKey: string, defaults: Record<string, boolean>) {
  try {
    const stored = JSON.parse(localStorage.getItem(storageKey) ?? '{}') as Record<string, unknown>
    return Object.fromEntries(Object.entries(defaults).map(([key, defaultVisible]) => [
      key,
      typeof stored[key] === 'boolean' ? stored[key] : defaultVisible,
    ]))
  }
  catch {
    return { ...defaults }
  }
}

export function useTableColumns(storageKey: string, defaults: Record<string, boolean>) {
  const columns = ref<Record<string, boolean>>(loadColumnVisibility(storageKey, defaults))

  watch(columns, (value) => {
    localStorage.setItem(storageKey, JSON.stringify(value))
  }, { deep: true })

  function toggleColumn(key: string, visible: boolean) {
    if (Object.hasOwn(defaults, key)) columns.value[key] = visible
  }

  return { columns, toggleColumn }
}
