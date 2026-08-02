<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

defineProps<{
  current: 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'settings'
  connected: boolean
}>()
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'software' | 'dictionaries' | 'settings']
}>()

const nav = [
  { id: 'workflows' as const, label: '工作流', icon: 'i-tabler-git-branch' },
  { id: 'software' as const, label: '软件', icon: 'i-tabler-library' },
  { id: 'dictionaries' as const, label: '词典', icon: 'i-tabler-book-2' },
]

async function native(action: 'minimize' | 'maximize' | 'close') {
  try {
    const window = getCurrentWindow()
    if (action === 'minimize') await window.minimize()
    else if (action === 'maximize') await window.toggleMaximize()
    else await window.close()
  }
  catch {
    // Browser previews do not expose native window controls.
  }
}
</script>

<template>
  <header class="flex h-12 shrink-0 select-none items-stretch border-b border-[var(--border)] bg-[var(--titlebar)] text-[var(--text-secondary)]" data-tauri-drag-region>
    <div class="flex items-center gap-2 border-r border-[var(--border)] px-3" data-tauri-drag-region>
      <span class="grid h-6 w-6 place-items-center rounded-[5px] bg-[var(--accent)] text-[10px] font-bold text-[var(--accent-foreground)]">G</span>
      <strong class="text-[13px] font-semibold tracking-[-0.015em] text-[var(--text)]">Glyphshift</strong>
      <span class="text-[9px] text-[var(--text-muted)]">v0.2</span>
    </div>
    <nav class="flex items-stretch" aria-label="主导航">
      <UButton
        v-for="item in nav"
        :key="item.id"
        color="neutral"
        variant="ghost"
        size="sm"
        :icon="item.icon"
        :label="item.label"
        :class="[
          'relative h-full min-w-[92px] rounded-none px-3 text-[11px]',
          current === item.id || (item.id === 'dictionaries' && current === 'dictionary-editor')
            ? 'font-semibold text-[var(--text)] after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-[var(--accent)]'
            : 'text-[var(--text-secondary)]',
        ]"
        :aria-current="current === item.id ? 'page' : undefined"
        @click="emit('navigate', item.id)"
      />
    </nav>
    <div class="ml-auto flex items-stretch" data-tauri-drag-region>
      <div class="flex items-center gap-1.5 px-3 text-[9px]" :class="connected ? 'text-[var(--success)]' : 'text-[var(--text-muted)]'">
        <span class="h-1.5 w-1.5 rounded-full bg-current" />{{ connected ? '桌面服务已连接' : '本地预览' }}
      </div>
      <UButton color="neutral" variant="ghost" icon="i-tabler-settings" class="h-full w-10 rounded-none" aria-label="设置" @click="emit('navigate', 'settings')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-minus" class="h-full w-10 rounded-none" aria-label="最小化窗口" @click="native('minimize')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-square" class="h-full w-10 rounded-none" aria-label="最大化窗口" @click="native('maximize')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-x" class="h-full w-10 rounded-none hover:bg-[var(--danger)] hover:text-white" aria-label="关闭窗口" @click="native('close')" />
    </div>
  </header>
</template>
