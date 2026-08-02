<script setup lang="ts">
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'dictionaries' | 'help']
}>()
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-y-auto bg-[var(--app-bg)] p-4" aria-labelledby="settings-title">
    <ManagementPageHeader
      title-id="settings-title"
      title="设置"
      description="当前没有需要手动配置的全局运行项；资产与运行组合各自在对应页面维护。"
      icon="i-tabler-settings"
    />

    <div class="max-w-[860px] border-y border-[var(--border)]">
      <section class="grid grid-cols-[28px_minmax(0,1fr)_auto] items-start gap-3 py-5">
        <UIcon name="i-tabler-books" class="mt-0.5 size-[19px] text-[var(--accent-strong)]" aria-hidden="true" />
        <div class="min-w-0">
          <div class="flex items-center gap-2"><h2 class="m-0 text-[13px] font-semibold">字典来源</h2><UBadge color="neutral" variant="soft" size="sm" label="本地资产" /></div>
          <p class="mb-0 mt-1 max-w-[68ch] text-[11px] leading-5 text-[var(--text-muted)]">Glyphshift 使用已安装到字典库的版本化词典。未来可从字典市场或网站下载安装，运行时始终读取本地资产。</p>
        </div>
        <UButton color="neutral" variant="outline" size="sm" label="打开词典库" @click="emit('navigate', 'dictionaries')" />
      </section>
      <section class="grid grid-cols-[28px_minmax(0,1fr)_auto] items-start gap-3 border-t border-[var(--border)] py-5">
        <UIcon name="i-tabler-git-branch" class="mt-0.5 size-[19px] text-[var(--accent-strong)]" aria-hidden="true" />
        <div class="min-w-0"><h2 class="m-0 text-[13px] font-semibold">运行组合</h2><p class="mb-0 mt-1 max-w-[68ch] text-[11px] leading-5 text-[var(--text-muted)]">拦截方式、词典和字体按软件目标组合，不提供会覆盖所有工作流的全局 Hook、字体或翻译开关。</p></div>
        <UButton color="neutral" variant="outline" size="sm" label="打开工作流" @click="emit('navigate', 'workflows')" />
      </section>
      <section class="grid grid-cols-[28px_minmax(0,1fr)_auto] items-start gap-3 border-t border-[var(--border)] py-5">
        <UIcon name="i-tabler-help-circle" class="mt-0.5 size-[19px] text-[var(--accent-strong)]" aria-hidden="true" />
        <div class="min-w-0"><h2 class="m-0 text-[13px] font-semibold">适配器说明</h2><p class="mb-0 mt-1 max-w-[68ch] text-[11px] leading-5 text-[var(--text-muted)]">当前可用 Adapter、所属平台、技术分类和能力统一放在帮助页查看。</p></div>
        <UButton color="neutral" variant="outline" size="sm" label="打开帮助" @click="emit('navigate', 'help')" />
      </section>
    </div>
  </section>
</template>
