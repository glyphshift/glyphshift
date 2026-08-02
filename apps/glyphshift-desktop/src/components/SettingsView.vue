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
    OWN-WORLD: 继承高密度 Windows 管理器、薄分隔线与克制 emerald 焦点。
    STORY: 用户扫描外观分组，修改语言或主题，并立即看到结果。
    FIRST VIEWPORT: 紧凑页头下是一组两行设置，标签说明在左，真实控件在右。
    FORM: established Operate surface；现有管理器结构的局部扩展。
    FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
  -->
  <section class="flex min-h-0 flex-1 flex-col overflow-y-auto bg-[var(--app-bg)] p-4" aria-labelledby="settings-title">
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

    <div class="max-w-[760px]">
      <h2 class="mb-2 mt-0 text-[11px] font-semibold text-[var(--text-secondary)]">{{ t('settings.appearance') }}</h2>
      <div class="border-y border-[var(--border)]">
        <section class="grid min-h-[70px] grid-cols-[28px_minmax(0,1fr)_220px] items-center gap-3 py-3 max-[720px]:grid-cols-[28px_minmax(0,1fr)]">
          <UIcon name="i-tabler-language" class="size-[18px] text-[var(--accent-strong)]" aria-hidden="true" />
          <div class="min-w-0">
            <h3 class="m-0 text-[12px] font-semibold">{{ t('settings.language') }}</h3>
            <p class="mb-0 mt-1 text-[10px] leading-4 text-[var(--text-muted)]">{{ t('settings.languageDescription') }}</p>
          </div>
          <USelect
            :model-value="appSettings.localePreference.value"
            :items="localeItems"
            value-key="value"
            label-key="label"
            :aria-label="t('settings.language')"
            :disabled="appSettings.settingsBusy.value"
            class="w-full max-[720px]:col-start-2"
            @update:model-value="updateLocale"
          />
        </section>
        <section class="grid min-h-[70px] grid-cols-[28px_minmax(0,1fr)_220px] items-center gap-3 border-t border-[var(--border)] py-3 max-[720px]:grid-cols-[28px_minmax(0,1fr)]">
          <UIcon name="i-tabler-sun-moon" class="size-[18px] text-[var(--accent-strong)]" aria-hidden="true" />
          <div class="min-w-0">
            <h3 class="m-0 text-[12px] font-semibold">{{ t('settings.theme') }}</h3>
            <p class="mb-0 mt-1 text-[10px] leading-4 text-[var(--text-muted)]">{{ t('settings.themeDescription') }}</p>
          </div>
          <USelect
            :model-value="appSettings.themePreference.value"
            :items="themeItems"
            value-key="value"
            label-key="label"
            :aria-label="t('settings.theme')"
            :disabled="appSettings.settingsBusy.value"
            class="w-full max-[720px]:col-start-2"
            @update:model-value="updateTheme"
          />
        </section>
      </div>
    </div>
  </section>
</template>
