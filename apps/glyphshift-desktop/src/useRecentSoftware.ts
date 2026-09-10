import { computed, type Ref } from 'vue'
import { useAppSettings } from './appSettings'
import { normalizeCatalog } from './settingsCatalogs'
import type { SoftwareRecord } from './model'

export function useRecentSoftware(software: Readonly<Ref<SoftwareRecord[]>>) {
  const app = useAppSettings()
  const ids = computed(() => {
    if (app.settings.value.recentSoftwareIds !== null) return app.settings.value.recentSoftwareIds
    try {
      const legacy = localStorage.getItem('glyphshift.recent-software.v1')
      if (legacy !== null) return normalizeCatalog(JSON.parse(legacy), 20, 128)
    } catch { /* An unreadable legacy cache does not prevent selecting a target. */ }
    return software.value.map(item => item.id).slice(0, 20)
  })
  const records = computed(() => ids.value.map(id => software.value.find(item => item.id === id)).filter((item): item is SoftwareRecord => Boolean(item)))
  return {
    ids, records,
    remember: (id: string) => app.setRecentSoftwareIds([id, ...ids.value.filter(value => value !== id)].slice(0, 20)),
    remove: (id: string) => app.setRecentSoftwareIds(ids.value.filter(value => value !== id)),
    clear: () => app.setRecentSoftwareIds([]),
  }
}
