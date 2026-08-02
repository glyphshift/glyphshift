<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import DictionaryLibrary from './components/DictionaryLibrary.vue'
import DictionaryProof from './components/DictionaryProof.vue'
import SettingsView from './components/SettingsView.vue'
import SoftwareTable from './components/SoftwareTable.vue'
import TitleBar from './components/TitleBar.vue'
import WorkflowTable from './components/WorkflowTable.vue'
import type { WorkflowDetail } from './model'
import { useWorkspace } from './useWorkspace'

type View = 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'settings'
const desktopApiVersion = 5

const workspace = useWorkspace()
const view = ref<View>('workflows')
const desktopShellReady = ref(false)
const shellCompatibilityError = ref('')

async function openWorkflow(id: string) {
  await workspace.loadWorkflow(id)
}

async function openDictionary(id: string) {
  if (await workspace.loadDictionary(id)) view.value = 'dictionary-editor'
}

async function createWorkflow(name: string, description: string, softwareIds: string[], dictionaryIds: string[]) {
  await workspace.createWorkflow(name, description, softwareIds.map(softwareId => ({ softwareId, dictionaryIds })))
}

async function saveWorkflow(detail: WorkflowDetail) {
  if (await workspace.saveWorkflow(detail)) workspace.workflowDetail.value = null
}

async function connectDesktopShell() {
  try {
    const status = await invoke<{ shellReady: boolean; apiVersion: number }>('desktop_status')
    if (status.shellReady && status.apiVersion !== desktopApiVersion) {
      shellCompatibilityError.value = '桌面接口已更新，请重新启动 Glyphshift。当前窗口不会继续调用不兼容的产品命令。'
      desktopShellReady.value = false
      return
    }
    shellCompatibilityError.value = ''
    desktopShellReady.value = status.shellReady && await workspace.connectDesktopBackend()
  }
  catch {
    desktopShellReady.value = false
  }
}

onMounted(() => {
  void connectDesktopShell()
})
</script>

<template>
  <UApp class="flex h-full min-h-0 flex-col overflow-hidden bg-[var(--app-bg)] text-[var(--text)]">
    <TitleBar :current="view" :connected="desktopShellReady" @navigate="view = $event" />
    <main class="flex min-h-0 flex-1 overflow-hidden">
      <section v-if="shellCompatibilityError" class="grid min-h-0 flex-1 place-items-center bg-[var(--app-bg)] p-6" role="alert">
        <UAlert
          color="warning"
          variant="soft"
          icon="i-tabler-refresh-alert"
          title="桌面组件需要重新加载"
          :description="shellCompatibilityError"
          class="max-w-[520px]"
        />
      </section>
      <WorkflowTable
        v-else-if="view === 'workflows'"
        :items="workspace.model.value.workflows"
        :software="workspace.model.value.software"
        :dictionaries="workspace.model.value.dictionaries"
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
        :hook-types="workspace.model.value.hookTypes"
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
        :hook-types="workspace.model.value.hookTypes"
        :font-families="workspace.model.value.fontFamilies"
        @back="view = 'dictionaries'"
        @save="workspace.saveDictionary"
      />
      <SettingsView v-else :translation-source="workspace.model.value.translationSource" @source="workspace.setTranslationSource" />
    </main>
  </UApp>
</template>
