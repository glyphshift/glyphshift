<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import type { AdapterOption } from '../model'

defineProps<{ adapters: AdapterOption[] }>()
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'dictionaries']
}>()

const columns: TableColumn<AdapterOption>[] = [
  { id: 'adapter', header: '适配器', meta: { class: { th: 'w-[30%]', td: 'w-[30%]' } } },
  { id: 'platform', header: '平台', meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'technology', header: '技术', meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'features', header: '能力' },
  { id: 'configuration', header: '配置', meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'version', header: '版本', meta: { class: { th: 'w-24', td: 'w-24' } } },
]

const featureLabels: Record<string, string> = {
  textObserve: '文字观察',
  textReplace: '文字替换',
  fontSubstitute: '字体替换',
  layoutAdjust: '布局调整',
  resourceReplace: '资源替换',
}

function platformLabel(value: string) {
  if (value === 'windows') return 'Windows'
  if (value === 'macos') return 'macOS'
  return value
}

function featureLabel(value: string) {
  return featureLabels[value] ?? value
}
</script>

<template>
  <!--
    THESIS: 帮助页解释当前真实能力，不堆砌泛用文档或连接状态。
    OWN-WORLD: 继承近黑 Windows 管理器、薄边界、满宽表格与 emerald 主动作。
    STORY: 用户先理解资产与 Adapter 的边界，再核对本机当前可用适配器。
    FIRST VIEWPORT: 页头、简短模型说明、Adapter 表格及直接工作入口。
    FORM: established Read/Operate surface，结构由当前信息任务直接确定。
    FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
  -->
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="help-title">
    <ManagementPageHeader
      title-id="help-title"
      title="帮助"
      description="了解 Glyphshift 如何组合字典、字体与 Adapter，并查看当前可用的适配器能力。"
      icon="i-tabler-help-circle"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-git-branch" label="打开工作流" @click="emit('navigate', 'workflows')" />
      </template>
    </ManagementPageHeader>

    <div class="min-h-0 flex-1 overflow-auto">
      <div class="max-w-[1120px]">
        <section class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-6 border-y border-[var(--border)] py-4">
          <div>
            <h2 class="m-0 text-[13px] font-semibold">Adapter 是具体执行方式</h2>
            <p class="mb-0 mt-1 max-w-[75ch] text-[11px] leading-5 text-[var(--text-muted)]">Platform 说明在哪个系统运行，Technology 用来分类；真正被工作流选择的是具体 Adapter。字典只保存文字规则，字体方案只保存候选字体，它们都不会携带拦截属性。</p>
          </div>
          <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-book-2" label="打开词典库" @click="emit('navigate', 'dictionaries')" />
        </section>

        <section class="mt-5" aria-labelledby="adapter-help-title">
          <div class="mb-2 flex items-end justify-between gap-4"><div><h2 id="adapter-help-title" class="m-0 text-[13px] font-semibold">当前适配器</h2><p class="mb-0 mt-1 text-[10px] text-[var(--text-muted)]">列表来自本机 Runtime Bundle；当前第一方 Adapter 没有可调参数。</p></div><span class="text-[10px] tabular-nums text-[var(--text-muted)]">{{ adapters.length }} 个可用</span></div>
          <div class="overflow-hidden rounded-[7px] border border-[var(--border)] bg-[var(--surface)]">
            <UTable :data="adapters" :columns="columns" :ui="{ base: 'min-w-[880px]' }">
              <template #adapter-cell="{ row }"><div class="font-semibold">{{ row.original.name }}</div><div class="mt-0.5 max-w-[58ch] text-[9px] leading-4 text-[var(--text-muted)]">{{ row.original.summary }}</div></template>
              <template #platform-cell="{ row }"><div class="flex flex-wrap gap-1"><UBadge v-for="platform in row.original.platforms" :key="platform" color="neutral" variant="soft" size="sm" :label="platformLabel(platform)" /></div></template>
              <template #technology-cell="{ row }"><div class="flex flex-wrap gap-1"><UBadge v-for="technology in row.original.technologies" :key="technology" color="neutral" variant="outline" size="sm" :label="technology" /></div></template>
              <template #features-cell="{ row }"><div class="flex flex-wrap gap-1"><UBadge v-for="feature in row.original.features" :key="feature" color="neutral" variant="soft" size="sm" :label="featureLabel(feature)" /></div></template>
              <template #configuration-cell="{ row }"><span>{{ row.original.configuration === 'none' ? '无需配置' : row.original.configuration }}</span></template>
              <template #version-cell="{ row }"><span class="tabular-nums">v{{ row.original.version }}</span></template>
              <template #empty><UEmpty icon="i-tabler-plug-off" title="没有发现可用适配器" description="当前无法创建可运行的工作流。请检查本地 Runtime Bundle 后重新启动 Glyphshift。" /></template>
            </UTable>
          </div>
        </section>
      </div>
    </div>
  </section>
</template>
