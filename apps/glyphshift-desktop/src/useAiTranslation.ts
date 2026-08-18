import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'
import { translateCommandError } from './commandError'
import type { DictionaryDetail, ProbeRunSummary } from './model'
import { hasDesktopRuntime } from './workspace/state'

export type AiProviderProtocol
  = 'open_ai_responses'
    | 'open_ai_chat_completions'
    | 'open_ai_compatible'
    | 'anthropic_messages'
    | 'gemini_generate_content'
    | 'ollama_chat'

export interface AiFilterPolicy {
  skipPureNumbersOrSymbols: boolean
  skipNumericMeasurements: boolean
  skipSingleCharacter: boolean
  skipTextContainingDigits: boolean
  skipUrls: boolean
  skipEmails: boolean
  skipFilePaths: boolean
  skipShortcuts: boolean
  maxSourceChars: number | null
  excludedPatterns: string[]
}

export interface AiProfile {
  id: string
  name: string
  protocol: AiProviderProtocol
  baseUrl: string
  modelId: string
  timeoutMs: number
  maxItemsPerRequest: number
  maxConcurrency: number
  maxRetries: number
  filterPolicy: AiFilterPolicy
  hasCredential: boolean
  credentialRequired: boolean
}

export type CredentialUpdate
  = { action: 'keep' }
    | { action: 'replace'; secret: string }
    | { action: 'clear' }

export interface AiProfileDraft extends Omit<AiProfile, 'hasCredential' | 'credentialRequired'> {
  credential: CredentialUpdate
}

export interface AiProfilesView {
  defaultProfileId: string | null
  profiles: AiProfile[]
}

export type AiSkipReason
  = 'empty_source'
    | 'already_translated'
    | 'ignored'
    | 'pure_number_or_symbols'
    | 'numeric_measurement'
    | 'single_character'
    | 'contains_digit'
    | 'url'
    | 'email'
    | 'file_path'
    | 'shortcut'
    | 'too_long'
    | 'custom_pattern'
    | 'duplicate_source'

export interface AiTranslationCandidate {
  itemId: string
  source: string
  protectedTokens: string[]
}

export interface AiSkippedItem {
  itemId: string
  source: string
  reason: AiSkipReason
}

export interface AiTranslationPlan {
  token: string
  scopeId: string
  snapshotRevision: number
  sourceLocale: string
  targetLocale: string
  candidates: AiTranslationCandidate[]
  skipped: AiSkippedItem[]
}

export interface AiValidatedTranslation {
  itemId: string
  source: string
  translation: string
}

export interface AiProviderError {
  category: string
  retryable: boolean
  retryAfterMs: number | null
  providerCode: string | null
  requestId: string | null
  httpStatus: number | null
  safeMessage: string
}

export type AiTranslationBatchStatus
  = 'queued' | 'running' | 'retrying' | 'completed' | 'failed' | 'cancelled'

export interface AiTranslationBatch {
  batchNumber: number
  itemCount: number
  status: AiTranslationBatchStatus
  attemptCount: number
  startedAfterMs: number | null
  elapsedMs: number
  lastError: AiProviderError | null
}

export interface AiTranslationJob {
  jobId: string
  planToken: string
  scopeId: string
  snapshotRevision: number
  status: 'queued' | 'running' | 'completed' | 'completed_with_failures' | 'cancelling' | 'cancelled'
  totalCount: number
  completedCount: number
  failedCount: number
  totalBatches: number
  batchSize: number
  maxConcurrency: number
  maxRetries: number
  finishedBatches: number
  failedBatches: number
  elapsedMs: number
  peakConcurrency: number
  batches: AiTranslationBatch[]
  results: AiValidatedTranslation[]
  errors: AiProviderError[]
}

export interface AiTranslationItemInput {
  itemId: string
  source: string
  translation?: string | null
  ignored?: boolean
}

export interface ProbeAiApplyView {
  probe: ProbeRunSummary
  appliedCount: number
  skippedCount: number
}

export interface AiConnectionReport {
  profileId: string
  testedAtMs: number
  status: 'passed' | 'failed'
  networkAndAuthentication: 'passed' | 'failed'
  modelDiscovery: 'not_tested'
  modelInvocation: 'passed' | 'failed'
  structuredOutput: 'passed' | 'failed'
  safeMessage: string
}

export const defaultAiFilterPolicy = (): AiFilterPolicy => ({
  skipPureNumbersOrSymbols: true,
  skipNumericMeasurements: true,
  skipSingleCharacter: true,
  skipTextContainingDigits: false,
  skipUrls: true,
  skipEmails: true,
  skipFilePaths: true,
  skipShortcuts: true,
  maxSourceChars: null,
  excludedPatterns: [],
})

export const providerDefaults: Record<AiProviderProtocol, { baseUrl: string; concurrency: number; credentialRequired: boolean }> = {
  open_ai_responses: { baseUrl: 'https://api.openai.com/v1', concurrency: 2, credentialRequired: true },
  open_ai_chat_completions: { baseUrl: 'https://api.openai.com/v1', concurrency: 2, credentialRequired: true },
  open_ai_compatible: { baseUrl: 'http://127.0.0.1:8000/v1', concurrency: 2, credentialRequired: false },
  anthropic_messages: { baseUrl: 'https://api.anthropic.com', concurrency: 2, credentialRequired: true },
  gemini_generate_content: { baseUrl: 'https://generativelanguage.googleapis.com/v1beta', concurrency: 2, credentialRequired: true },
  ollama_chat: { baseUrl: 'http://127.0.0.1:11434/api', concurrency: 1, credentialRequired: false },
}

const REQUEST_TOKEN_RESERVE = 384
const ITEM_TOKEN_RESERVE = 3

function estimatedTextTokens(value: string) {
  let ascii = 0
  let nonAscii = 0
  for (const character of value) {
    if (/^[\x00-\x7F]$/.test(character)) ascii += 1
    else nonAscii += 1
  }
  return nonAscii + Math.ceil(ascii / 4)
}

export function estimateAiTranslationInput(plan: AiTranslationPlan, maxItemsPerRequest: number) {
  const batchSize = Math.max(1, Math.floor(maxItemsPerRequest))
  const totalBatches = Math.ceil(plan.candidates.length / batchSize)
  const itemTokens = plan.candidates.reduce((total, candidate) => (
    total
    + ITEM_TOKEN_RESERVE
    + estimatedTextTokens(candidate.source)
  ), 0)
  return {
    estimatedInputTokens: totalBatches * REQUEST_TOKEN_RESERVE + itemTokens,
    totalBatches,
  }
}

const BROWSER_STORAGE_KEY = 'glyphshift.ai-profiles.v2'
const catalog = ref<AiProfilesView>({ defaultProfileId: null, profiles: [] })
const busy = ref(false)
const error = ref('')
const currentJob = ref<AiTranslationJob | null>(null)
const elapsedMs = ref(0)
const connectionReports = ref<Record<string, AiConnectionReport>>({})
const browserPlans = new Map<string, AiTranslationPlan>()
const browserJobs = new Map<string, AiTranslationJob>()
const browserJobStartedAt = new Map<string, number>()
let browserPlanSequence = 0
let browserJobSequence = 0
let connected = false
let elapsedTimer: number | undefined

function formatElapsedDuration(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)
  const trailing = `${minutes}:${seconds.toString().padStart(2, '0')}`
  return hours ? `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}` : trailing
}

const elapsed = computed(() => formatElapsedDuration(elapsedMs.value))

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function persistBrowserCatalog() {
  localStorage.setItem(BROWSER_STORAGE_KEY, JSON.stringify(catalog.value))
}

function normalizeProfile(profile: AiProfile): AiProfile {
  return {
    ...profile,
    maxItemsPerRequest: Number.isInteger(profile.maxItemsPerRequest)
      && profile.maxItemsPerRequest >= 1
      && profile.maxItemsPerRequest <= 1_000
      ? profile.maxItemsPerRequest
      : 50,
    maxRetries: Number.isInteger(profile.maxRetries) && profile.maxRetries >= 0 && profile.maxRetries <= 10
      ? profile.maxRetries
      : 2,
  }
}

function normalizeCatalog(value: AiProfilesView | null): AiProfilesView {
  return value && Array.isArray(value.profiles)
    ? { ...value, profiles: value.profiles.map(normalizeProfile) }
    : { defaultProfileId: null, profiles: [] }
}

function browserCatalog(): AiProfilesView {
  try {
    const value = JSON.parse(localStorage.getItem(BROWSER_STORAGE_KEY) ?? 'null') as AiProfilesView | null
    if (value && Array.isArray(value.profiles)) return normalizeCatalog(value)
  }
  catch {
    // A malformed browser-only preview value is treated as an empty catalog.
  }
  return { defaultProfileId: null, profiles: [] }
}

async function connect(force = false) {
  if (connected && !force) return catalog.value
  if (hasDesktopRuntime()) {
    const loaded = await invoke<AiProfilesView | null>('desktop_ai_profiles')
    catalog.value = normalizeCatalog(loaded)
  }
  else catalog.value = browserCatalog()
  connected = true
  return catalog.value
}

async function saveProfile(profile: AiProfileDraft, makeDefault: boolean) {
  busy.value = true
  error.value = ''
  try {
    if (hasDesktopRuntime()) {
      catalog.value = await invoke<AiProfilesView>('desktop_save_ai_profile', {
        request: { profile, makeDefault },
      })
    }
    else {
      const previous = catalog.value.profiles.find(item => item.id === profile.id)
      const defaults = providerDefaults[profile.protocol]
      const next: AiProfile = {
        ...clone(profile),
        filterPolicy: clone(profile.filterPolicy),
        hasCredential: profile.credential.action === 'replace'
          ? Boolean(profile.credential.secret.trim())
          : profile.credential.action === 'clear' ? false : previous?.hasCredential ?? false,
        credentialRequired: defaults.credentialRequired,
      }
      delete (next as Partial<AiProfileDraft>).credential
      catalog.value = {
        defaultProfileId: makeDefault || !catalog.value.defaultProfileId
          ? next.id
          : catalog.value.defaultProfileId,
        profiles: [...catalog.value.profiles.filter(item => item.id !== next.id), next]
          .sort((left, right) => left.name.localeCompare(right.name)),
      }
      persistBrowserCatalog()
    }
    return catalog.value
  }
  catch (reason) {
    error.value = translateCommandError(reason)
    throw reason
  }
  finally {
    busy.value = false
  }
}

async function setDefaultProfile(profileId: string) {
  busy.value = true
  error.value = ''
  try {
    if (hasDesktopRuntime()) {
      catalog.value = await invoke<AiProfilesView>('desktop_set_default_ai_profile', { profileId })
    }
    else {
      catalog.value = { ...catalog.value, defaultProfileId: profileId }
      persistBrowserCatalog()
    }
  }
  catch (reason) {
    error.value = translateCommandError(reason)
    throw reason
  }
  finally {
    busy.value = false
  }
}

async function deleteProfile(profileId: string) {
  busy.value = true
  error.value = ''
  try {
    if (hasDesktopRuntime()) {
      catalog.value = await invoke<AiProfilesView>('desktop_delete_ai_profile', { profileId })
    }
    else {
      const profiles = catalog.value.profiles.filter(item => item.id !== profileId)
      catalog.value = {
        profiles,
        defaultProfileId: catalog.value.defaultProfileId === profileId
          ? profiles[0]?.id ?? null
          : catalog.value.defaultProfileId,
      }
      persistBrowserCatalog()
    }
  }
  catch (reason) {
    error.value = translateCommandError(reason)
    throw reason
  }
  finally {
    busy.value = false
  }
}

function skipReason(item: AiTranslationItemInput, policy: AiFilterPolicy): AiSkipReason | null {
  const source = item.source.trim()
  if (item.translation?.trim()) return 'already_translated'
  if (item.ignored) return 'ignored'
  if (!source) return 'empty_source'
  if (policy.skipSingleCharacter && [...source].length === 1) return 'single_character'
  if (policy.skipUrls && /^(?:https?|ftp):\/\//i.test(source)) return 'url'
  if (policy.skipEmails && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(source)) return 'email'
  if (policy.skipFilePaths && /^(?:[a-z]:[\\/]|\\\\|\/)[^\n]+/i.test(source)) return 'file_path'
  if (policy.skipShortcuts && /^(?:(?:ctrl|alt|shift|cmd|command|win|super)\s*\+\s*)+[\w\d]+$/i.test(source)) return 'shortcut'
  if (policy.skipNumericMeasurements && /^\d+(?:[.,]\d+)?\s*(?:[x×]\s*\d+(?:[.,]\d+)?|fps|hz|px|%|ms|s|kb|mb|gb|°c)$/i.test(source)) return 'numeric_measurement'
  if (policy.skipPureNumbersOrSymbols && !/[\p{L}]/u.test(source)) return 'pure_number_or_symbols'
  if (policy.skipTextContainingDigits && /\d/.test(source)) return 'contains_digit'
  if (policy.maxSourceChars && [...source].length > policy.maxSourceChars) return 'too_long'
  if (policy.excludedPatterns.some(pattern => {
    try { return new RegExp(pattern).test(source) }
    catch { return false }
  })) return 'custom_pattern'
  return null
}

function browserPlan(input: {
  scopeId: string
  snapshotRevision: number
  sourceLocale: string
  targetLocale: string
  items: AiTranslationItemInput[]
  profileId?: string | null
}): AiTranslationPlan {
  const profile = catalog.value.profiles.find(item => item.id === (input.profileId ?? catalog.value.defaultProfileId))
  if (!profile) throw new Error('AI profile required')
  const candidates: AiTranslationCandidate[] = []
  const skipped: AiSkippedItem[] = []
  const sources = new Set<string>()
  input.items.forEach(item => {
    const source = item.source.trim()
    const reason = skipReason(item, profile.filterPolicy)
      ?? (sources.has(source) ? 'duplicate_source' : null)
    if (reason) skipped.push({ itemId: item.itemId, source, reason })
    else {
      sources.add(source)
      candidates.push({
        itemId: item.itemId,
        source,
        protectedTokens: source.match(/\$\{[^}]+\}|\{[^}]+\}|%(?:\d+|s)/g) ?? [],
      })
    }
  })
  const plan: AiTranslationPlan = {
    token: `browser-plan-${++browserPlanSequence}`,
    scopeId: input.scopeId,
    snapshotRevision: input.snapshotRevision,
    sourceLocale: input.sourceLocale,
    targetLocale: input.targetLocale,
    candidates,
    skipped,
  }
  browserPlans.set(plan.token, plan)
  return plan
}

async function planDictionary(detail: DictionaryDetail, profileId?: string | null) {
  const input = {
    profileId: profileId ?? null,
    scopeId: `dictionary:${detail.metadata.id}`,
    snapshotRevision: detail.revision,
    sourceLocale: detail.metadata.sourceLocale,
    targetLocale: detail.metadata.targetLocale,
    items: detail.entries.map((entry, index) => ({
      itemId: `dictionary-row-${index + 1}`,
      source: entry.source,
      translation: entry.translation || null,
      ignored: false,
    })),
  }
  return hasDesktopRuntime()
    ? invoke<AiTranslationPlan>('desktop_plan_ai_translation', { request: input })
    : browserPlan(input)
}

async function planProbe(
  runId: string,
  profileId?: string | null,
  browserInput?: { snapshotRevision: number; sourceLocale: string; targetLocale: string; items: AiTranslationItemInput[] },
) {
  if (hasDesktopRuntime()) {
    return invoke<AiTranslationPlan>('desktop_plan_probe_ai_translation', {
      request: { runId, profileId: profileId ?? null },
    })
  }
  if (!browserInput) throw new Error('Browser probe preview requires visible entries')
  return browserPlan({
    profileId,
    scopeId: `probe:${runId}`,
    ...browserInput,
  })
}

function browserTranslation(source: string) {
  const known: Record<string, string> = {
    Close: '关闭',
    Open: '打开',
    Save: '保存',
    Cancel: '取消',
    Settings: '设置',
  }
  return known[source] ?? `AI · ${source}`
}

function browserBatches(plan: AiTranslationPlan, profileId?: string | null) {
  const profile = catalog.value.profiles.find(item => (
    item.id === (profileId ?? catalog.value.defaultProfileId)
  ))
  if (!profile) throw new Error('AI profile required')
  const maxItems = profile.maxItemsPerRequest
  const batches: AiTranslationCandidate[][] = []
  for (let start = 0; start < plan.candidates.length; start += maxItems) {
    batches.push(plan.candidates.slice(start, start + maxItems))
  }
  return batches
}

async function startTranslation(planToken: string, profileId?: string | null) {
  if (hasDesktopRuntime()) {
    return invoke<string>('desktop_start_ai_translation', {
      request: { planToken, profileId: profileId ?? null },
    })
  }
  const plan = browserPlans.get(planToken)
  if (!plan) throw new Error('Unknown browser AI plan')
  const profile = catalog.value.profiles.find(item => (
    item.id === (profileId ?? catalog.value.defaultProfileId)
  ))
  if (!profile) throw new Error('AI profile required')
  const batches = browserBatches(plan, profileId)
  const batchSize = profile.maxItemsPerRequest
  const jobId = `browser-job-${++browserJobSequence}`
  const job: AiTranslationJob = {
    jobId,
    planToken,
    scopeId: plan.scopeId,
    snapshotRevision: plan.snapshotRevision,
    status: 'running',
    totalCount: plan.candidates.length,
    completedCount: 0,
    failedCount: 0,
    totalBatches: batches.length,
    batchSize,
    maxConcurrency: profile.maxConcurrency,
    maxRetries: profile.maxRetries,
    finishedBatches: 0,
    failedBatches: 0,
    elapsedMs: 0,
    peakConcurrency: 0,
    batches: batches.map((batch, index) => ({
      batchNumber: index + 1,
      itemCount: batch.length,
      status: 'queued',
      attemptCount: 0,
      startedAfterMs: null,
      elapsedMs: 0,
      lastError: null,
    })),
    results: [],
    errors: [],
  }
  browserJobs.set(jobId, job)
  const startedAtMs = Date.now()
  browserJobStartedAt.set(jobId, startedAtMs)
  let nextBatchIndex = 0
  let activeBatches = 0
  const startAvailableBatches = () => {
    while (
      job.status === 'running'
      && activeBatches < profile.maxConcurrency
      && nextBatchIndex < batches.length
    ) {
      const index = nextBatchIndex++
      const batch = batches[index]
      const batchState = job.batches[index]
      if (!batch || !batchState) continue
      activeBatches += 1
      batchState.status = 'running'
      batchState.attemptCount = 1
      batchState.startedAfterMs = Date.now() - startedAtMs
      job.peakConcurrency = Math.max(job.peakConcurrency, activeBatches)
      window.setTimeout(() => {
        if (job.status !== 'running') return
        const completedAtMs = Date.now() - startedAtMs
        batchState.status = 'completed'
        batchState.elapsedMs = completedAtMs - (batchState.startedAfterMs ?? 0)
        job.results.push(...batch.map(item => ({
          itemId: item.itemId,
          source: item.source,
          translation: browserTranslation(item.source),
        })))
        job.completedCount = job.results.length
        job.finishedBatches += 1
        job.elapsedMs = completedAtMs
        activeBatches -= 1
        if (job.finishedBatches === batches.length) job.status = 'completed'
        else startAvailableBatches()
      }, 80)
    }
  }
  startAvailableBatches()
  return jobId
}

async function queryJob(jobId: string) {
  if (hasDesktopRuntime()) return invoke<AiTranslationJob>('desktop_ai_translation_job', { jobId })
  const job = browserJobs.get(jobId)
  if (!job) throw new Error('Unknown browser AI job')
  const visible = clone(job)
  const startedAtMs = browserJobStartedAt.get(jobId)
  if (startedAtMs !== undefined && !['completed', 'completed_with_failures', 'cancelled'].includes(job.status)) {
    visible.elapsedMs = Date.now() - startedAtMs
    visible.batches.forEach((batch) => {
      if (['running', 'retrying'].includes(batch.status) && batch.startedAfterMs !== null) {
        batch.elapsedMs = visible.elapsedMs - batch.startedAfterMs
      }
    })
  }
  return visible
}

async function runPlan(plan: AiTranslationPlan, profileId?: string | null) {
  busy.value = true
  error.value = ''
  currentJob.value = null
  elapsedMs.value = 0
  const startedAtMs = Date.now()
  if (elapsedTimer !== undefined) window.clearInterval(elapsedTimer)
  elapsedTimer = window.setInterval(() => {
    elapsedMs.value = Date.now() - startedAtMs
  }, 250)
  try {
    const jobId = await startTranslation(plan.token, profileId)
    const deadline = Date.now() + 10 * 60_000
    while (Date.now() < deadline) {
      const job = await queryJob(jobId)
      currentJob.value = job
      if (['completed', 'completed_with_failures', 'cancelled'].includes(job.status)) return job
      await new Promise(resolve => window.setTimeout(resolve, 120))
    }
    throw new Error('AI translation timed out locally')
  }
  catch (reason) {
    error.value = translateCommandError(reason)
    throw reason
  }
  finally {
    elapsedMs.value = Date.now() - startedAtMs
    if (elapsedTimer !== undefined) {
      window.clearInterval(elapsedTimer)
      elapsedTimer = undefined
    }
    busy.value = false
  }
}

async function testProfile(profileId: string) {
  error.value = ''
  try {
    const input = {
      profileId,
      scopeId: `connection:${profileId}`,
      snapshotRevision: 1,
      sourceLocale: 'en-US',
      targetLocale: 'zh-CN',
      items: [{
        itemId: 'connection-test',
        source: 'Connection test',
        translation: null,
        ignored: false,
      }],
    }
    const plan = hasDesktopRuntime()
      ? await invoke<AiTranslationPlan>('desktop_plan_ai_translation', { request: input })
      : browserPlan(input)
    const job = await runPlan(plan, profileId)
    const passed = job.status === 'completed' && job.results.length === 1
    const report: AiConnectionReport = {
      profileId,
      testedAtMs: Date.now(),
      status: passed ? 'passed' : 'failed',
      networkAndAuthentication: passed ? 'passed' : 'failed',
      modelDiscovery: 'not_tested',
      modelInvocation: passed ? 'passed' : 'failed',
      structuredOutput: passed ? 'passed' : 'failed',
      safeMessage: passed ? '' : job.errors[0]?.safeMessage ?? '',
    }
    connectionReports.value = { ...connectionReports.value, [profileId]: report }
    return report
  }
  catch (reason) {
    const report: AiConnectionReport = {
      profileId,
      testedAtMs: Date.now(),
      status: 'failed',
      networkAndAuthentication: 'failed',
      modelDiscovery: 'not_tested',
      modelInvocation: 'failed',
      structuredOutput: 'failed',
      safeMessage: error.value || translateCommandError(reason),
    }
    connectionReports.value = { ...connectionReports.value, [profileId]: report }
    return report
  }
}

async function cancelCurrentJob() {
  const job = currentJob.value
  if (!job || ['completed', 'completed_with_failures', 'cancelled'].includes(job.status)) return
  if (hasDesktopRuntime()) await invoke('desktop_cancel_ai_translation', { jobId: job.jobId })
  else {
    const browserJob = browserJobs.get(job.jobId)
    if (browserJob) {
      browserJob.status = 'cancelled'
      browserJob.batches.forEach((batch) => {
        if (!['completed', 'failed', 'cancelled'].includes(batch.status)) batch.status = 'cancelled'
      })
    }
  }
  currentJob.value = {
    ...job,
    status: 'cancelled',
    batches: (job.batches ?? []).map(batch => (
      ['completed', 'failed', 'cancelled'].includes(batch.status)
        ? batch
        : { ...batch, status: 'cancelled' as const }
    )),
  }
}

function dismissCurrentJob() {
  const job = currentJob.value
  if (!job || !['completed', 'completed_with_failures', 'cancelled'].includes(job.status)) return
  currentJob.value = null
}

async function applyProbeResults(runId: string, job: AiTranslationJob): Promise<ProbeAiApplyView> {
  if (hasDesktopRuntime()) {
    return invoke<ProbeAiApplyView>('desktop_apply_probe_ai_results', {
      request: {
        runId,
        snapshotRevision: job.snapshotRevision,
        results: job.results,
      },
    })
  }
  return {
    probe: {} as ProbeRunSummary,
    appliedCount: job.results.length,
    skippedCount: 0,
  }
}

export function useAiTranslation() {
  return {
    catalog,
    profiles: computed(() => catalog.value.profiles),
    defaultProfile: computed(() => catalog.value.profiles.find(item => item.id === catalog.value.defaultProfileId) ?? null),
    busy,
    error,
    currentJob,
    elapsed,
    connectionReports,
    connect,
    saveProfile,
    setDefaultProfile,
    deleteProfile,
    planDictionary,
    planProbe,
    runPlan,
    testProfile,
    cancelCurrentJob,
    dismissCurrentJob,
    applyProbeResults,
  }
}
