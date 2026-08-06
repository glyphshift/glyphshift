import { invoke } from '@tauri-apps/api/core'

export type DesktopAcquisitionGranularity = 'word' | 'control' | 'line' | 'region'
export type DesktopAcquisitionProvenance = 'structured' | 'visual'

export interface DesktopPointAcquisitionInput {
  softwareId: string
  targetId: number
  adapterId: string
  x: number
  y: number
  cancellationId: string
}

export interface DesktopAcquisitionRect {
  left: number
  top: number
  right: number
  bottom: number
}

export interface DesktopAcquisitionBlock {
  source: string
  anchors: DesktopAcquisitionRect[]
  granularity: DesktopAcquisitionGranularity
  provenance: DesktopAcquisitionProvenance
  confidenceBasisPoints?: number
}

export interface DesktopPointAcquisitionResult {
  blocks: DesktopAcquisitionBlock[]
}

export function acquireDesktopPoint(
  request: DesktopPointAcquisitionInput,
): Promise<DesktopPointAcquisitionResult> {
  return invoke<DesktopPointAcquisitionResult>('desktop_acquire_point', { request })
}

export function cancelDesktopPointAcquisition(cancellationId: string): Promise<void> {
  return invoke<void>('desktop_cancel_point_acquisition', { cancellationId })
}
