<script setup lang="ts">
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
import ConfirmDialog from './ConfirmDialog.vue'
import ManagementFormModal from './ManagementFormModal.vue'

interface ProfileForm {
  id: string
  name: string
  protocol: AiProviderProtocol
  baseUrl: string
  modelId: string
  reasoningEffort: AiReasoningEffort
  timeoutMinutes: number
  maxItemsPerRequest: number
  maxConcurrency: number
  maxRetries: number
  filterPolicy: AiFilterPolicy
  secret: string
  clearCredential: boolean
  makeDefault: boolean
}

const { t } = useI18n()
const ai = useAiTranslation()
const editorOpen = ref(false)
const editingProfile = ref<AiProfile | null>(null)
const pendingDelete = ref<AiProfile | null>(null)
const excludedPatternsText = ref('')

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
    modelId: '',
    reasoningEffort: defaultAiReasoningEffort(protocol),
    timeoutMinutes: 30,
    maxItemsPerRequest: 50,
    maxConcurrency: providerDefaults[protocol].concurrency,
    maxRetries: 2,
    filterPolicy: defaultAiFilterPolicy(),
    secret: '',
    clearCredential: false,
    makeDefault: !ai.catalog.value.defaultProfileId,
  }
}

const form = ref<ProfileForm>(newProfileForm())
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
  form.value.maxConcurrency = providerDefaults[protocol].concurrency
  form.value.reasoningEffort = defaultAiReasoningEffort(protocol)
  if (protocol === 'ollama_chat' || protocol === 'codex_subscription') {
    form.value.secret = ''
    form.value.clearCredential = false
  }
})

function protocolLabel(protocol: AiProviderProtocol) {
  return providerItems.value.find(item => item.value === protocol)?.label ?? protocol
}

function reasoningLabel(profile: AiProfile) {
  return t(`ai.reasoningOption.${profile.reasoningEffort}`)
}

function openCreate() {
  editingProfile.value = null
  form.value = newProfileForm()
  excludedPatternsText.value = ''
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
    reasoningEffort: profile.reasoningEffort,
    timeoutMinutes: Math.ceil(profile.timeoutMs / 60_000),
    maxItemsPerRequest: profile.maxItemsPerRequest,
    maxConcurrency: profile.maxConcurrency,
    maxRetries: profile.maxRetries,
    filterPolicy: JSON.parse(JSON.stringify(profile.filterPolicy)),
    secret: '',
    clearCredential: false,
    makeDefault: ai.catalog.value.defaultProfileId === profile.id,
  }
  excludedPatternsText.value = profile.filterPolicy.excludedPatterns.join('\n')
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
    credential: value.clearCredential
      ? { action: 'clear' }
      : value.secret.trim()
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

onMounted(() => void ai.connect())
</script>

<template>
  <section data-testid="ai-profile-settings" class="border-t border-[var(--border)] py-4" aria-labelledby="ai-profile-settings-title">
    <div class="flex items-start justify-between gap-4">
      <div class="min-w-0">
        <h3 id="ai-profile-settings-title" class="type-body m-0 font-semibold text-[var(--text)]">{{ t('ai.profilesTitle') }}</h3>
        <p class="type-metadata mb-0 mt-1 max-w-[70ch] leading-4 text-[var(--text-muted)]">{{ t('ai.profilesDescription') }}</p>
      </div>
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
            <span class="text-[var(--text-muted)]">{{ t('ai.modelDiscoveryNotTested') }}</span>
            <span v-if="ai.connectionReports.value[profile.id]?.safeMessage" class="truncate text-[var(--danger)]">{{ ai.connectionReports.value[profile.id]?.safeMessage }}</span>
          </div>
        </div>
        <UBadge
          :color="profile.credentialRequired && !profile.hasCredential ? 'warning' : 'neutral'"
          variant="soft"
          size="sm"
          :label="profile.protocol === 'codex_subscription' ? t('ai.codexSubscription') : profile.protocol === 'ollama_chat' ? t('ai.localProvider') : profile.hasCredential ? t('ai.credentialStored') : t('ai.credentialMissing')"
        />
        <div class="flex shrink-0 items-center gap-1">
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-tabler-plug-connected"
            :label="t('ai.testConnection')"
            :aria-label="t('ai.testConnectionNamed', { name: profile.name })"
            :title="t('ai.testConnectionCostHint')"
            :loading="ai.busy.value && ai.currentJob.value?.scopeId === `connection:${profile.id}`"
            :disabled="ai.busy.value || ai.taskRunning.value || (profile.credentialRequired && !profile.hasCredential)"
            @click="ai.testProfile(profile.id)"
          />
          <UButton v-if="ai.catalog.value.defaultProfileId !== profile.id" color="neutral" variant="ghost" size="xs" :label="t('ai.makeDefault')" :disabled="ai.busy.value" @click="ai.setDefaultProfile(profile.id)" />
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('ai.editProfile', { name: profile.name })" @click="openEdit(profile)" />
          <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('ai.deleteProfile', { name: profile.name })" @click="pendingDelete = profile" />
        </div>
      </div>
    </div>
    <div v-else class="flex min-h-24 items-center gap-3 py-4 text-[var(--text-muted)]">
      <UIcon name="i-tabler-sparkles-off" class="size-6 shrink-0" aria-hidden="true" />
      <p class="type-metadata m-0 max-w-[68ch] leading-4">{{ t('ai.emptyProfiles') }}</p>
    </div>

    <p class="type-metadata m-0 border-t border-[var(--border)] pt-3 leading-4 text-[var(--text-muted)]">{{ t('ai.credentialStorageHint') }}</p>

  </section>

  <ManagementFormModal
    :open="editorOpen"
    :title="editingProfile ? t('ai.editProfileTitle') : t('ai.addProfile')"
    :description="t('ai.profileDialogDescription')"
    :confirm-label="t('ai.saveProfile')"
    :confirm-disabled="!formValid"
    :busy="ai.busy.value"
    width="lg"
    @update:open="editorOpen = $event"
    @confirm="save"
  >
    <div class="grid grid-cols-2 gap-x-4 gap-y-3 @max-[560px]:grid-cols-1">
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
        <UInput v-model="form.baseUrl" :aria-label="t('ai.baseUrl')" spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField :label="t('ai.modelId')" required>
        <UInput v-model="form.modelId" :aria-label="t('ai.modelId')" spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField v-if="reasoningConfigurable" :label="t('ai.reasoningEffort')">
        <USelect v-model="form.reasoningEffort" :items="reasoningItems" value-key="value" label-key="label" :aria-label="t('ai.reasoningEffort')" class="w-full" />
        <p class="type-caption m-0 mt-1 leading-4 text-[var(--text-muted)]">{{ reasoningHint }}</p>
      </UFormField>
      <UFormField v-if="showsCredential" :label="t('ai.apiKey')" :required="credentialRequired && !editingProfile?.hasCredential">
        <UInput v-model="form.secret" type="password" :aria-label="t('ai.apiKey')" :placeholder="editingProfile?.hasCredential ? t('ai.keepCredentialPlaceholder') : ''" autocomplete="new-password" class="w-full" />
      </UFormField>
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
      <h3 class="m-0 text-[12px] font-semibold text-[var(--text)]">{{ t('ai.filterTitle') }}</h3>
      <p class="type-metadata mb-3 mt-1 leading-4 text-[var(--text-muted)]">{{ t('ai.filterDescription') }}</p>
      <div class="grid grid-cols-2 gap-x-6 gap-y-3 @max-[560px]:grid-cols-1">
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterPureNumbers') }}</span><USwitch v-model="form.filterPolicy.skipPureNumbersOrSymbols" /></label>
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterMeasurements') }}</span><USwitch v-model="form.filterPolicy.skipNumericMeasurements" /></label>
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterSingleCharacter') }}</span><USwitch v-model="form.filterPolicy.skipSingleCharacter" /></label>
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterContainingDigits') }}</span><USwitch v-model="form.filterPolicy.skipTextContainingDigits" /></label>
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterUrls') }}</span><USwitch v-model="form.filterPolicy.skipUrls" /></label>
        <label class="flex items-center justify-between gap-3 text-[11px] text-[var(--text-secondary)]"><span>{{ t('ai.filterPaths') }}</span><USwitch v-model="form.filterPolicy.skipFilePaths" /></label>
      </div>
      <div class="mt-4 grid grid-cols-[140px_minmax(0,1fr)] gap-4 @max-[560px]:grid-cols-1">
        <UFormField :label="t('ai.maxSourceChars')">
          <UInput v-model.number="form.filterPolicy.maxSourceChars" type="number" min="1" :aria-label="t('ai.maxSourceChars')" :placeholder="t('ai.unlimited')" class="w-full" />
        </UFormField>
        <UFormField :label="t('ai.excludedPatterns')" :hint="t('ai.excludedPatternsHint')">
          <UTextarea v-model="excludedPatternsText" :aria-label="t('ai.excludedPatterns')" :rows="3" class="w-full" />
        </UFormField>
      </div>
    </div>

    <div class="mt-4 flex flex-wrap items-center gap-5 border-t border-[var(--border)] pt-4">
      <UCheckbox v-model="form.makeDefault" :label="t('ai.useAsDefault')" />
      <UCheckbox v-if="showsCredential && editingProfile?.hasCredential" v-model="form.clearCredential" color="warning" :label="t('ai.clearCredential')" />
    </div>
  </ManagementFormModal>

  <ConfirmDialog
    :open="Boolean(pendingDelete)"
    :title="t('ai.deleteProfileTitle')"
    :description="t('ai.deleteProfileDescription', { name: pendingDelete?.name ?? '' })"
    @update:open="$event || (pendingDelete = null)"
    @confirm="confirmDelete"
  />
</template>
