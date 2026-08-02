<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { en, zh_cn } from '@nuxt/ui/locale'
import { useI18n } from 'vue-i18n'
import DictionaryLibrary from './components/DictionaryLibrary.vue'
import DictionaryProof from './components/DictionaryProof.vue'
import FontProfileLibrary from './components/FontProfileLibrary.vue'
import CaptureView from './components/CaptureView.vue'
import HelpView from './components/HelpView.vue'
import SettingsView from './components/SettingsView.vue'
import SoftwareTable from './components/SoftwareTable.vue'
import TitleBar from './components/TitleBar.vue'
import WorkflowTable from './components/WorkflowTable.vue'
import { useAppSettings } from './appSettings'
import type { FontProfileDetail, WorkflowDetail, WorkflowTarget } from './model'
import { useWorkspace } from './useWorkspace'

type View = 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'fonts' | 'capture' | 'help' | 'settings'
const desktopApiVersion = 9

const { t } = useI18n()
const appSettings = useAppSettings()
const workspace = useWorkspace()
const view = ref<View>('workflows')
const shellCompatibilityErrorKey = ref('')
const shellCompatibilityError = computed(() => shellCompatibilityErrorKey.value ? t(shellCompatibilityErrorKey.value) : '')
const nuxtLocale = computed(() => appSettings.effectiveLocale.value === 'en-US' ? en : zh_cn)

async function openWorkflow(id: string) {
  await workspace.loadWorkflow(id)
}

async function openDictionary(id: string) {
  if (await workspace.loadDictionary(id)) view.value = 'dictionary-editor'
}

async function openFontProfile(id: string) {
  await workspace.loadFontProfile(id)
}

async function createWorkflow(name: string, description: string, targets: WorkflowTarget[]) {
  await workspace.createWorkflow(name, description, targets)
}

async function saveFontProfile(detail: FontProfileDetail) {
  if (await workspace.saveFontProfile(detail)) workspace.fontProfileDetail.value = null
}

async function saveWorkflow(detail: WorkflowDetail) {
  if (await workspace.saveWorkflow(detail)) workspace.workflowDetail.value = null
}

async function connectDesktopShell() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const status = await invoke<{ shellReady: boolean; apiVersion: number }>('desktop_status')
    if (status.shellReady && status.apiVersion !== desktopApiVersion) {
      shellCompatibilityErrorKey.value = 'app.desktopApiChanged'
      return
    }
    shellCompatibilityErrorKey.value = ''
    if (!status.shellReady || !await workspace.connectDesktopBackend()) {
      shellCompatibilityErrorKey.value = 'app.desktopUnavailable'
    }
  }
  catch {
    shellCompatibilityErrorKey.value = 'app.desktopUnavailable'
  }
}

onMounted(() => {
  void connectDesktopShell()
})
</script>

<template>
  <UApp :locale="nuxtLocale" class="flex h-full min-h-0 flex-col overflow-hidden bg-[var(--app-bg)] text-[var(--text)]">
    <TitleBar :current="view" @navigate="view = $event" />
    <main class="flex min-h-0 flex-1 overflow-hidden">
      <section v-if="shellCompatibilityError && view !== 'help'" class="grid min-h-0 flex-1 place-items-center bg-[var(--app-bg)] p-6" role="alert">
        <UAlert
          color="warning"
          variant="soft"
          icon="i-tabler-refresh-alert"
          :title="t('app.desktopReloadTitle')"
          :description="shellCompatibilityError"
          class="max-w-[520px]"
        />
      </section>
      <WorkflowTable
        v-else-if="view === 'workflows'"
        :items="workspace.model.value.workflows"
        :software="workspace.model.value.software"
        :dictionaries="workspace.model.value.dictionaries"
        :font-profiles="workspace.model.value.fontProfiles"
        :adapters="workspace.model.value.adapters"
        :activation-ids="workspace.activationIds.value"
        :runtime-status="workspace.model.value.workflowRuntimeStatus"
        :busy="workspace.workspaceBusy.value"
        :refreshing="workspace.refreshing.value"
        :messages="workspace.messages.value"
        :editing="workspace.workflowDetail.value"
        @toggle="workspace.setWorkflowEnabled"
        @refresh="workspace.refreshWorkflows"
        @open="openWorkflow"
        @create="createWorkflow"
        @save="saveWorkflow"
        @close-edit="workspace.workflowDetail.value = null"
        @copy="workspace.copyWorkflow"
        @remove="workspace.removeWorkflows"
        @toggle-many="workspace.setWorkflowsEnabled"
        @navigate="view = $event"
      />
      <SoftwareTable
        v-else-if="view === 'software'"
        :items="workspace.model.value.software"
        :busy="workspace.softwareBusy.value"
        :messages="workspace.messages.value"
        @add="workspace.addSoftware"
        @update-software="workspace.updateSoftware"
        @remove="workspace.removeSoftware"
      />
      <DictionaryLibrary
        v-else-if="view === 'dictionaries'"
        :items="workspace.model.value.dictionaries"
        :busy="workspace.workspaceBusy.value"
        :messages="workspace.messages.value"
        @open="openDictionary"
        @create="workspace.createDictionary"
        @remove="workspace.removeDictionaries"
      />
      <DictionaryProof
        v-else-if="view === 'dictionary-editor' && workspace.dictionaryDetail.value"
        :detail="workspace.dictionaryDetail.value"
        :busy="workspace.workspaceBusy.value"
        @back="view = 'dictionaries'"
        @save="workspace.saveDictionary"
      />
      <FontProfileLibrary
        v-else-if="view === 'fonts'"
        :items="workspace.model.value.fontProfiles"
        :workflows="workspace.model.value.workflows"
        :installed-families="workspace.model.value.fontFamilies"
        :editing="workspace.fontProfileDetail.value"
        :busy="workspace.workspaceBusy.value"
        :messages="workspace.messages.value"
        @open="openFontProfile"
        @close-edit="workspace.fontProfileDetail.value = null"
        @create="workspace.createFontProfile"
        @save="saveFontProfile"
        @remove="workspace.removeFontProfiles"
      />
      <CaptureView
        v-else-if="view === 'capture'"
        :software="workspace.model.value.software"
        :adapters="workspace.model.value.adapters"
        :capture="workspace.model.value.capture"
        :result="workspace.captureResult.value"
        :busy="workspace.captureBusy.value"
        :message="workspace.messages.value.capture ?? ''"
        @start="workspace.startCapture"
        @stop="workspace.stopCapture"
      />
      <HelpView v-else-if="view === 'help'" :adapters="workspace.model.value.adapters" @navigate="view = $event" />
      <SettingsView v-else @navigate="view = $event" />
    </main>
  </UApp>
</template>
