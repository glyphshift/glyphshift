import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

type Library = 'workflows' | 'dictionaries'
export type LibrarySort = 'recent' | 'oldest' | 'titleAsc' | 'titleDesc'
const modes: LibrarySort[] = ['recent', 'oldest', 'titleAsc', 'titleDesc']
const usageKey = 'glyphshift.library-usage'
function read(key: string): unknown {
  try { return JSON.parse(localStorage.getItem(key) ?? 'null') } catch { return null }
}
function write(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)) } catch { /* Keep session preferences when storage is unavailable. */ }
}
const usage = ref<Record<string, number>>({})
const stored = read(usageKey)
if (stored && typeof stored === 'object' && !Array.isArray(stored)) {
  usage.value = Object.fromEntries(Object.entries(stored).filter(([, value]) => typeof value === 'number' && Number.isFinite(value) && value >= 0))
}
export function markLibraryUsed(library: Library, id: string) {
  usage.value = { ...usage.value, [`${library}:${id}`]: Date.now() }
  write(usageKey, usage.value)
}
export function useLibrarySort(library: Library) {
  const { t, locale } = useI18n()
  const key = `glyphshift.library-sort.${library}`
  const storedMode = read(key)
  const mode = ref<LibrarySort>(modes.includes(storedMode as LibrarySort) ? storedMode as LibrarySort : 'recent')
  watch(mode, value => write(key, value))
  const options = computed(() => modes.map(value => ({ value, label: t(`librarySort.${value}`) })))
  function sort<T>(items: T[], id: (item: T) => string, title: (item: T) => string): T[] {
    const collator = new Intl.Collator(locale.value, { numeric: true, sensitivity: 'base' })
    return [...items].sort((a, b) => {
      const byTitle = collator.compare(title(a), title(b)) || id(a).localeCompare(id(b))
      if (mode.value === 'titleAsc') return byTitle
      if (mode.value === 'titleDesc') return -byTitle
      const byUsage = (usage.value[`${library}:${id(b)}`] ?? 0) - (usage.value[`${library}:${id(a)}`] ?? 0)
      return (mode.value === 'oldest' ? -byUsage : byUsage) || byTitle
    })
  }
  return { mode, options, sort }
}
