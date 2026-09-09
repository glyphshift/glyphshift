import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useAppSettings } from './appSettings'

interface Release { version: string; releaseNotes: string; downloadUrl: string }
const release = ref<Release | null>(null)
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
    opening.value = true
    openFailed.value = false
    try { await openUrl(release.value.downloadUrl) } catch { openFailed.value = true }
    finally { opening.value = false }
  }
  return { release, checking, status, opening, openFailed, check, cancelAutomatic, download,
    visible: computed(() => Boolean(release.value) && !dismissed.value),
    dismiss: () => { dismissed.value = true; status.value = 'idle' },
  }
}
