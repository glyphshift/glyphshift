<script setup lang="ts">
withDefaults(defineProps<{
  title: string
  description?: string
  headingLevel?: 2 | 3 | 4
}>(), {
  description: undefined,
  headingLevel: 2,
})
</script>

<template>
  <section
    class="@container mx-auto w-full max-w-[980px] overflow-hidden rounded-[10px] border border-[var(--border)] bg-[var(--surface-inset)]"
    :aria-label="title"
  >
    <header class="border-b border-[var(--border)] px-5 py-4">
      <component
        :is="`h${headingLevel}`"
        class="m-0 text-[13px] font-semibold tracking-[-0.01em] text-[var(--text)]"
      >
        {{ title }}
      </component>
      <p v-if="description" class="mb-0 mt-1 max-w-[72ch] text-[10px] leading-4 text-[var(--text-muted)]">
        {{ description }}
      </p>
    </header>

    <div class="px-5">
      <slot />
    </div>

    <div v-if="$slots.after" class="border-t border-[var(--border)] px-5 py-4">
      <slot name="after" />
    </div>
  </section>
</template>
