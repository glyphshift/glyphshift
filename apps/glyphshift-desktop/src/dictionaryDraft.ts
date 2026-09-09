import type { DictionaryDetail, DictionaryEntry } from './model'

// Rebase a draft onto a new persisted revision. Conflicting local edits remain visible,
// but callers must get an explicit choice before allowing them to be saved.
export function mergeDictionaryDraft(base: DictionaryDetail, local: DictionaryDetail, remote: DictionaryDetail) {
  const conflicts: string[] = []
  const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)
  function merge<T>(before: T, mine: T, theirs: T, label: string): T {
    if (equal(mine, before)) return theirs
    if (equal(theirs, before) || equal(mine, theirs)) return mine
    conflicts.push(label)
    return mine
  }
  const metadata = { ...remote.metadata }
  for (const key of Object.keys(local.metadata) as (keyof typeof metadata)[]) {
    Object.assign(metadata, { [key]: merge(base.metadata[key], local.metadata[key], remote.metadata[key], `metadata.${key}`) })
  }
  const bySource = (entries: DictionaryEntry[]) => new Map(entries.map(entry => [entry.source, entry]))
  const original = bySource(base.entries)
  const edited = bySource(local.entries)
  const localGroups = new Map<string, DictionaryEntry[]>()
  for (const entry of local.entries) {
    const group = localGroups.get(entry.source) ?? []
    group.push(entry)
    localGroups.set(entry.source, group)
  }
  const incoming = bySource(remote.entries)
  const sources = new Set([...edited.keys(), ...incoming.keys(), ...original.keys()])
  const entries: DictionaryEntry[] = []
  for (const source of sources) {
    const matchingLocal = localGroups.get(source) ?? []
    if (matchingLocal.length > 1) {
      // Partially edited source fields can temporarily collide. Do not collapse rows.
      entries.push(...matchingLocal.map(entry => ({ ...entry })))
      conflicts.push(source)
      continue
    }
    const entry = merge(original.get(source), edited.get(source), incoming.get(source), source)
    if (entry) entries.push({ ...entry })
  }
  return { detail: { metadata, revision: remote.revision, entries }, conflicts }
}
