<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import DictionaryLibrary from './components/DictionaryLibrary.vue'
import DictionaryProof from './components/DictionaryProof.vue'
import FontProfileLibrary from './components/FontProfileLibrary.vue'
import HelpView from './components/HelpView.vue'
import SettingsView from './components/SettingsView.vue'
import SoftwareTable from './components/SoftwareTable.vue'
import TitleBar from './components/TitleBar.vue'
import WorkflowTable from './components/WorkflowTable.vue'
import type { FontProfileDetail, WorkflowDetail, WorkflowTarget } from './model'
import { useWorkspace } from './useWorkspace'

type View = 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'fonts' | 'help' | 'settings'
const desktopApiVersion = 7

const workspace = useWorkspace()
const view = ref<View>('workflows')
const shellCompatibilityError = ref('')

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
      shellCompatibilityError.value = '桌面接口已更新，请重新启动 Glyphshift。当前窗口不会继续调用不兼容的产品命令。'
      return
    }
    shellCompatibilityError.value = ''
    if (!status.shellReady || !await workspace.connectDesktopBackend()) {
      shellCompatibilityError.value = '桌面组件启动失败，无法读取本地产品数据。请重新启动 Glyphshift；如果问题持续，请查看帮助中的排查说明。'
    }
  }
  catch {
    shellCompatibilityError.value = '桌面组件启动失败，无法读取本地产品数据。请重新启动 Glyphshift；如果问题持续，请查看帮助中的排查说明。'
  }
}

onMounted(() => {
  void connectDesktopShell()
})
</script>

<template>
  <UApp class="flex h-full min-h-0 flex-col overflow-hidden bg-[var(--app-bg)] text-[var(--text)]">
    <TitleBar :current="view" @navigate="view = $event" />
    <main class="flex min-h-0 flex-1 overflow-hidden">
      <section v-if="shellCompatibilityError && view !== 'help'" class="grid min-h-0 flex-1 place-items-center bg-[var(--app-bg)] p-6" role="alert">
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
      <HelpView v-else-if="view === 'help'" :adapters="workspace.model.value.adapters" @navigate="view = $event" />
      <SettingsView v-else @navigate="view = $event" />
    </main>
  </UApp>
</template>
