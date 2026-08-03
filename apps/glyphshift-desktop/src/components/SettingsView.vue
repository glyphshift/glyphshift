<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings, type LocalePreference, type ThemePreference } from '../appSettings'

const { t } = useI18n()
const appSettings = useAppSettings()

const localeItems = computed(() => [
  { value: 'system' as const, label: t('settings.locale.system') },
  { value: 'zh-CN' as const, label: t('settings.locale.zhCN') },
  { value: 'en-US' as const, label: t('settings.locale.enUS') },
])
const themeItems = computed(() => [
  { value: 'system' as const, label: t('settings.themeOption.system') },
  { value: 'dark' as const, label: t('settings.themeOption.dark') },
  { value: 'light' as const, label: t('settings.themeOption.light') },
])
const settingsRowUi = {
  root: '!grid min-h-[76px] grid-cols-[minmax(0,1fr)_220px] items-center justify-items-stretch gap-6 border-b border-[var(--border)] px-4 py-3 last:border-b-0 max-[720px]:grid-cols-1 max-[720px]:gap-2',
  wrapper: 'min-w-0',
  label: 'flex items-center gap-2.5 text-[11px] font-semibold',
  description: 'mt-1 pl-7 text-[10px] leading-4 text-[var(--text-muted)]',
  container: 'min-w-0 w-full max-[720px]:pl-7',
}

function updateLocale(value: unknown) {
  void appSettings.setLocalePreference(value as LocalePreference).catch(() => undefined)
}

function updateTheme(value: unknown) {
  void appSettings.setThemePreference(value as ThemePreference).catch(() => undefined)
}
</script>

<template>
  <!--
    THESIS: 设置只呈现立即生效的应用级偏好，不用说明文字冒充功能。
    OWN-WORLD: 继承高密度 Windows 管理器、薄分隔线与克制 cobalt 焦点。
    STORY: 用户扫描外观分组，修改语言或主题，并立即看到结果。
    FIRST VIEWPORT: 紧凑页头下是一组两行设置，标签说明在左，真实控件在右。
    FORM: established Operate surface；现有管理器结构的局部扩展。
    FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
  -->
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--app-bg)] p-4" aria-labelledby="settings-title">
    <ManagementPageHeader
      title-id="settings-title"
      :title="t('settings.title')"
      :description="t('settings.description')"
      icon="i-tabler-settings"
    />

    <UAlert
      v-if="appSettings.settingsError.value"
      role="alert"
      color="error"
      variant="soft"
      :title="t('settings.saveFailed')"
      :description="appSettings.settingsError.value"
      class="mb-4 max-w-[760px]"
    />

    <ManagementWorkspaceSurface>
      <div class="h-full overflow-y-auto p-5 [scrollbar-gutter:stable]">
        <div class="max-w-[920px]">
          <h2 class="mb-3 mt-0 text-[12px] font-semibold text-[var(--text-secondary)]">{{ t('settings.appearance') }}</h2>
          <div class="overflow-hidden border-y border-[var(--border)]">
            <UFormField
              orientation="horizontal"
              :label="t('settings.language')"
              :description="t('settings.languageDescription')"
              :ui="settingsRowUi"
            >
              <template #label>
                <UIcon name="i-tabler-language" class="size-[18px] text-[var(--accent-strong)]" aria-hidden="true" />
                <span>{{ t('settings.language') }}</span>
              </template>
              <USelect
                :model-value="appSettings.localePreference.value"
                :items="localeItems"
                value-key="value"
                label-key="label"
                :aria-label="t('settings.language')"
                :disabled="appSettings.settingsBusy.value"
                class="w-full"
                @update:model-value="updateLocale"
              />
            </UFormField>

            <UFormField
              orientation="horizontal"
              :label="t('settings.theme')"
              :description="t('settings.themeDescription')"
              :ui="settingsRowUi"
            >
              <template #label>
                <UIcon name="i-tabler-sun-moon" class="size-[18px] text-[var(--accent-strong)]" aria-hidden="true" />
                <span>{{ t('settings.theme') }}</span>
              </template>
              <USelect
                :model-value="appSettings.themePreference.value"
                :items="themeItems"
                value-key="value"
                label-key="label"
                :aria-label="t('settings.theme')"
                :disabled="appSettings.settingsBusy.value"
                class="w-full"
                @update:model-value="updateTheme"
              />
            </UFormField>
          </div>
        </div>
      </div>
    </ManagementWorkspaceSurface>
  </section>
</template>
