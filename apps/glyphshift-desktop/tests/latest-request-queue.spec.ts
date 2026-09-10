import { test, expect } from '@playwright/test'
import { latestRequestQueue } from '../src/latestRequestQueue'

test('slow reads collapse bursts into one latest follow-up and dispose cancels pending work', async () => {
  let release: (() => void) | undefined
  let reads = 0
  let input = 0
  const observed: number[] = []
  const queue = latestRequestQueue(async () => {
    reads++
    observed.push(input)
    if (reads === 1) await new Promise<void>(resolve => { release = resolve })
  })
  const first = queue.request()
  for (input = 1; input <= 100; input++) void queue.request()
  expect(reads).toBe(1)
  release!()
  await first
  expect(observed).toEqual([0, 101])
  queue.dispose()
  await queue.request()
  expect(reads).toBe(2)
})

test('disposing during a read suppresses its queued follow-up', async () => {
  let release: (() => void) | undefined
  let reads = 0
  const queue = latestRequestQueue(async () => {
    reads++
    await new Promise<void>(resolve => { release = resolve })
  })
  const first = queue.request()
  void queue.request()
  queue.dispose()
  release!()
  await first
  expect(reads).toBe(1)
})
