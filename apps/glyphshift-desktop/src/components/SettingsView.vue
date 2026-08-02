<script setup lang="ts">
import { reactive, ref, watch } from 'vue'

const props = defineProps<{
  translationSource: string
}>()

const emit = defineEmits<{
  source: [value: string]
}>()

const form = reactive({ source: props.translationSource })
const saved = ref(false)

watch(() => props.translationSource, value => { form.source = value })

function saveSource() {
  emit('source', form.source.trim())
  saved.value = true
  window.setTimeout(() => (saved.value = false), 1800)
}
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-y-auto bg-[var(--app-bg)] p-4" aria-labelledby="settings-title">
    <ManagementPageHeader
      title-id="settings-title"
      title="设置"
      description="只管理 Glyphshift 自身；软件身份、运行组合和词典分别在对应页面维护。"
      icon="i-tabler-settings"
    />

    <div class="max-w-[760px] border-t border-[var(--border)] py-5">
      <div class="flex items-start gap-3">
        <UIcon name="i-tabler-cloud-download" class="mt-0.5 size-[19px] text-[var(--accent-strong)]" />
        <div>
          <h2 class="m-0 text-[13px] font-semibold">在线翻译源</h2>
          <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--text-muted)]">可选。未配置时只使用本地词典，不显示虚构的在线内容。</p>
        </div>
      </div>

      <UForm :state="form" class="mt-3 grid grid-cols-[minmax(0,1fr)_auto] items-end gap-2" @submit="saveSource">
        <UFormField label="HTTPS 地址" name="source">
          <UInput v-model="form.source" inputmode="url" size="sm" class="w-full" placeholder="https://…" aria-label="在线翻译源地址" />
        </UFormField>
        <UButton type="submit" color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" label="保存" />
      </UForm>
      <UAlert v-if="saved" color="success" variant="soft" description="设置已保存。" class="mt-3" />
    </div>
  </section>
</template>
