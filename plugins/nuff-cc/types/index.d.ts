// The $.state contract of nuff-cc: every value its hooks keep for the session.

/** The context window's fill the band shows: input tokens and the window. */
export type ContextFigures = { tokens: number; window: number }

/**
 * The main conversation's prompt cache, as the band counts it down:
 * - `none`: nothing to show yet (a fresh session before its first cached step)
 * - `warm`: cached at `anchor` (`$.clock.now()` milliseconds)
 * - `cold`: known to be expired or invalidated
 */
export type CacheState =
  | { kind: 'none' }
  | { kind: 'warm'; anchor: number }
  | { kind: 'cold' }

/** A Dumb swimmer zone of the context size: below 300k, from 300k, from 500k. */
export type Zone = 'clear' | 'drift' | 'dumb'

/**
 * The Dumb swimmer's state: the zone of the last main-conversation tool call,
 * and how many of this turn's calls were in the Dumb zone.
 */
export type SwimmerState = { zone: Zone; calls: number }

declare module 'claude-code' {
  interface PluginState {
    'nuff-cc': {
      context: Shaped<ContextFigures | null>
      cache: Shaped<CacheState>
      /** The session's cost in US dollars, as /cost totals it; null before a figure. */
      cost: number | null
      /** `DISABLE_PROMPT_CACHING` is set: no countdown. */
      isCacheOff: boolean
      /** The last minute tick, in `$.clock.now()` milliseconds: redraws the band. */
      tick: number
      swimmer: Shaped<SwimmerState>
    }
  }
}
