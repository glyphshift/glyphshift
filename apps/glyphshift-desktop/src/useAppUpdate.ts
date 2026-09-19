import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppSettings } from './appSettings'

interface UpdateSource {
  id: string
  kind: string
  label: string
  action: 'install' | 'open'
  provider: string
  extractionCode: string
}
interface Release { version: string; releaseNotes: string; sources: UpdateSource[] }
const release = ref<Release | null>(null)
const selectedSourceId = ref('')
const checking = ref(false)
const status = ref<'idle' | 'latest' | 'available' | 'failed'>('idle')
const dismissed = ref(false)
const opening = ref(false)
const openFailed = ref(false)
let generation = 0

export function useAppUpdate() {
  const settings = useAppSettings()
  async function check(manual = true) {
    if (checking.value || (!manual && !settings.settings.value.checkUpdatesOnStartup)) return
    const attempt = ++generation
    checking.value = true
    status.value = 'idle'
    try {
      const result = await invoke<Release | null>('desktop_check_update')
      if (attempt !== generation) return
      release.value = result
      selectedSourceId.value = result?.sources.find(source => source.action === 'install')?.id ?? result?.sources[0]?.id ?? ''
      status.value = result ? 'available' : 'latest'
      dismissed.value = false
      openFailed.value = false
    } catch {
      if (attempt === generation) status.value = manual ? 'failed' : 'idle'
    } finally {
      if (attempt === generation) checking.value = false
    }
  }
  function cancelAutomatic() {
    generation++
    checking.value = false
    dismissed.value = true
    status.value = 'idle'
  }
  async function download() {
    if (!release.value || opening.value) return
    const source = release.value.sources.find(item => item.id === selectedSourceId.value)
    if (!source) return
    opening.value = true
    openFailed.value = false
    try {
      const args = { version: release.value.version, sourceId: source.id }
      if (source.action === 'install') await invoke('desktop_install_update', args)
      else await invoke('desktop_open_update_source', args)
    } catch { openFailed.value = true }
    finally { opening.value = false }
  }
  const selectedSource = computed(() => release.value?.sources.find(source => source.id === selectedSourceId.value) ?? null)
  const sourceItems = computed(() => release.value?.sources.map(source => ({ value: source.id, label: source.label })) ?? [])
  return { release, selectedSourceId, selectedSource, sourceItems, checking, status, opening, openFailed, check, cancelAutomatic, download,
    visible: computed(() => Boolean(release.value) && !dismissed.value),
    dismiss: () => { dismissed.value = true; status.value = 'idle' },
  }
}
