<script setup lang="ts">
defineProps<{
  title: string
  icon: string
  titleId: string
  contentTestId?: string
}>()
</script>

<template>
  <section
    class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-[var(--app-bg)] p-4"
    :aria-labelledby="titleId"
  >
    <ManagementPageHeader
      :title-id="titleId"
      :title="title"
      :icon="icon"
    />

    <ManagementWorkspaceSurface variant="canvas">
      <div data-testid="utility-page-scroll" class="h-full overflow-y-auto">
        <div
          :data-testid="contentTestId"
          class="w-full"
        >
          <slot />
        </div>
      </div>
    </ManagementWorkspaceSurface>
  </section>
</template>
