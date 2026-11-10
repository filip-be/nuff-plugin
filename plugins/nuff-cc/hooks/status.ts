// Pure logic of the desktop status line: no `$` calls, so it is unit-tested alone.
import type { CacheState } from '../types'

/** The prompt-cache TTL the countdown assumes: hardcoded, not detected. */
export const CACHE_TTL_MS = 60 * 60_000

const MINUTE_MS = 60_000
const WARM_MINUTES = 15

export type StatusColor = 'success' | 'warning' | 'error'

/** One segment of the band: its text and how it is coloured. */
export type Segment = { text: string; color?: StatusColor; isDim?: boolean }

/** `173k` for 173,456 tokens (thousands, rounded down); under 1k the plain number. */
export function formatTokens(tokens: number): string {
  const whole = Math.max(0, Math.floor(tokens))

  return whole < 1000 ? String(whole) : `${Math.floor(whole / 1000)}k`
}

/** The context segment's colour: `error` first, then `warning`, else default text. */
export function contextColor(tokens: number, window: number): StatusColor | undefined {
  const share = window > 0 ? tokens / window : 0

  if (tokens >= 500_000 || share >= 0.8) {
    return 'error'
  }
  if (tokens >= 300_000 || share >= 0.6) {
    return 'warning'
  }

  return undefined
}

export function contextSegment(
  figures: { tokens: number; window: number } | null,
): Segment | null {
  if (figures === null) {
    return null
  }

  return {
    text: formatTokens(figures.tokens),
    color: contextColor(figures.tokens, figures.window),
  }
}

/** The cache segment at `now`, or null while there is nothing to show. */
export function cacheSegment(
  cache: CacheState,
  isCacheOff: boolean,
  now: number,
): Segment | null {
  if (isCacheOff) {
    return { text: 'cache off', isDim: true }
  }
  if (cache.kind === 'none') {
    return null
  }

  const left = cache.kind === 'warm' ? cache.anchor + CACHE_TTL_MS - now : 0

  if (left <= 0) {
    return { text: 'cold', color: 'error' }
  }

  const minutes = Math.min(Math.floor(left / MINUTE_MS), CACHE_TTL_MS / MINUTE_MS)

  if (minutes < 1) {
    return { text: '◷ <1m', color: 'warning' }
  }

  return {
    text: `◷ ${minutes}m`,
    color: minutes > WARM_MINUTES ? 'success' : 'warning',
  }
}

/** `$1.23` for a session cost in US dollars, rounded to the cent. */
export function formatCost(usd: number): string {
  return `$${Math.max(0, usd).toFixed(2)}`
}

/** The cost segment, or null until the session has cost anything. */
export function costSegment(usd: number | null): Segment | null {
  if (usd === null || !(usd > 0)) {
    return null
  }

  return { text: formatCost(usd) }
}

/** Whether a step's usage shows prompt-cache activity (a read or a write). */
export function hasCacheActivity(
  usage: { cache_read_input_tokens?: number | null; cache_creation_input_tokens?: number | null } | null,
): boolean {
  if (usage === null) {
    return false
  }

  return (usage.cache_read_input_tokens ?? 0) + (usage.cache_creation_input_tokens ?? 0) > 0
}

/** `DISABLE_PROMPT_CACHING` read as a flag: set and not `0`/`false`. */
export function isFlagSet(value: string | undefined): boolean {
  if (value === undefined) {
    return false
  }

  const flag = value.trim().toLowerCase()

  return flag !== '' && flag !== '0' && flag !== 'false'
}
