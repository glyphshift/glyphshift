<script setup lang="ts">
import { ref, watch } from 'vue'

defineOptions({ inheritAttrs: false })

const props = defineProps<{
  title?: string
  description?: string
  dismissKey?: string | number | null
}>()
const emit = defineEmits<{ dismiss: [] }>()
const open = ref(true)

watch([
  () => props.title,
  () => props.description,
  () => props.dismissKey,
], () => { open.value = true })

function updateOpen(value: boolean) {
  open.value = value
  if (!value) emit('dismiss')
}
</script>

<template>
  <UAlert
    v-if="open"
    v-bind="$attrs"
    :title="props.title"
    :description="props.description"
    close
    @update:open="updateOpen"
  />
</template>
