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
