import type { AnalyticsTier } from '@/shared/types'

type Props = Record<string, string | number>

// Compatibility API: legacy preferences cannot enable collection in LingWindow.
export function setTrackingEnabled(_next: boolean) {}

export function setTrackingTier(_next: AnalyticsTier) {}

export function track(_name: string, _props?: Props) {}

export function syncTrackingFromPrefs() {}
