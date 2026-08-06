import { computed, ref } from 'vue'

export function useCaptureScrollbar() {
  const tableShell = ref<HTMLElement>()
  const scrollState = ref({ top: 0, clientHeight: 0, scrollHeight: 0 })
  let drag: { pointerId: number; startY: number; startTop: number } | undefined

  const trackHeight = computed(() => Math.max(0, scrollState.value.clientHeight - 16))
  const scrollThumbHeight = computed(() => {
    const { clientHeight, scrollHeight } = scrollState.value
    if (!clientHeight || scrollHeight <= clientHeight) return 0
    return Math.max(32, trackHeight.value * clientHeight / scrollHeight)
  })
  const scrollThumbTop = computed(() => {
    const { top, clientHeight, scrollHeight } = scrollState.value
    const scrollRange = scrollHeight - clientHeight
    const thumbRange = trackHeight.value - scrollThumbHeight.value
    return scrollRange > 0 && thumbRange > 0 ? top / scrollRange * thumbRange : 0
  })

  function scrollElement() {
    return tableShell.value?.querySelector<HTMLElement>('[data-testid="capture-table-scroll"]')
  }

  function updateScrollMetrics() {
    const element = scrollElement()
    if (!element) return
    scrollState.value = {
      top: element.scrollTop,
      clientHeight: element.clientHeight,
      scrollHeight: element.scrollHeight,
    }
  }

  function jumpScrollbar(event: PointerEvent) {
    const element = scrollElement()
    const track = event.currentTarget as HTMLElement
    if (!element || !scrollThumbHeight.value) return
    const bounds = track.getBoundingClientRect()
    const thumbRange = Math.max(1, bounds.height - scrollThumbHeight.value)
    const target = Math.min(thumbRange, Math.max(0, event.clientY - bounds.top - scrollThumbHeight.value / 2))
    element.scrollTop = target / thumbRange * (element.scrollHeight - element.clientHeight)
    updateScrollMetrics()
  }

  function beginScrollbarDrag(event: PointerEvent) {
    const element = scrollElement()
    if (!element) return
    drag = { pointerId: event.pointerId, startY: event.clientY, startTop: element.scrollTop }
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    event.preventDefault()
  }

  function dragScrollbar(event: PointerEvent) {
    const element = scrollElement()
    if (!element || !drag || drag.pointerId !== event.pointerId) return
    const scrollRange = element.scrollHeight - element.clientHeight
    const thumbRange = trackHeight.value - scrollThumbHeight.value
    if (thumbRange <= 0) return
    element.scrollTop = drag.startTop + (event.clientY - drag.startY) * scrollRange / thumbRange
    updateScrollMetrics()
  }

  function endScrollbarDrag(event: PointerEvent) {
    if (drag?.pointerId === event.pointerId) drag = undefined
  }

  function startScrollTracking() {
    window.addEventListener('resize', updateScrollMetrics)
  }

  function stopScrollTracking() {
    window.removeEventListener('resize', updateScrollMetrics)
  }

  return {
    tableShell,
    scrollThumbHeight,
    scrollThumbTop,
    updateScrollMetrics,
    jumpScrollbar,
    beginScrollbarDrag,
    dragScrollbar,
    endScrollbarDrag,
    startScrollTracking,
    stopScrollTracking,
  }
}
