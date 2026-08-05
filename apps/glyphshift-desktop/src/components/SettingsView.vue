<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings, type CloseBehavior, type LocalePreference, type ThemePreference } from '../appSettings'

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
const closeBehaviorItems = computed(() => [
  { value: 'minimize' as const, label: t('settings.closeBehaviorOption.minimize') },
  { value: 'quit' as const, label: t('settings.closeBehaviorOption.quit') },
])
function updateLocale(value: unknown) {
  void appSettings.setLocalePreference(value as LocalePreference).catch(() => undefined)
}

function updateTheme(value: unknown) {
  void appSettings.setThemePreference(value as ThemePreference).catch(() => undefined)
}

function updateLaunchAtStartup(value: boolean) {
  void appSettings.setLaunchAtStartup(value).catch(() => undefined)
}

function updateCloseBehavior(value: unknown) {
  void appSettings.setCloseBehavior(value as CloseBehavior).catch(() => undefined)
}

function updateLaunchElevated(value: boolean) {
  void appSettings.setLaunchElevated(value).catch(() => undefined)
}

onMounted(() => void appSettings.refreshPrivilegeStatus())
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
    <ManagementDetailHeader
      title-id="settings-title"
      :title="t('settings.title')"
      :description="t('settings.description')"
    />

    <UAlert
      v-if="appSettings.settingsError.value"
      role="alert"
      color="error"
      variant="soft"
      :title="t('settings.saveFailed')"
      :description="appSettings.settingsError.value"
      class="mx-auto mb-4 w-full max-w-[980px]"
    />

    <ManagementWorkspaceSurface variant="canvas">
      <div class="h-full overflow-y-auto p-5 [scrollbar-gutter:stable]">
        <div class="space-y-4">
        <ManagementFormSection :title="t('settings.appearance')" :description="t('settings.appearanceDescription')">
          <ManagementFormRow
            :label="t('settings.language')"
            :description="t('settings.languageDescription')"
            icon="i-tabler-language"
            control-width="compact"
          >
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
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.theme')"
            :description="t('settings.themeDescription')"
            icon="i-tabler-sun-moon"
            control-width="compact"
          >
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
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection :title="t('settings.behavior')" :description="t('settings.behaviorDescription')">
          <ManagementFormRow
            :label="t('settings.launchAtStartup')"
            :description="t('settings.launchAtStartupDescription')"
            icon="i-tabler-rocket"
            control-width="compact"
          >
            <div class="flex justify-end">
              <USwitch
                :model-value="appSettings.launchAtStartup.value"
                :aria-label="t('settings.launchAtStartup')"
                :disabled="appSettings.settingsBusy.value"
                @update:model-value="updateLaunchAtStartup"
              />
            </div>
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.closeBehavior')"
            :description="t('settings.closeBehaviorDescription')"
            icon="i-tabler-door-exit"
            control-width="compact"
          >
            <USelect
              :model-value="appSettings.closeBehavior.value"
              :items="closeBehaviorItems"
              value-key="value"
              label-key="label"
              :aria-label="t('settings.closeBehavior')"
              :disabled="appSettings.settingsBusy.value"
              class="w-full"
              @update:model-value="updateCloseBehavior"
            />
          </ManagementFormRow>
        </ManagementFormSection>

        <ManagementFormSection :title="t('settings.privilege')" :description="t('settings.privilegeDescription')">
          <ManagementFormRow
            :label="t('settings.launchElevated')"
            :description="t('settings.launchElevatedDescription')"
            icon="i-tabler-shield-up"
            control-width="compact"
          >
            <div class="flex justify-end">
              <USwitch
                :model-value="appSettings.launchElevated.value"
                :aria-label="t('settings.launchElevated')"
                :disabled="appSettings.settingsBusy.value || appSettings.privilegeBusy.value"
                @update:model-value="updateLaunchElevated"
              />
            </div>
          </ManagementFormRow>

          <ManagementFormRow
            :label="t('settings.currentPrivilege')"
            :description="appSettings.elevated.value ? t('settings.elevatedDescription') : t('settings.standardDescription')"
            icon="i-tabler-shield-lock"
            control-width="compact"
          >
            <div class="flex justify-end">
              <UBadge
                :color="appSettings.elevated.value ? 'warning' : 'neutral'"
                variant="soft"
                :label="appSettings.elevated.value ? t('settings.elevated') : t('settings.standard')"
              />
            </div>
          </ManagementFormRow>
        </ManagementFormSection>
        </div>
      </div>
    </ManagementWorkspaceSurface>
  </section>
</template>
