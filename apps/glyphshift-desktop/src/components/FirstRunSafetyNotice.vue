<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  open: boolean
  busy?: boolean
  error?: string
}>(), {
  busy: false,
  error: '',
})

const emit = defineEmits<{ confirm: [] }>()
const { t } = useI18n()
const exceptions = computed(() => [
  { icon: 'i-tabler-components-off', color: 'text-[var(--text-muted)]', title: t('firstRun.safety.compatibilityTitle'), description: t('firstRun.safety.compatibilityDescription') },
  { icon: 'i-tabler-shield-lock', color: 'text-error', title: t('firstRun.safety.securityTitle'), description: t('firstRun.safety.securityDescription') },
  { icon: 'i-tabler-app-window', color: 'text-warning', title: t('firstRun.safety.crashTitle'), description: t('firstRun.safety.crashDescription') },
])
</script>

<template>
  <UModal
    :open="open"
    :title="t('firstRun.safety.title')"
    :description="t('firstRun.safety.description')"
    :dismissible="false"
    :close="false"
    :ui="{
      content: 'max-w-[560px]',
      header: 'min-h-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'type-metadata mt-1 max-w-[68ch] leading-5',
      body: 'px-5 py-4',
      footer: 'px-5 py-4',
    }"
    data-testid="first-run-safety"
  >
    <template #body>
      <div class="space-y-4">
        <UAlert
          color="success"
          variant="soft"
          icon="i-tabler-shield-check"
          :title="t('firstRun.safety.normalTitle')"
          :description="t('firstRun.safety.normalDescription')"
        />
        <div>
          <h2 class="m-0 type-section-title font-semibold text-[var(--text)]">{{ t('firstRun.safety.exceptionsTitle') }}</h2>
          <ul class="mb-0 mt-3 space-y-3 p-0" :aria-label="t('firstRun.safety.riskListLabel')">
          <li v-for="exception in exceptions" :key="exception.title" class="flex gap-3">
            <UIcon :name="exception.icon" class="mt-0.5 size-5 shrink-0" :class="exception.color" aria-hidden="true" />
            <div class="min-w-0">
              <p class="m-0 type-label font-semibold text-[var(--text)]">{{ exception.title }}</p>
              <p class="mb-0 mt-1 type-metadata leading-5 text-[var(--text-muted)]">{{ exception.description }}</p>
            </div>
          </li>
          </ul>
        </div>
        <UAlert
          color="neutral"
          variant="soft"
          icon="i-tabler-device-floppy"
          :title="t('firstRun.safety.recommendationTitle')"
          :description="t('firstRun.safety.recommendationDescription')"
        />
        <p class="m-0 type-label leading-5 text-[var(--text)]">{{ t('firstRun.safety.beforeYouStart') }}</p>
        <p class="m-0 type-metadata leading-5 text-[var(--text-muted)]">
          {{ t('firstRun.safety.disclaimer') }}
        </p>
        <p v-if="props.error" role="alert" class="m-0 type-metadata leading-5 text-[var(--danger)]">
          {{ props.error }}
        </p>
      </div>
    </template>
    <template #footer>
      <UButton
        color="primary"
        variant="solid"
        size="sm"
        :label="t('firstRun.safety.confirm')"
        :loading="busy"
        class="ml-auto"
        @click="emit('confirm')"
      />
    </template>
  </UModal>
</template>
