<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { open, save } from '@tauri-apps/plugin-dialog'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type {
  DictionaryCatalogInstallRequest,
  DictionaryCatalogPage,
  DictionaryCatalogQueryRequest,
  DictionaryCatalogRelease,
  DictionaryMetadata,
  DictionarySummary,
  ArtifactWarning,
} from '../model'
import {
  editableRowIndex,
  managementActionsColumnMeta,
  managementIdentityColumnMeta,
  managementSelectionColumnMeta,
} from '../tableInteraction'
import { useTableColumns } from '../useTableColumns'

const props = defineProps<{
  items: DictionarySummary[]
  busy: boolean
  messages: Record<string, string>
  catalogPage: DictionaryCatalogPage
  catalogBusy: boolean
  catalogError: string
  presentationLocale: string
  artifactWarnings: ArtifactWarning[]
}>()
const emit = defineEmits<{
  open: [id: string]
  create: [metadata: Omit<DictionaryMetadata, 'id'>]
  importDictionary: [inputPath: string]
  exportDictionary: [dictionaryId: string, outputPath: string]
  remove: [ids: string[]]
  queryCatalog: [request: DictionaryCatalogQueryRequest]
  installCatalog: [request: DictionaryCatalogInstallRequest]
}>()
const { t } = useI18n()
const dictionaryWarnings = computed(() => props.artifactWarnings.filter(warning => warning.artifactKind === 'dictionary'))
const dictionaryWarningDescription = computed(() => t('dictionaries.skippedArtifactsDescription', {
  count: dictionaryWarnings.value.length,
  ids: dictionaryWarnings.value.map(warning => warning.artifactId).join('、'),
}))

const mode = ref<'local' | 'catalog'>('local')
const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const creating = ref(false)
const exportError = ref('')
const pendingRemoval = ref<DictionarySummary[]>([])
const pendingInstall = ref<DictionaryCatalogRelease | null>(null)
const catalogQuery = ref('')
const catalogTag = ref('')
const catalogPageNumber = ref(1)
const catalogPageSize = ref(20)
const catalogCursors = ref<(string | null)[]>([null])
const { columns: localVisibleColumns, toggleColumn: toggleLocalColumn } = useTableColumns('glyphshift.table-columns.dictionaries.local', {
  languages: true,
  release: true,
  installation: true,
  rules: true,
})
const { columns: catalogVisibleColumns, toggleColumn: toggleCatalogColumn } = useTableColumns('glyphshift.table-columns.dictionaries.catalog', {
  languages: true,
  release: true,
  tags: true,
})

function emptyMetadata(): DictionaryMetadata {
  return {
    id: '',
    name: '',
    description: '',
    sourceLocale: 'en-US',
    targetLocale: 'zh-CN',
    releaseVersion: '0.1.0',
    authors: [],
    license: null,
    homepage: null,
    tags: [],
  }
}

const createDraft = ref<DictionaryMetadata>(emptyMetadata())

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter((item) => {
    const metadata = item.metadata
    return !needle || `${metadata.name} ${metadata.description} ${metadata.sourceLocale} ${metadata.targetLocale} ${metadata.releaseVersion} ${metadata.tags.join(' ')}`.toLocaleLowerCase().includes(needle)
  })
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.metadata.id)))
const localColumnOptions = computed(() => [
  { key: 'languages', label: t('dictionaries.columns.languages'), visible: localVisibleColumns.value.languages },
  { key: 'release', label: t('dictionaries.columns.release'), visible: localVisibleColumns.value.release },
  { key: 'installation', label: t('dictionaries.columns.installation'), visible: localVisibleColumns.value.installation },
  { key: 'rules', label: t('dictionaries.columns.rules'), visible: localVisibleColumns.value.rules },
])
const catalogColumnOptions = computed(() => [
  { key: 'languages', label: t('dictionaries.columns.languages'), visible: catalogVisibleColumns.value.languages },
  { key: 'release', label: t('dictionaries.columns.release'), visible: catalogVisibleColumns.value.release },
  { key: 'tags', label: t('dictionaries.columns.tags'), visible: catalogVisibleColumns.value.tags },
])
const tableColumns = computed<TableColumn<DictionarySummary>[]>(() => [
  { id: 'select', header: '', meta: managementSelectionColumnMeta() },
  { id: 'dictionary', header: t('dictionaries.columns.dictionary'), meta: managementIdentityColumnMeta('w-64') },
  ...(localVisibleColumns.value.languages ? [{ id: 'languages', header: t('dictionaries.columns.languages'), meta: { class: { th: 'w-40', td: 'w-40' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(localVisibleColumns.value.release ? [{ id: 'release', header: t('dictionaries.columns.release'), meta: { class: { th: 'w-28', td: 'w-28' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(localVisibleColumns.value.installation ? [{ id: 'installation', header: t('dictionaries.columns.installation'), meta: { class: { th: 'w-40', td: 'w-40' } } } satisfies TableColumn<DictionarySummary>] : []),
  ...(localVisibleColumns.value.rules ? [{ id: 'rules', header: t('dictionaries.columns.rules'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } } satisfies TableColumn<DictionarySummary>] : []),
  { id: 'actions', header: t('dictionaries.columns.actions'), meta: managementActionsColumnMeta('w-28') },
])
const catalogColumns = computed<TableColumn<DictionaryCatalogRelease>[]>(() => [
  { id: 'dictionary', header: t('dictionaries.columns.dictionary'), meta: managementIdentityColumnMeta('w-64', false) },
  ...(catalogVisibleColumns.value.languages ? [{ id: 'languages', header: t('dictionaries.columns.languages'), meta: { class: { th: 'w-40', td: 'w-40' } } } satisfies TableColumn<DictionaryCatalogRelease>] : []),
  ...(catalogVisibleColumns.value.release ? [{ id: 'release', header: t('dictionaries.columns.release'), meta: { class: { th: 'w-40', td: 'w-40' } } } satisfies TableColumn<DictionaryCatalogRelease>] : []),
  ...(catalogVisibleColumns.value.tags ? [{ id: 'tags', header: t('dictionaries.columns.tags') } satisfies TableColumn<DictionaryCatalogRelease>] : []),
  { id: 'actions', header: t('dictionaries.columns.actions'), meta: managementActionsColumnMeta('w-28') },
])
const removalDescription = computed(() => pendingRemoval.value.length === 1
  ? t('dictionaries.deleteOne', { name: pendingRemoval.value[0]?.metadata.name ?? '' })
  : t('dictionaries.deleteMany', { count: pendingRemoval.value.length }))
const replacementDescription = computed(() => {
  const release = pendingInstall.value
  if (!release) return ''
  const current = localDictionary(release.dictionaryId)
  return current?.installation.state === 'modified'
    ? t('dictionaries.catalog.replaceModifiedDescription', { name: current.metadata.name, version: release.releaseVersion })
    : t('dictionaries.catalog.replaceLocalDescription', { name: current?.metadata.name ?? release.name, version: release.releaseVersion })
})

watch([query, pageSize], () => { page.value = 1 })
watch(pageCount, count => { page.value = Math.min(page.value, count) })
watch(catalogPageSize, () => {
  if (mode.value === 'catalog') requestCatalog(true)
})
watch(() => props.presentationLocale, () => {
  if (mode.value === 'catalog') requestCatalog(true)
})

function setMode(next: 'local' | 'catalog') {
  mode.value = next
  exportError.value = ''
  if (next === 'catalog') requestCatalog(true)
}

function openOnDoubleClick(event: MouseEvent) {
  const index = editableRowIndex(event)
  const item = index === null ? null : pageItems.value[index]
  if (item) emit('open', item.metadata.id)
}

function toggleSelection(id: string) {
  const next = new Set(selected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selected.value = next
}

function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) pageItems.value.forEach(item => next.delete(item.metadata.id))
  else pageItems.value.forEach(item => next.add(item.metadata.id))
  selected.value = next
}

function resetForm() {
  createDraft.value = emptyMetadata()
}

function openCreate() {
  resetForm()
  creating.value = true
}

function closeCreate() {
  creating.value = false
  resetForm()
}

function submit() {
  const metadata = createDraft.value
  if (!metadata.name.trim() || !metadata.sourceLocale.trim() || !metadata.targetLocale.trim()) return
  emit('create', {
    name: metadata.name.trim(),
    description: metadata.description.trim(),
    sourceLocale: metadata.sourceLocale.trim(),
    targetLocale: metadata.targetLocale.trim(),
    releaseVersion: metadata.releaseVersion,
    authors: metadata.authors.map(value => value.trim()).filter(Boolean),
    license: metadata.license?.trim() || null,
    homepage: metadata.homepage?.trim() || null,
    tags: [...new Set(metadata.tags.map(value => value.trim()).filter(Boolean))],
  })
  closeCreate()
}

function confirmRemoval() {
  const ids = pendingRemoval.value.map(item => item.metadata.id)
  emit('remove', ids)
  selected.value = new Set([...selected.value].filter(id => !ids.includes(id)))
  pendingRemoval.value = []
}

function requestCatalog(reset = false) {
  if (reset) {
    catalogPageNumber.value = 1
    catalogCursors.value = [null]
  }
  emit('queryCatalog', {
    text: catalogQuery.value.trim(),
    sourceLocale: null,
    targetLocale: null,
    tag: catalogTag.value || null,
    cursor: catalogCursors.value[catalogCursors.value.length - 1] ?? null,
    pageSize: catalogPageSize.value,
    requestedPresentationLocale: props.presentationLocale,
  })
}

function filterCatalogByTag(tag: string) {
  if (catalogTag.value === tag) return
  catalogTag.value = tag
  requestCatalog(true)
}

function clearCatalogTag() {
  if (!catalogTag.value) return
  catalogTag.value = ''
  requestCatalog(true)
}

function previousCatalogPage() {
  if (catalogPageNumber.value <= 1) return
  catalogCursors.value = catalogCursors.value.slice(0, -1)
  catalogPageNumber.value -= 1
  requestCatalog()
}

function nextCatalogPage() {
  if (!props.catalogPage.nextCursor) return
  catalogCursors.value = [...catalogCursors.value, props.catalogPage.nextCursor]
  catalogPageNumber.value += 1
  requestCatalog()
}

function localDictionary(dictionaryId: string) {
  return props.items.find(item => item.metadata.id === dictionaryId)
}

function installationLabel(item: DictionarySummary) {
  return t(`dictionaries.installation.${item.installation.state}`)
}

function installationColor(item: DictionarySummary) {
  if (item.installation.state === 'verified') return 'success' as const
  if (item.installation.state === 'modified' || item.installation.state === 'missing') return 'warning' as const
  return 'neutral' as const
}

function installationDetail(item: DictionarySummary) {
  if (item.installation.verifiedPublisher) return item.installation.verifiedPublisher
  return t('dictionaries.installation.localOnly')
}

function installButtonLabel(release: DictionaryCatalogRelease) {
  const current = localDictionary(release.dictionaryId)
  if (!current) return t('dictionaries.catalog.install')
  if (current.installation.state === 'verified'
    && current.installation.installedRelease === release.releaseVersion) return t('dictionaries.catalog.installed')
  return t('dictionaries.catalog.update')
}

function beginInstall(release: DictionaryCatalogRelease) {
  const current = localDictionary(release.dictionaryId)
  if (current && current.installation.state !== 'verified') {
    pendingInstall.value = release
    return
  }
  emitInstall(release, current ? 'replace_verified' : 'reject_existing')
}

function emitInstall(
  release: DictionaryCatalogRelease,
  replacement: DictionaryCatalogInstallRequest['replacement'],
) {
  emit('installCatalog', {
    catalogId: release.catalogId,
    dictionaryId: release.dictionaryId,
    releaseVersion: release.releaseVersion,
    replacement,
  })
}

function confirmInstall() {
  if (pendingInstall.value) emitInstall(pendingInstall.value, 'replace_any')
  pendingInstall.value = null
}

async function chooseImport() {
  if (!('__TAURI_INTERNALS__' in window)) return
  const inputPath = await open({
    directory: false,
    multiple: false,
    title: t('dictionaries.importDialogTitle'),
    filters: [{ name: t('dictionaries.jsonFile'), extensions: ['json'] }],
  })
  if (typeof inputPath === 'string') emit('importDictionary', inputPath)
}

async function chooseExport(item: DictionarySummary) {
  exportError.value = ''
  if (!('__TAURI_INTERNALS__' in window)) {
    exportError.value = t('dictionaries.exportDialogFailed')
    return
  }
  try {
    const outputPath = await save({
      title: t('dictionaries.exportDialogTitle'),
      defaultPath: `${item.metadata.id}.json`,
      filters: [{ name: t('dictionaries.jsonFile'), extensions: ['json'] }],
    })
    if (outputPath) emit('exportDictionary', item.metadata.id, outputPath)
  } catch {
    exportError.value = t('dictionaries.exportDialogFailed')
  }
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-library-title">
    <!--
      THESIS: 本地资产与可信目录是同一词典任务的两种明确模式，不新增一级导航或混合两套状态。
      OWN-WORLD: 继承 Glyphshift 紧凑管理表、薄边界、近黑表面和单一 cobalt 操作色。
      STORY: 用户先识别当前模式，再搜索、检查来源状态，并创建、安装或更新词典。
      FIRST VIEWPORT: 标题右侧是双模式切换；其下始终是一张满高搜索表和固定分页。
      FORM: 既有 Operate 表格体系的局部扩展；本地表显示 provenance，目录表使用游标分页。
      FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
    -->
    <ManagementPageHeader
      title-id="dictionary-library-title"
      :title="t('dictionaries.title')"
      icon="i-tabler-language"
    >
      <template #actions>
        <div class="flex items-center gap-2">
          <div class="flex items-center rounded-md border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5" role="group" :aria-label="t('dictionaries.modeLabel')">
            <UButton
              :color="mode === 'local' ? 'primary' : 'neutral'"
              :variant="mode === 'local' ? 'soft' : 'ghost'"
              size="sm"
              icon="i-tabler-books"
              :aria-pressed="mode === 'local'"
              @click="setMode('local')"
            >
              <span>{{ t('dictionaries.localMode') }}</span>
              <UBadge :color="mode === 'local' ? 'primary' : 'neutral'" variant="soft" size="sm" :label="String(items.length)" />
            </UButton>
            <UButton
              :color="mode === 'catalog' ? 'primary' : 'neutral'"
              :variant="mode === 'catalog' ? 'soft' : 'ghost'"
              size="sm"
              icon="i-tabler-world-search"
              :aria-pressed="mode === 'catalog'"
              :label="t('dictionaries.catalogMode')"
              @click="setMode('catalog')"
            />
          </div>
          <UButton v-if="mode === 'local'" color="neutral" variant="outline" size="sm" icon="i-tabler-file-import" :label="t('dictionaries.importFile')" :disabled="busy" @click="chooseImport" />
          <UButton v-if="mode === 'local'" color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('dictionaries.create')" :disabled="busy" @click="openCreate" />
        </div>
      </template>
    </ManagementPageHeader>

    <UAlert v-if="mode === 'local' && (messages.dictionaries || exportError)" role="alert" color="error" variant="soft" :title="t('dictionaries.error')" :description="messages.dictionaries || exportError" class="mb-3" />
    <UAlert
      v-if="mode === 'local' && dictionaryWarnings.length"
      role="status"
      color="warning"
      variant="soft"
      icon="i-tabler-file-alert"
      :title="t('dictionaries.skippedArtifactsTitle')"
      :description="dictionaryWarningDescription"
      class="mb-3"
    />

    <ManagementTableFrame
      v-if="mode === 'local'"
      v-model:query="query"
      v-model:page="page"
      v-model:page-size="pageSize"
      :search-placeholder="t('dictionaries.searchPlaceholder')"
      :search-label="t('dictionaries.searchLabel')"
      :column-options="localColumnOptions"
      :columns-label="t('table.columns')"
      :selected-count="selected.size"
      :selected-label="t('dictionaries.itemLabel')"
      :total="filtered.length"
      :item-label="t('dictionaries.itemLabel')"
      @toggle-column="toggleLocalColumn"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('dictionaries.bulkDelete')" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.metadata.id))" />
      </template>

      <UTable data-testid="dictionary-management-table" role="region" tabindex="0" aria-labelledby="dictionary-library-title" :data="pageItems" :columns="tableColumns" sticky class="management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[860px]' }" @dblclick="openOnDoubleClick">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" :aria-label="t('dictionaries.selectPage')" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.metadata.id)" :aria-label="t('common.selectNamed', { name: row.original.metadata.name })" @update:model-value="toggleSelection(row.original.metadata.id)" />
        </template>
        <template #dictionary-cell="{ row }">
          <UButton color="neutral" variant="link" class="block min-w-0 max-w-full justify-start p-0 text-left" @click="emit('open', row.original.metadata.id)">
            <span class="block truncate font-semibold text-[var(--text)]">{{ row.original.metadata.name }}</span>
            <span class="type-metadata mt-0.5 block truncate text-[var(--text-muted)]">{{ row.original.metadata.description || t('common.noDescription') }}</span>
          </UButton>
        </template>
        <template #languages-cell="{ row }">
          <div class="flex items-center whitespace-nowrap">
            <span>{{ row.original.metadata.sourceLocale }}</span>
            <UIcon name="i-tabler-arrow-right" class="mx-1 shrink-0 text-[var(--text-muted)]" />
            <span>{{ row.original.metadata.targetLocale }}</span>
          </div>
        </template>
        <template #release-cell="{ row }">
          <div>v{{ row.original.metadata.releaseVersion }}</div>
        </template>
        <template #installation-cell="{ row }">
          <UBadge :color="installationColor(row.original)" variant="soft" size="sm" :label="installationLabel(row.original)" />
          <div class="type-metadata mt-1 max-w-36 truncate text-[var(--text-muted)]">{{ installationDetail(row.original) }}</div>
        </template>
        <template #rules-cell="{ row }">{{ row.original.entryCount }}</template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-0.5">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('common.editNamed', { name: row.original.metadata.name })" @click="emit('open', row.original.metadata.id)" />
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-file-export" :aria-label="t('dictionaries.exportNamed', { name: row.original.metadata.name })" :disabled="busy" @click="chooseExport(row.original)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.metadata.name })" :disabled="busy" @click="pendingRemoval = [row.original]" />
          </div>
        </template>
        <template #empty>
          <UEmpty icon="i-tabler-language" :title="items.length ? t('dictionaries.noMatch') : t('dictionaries.empty')" :description="items.length ? t('dictionaries.adjustSearch') : t('dictionaries.emptyDescription')" />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementTableFrame
      v-else
      v-model:query="catalogQuery"
      v-model:page-size="catalogPageSize"
      :page="catalogPageNumber"
      :search-placeholder="t('dictionaries.catalog.searchPlaceholder')"
      :search-label="t('dictionaries.catalog.searchLabel')"
      :column-options="catalogColumnOptions"
      :columns-label="t('table.columns')"
      :total="catalogPage.releases.length"
      :item-label="t('dictionaries.catalog.itemLabel')"
      pagination-mode="cursor"
      :cursor-page="catalogPageNumber"
      :has-next-page="Boolean(catalogPage.nextCursor)"
      :footer-summary="t('dictionaries.catalog.pageSummary', { page: catalogPageNumber, count: catalogPage.releases.length })"
      @search="requestCatalog(true)"
      @toggle-column="toggleCatalogColumn"
      @previous-page="previousCatalogPage"
      @next-page="nextCatalogPage"
    >
      <template #toolbar-actions>
        <UButton
          v-if="catalogTag"
          color="primary"
          variant="soft"
          size="sm"
          icon="i-tabler-tag"
          trailing-icon="i-tabler-x"
          :label="t('dictionaries.catalog.activeTag', { tag: catalogTag })"
          :aria-label="t('dictionaries.catalog.clearTag', { tag: catalogTag })"
          @click="clearCatalogTag"
        />
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-search" :label="t('dictionaries.catalog.searchAction')" :loading="catalogBusy" @click="requestCatalog(true)" />
      </template>

      <UTable data-testid="dictionary-catalog-table" role="region" tabindex="0" aria-labelledby="dictionary-library-title" :data="catalogPage.releases" :columns="catalogColumns" :loading="catalogBusy" sticky class="management-table-scroll" :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[860px]' }">
        <template #dictionary-cell="{ row }">
          <div class="min-w-0">
            <div class="truncate font-semibold text-[var(--text)]">{{ row.original.name }}</div>
            <div class="type-metadata mt-0.5 truncate text-[var(--text-muted)]">{{ row.original.summary || t('common.noDescription') }}</div>
          </div>
        </template>
        <template #languages-cell="{ row }">
          <div class="flex items-center whitespace-nowrap">
            <span>{{ row.original.sourceLocale }}</span>
            <UIcon name="i-tabler-arrow-right" class="mx-1 shrink-0 text-[var(--text-muted)]" />
            <span>{{ row.original.targetLocale }}</span>
          </div>
        </template>
        <template #release-cell="{ row }">
          <div>v{{ row.original.releaseVersion }}</div>
          <div class="type-metadata mt-0.5 max-w-36 truncate text-[var(--text-muted)]">{{ row.original.publisherIdentity }}</div>
        </template>
        <template #tags-cell="{ row }">
          <div class="flex flex-wrap gap-1">
            <UButton
              v-for="tag in row.original.tags.slice(0, 3)"
              :key="tag"
              :color="catalogTag === tag ? 'primary' : 'neutral'"
              variant="soft"
              size="xs"
              :label="tag"
              :aria-label="t('dictionaries.catalog.filterTag', { tag })"
              @click="filterCatalogByTag(tag)"
            />
            <span v-if="!row.original.tags.length" class="text-[var(--text-muted)]">—</span>
          </div>
        </template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center">
            <UButton
              color="primary"
              :variant="localDictionary(row.original.dictionaryId) ? 'soft' : 'solid'"
              size="xs"
              :icon="localDictionary(row.original.dictionaryId) ? 'i-tabler-refresh' : 'i-tabler-download'"
              :label="installButtonLabel(row.original)"
              :disabled="catalogBusy || (localDictionary(row.original.dictionaryId)?.installation.state === 'verified' && localDictionary(row.original.dictionaryId)?.installation.installedRelease === row.original.releaseVersion)"
              @click="beginInstall(row.original)"
            />
          </div>
        </template>
        <template #empty>
          <UEmpty
            :icon="catalogError ? 'i-tabler-cloud-off' : 'i-tabler-world-search'"
            :title="catalogError ? t('dictionaries.catalog.unavailable') : t('dictionaries.catalog.empty')"
            :description="catalogError || t('dictionaries.catalog.emptyDescription')"
          >
            <template #actions>
              <UButton v-if="catalogError" color="neutral" variant="outline" size="sm" icon="i-tabler-refresh" :label="t('common.retry')" :loading="catalogBusy" @click="requestCatalog()" />
            </template>
          </UEmpty>
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="creating"
      :title="t('dictionaries.create')"
      :description="t('dictionaries.createDescription')"
      :confirm-label="t('dictionaries.createConfirm')"
      :confirm-disabled="busy || !createDraft.name.trim() || !createDraft.sourceLocale.trim() || !createDraft.targetLocale.trim() || !createDraft.releaseVersion.trim()"
      :busy="busy"
      width="md"
      @update:open="$event || closeCreate()"
      @confirm="submit"
    >
      <DictionaryMetadataForm v-model="createDraft" />
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('dictionaries.deleteTitle')"
      :description="removalDescription"
      :confirm-label="t('dictionaries.deleteConfirm')"
      :busy="busy"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />

    <ConfirmDialog
      :open="Boolean(pendingInstall)"
      :title="t('dictionaries.catalog.replaceTitle')"
      :description="replacementDescription"
      :busy="catalogBusy"
      confirm-color="warning"
      :confirm-label="t('dictionaries.catalog.replaceConfirm')"
      @update:open="$event || (pendingInstall = null)"
      @confirm="confirmInstall"
    />
  </section>
</template>
