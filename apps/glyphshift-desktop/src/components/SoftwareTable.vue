<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type { SoftwarePreflight, SoftwareRecord } from '../model'
import { editableRowIndex } from '../tableInteraction'
import { usePageEscape } from '../usePageEscape'

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
const columns = ref({ description: true, binding: true })
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
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'software', header: t('software.columns.software'), meta: { class: { th: 'w-[22%]', td: 'w-[22%]' } } },
  ...(columns.value.description ? [{ id: 'description', header: t('software.columns.description'), meta: { class: { th: 'w-[28%]', td: 'w-[28%]' } } } satisfies TableColumn<SoftwareRecord>] : []),
  ...(columns.value.binding ? [{ id: 'binding', header: t('software.columns.binding') } satisfies TableColumn<SoftwareRecord>] : []),
  { id: 'actions', header: t('software.columns.actions'), meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } },
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

function toggleColumn(key: string, visible: boolean) {
  if (key === 'description' || key === 'binding') columns.value[key] = visible
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
  adding.value = true
}

function updateAddOpen(open: boolean) {
  adding.value = open
  if (!open) {
    addSubmitting.value = false
    emit('clearPreflight')
  }
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
      :description="t('software.description')"
      icon="i-tabler-apps"
    >
      <template #actions>
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-tabler-focus-2"
          :label="t('software.quickCapture')"
          :disabled="busy || captureArmed"
          @click="emit('armCapture')"
        />
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('software.add')" :disabled="busy" @click="startAdd" />
      </template>
    </ManagementPageHeader>

    <div
      v-if="captureArmed"
      :aria-label="t('software.captureWaitingTitle')"
      class="mb-3 flex items-center gap-3 rounded-[var(--radius-control)] border border-[color-mix(in_srgb,var(--primary)_35%,var(--border))] bg-[color-mix(in_srgb,var(--primary)_10%,var(--surface))] px-3 py-2"
      role="alert"
    >
      <UIcon name="i-tabler-focus-centered" class="size-4 shrink-0 text-[var(--primary)]" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <p class="m-0 text-xs font-semibold text-[var(--text)]">{{ t('software.captureWaitingTitle') }}</p>
        <p class="m-0 mt-0.5 text-[11px] leading-4 text-[var(--text-muted)]">{{ t('software.captureWaitingDescription', { shortcut: captureShortcut }) }}</p>
      </div>
      <UButton color="neutral" variant="outline" size="xs" :label="t('software.cancelCapture')" @click="emit('cancelCapture')" />
    </div>

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

      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[760px]' }" @dblclick="openOnDoubleClick">
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
          <div class="mt-0.5 truncate text-[9px] text-[var(--text-muted)]">{{ row.original.vendor }} · {{ row.original.version }}</div>
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
      :description="t('software.addDescription')"
      :confirm-label="t('software.addConfirm')"
      :confirm-disabled="busy || !addName.trim() || !addPath.trim() || !activePreflight?.canAdd"
      :busy="addSubmitting"
      @update:open="updateAddOpen"
      @confirm="submitAdd"
    >
      <div class="space-y-3">
        <UFormField :label="t('software.name')" required>
          <UInput v-model="addName" autofocus size="sm" class="w-full" :aria-label="t('software.name')" />
        </UFormField>
        <UFormField :label="t('software.softwareDescription')">
          <UTextarea v-model="addDescription" :maxlength="512" :rows="2" autoresize class="w-full" :placeholder="t('software.descriptionPlaceholder')" :aria-label="t('software.softwareDescription')" />
        </UFormField>
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
        :description="t('software.editDescription')"
        :back-label="t('software.backToList')"
        @back="requestCloseEdit"
      >
        <template #status><UBadge v-if="editDirty" color="warning" variant="subtle" size="sm" :label="t('common.unsaved')" /></template>
        <template #actions>
          <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('software.saveChanges')" :loading="busy" :disabled="busy || !editName.trim() || !editPath.trim()" @click="saveEdit" />
        </template>
      </ManagementDetailHeader>
      <div class="min-h-0 flex-1 overflow-y-auto border-y border-[var(--border)] py-5 [scrollbar-gutter:stable]">
        <div class="max-w-2xl space-y-4 px-1">
        <UFormField :label="t('software.displayName')" required>
          <UInput v-model="editName" size="sm" class="w-full" :aria-label="t('software.displayName')" />
        </UFormField>
        <UFormField :label="t('software.softwareDescription')">
          <UTextarea v-model="editDescription" :maxlength="512" :rows="2" autoresize class="w-full" :aria-label="t('software.softwareDescription')" />
        </UFormField>
        <UFormField :label="t('software.executablePath')" required>
          <UInput v-model="editPath" size="sm" class="w-full" :aria-label="t('software.executablePath')" />
        </UFormField>
        </div>
      </div>
    </template>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('software.deleteTitle')"
      :description="t('software.deleteDescription', { count: pendingRemoval.length })"
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
