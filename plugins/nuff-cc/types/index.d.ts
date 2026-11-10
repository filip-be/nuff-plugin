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
    }
  }
}
