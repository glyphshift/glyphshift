import { onBeforeUnmount, onMounted } from 'vue'

const blockingOverlaySelector = '[role="dialog"], [role="listbox"], [role="menu"]'

function hasVisibleBlockingOverlay() {
  return [...document.querySelectorAll<HTMLElement>(blockingOverlaySelector)].some((element) => {
    if (element.hidden || element.getAttribute('aria-hidden') === 'true') return false
    if (element.dataset.state && element.dataset.state !== 'open') return false
    const style = window.getComputedStyle(element)
    return element.getClientRects().length > 0
      && style.display !== 'none'
      && style.visibility !== 'hidden'
      && style.opacity !== '0'
  })
}

export function usePageEscape(active: () => boolean, back: () => void) {
  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing || !active()) return
    if (hasVisibleBlockingOverlay()) return
    event.preventDefault()
    back()
  }

  onMounted(() => window.addEventListener('keydown', handleKeydown, true))
  onBeforeUnmount(() => window.removeEventListener('keydown', handleKeydown, true))
}
