import type { DictionaryEntry, DictionaryMetadata } from './model'

export interface DictionaryImportData {
  entries: DictionaryEntry[]
  metadata: DictionaryMetadata
  mode: 'append' | 'overwrite' | 'replace'
}

export function mergeDictionaryEntries(existing: DictionaryEntry[], incoming: DictionaryEntry[], mode: DictionaryImportData['mode']): DictionaryEntry[] {
  const entries = new Map((mode === 'replace' ? [] : existing).map(entry => [entry.source, { ...entry }]))
  for (const entry of incoming) {
    if (mode !== 'append' || !entries.has(entry.source)) entries.set(entry.source, { ...entry })
  }
  return [...entries.values()]
}

// File order decides conflicting non-empty translations; empty values never erase them.
export function combineImportEntries(files: DictionaryEntry[][], priority: 'first' | 'last'): DictionaryEntry[] {
  const entries = new Map<string, DictionaryEntry>()
  for (const file of files) for (const entry of file) {
    const old = entries.get(entry.source)
    if (!old || (!old.translation.trim() && entry.translation.trim()) || (priority === 'last' && entry.translation.trim())) {
      entries.set(entry.source, { ...entry })
    }
  }
  return [...entries.values()]
}
