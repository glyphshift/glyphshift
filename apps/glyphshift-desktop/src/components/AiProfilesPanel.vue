<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import defaultTranslationPrompt from '../../../../crates/product/ai-translation/src/default-translation-prompt.txt?raw'
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  defaultAiFilterPolicy,
  defaultAiReasoningEffort,
  providerDefaults,
  useAiTranslation,
  type AiFilterPolicy,
  type AiProfile,
  type AiProviderProtocol,
  type AiReasoningEffort,
} from '../useAiTranslation'
import { aiPresets } from '../aiPresets'
import ConfirmDialog from './ConfirmDialog.vue'
import ManagementFormModal from './ManagementFormModal.vue'

withDefaults(defineProps<{ showCreate?: boolean }>(), { showCreate: true })

interface ProfileForm {
  id: string
  name: string
  protocol: AiProviderProtocol
  baseUrl: string
  modelId: string
  translationPrompt: string
  reasoningEffort: AiReasoningEffort
  timeoutMinutes: number
  maxItemsPerRequest: number
  maxConcurrency: number
  maxRetries: number
  filterPolicy: AiFilterPolicy
  secret: string
  makeDefault: boolean
}

const { t } = useI18n()
const ai = useAiTranslation()
const editorOpen = ref(false)
const editingProfile = ref<AiProfile | null>(null)
const pendingDelete = ref<AiProfile | null>(null)
const excludedPatternsText = ref('')
const advancedOpen = ref(false)
const secretVisible = ref(false)
const presetId = ref('custom')
const presetItems = computed(() => [
  { value: 'custom', label: t('ai.presetCustom') },
  ...aiPresets.map(preset => ({ value: preset.id, label: t(`ai.presetNames.${preset.id}`) })),
])

function applyPreset(id: string) {
  presetId.value = id
  const preset = aiPresets.find(item => item.id === id)
  if (!preset || editingProfile.value) return
  form.value.protocol = preset.protocol
  form.value.name = t(`ai.presetNames.${preset.id}`)
  form.value.baseUrl = preset.baseUrl
  form.value.modelId = preset.modelId
  form.value.reasoningEffort = preset.reasoningEffort
  form.value.secret = ''
  secretVisible.value = false
}


const providerItems = computed(() => ([
  { value: 'codex_subscription' as const, label: t('ai.protocol.codexSubscription') },
  { value: 'open_ai_responses' as const, label: t('ai.protocol.openAiResponses') },
  { value: 'open_ai_chat_completions' as const, label: t('ai.protocol.openAiChat') },
  { value: 'open_ai_compatible' as const, label: t('ai.protocol.openAiCompatible') },
  { value: 'anthropic_messages' as const, label: t('ai.protocol.anthropic') },
  { value: 'gemini_generate_content' as const, label: t('ai.protocol.gemini') },
  { value: 'ollama_chat' as const, label: t('ai.protocol.ollama') },
]))

function newProfileForm(): ProfileForm {
  const protocol: AiProviderProtocol = 'open_ai_responses'
  return {
    id: `profile.${Date.now().toString(36)}`,
    name: '',
    protocol,
    baseUrl: providerDefaults[protocol].baseUrl,
    modelId: providerDefaults[protocol].modelId,
    translationPrompt: defaultTranslationPrompt.trim(),
    reasoningEffort: defaultAiReasoningEffort(protocol),
    timeoutMinutes: 30,
    maxItemsPerRequest: 50,
    maxConcurrency: providerDefaults[protocol].concurrency,
    maxRetries: 2,
    filterPolicy: defaultAiFilterPolicy(),
    secret: '',
    makeDefault: !ai.catalog.value.defaultProfileId,
  }
}

const form = ref<ProfileForm>(newProfileForm())
const modelItems = ref<string[]>([])
const modelMenuRequested = ref(false)
const modelFilterActive = ref(false)
const modelMenuOpen = computed({ get: () => modelItems.value.length > 0 && modelMenuRequested.value, set: (value: boolean) => { modelMenuRequested.value = value; if (!value) modelFilterActive.value = false } })
const modelsLoading = ref(false)
const modelsError = ref('')
const modelsFetched = ref(false)
let modelRequestVersion = 0
watch(() => [form.value.protocol, form.value.baseUrl, form.value.secret, editorOpen.value], () => {
  modelRequestVersion++
  modelItems.value = []
  modelFilterActive.value = false
  modelsError.value = ''
  modelsFetched.value = false
  modelsLoading.value = false
}, { flush: 'sync' })
async function fetchModels() {
  if (modelsLoading.value) return
  modelsError.value = ''
  if (credentialRequired.value && !form.value.secret.trim()) {
    modelsError.value = t('ai.modelErrors.missing_key')
    return
  }
  const version = ++modelRequestVersion
  modelsLoading.value = true
  try {
    const models = await invoke<string[]>('desktop_ai_models', { request: {
      protocol: form.value.protocol, baseUrl: form.value.baseUrl.trim(), secret: form.value.secret.trim(),
    } })
    if (version !== modelRequestVersion) return
    modelItems.value = models
    modelFilterActive.value = false
    modelsFetched.value = true
  } catch (error) {
    if (version !== modelRequestVersion) return
    const code = typeof error === 'string' && ['missing_key', 'invalid_url', 'unauthorized', 'forbidden', 'unsupported', 'rate_limited', 'redirect', 'service_error', 'network', 'timeout', 'invalid_response', 'empty', 'too_large'].includes(error) ? error : 'network'
    modelsError.value = t(`ai.modelErrors.${code}`)
  } finally {
    if (version === modelRequestVersion) modelsLoading.value = false
  }
}
const documentationUrl = computed(() => aiPresets.find(preset => preset.baseUrl === form.value.baseUrl.trim().replace(/\/$/, ''))?.documentationUrl)
async function openDocumentation() {
  if (!documentationUrl.value) return
  try {
    if ('__TAURI_INTERNALS__' in window) await openUrl(documentationUrl.value)
    else window.open(documentationUrl.value, '_blank', 'noopener,noreferrer')
  } catch { ai.error.value = t('ai.documentationFailed') }
}
const usesCodexSubscription = computed(() => form.value.protocol === 'codex_subscription')
const showsCredential = computed(() => !['ollama_chat', 'codex_subscription'].includes(form.value.protocol))
const credentialRequired = computed(() => providerDefaults[form.value.protocol].credentialRequired)
const reasoningConfigurable = computed(() => [
  'codex_subscription',
  'open_ai_responses',
  'open_ai_chat_completions',
  'open_ai_compatible',
].includes(form.value.protocol))
const usesDeepSeek = computed(() => (
  form.value.modelId.trim().toLocaleLowerCase().startsWith('deepseek-')
  || form.value.baseUrl.toLocaleLowerCase().includes('deepseek.com')
))
const reasoningItems = computed(() => {
  const items: Array<{ value: AiReasoningEffort; label: string }> = [
    {
      value: 'disabled' as const,
      label: t('ai.reasoningOption.disabled'),
    },
    { value: 'automatic' as const, label: t('ai.reasoningOption.automatic') },
  ]
  if (!usesDeepSeek.value) {
    items.push(
      { value: 'low' as const, label: t('ai.reasoningOption.low') },
      { value: 'medium' as const, label: t('ai.reasoningOption.medium') },
    )
  }
  items.push(
    { value: 'high' as const, label: t('ai.reasoningOption.high') },
    { value: 'maximum' as const, label: t('ai.reasoningOption.maximum') },
  )
  return items
})
const reasoningHint = computed(() => {
  if (usesDeepSeek.value) return t('ai.reasoningHintDeepSeek')
  if (usesCodexSubscription.value) return t('ai.reasoningHintCodex')
  return t('ai.reasoningHint')
})
const formValid = computed(() => Boolean(
  form.value.name.trim()
  && form.value.baseUrl.trim()
  && form.value.translationPrompt.length <= 16_000
  && form.value.modelId.trim()
  && Number.isInteger(form.value.timeoutMinutes)
  && form.value.timeoutMinutes >= 1
  && form.value.timeoutMinutes <= 60
  && Number.isInteger(form.value.maxItemsPerRequest)
  && form.value.maxItemsPerRequest >= 1
  && form.value.maxItemsPerRequest <= 1_000
  && Number.isInteger(form.value.maxConcurrency)
  && form.value.maxConcurrency >= 1
  && form.value.maxConcurrency <= 16
  && Number.isInteger(form.value.maxRetries)
  && form.value.maxRetries >= 0
  && form.value.maxRetries <= 10
  && (!credentialRequired.value
    || form.value.secret.trim()
    || editingProfile.value?.hasCredential),
))

watch(() => form.value.protocol, (protocol, previous) => {
  if (!editorOpen.value || protocol === previous || editingProfile.value?.protocol === protocol) return
  if (!editingProfile.value || form.value.baseUrl === providerDefaults[previous].baseUrl) {
    form.value.baseUrl = providerDefaults[protocol].baseUrl
  }
  if (!editingProfile.value || form.value.modelId === providerDefaults[previous].modelId) {
    form.value.modelId = providerDefaults[protocol].modelId
  }
  form.value.maxConcurrency = providerDefaults[protocol].concurrency
  form.value.reasoningEffort = defaultAiReasoningEffort(protocol)
  if (protocol === 'ollama_chat' || protocol === 'codex_subscription') {
    form.value.secret = ''
  }
}, { flush: 'sync' })

function protocolLabel(protocol: AiProviderProtocol) {
  return providerItems.value.find(item => item.value === protocol)?.label ?? protocol
}

function reasoningLabel(profile: AiProfile) {
  return t(`ai.reasoningOption.${profile.reasoningEffort}`)
}

function openCreate() {
  presetId.value = 'custom'
  editingProfile.value = null
  form.value = newProfileForm()
  excludedPatternsText.value = ''
  advancedOpen.value = false
  secretVisible.value = false
  editorOpen.value = true
}

function openEdit(profile: AiProfile) {
  editingProfile.value = profile
  form.value = {
    id: profile.id,
    name: profile.name,
    protocol: profile.protocol,
    baseUrl: profile.baseUrl,
    modelId: profile.modelId,
    translationPrompt: profile.translationPrompt ?? defaultTranslationPrompt.trim(),
    reasoningEffort: profile.reasoningEffort,
    timeoutMinutes: Math.ceil(profile.timeoutMs / 60_000),
    maxItemsPerRequest: profile.maxItemsPerRequest,
    maxConcurrency: profile.maxConcurrency,
    maxRetries: profile.maxRetries,
    filterPolicy: JSON.parse(JSON.stringify(profile.filterPolicy)),
    secret: profile.credential ?? '',
    makeDefault: ai.catalog.value.defaultProfileId === profile.id,
  }
  excludedPatternsText.value = profile.filterPolicy.excludedPatterns.join('\n')
  advancedOpen.value = false
  secretVisible.value = false
  editorOpen.value = true
}

async function save() {
  if (!formValid.value || ai.busy.value) return
  const value = form.value
  const patterns = excludedPatternsText.value
    .split('\n')
    .map(pattern => pattern.trim())
    .filter(Boolean)
  await ai.saveProfile({
    id: value.id,
    name: value.name.trim(),
    protocol: value.protocol,
    baseUrl: value.baseUrl.trim(),
    modelId: value.modelId.trim(),
    translationPrompt: value.translationPrompt.trim() === defaultTranslationPrompt.trim() ? null : value.translationPrompt.trim() || null,
    reasoningEffort: value.reasoningEffort,
    timeoutMs: Number(value.timeoutMinutes) * 60_000,
    maxItemsPerRequest: Number(value.maxItemsPerRequest),
    maxConcurrency: Number(value.maxConcurrency),
    maxRetries: Number(value.maxRetries),
    filterPolicy: {
      ...value.filterPolicy,
      maxSourceChars: Number(value.filterPolicy.maxSourceChars) > 0
        ? Number(value.filterPolicy.maxSourceChars)
        : null,
      excludedPatterns: patterns,
    },
    credential: value.secret.trim()
      ? { action: 'replace', secret: value.secret.trim() }
      : { action: 'keep' },
  }, value.makeDefault)
  editorOpen.value = false
}

async function confirmDelete() {
  if (!pendingDelete.value) return
  await ai.deleteProfile(pendingDelete.value.id)
  pendingDelete.value = null
}

defineExpose({ openCreate })

onMounted(() => void ai.connect())
</script>

<template>
  <section data-testid="ai-profile-settings" class="py-2" :aria-label="t('ai.profilesTitle')">
    <div v-if="showCreate" class="flex items-start justify-end gap-4">
      <UButton color="primary" variant="soft" size="sm" icon="i-tabler-plus" :label="t('ai.addProfile')" class="shrink-0" @click="openCreate" />
    </div>

    <UAlert
      v-if="ai.error.value"
      role="alert"
      color="error"
      variant="soft"
      :title="t('ai.errorTitle')"
      :description="ai.error.value"
      class="mt-4"
    />

    <div v-if="ai.profiles.value.length" class="mt-3 divide-y divide-[var(--border)]">
      <div v-for="profile in ai.profiles.value" :key="profile.id" class="flex min-h-[76px] items-center gap-4 py-3">
        <div class="grid size-9 shrink-0 place-items-center rounded-[var(--radius-control)] bg-[var(--accent-soft)] text-[var(--accent-strong)]">
          <UIcon :name="profile.protocol === 'ollama_chat' ? 'i-tabler-server-2' : profile.protocol === 'codex_subscription' ? 'i-tabler-brand-openai' : 'i-tabler-sparkles'" class="size-5" aria-hidden="true" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex min-w-0 items-center gap-2">
            <strong class="truncate text-[12px] text-[var(--text)]">{{ profile.name }}</strong>
            <UBadge v-if="ai.catalog.value.defaultProfileId === profile.id" color="primary" variant="soft" size="sm" :label="t('ai.defaultProfile')" />
          </div>
          <p class="type-metadata m-0 mt-1 truncate leading-4 text-[var(--text-muted)]">
            {{ protocolLabel(profile.protocol) }} · {{ profile.modelId }}<template v-if="profile.protocol !== 'codex_subscription'"> · {{ profile.baseUrl }}</template>
          </p>
          <p class="type-metadata m-0 mt-0.5 truncate leading-4 text-[var(--text-muted)]">
            {{ t('ai.profileRequestPolicy', { reasoning: reasoningLabel(profile), batchSize: profile.maxItemsPerRequest, timeout: Math.ceil(profile.timeoutMs / 60_000), concurrency: profile.maxConcurrency, retries: profile.maxRetries }) }}
          </p>
          <div v-if="ai.connectionReports.value[profile.id]" class="type-metadata mt-1.5 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5 leading-4">
            <span :class="ai.connectionReports.value[profile.id]?.status === 'passed' ? 'text-[var(--success)]' : 'text-[var(--danger)]'">
              {{ ai.connectionReports.value[profile.id]?.status === 'passed' ? t('ai.connectionPassed') : t('ai.connectionFailed') }}
            </span>
            <span v-if="ai.connectionReports.value[profile.id]?.safeMessage" class="truncate text-[var(--danger)]">{{ ai.connectionReports.value[profile.id]?.safeMessage }}</span>
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-tabler-plug-connected"
            :label="t('ai.testConnection')"
            :aria-label="t('ai.testConnectionNamed', { name: profile.name })"
            :title="t('ai.testConnectionCostHint')"
            :loading="ai.testingProfileId.value === profile.id"
            :disabled="Boolean(ai.testingProfileId.value) || ai.busy.value || ai.taskRunning.value || (profile.credentialRequired && !profile.hasCredential)"
            @click="ai.testProfile(profile.id)"
          />
          <UButton v-if="ai.catalog.value.defaultProfileId !== profile.id" color="neutral" variant="ghost" size="xs" :label="t('ai.makeDefault')" :title="t('ai.useAsDefault')" :disabled="ai.busy.value" @click="ai.setDefaultProfile(profile.id)" />
          <UButton :title="t('ai.editProfile', { name: profile.name })" color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :disabled="ai.testingProfileId.value === profile.id" :aria-label="t('ai.editProfile', { name: profile.name })" @click="openEdit(profile)" />
          <UButton :title="t('ai.deleteProfile', { name: profile.name })" color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('ai.deleteProfile', { name: profile.name })" @click="pendingDelete = profile" />
        </div>
      </div>
    </div>
  </section>

  <ManagementFormModal
    :open="editorOpen"
    :title="editingProfile ? t('ai.editProfileTitle') : t('ai.addProfile')"
    :confirm-label="t('ai.saveProfile')"
    :confirm-disabled="!formValid"
    :busy="ai.busy.value"
    width="lg"
    @update:open="editorOpen = $event"
    @confirm="save"
  >
    <div class="grid grid-cols-2 gap-x-4 gap-y-3 @max-[560px]:grid-cols-1">
      <UFormField v-if="!editingProfile" :label="t('ai.preset')" class="col-span-2 @max-[560px]:col-span-1">
        <USelect :model-value="presetId" :items="presetItems" value-key="value" label-key="label" :aria-label="t('ai.preset')" :title="t('ai.presetHint')" class="w-full" @update:model-value="applyPreset(String($event))" />
      </UFormField>
      <UFormField :label="t('ai.profileName')" required>
        <UInput v-model="form.name" :aria-label="t('ai.profileName')" :maxlength="128" class="w-full" />
      </UFormField>
      <UFormField :label="t('ai.protocolLabel')" required>
        <USelect v-model="form.protocol" :items="providerItems" value-key="value" label-key="label" :aria-label="t('ai.protocolLabel')" class="w-full" />
      </UFormField>
      <div v-if="usesCodexSubscription" class="col-span-2 flex items-start gap-2.5 rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2.5 @max-[560px]:col-span-1">
        <UIcon name="i-tabler-brand-openai" class="mt-0.5 size-4 shrink-0 text-[var(--accent-strong)]" aria-hidden="true" />
        <p class="type-metadata m-0 leading-4 text-[var(--text-muted)]">{{ t('ai.codexProfileHint') }}</p>
      </div>
      <UFormField v-else :label="t('ai.baseUrl')" required class="col-span-2 @max-[560px]:col-span-1">
        <div class="flex items-center gap-2">
          <UInput v-model="form.baseUrl" :aria-label="t('ai.baseUrl')" spellcheck="false" class="w-full" />
          <UTooltip v-if="documentationUrl" :text="t('ai.serviceDocumentation')">
            <UButton icon="i-tabler-help" color="neutral" variant="ghost" :aria-label="t('ai.serviceDocumentation')" :title="t('ai.serviceDocumentation')" @click="openDocumentation" />
          </UTooltip>
        </div>
      </UFormField>
      <UFormField v-if="showsCredential" :label="t('ai.apiKey')" :required="credentialRequired && !editingProfile?.hasCredential">
        <UInput
          v-model="form.secret"
          :type="secretVisible ? 'text' : 'password'"
          :aria-label="t('ai.apiKey')"
          autocomplete="off"
          class="w-full"
        >
          <template #trailing>
            <UButton
              color="neutral"
              variant="link"
              size="xs"
              :icon="secretVisible ? 'i-tabler-eye-off' : 'i-tabler-eye'"
              :aria-label="secretVisible ? t('ai.hideApiKey') : t('ai.showApiKey')"
              :title="secretVisible ? t('ai.hideApiKey') : t('ai.showApiKey')"
              :aria-pressed="secretVisible"
              @click="secretVisible = !secretVisible"
            />
          </template>
        </UInput>
        <p class="type-caption m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ t('ai.plainCredentialHint') }}</p>
      </UFormField>
      <UFormField v-if="reasoningConfigurable" :label="t('ai.reasoningEffort')">
        <USelect v-model="form.reasoningEffort" :items="reasoningItems" value-key="value" label-key="label" :aria-label="t('ai.reasoningEffort')" class="w-full" />
        <p class="type-caption m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ reasoningHint }}</p>
      </UFormField>
      <UFormField :label="t('ai.modelId')" required class="col-span-2 @max-[560px]:col-span-1">
        <div class="flex items-start gap-2">
          <UInputMenu v-model="form.modelId" v-model:open="modelMenuOpen" mode="autocomplete" :items="modelItems" :ignore-filter="!modelFilterActive" @input="modelFilterActive = true" :aria-label="t('ai.modelId')" :placeholder="t('ai.modelPlaceholder')" :reset-search-term-on-blur="false" :open-on-focus="false" spellcheck="false" class="min-w-0 flex-1">
            <template #empty>{{ t('ai.modelListEmpty') }}</template>
          </UInputMenu>
          <UButton v-if="!usesCodexSubscription" color="neutral" variant="soft" icon="i-tabler-refresh" :label="t('ai.fetchModels')" :loading="modelsLoading" class="shrink-0" @click="fetchModels" />
        </div>
        <p v-if="modelsError" role="alert" class="type-caption m-0 mt-1 leading-4 text-[var(--danger)]">{{ modelsError }}</p>
        <p v-else-if="modelsFetched" role="status" class="type-caption m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ t('ai.modelsFetched', { count: modelItems.length }) }}</p>
      </UFormField>
    </div>

    <UCollapsible v-model:open="advancedOpen" class="mt-4 border-y border-[var(--border)]">
      <UButton
        color="neutral"
        variant="ghost"
        class="w-full justify-between rounded-none px-0 py-3"
        :label="t('ai.advancedSettings')"
        :trailing-icon="advancedOpen ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'"
      />
      <template #content>
        <div class="pb-4">
          <p class="type-metadata mb-3 mt-0 leading-4 text-[var(--text-muted)]">{{ t('ai.advancedSettingsDescription') }}</p>
          <div class="grid grid-cols-2 gap-x-4 gap-y-3 @max-[560px]:grid-cols-1">
            <UFormField :label="t('ai.timeout')">
              <UInput v-model.number="form.timeoutMinutes" type="number" min="1" max="60" step="1" :aria-label="t('ai.timeout')" class="w-full" />
            </UFormField>
            <UFormField :label="t('ai.maxItemsPerRequest')">
              <UInput v-model.number="form.maxItemsPerRequest" type="number" min="1" max="1000" step="1" :aria-label="t('ai.maxItemsPerRequest')" class="w-full" />
            </UFormField>
            <UFormField :label="t('ai.concurrency')">
              <UInput v-model.number="form.maxConcurrency" type="number" min="1" max="16" step="1" :aria-label="t('ai.concurrency')" class="w-full" />
            </UFormField>
            <UFormField :label="t('ai.maxRetries')" :hint="t('ai.maxRetriesHint')">
              <UInput v-model.number="form.maxRetries" type="number" min="0" max="10" step="1" :aria-label="t('ai.maxRetries')" class="w-full" />
            </UFormField>
          </div>

          <div class="mt-5 border-t border-[var(--border)] pt-4">
            <div class="mb-2 flex items-center justify-between gap-2">
              <label for="ai-translation-prompt" class="text-[12px] font-semibold text-[var(--text)]">{{ t('ai.translationPrompt') }}</label>
              <UButton color="neutral" variant="ghost" size="xs" :label="t('ai.restorePrompt')" :title="t('ai.restorePrompt')" @click="form.translationPrompt = defaultTranslationPrompt.trim()" />
            </div>
            <UTextarea id="ai-translation-prompt" v-model="form.translationPrompt" :aria-label="t('ai.translationPrompt')" :rows="6" :maxlength="16000" class="w-full" />
            <p class="type-metadata mt-2 text-[var(--text-muted)]">{{ t('ai.translationPromptHint') }}</p>
          </div>
        </div>
      </template>
    </UCollapsible>

    <div class="mt-4 flex flex-wrap items-center gap-5 border-t border-[var(--border)] pt-4">
      <UCheckbox v-model="form.makeDefault" :label="t('ai.useAsDefault')" />
    </div>
  </ManagementFormModal>

  <ConfirmDialog
    :open="Boolean(pendingDelete)"
    :title="t('ai.deleteProfileTitle')"
    :description="t('ai.deleteProfileDescription', { name: pendingDelete?.name ?? '' })"
    :confirm-label="t('ai.deleteProfileConfirm')"
    @update:open="$event || (pendingDelete = null)"
    @confirm="confirmDelete"
  />
</template>
