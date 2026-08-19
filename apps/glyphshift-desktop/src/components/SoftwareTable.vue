<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import { translateCommandError } from '../commandError'
import type { SoftwarePreflight, SoftwareRecord } from '../model'
import {
  editableRowIndex,
  managementActionsColumnMeta,
  managementIdentityColumnMeta,
  managementSelectionColumnMeta,
} from '../tableInteraction'
import { usePageEscape } from '../usePageEscape'
import { useTableColumns } from '../useTableColumns'

const props = defineProps<{
  items: SoftwareRecord[]
  busy: boolean
  messages: Record<string, string>
  preflight: SoftwarePreflight | null
  preflightBusy: boolean
  captureArmed: boolean
  captureShortcut: string
  captureResult: SoftwarePreflight | null
}>()
const emit = defineEmits<{
  add: [name: string, description: string, path: string]
  validate: [path: string]
  clearPreflight: []
  armCapture: []
  cancelCapture: []
  updateSoftware: [id: string, name: string, description: string, path: string]
  remove: [ids: string[]]
  'dirty-change': [dirty: boolean]
}>()
const { t } = useI18n()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const { columns, toggleColumn } = useTableColumns('glyphshift.table-columns.software', {
  description: true,
  binding: true,
})
const editing = ref<SoftwareRecord | null>(null)
const editName = ref('')
const editDescription = ref('')
const editPath = ref('')
const editBaseline = ref('')
const discardEditOpen = ref(false)
const pendingRemoval = ref<string[]>([])
const adding = ref(false)
const addName = ref('')
const addDescription = ref('')
const addPath = ref('')
const addSubmitting = ref(false)
const addStartCount = ref(0)
const runningTargets = ref<SoftwarePreflight[]>([])
const runningTargetPath = ref('')
const runningTargetsLoading = ref(false)
const runningTargetsLoaded = ref(false)
const runningTargetsError = ref('')
let runningTargetsRequest = 0

function normalizedPath(path: string) {
  return path.trim().replace(/^\\\\\?\\/, '').replace(/\//g, '\\').toLocaleLowerCase()
}

const activePreflight = computed(() => {
  if (!props.preflight || normalizedPath(props.preflight.executablePath) !== normalizedPath(addPath.value)) return null
  return props.preflight
})
const preflightPresentation = computed(() => {
  const result = activePreflight.value
  if (!result) return null
  switch (result.state) {
    case 'ready':
      return {
        color: 'success' as const,
        icon: 'i-tabler-circle-check',
        title: t('software.preflight.readyTitle'),
        description: t('software.preflight.readyDescription'),
      }
    case 'already_added':
      return {
        color: 'error' as const,
        icon: 'i-tabler-copy-x',
        title: t('software.preflight.alreadyAddedTitle'),
        description: t('software.preflight.alreadyAddedDescription', { name: result.existingName ?? result.executableName }),
      }
    case 'not_running':
      return {
        color: 'warning' as const,
        icon: 'i-tabler-player-pause',
        title: t('software.preflight.notRunningTitle'),
        description: t('software.preflight.notRunningDescription'),
      }
    case 'runtime_unavailable':
      return {
        color: 'error' as const,
        icon: 'i-tabler-plug-connected-x',
        title: t('software.preflight.runtimeUnavailableTitle'),
        description: t('software.preflight.runtimeUnavailableDescription'),
      }
    case 'self_target':
      return {
        color: 'error' as const,
        icon: 'i-tabler-app-window',
        title: t('software.preflight.selfTargetTitle'),
        description: t('software.preflight.selfTargetDescription'),
      }
    case 'unsupported_architecture':
      return {
        color: 'error' as const,
        icon: 'i-tabler-cpu-off',
        title: t('software.preflight.unsupportedArchitectureTitle'),
        description: t('software.preflight.unsupportedArchitectureDescription', { architecture: result.architecture }),
      }
  }
})
const runningTargetItems = computed(() => runningTargets.value.map(target => ({
  value: target.executablePath,
  label: `${target.existingName ?? target.suggestedName} · ${target.executableName} · ${target.architecture}`,
})))

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter(item => !needle
    || `${item.name} ${item.description} ${item.vendor} ${item.version} ${item.executablePath ?? item.executableName}`
      .toLocaleLowerCase()
      .includes(needle))
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.id)))
const columnOptions = computed(() => [
  { key: 'description', label: t('software.columns.description'), visible: columns.value.description },
  { key: 'binding', label: t('software.columns.binding'), visible: columns.value.binding },
])
const tableColumns = computed<TableColumn<SoftwareRecord>[]>(() => [
  { id: 'select', header: '', meta: managementSelectionColumnMeta() },
  { id: 'software', header: t('software.columns.software'), meta: managementIdentityColumnMeta('w-52') },
  ...(columns.value.description ? [{ id: 'description', header: t('software.columns.description'), meta: { class: { th: 'w-[28%]', td: 'w-[28%]' } } } satisfies TableColumn<SoftwareRecord>] : []),
  ...(columns.value.binding ? [{ id: 'binding', header: t('software.columns.binding') } satisfies TableColumn<SoftwareRecord>] : []),
  { id: 'actions', header: t('software.columns.actions'), meta: managementActionsColumnMeta('w-24') },
])
const editDirty = computed(() => Boolean(editing.value) && JSON.stringify({
  name: editName.value,
  description: editDescription.value,
  path: editPath.value,
}) !== editBaseline.value)

watch(editDirty, dirty => emit('dirty-change', dirty), { immediate: true })

watch([query, pageSize], () => { page.value = 1 })
watch(() => filtered.value.length, () => {
  if (page.value > pageCount.value) page.value = pageCount.value
})
watch(() => props.items.length, (count) => {
  if (adding.value && addSubmitting.value && count > addStartCount.value) {
    adding.value = false
    addSubmitting.value = false
  }
})
watch(() => props.items.map(item => item.id), (ids) => {
  const existing = new Set(ids)
  selected.value = new Set([...selected.value].filter(id => existing.has(id)))
})
watch(() => props.messages.software, (message) => {
  if (message) addSubmitting.value = false
})
watch(() => props.captureResult, (result) => {
  if (!result) return
  const existingIndex = runningTargets.value.findIndex(item => normalizedPath(item.executablePath) === normalizedPath(result.executablePath))
  if (existingIndex >= 0) runningTargets.value.splice(existingIndex, 1, result)
  else runningTargets.value.unshift(result)
  runningTargetPath.value = result.executablePath
  addName.value = result.suggestedName
  addDescription.value = ''
  addPath.value = result.executablePath
  addSubmitting.value = false
  addStartCount.value = props.items.length
  adding.value = true
}, { immediate: true })

function toggle(id: string) {
  const next = new Set(selected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selected.value = next
}

function openOnDoubleClick(event: MouseEvent) {
  const index = editableRowIndex(event)
  const item = index === null ? null : pageItems.value[index]
  if (item) startEdit(item)
}

function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) pageItems.value.forEach(item => next.delete(item.id))
  else pageItems.value.forEach(item => next.add(item.id))
  selected.value = next
}

function startEdit(item: SoftwareRecord) {
  editing.value = item
  editName.value = item.name
  editDescription.value = item.description
  editPath.value = item.executablePath ?? ''
  editBaseline.value = JSON.stringify({ name: editName.value, description: editDescription.value, path: editPath.value })
}

function closeEdit() {
  discardEditOpen.value = false
  editing.value = null
}

function requestCloseEdit() {
  if (editDirty.value) discardEditOpen.value = true
  else closeEdit()
}

function saveEdit() {
  if (!editing.value || !editName.value.trim() || !editPath.value.trim()) return
  emit('updateSoftware', editing.value.id, editName.value.trim(), editDescription.value.trim(), editPath.value.trim())
  editing.value = null
}

function closeRemoval() {
  pendingRemoval.value = []
}

function confirmRemoval() {
  emit('remove', [...pendingRemoval.value])
  pendingRemoval.value = []
}

function startAdd() {
  emit('clearPreflight')
  addName.value = ''
  addDescription.value = ''
  addPath.value = ''
  addSubmitting.value = false
  addStartCount.value = props.items.length
  runningTargets.value = []
  runningTargetPath.value = ''
  runningTargetsLoaded.value = false
  runningTargetsError.value = ''
  adding.value = true
  void loadRunningTargets()
}

function updateAddOpen(open: boolean) {
  adding.value = open
  if (!open) {
    addSubmitting.value = false
    if (props.captureArmed) emit('cancelCapture')
    emit('clearPreflight')
  }
}

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

async function loadRunningTargets(force = false) {
  if (runningTargetsLoading.value || (runningTargetsLoaded.value && !force)) return
  const request = ++runningTargetsRequest
  runningTargetsLoading.value = true
  runningTargetsError.value = ''
  try {
    const result = hasDesktopRuntime()
      ? await invoke<SoftwarePreflight[]>('desktop_running_software_targets')
      : []
    if (request !== runningTargetsRequest) return
    runningTargets.value = Array.isArray(result) ? result : []
    runningTargetsLoaded.value = true
  }
  catch (error) {
    if (request !== runningTargetsRequest) return
    runningTargets.value = []
    runningTargetPath.value = ''
    runningTargetsLoaded.value = true
    runningTargetsError.value = translateCommandError(error)
  }
  finally {
    if (request === runningTargetsRequest) runningTargetsLoading.value = false
  }
}

function chooseRunningTarget(value: unknown) {
  const path = String(value ?? '')
  const target = runningTargets.value.find(item => item.executablePath === path)
  runningTargetPath.value = path
  if (!target) return
  addName.value = target.existingName ?? target.suggestedName
  addPath.value = target.executablePath
  emit('validate', target.executablePath)
}

function toggleForegroundCapture() {
  if (props.captureArmed) emit('cancelCapture')
  else emit('armCapture')
}

async function browseExecutable() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: t('software.selectWindowsApp'),
      filters: [{ name: t('software.windowsApp'), extensions: ['exe'] }],
    })
    if (typeof selected !== 'string') return
    addPath.value = selected
    if (!addName.value.trim()) addName.value = (selected.split(/[\\/]/).pop() ?? '').replace(/\.exe$/i, '')
    emit('validate', selected)
  }
  catch {
    // 浏览器预览仍允许手动输入合成路径；桌面壳会提供真实文件选择器。
  }
}

function validateExecutable() {
  if (addPath.value.trim()) emit('validate', addPath.value.trim())
}

function submitAdd() {
  if (!addName.value.trim() || !addPath.value.trim() || !activePreflight.value?.canAdd) return
  addSubmitting.value = true
  emit('add', addName.value.trim(), addDescription.value.trim(), addPath.value.trim())
}

usePageEscape(() => Boolean(editing.value), requestCloseEdit)

</script>

<template>
  <section :data-testid="editing ? 'software-editor' : undefined" class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" :aria-labelledby="editing ? 'software-editor-title' : 'software-title'">
    <template v-if="!editing">
    <ManagementPageHeader
      title-id="software-title"
      :title="t('software.title')"
      icon="i-tabler-apps"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('software.add')" :disabled="busy" @click="startAdd" />
      </template>
    </ManagementPageHeader>

    <div v-if="messages.software" :aria-label="t('software.error')" class="mb-3" role="alert">
      <UAlert color="error" variant="soft" :title="t('software.error')" :description="messages.software" />
    </div>

    <ManagementTableFrame
      v-model:query="query"
      v-model:page="page"
      v-model:page-size="pageSize"
      :search-placeholder="t('software.searchPlaceholder')"
      :search-label="t('software.searchLabel')"
      :column-options="columnOptions"
      :columns-label="t('table.columns')"
      :selected-count="selected.size"
      :selected-label="t('software.itemLabel')"
      :total="filtered.length"
      :item-label="t('software.itemLabel')"
      @toggle-column="toggleColumn"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('software.bulkDelete')" :disabled="busy" @click="pendingRemoval = [...selected]" />
      </template>

      <UTable data-testid="software-management-table" role="region" tabindex="0" aria-labelledby="software-title" :data="pageItems" :columns="tableColumns" sticky class="management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[760px]' }" @dblclick="openOnDoubleClick">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" :aria-label="t('software.selectPage')" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.id)" :aria-label="t('common.selectNamed', { name: row.original.name })" @update:model-value="toggle(row.original.id)" />
        </template>
        <template #software-cell="{ row }">
          <div class="truncate font-semibold" :title="row.original.name">{{ row.original.name }}</div>
        </template>
        <template #description-cell="{ row }">
          <div class="truncate text-[var(--text-muted)]" :title="row.original.description || undefined">
            {{ row.original.description || t('common.noDescription') }}
          </div>
        </template>
        <template #binding-cell="{ row }">
          <div class="truncate" :title="row.original.executablePath ?? row.original.executableName">
            {{ row.original.executablePath ?? row.original.executableName }}
          </div>
          <div class="type-metadata mt-0.5 truncate text-[var(--text-muted)]">{{ row.original.vendor }} · {{ row.original.version }}</div>
        </template>
        <template #actions-cell="{ row }">
          <div class="flex items-center justify-center gap-1">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('common.editNamed', { name: row.original.name })" @click="startEdit(row.original)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.name })" @click="pendingRemoval = [row.original.id]" />
          </div>
        </template>
        <template #empty>
          <UEmpty
            icon="i-tabler-apps"
            :title="items.length ? t('software.noMatch') : t('software.empty')"
            :description="items.length ? t('software.adjustSearch') : t('software.emptyDescription')"
          >
            <template v-if="!items.length" #actions>
              <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('software.add')" @click="startAdd" />
            </template>
          </UEmpty>
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="adding"
      :title="t('software.addTitle')"
      :confirm-label="t('software.addConfirm')"
      :confirm-disabled="busy || !addName.trim() || !addPath.trim() || !activePreflight?.canAdd"
      :busy="addSubmitting"
      @update:open="updateAddOpen"
      @confirm="submitAdd"
    >
      <div class="space-y-4">
        <UFormField :label="t('software.name')" required>
          <UInput v-model="addName" autofocus size="sm" class="w-full" :aria-label="t('software.name')" />
        </UFormField>
        <UFormField :label="t('software.softwareDescription')">
          <UTextarea v-model="addDescription" :maxlength="512" :rows="2" autoresize class="w-full" :placeholder="t('software.descriptionPlaceholder')" :aria-label="t('software.softwareDescription')" />
        </UFormField>

        <div class="space-y-2 rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
          <div class="flex items-center justify-between gap-3">
            <div>
              <p class="m-0 text-xs font-semibold text-[var(--text)]">{{ t('software.runningSoftware') }}</p>
              <p class="type-metadata m-0 mt-0.5 leading-4 text-[var(--text-muted)]">{{ t('software.runningSoftwareHint') }}</p>
            </div>
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-refresh" :aria-label="t('software.refreshRunningSoftware')" :loading="runningTargetsLoading" @click="loadRunningTargets(true)" />
          </div>
          <USelect
            :model-value="runningTargetPath"
            :items="runningTargetItems"
            value-key="value"
            label-key="label"
            :placeholder="t('software.runningSoftwarePlaceholder')"
            :aria-label="t('software.runningSoftware')"
            :loading="runningTargetsLoading"
            :disabled="runningTargetsLoading || !runningTargetItems.length"
            class="w-full"
            @update:model-value="chooseRunningTarget"
          />
          <UAlert v-if="runningTargetsError" color="error" variant="soft" icon="i-tabler-alert-circle" :title="t('software.error')" :description="runningTargetsError" />
          <p v-else-if="runningTargetsLoaded && !runningTargets.length" class="type-metadata m-0 leading-4 text-[var(--text-muted)]">{{ t('software.runningSoftwareEmpty') }}</p>
          <div class="flex items-center justify-between gap-3 pt-1">
            <p class="type-metadata m-0 leading-4 text-[var(--text-muted)]">{{ t('software.captureWaitingDescription', { shortcut: captureShortcut }) }}</p>
            <UButton color="neutral" :variant="captureArmed ? 'soft' : 'outline'" size="sm" :icon="captureArmed ? 'i-tabler-x' : 'i-tabler-focus-centered'" :label="captureArmed ? t('software.cancelCapture') : t('software.quickCapture')" @click="toggleForegroundCapture" />
          </div>
        </div>

        <UAlert v-if="captureArmed" color="primary" variant="soft" icon="i-tabler-keyboard" :title="t('software.captureWaitingTitle')" :description="t('software.captureWaitingDescription', { shortcut: captureShortcut })" />

        <UFormField :label="t('software.executablePath')" required>
          <div class="flex gap-2">
            <UInput v-model="addPath" size="sm" class="min-w-0 flex-1" :placeholder="t('software.pathPlaceholder')" :aria-label="t('software.executablePath')" />
            <UButton color="neutral" variant="outline" size="sm" :label="t('software.browse')" @click="browseExecutable" />
            <UButton
              color="neutral"
              variant="outline"
              size="sm"
              :loading="preflightBusy"
              :label="t('software.validate')"
              :disabled="!addPath.trim()"
              @click="validateExecutable"
            />
          </div>
        </UFormField>
        <UAlert
          v-if="preflightPresentation"
          :color="preflightPresentation.color"
          variant="soft"
          :icon="preflightPresentation.icon"
          :title="preflightPresentation.title"
          :description="preflightPresentation.description"
        />
        <p v-else-if="messages.software" class="m-0 text-xs leading-5 text-[var(--error)]" role="alert">{{ messages.software }}</p>
        <p v-else class="m-0 text-xs leading-5 text-[var(--text-muted)]">{{ t('software.preflightRequired') }}</p>
      </div>
    </ManagementFormModal>
    </template>

    <template v-else>
      <ManagementDetailHeader
        title-id="software-editor-title"
        :title="editName.trim() || editing.name"
        :back-label="t('software.backToList')"
        @back="requestCloseEdit"
      >
        <template #status><UBadge v-if="editDirty" color="warning" variant="subtle" size="sm" :label="t('common.unsaved')" /></template>
        <template #actions>
          <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('software.saveChanges')" :loading="busy" :disabled="busy || !editName.trim() || !editPath.trim()" @click="saveEdit" />
        </template>
      </ManagementDetailHeader>
      <ManagementWorkspaceSurface variant="canvas">
        <div class="h-full overflow-y-auto p-5 [scrollbar-gutter:stable]">
          <ManagementFormSection :title="t('software.informationHeading')">
            <ManagementFormRow :label="t('software.displayName')" required>
              <UInput v-model="editName" size="sm" class="w-full" :aria-label="t('software.displayName')" />
            </ManagementFormRow>
            <ManagementFormRow :label="t('software.softwareDescription')" multiline>
              <UTextarea v-model="editDescription" :maxlength="512" :rows="3" autoresize class="w-full" :aria-label="t('software.softwareDescription')" />
            </ManagementFormRow>
            <ManagementFormRow :label="t('software.executablePath')" required>
              <UInput v-model="editPath" size="sm" class="w-full" :aria-label="t('software.executablePath')" />
            </ManagementFormRow>
          </ManagementFormSection>
        </div>
      </ManagementWorkspaceSurface>
    </template>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('software.deleteTitle')"
      :description="t('software.deleteDescription', { count: pendingRemoval.length })"
      :confirm-label="t('software.deleteConfirm')"
      :busy="busy"
      @update:open="$event || closeRemoval()"
      @confirm="confirmRemoval"
    />
    <ConfirmDialog
      :open="discardEditOpen"
      :title="t('common.discardTitle')"
      :description="t('common.discardDescription')"
      :cancel-label="t('common.continueEditing')"
      :confirm-label="t('common.discardChanges')"
      confirm-color="warning"
      @update:open="$event || (discardEditOpen = false)"
      @confirm="closeEdit"
    />
  </section>
</template>
