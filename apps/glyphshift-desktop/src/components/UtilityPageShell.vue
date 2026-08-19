<script setup lang="ts">
withDefaults(defineProps<{
  title: string
  description?: string
  titleId: string
  contentTestId?: string
}>(), {
  description: undefined,
})
</script>

<template>
  <section
    class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-[var(--app-bg)] p-4"
    :aria-labelledby="titleId"
    style="--utility-page-content-width: 980px; --utility-page-axis-inset: 30px"
  >
    <ManagementDetailHeader
      :title-id="titleId"
      :title="title"
      :description="description"
      content-width="utility"
      flush-after
    />

    <ManagementWorkspaceSurface variant="canvas">
      <div data-testid="utility-page-scroll" class="h-full overflow-y-auto p-5 [scrollbar-gutter:stable_both-edges]">
        <div
          :data-testid="contentTestId"
          class="mx-auto w-full"
          style="max-width: var(--utility-page-content-width)"
        >
          <slot />
        </div>
      </div>
    </ManagementWorkspaceSurface>
  </section>
</template>
