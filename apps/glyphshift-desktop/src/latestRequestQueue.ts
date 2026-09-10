/** Run one read at a time, coalescing requests received during it into one fresh read. */
export function latestRequestQueue(read: () => Promise<void>) {
  let active: Promise<void> | undefined
  let pending = false
  let disposed = false
  return {
    request(): Promise<void> {
      if (disposed) return Promise.resolve()
      pending = true
      if (!active) {
        active = (async () => {
          try {
            while (pending && !disposed) {
              pending = false
              await read()
            }
          } finally { active = undefined }
        })()
      }
      return active
    },
    dispose() { disposed = true; pending = false },
  }
}
