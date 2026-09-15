import { computed, nextTick, ref, toValue, watch, type MaybeRefOrGetter } from 'vue'

type TourReference = Element | { getBoundingClientRect: () => DOMRect | DOMRectReadOnly }

export interface GuidedTourStep {
  target?: MaybeRefOrGetter<string | TourReference | null | undefined>
  [key: string]: unknown
}

export function useGuidedTour(steps: MaybeRefOrGetter<GuidedTourStep[]>) {
  const open = ref(false)
  const index = ref(0)
  const list = computed(() => toValue(steps) ?? [])
  const total = computed(() => list.value.length)
  const current = computed(() => list.value[Math.min(index.value, Math.max(total.value - 1, 0))])
  const centerReference: TourReference = {
    getBoundingClientRect() {
      const x = window.innerWidth / 2
      const y = window.innerHeight / 2
      return new DOMRect(x, y, 0, 0)
    },
  }
  const reference = computed<TourReference | undefined>(() => {
    if (!open.value || typeof window === 'undefined') return undefined
    const target = toValue(current.value?.target)
    if (target == null) return centerReference
    if (typeof target === 'string') return document.querySelector(target) ?? undefined
    return target
  })

  function goTo(next: number) {
    if (!total.value) return
    index.value = Math.min(Math.max(next, 0), total.value - 1)
    open.value = true
  }
  function start(next = 0) { goTo(next) }
  function finish() { open.value = false }

  watch([open, index], async () => {
    if (!open.value) return
    await nextTick()
    const target = reference.value
    if (target instanceof Element) target.scrollIntoView({ behavior: 'smooth', block: 'center' })
  })

  return { open, index, list, total, current, reference, start, goTo, finish }
}
