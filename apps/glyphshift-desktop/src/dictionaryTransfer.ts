import { invoke } from '@tauri-apps/api/core'
import type { DictionaryEntry, DictionaryMetadata } from './model'
import { translateCommandError } from './commandError'

export const dictionaryFormats = [
  { extension: 'json', label: 'JSON', importable: true, exportable: true },
  { extension: 'csv', label: 'CSV', importable: true, exportable: true },
  { extension: 'srt', label: 'SRT', importable: true, exportable: false },
] as const
export type ImportFormat = typeof dictionaryFormats[number]['extension']
export type ImportFile = { name: string; entries: DictionaryEntry[]; metadata: DictionaryMetadata }
export function emptyImportMetadata(name: string): DictionaryMetadata {
  return { id: '', name, description: '', sourceLocale: '', targetLocale: '', releaseVersion: '0.1.0', authors: [], tags: [], license: null, homepage: null }
}
// Preview every file before any dictionary is created. Keep failures associated with filenames.
export async function previewImportFiles(paths: string[]): Promise<ImportFile[]> {
  const files: ImportFile[] = []
  const errors: string[] = []
  for (const inputPath of paths) {
    const name = inputPath.split(/[\\/]/).pop() ?? inputPath
    try {
      const preview = await invoke<{ entries: DictionaryEntry[]; metadata: DictionaryMetadata | null }>('desktop_preview_dictionary_import', { inputPath })
      files.push({ name, entries: preview.entries, metadata: preview.metadata ?? emptyImportMetadata(name.replace(/\.[^.]+$/, '')) })
    } catch (cause) { errors.push(`${name}: ${translateCommandError(cause)}`) }
  }
  if (errors.length) throw new Error(errors.join('\n'))
  return files
}
export async function exportDictionaryFile(dictionaryId: string, outputPath: string) {
  await invoke('desktop_export_dictionary', { dictionaryId, outputPath })
}
