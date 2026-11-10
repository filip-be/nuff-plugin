import { atom, read, update } from 'claude-code'
import type { EngineInterface, On } from 'claude-code'

import type { CacheState, ContextFigures } from '../types'
import { cacheSegment, contextSegment, hasCacheActivity, isFlagSet } from './status'
import type { Segment } from './status'

// Change a tag when its value's fields change: it declines what an earlier
// version stored, which outlives a reload.
const context = atom({ plugin: 'nuff-cc', key: 'context' } as const, null, {
  shape: 'context-1',
})
const cache = atom({ plugin: 'nuff-cc', key: 'cache' } as const, { kind: 'none' }, {
  shape: 'cache-1',
})
const isCacheOff = atom({ plugin: 'nuff-cc', key: 'isCacheOff' } as const, false)
const tick = atom({ plugin: 'nuff-cc', key: 'tick' } as const, 0)

const TICK_MS = 60_000
const COLD: CacheState = { kind: 'cold' }

// Called from the plugin's one `session.start` hook: seeds the context figures
// (a resumed or forked session shows them at once), reads the cache switch and
// starts the minute tick. A reload cancels the old timer and runs this again.
export async function startStatusLine($: EngineInterface): Promise<void> {
  const figures = toFigures((await $.session.usage()).context)
  if (figures !== null) {
    await update($, context, () => figures)
  }

  const isOff = isFlagSet(await $.env.get('DISABLE_PROMPT_CACHING'))
  await update($, isCacheOff, () => isOff)

  $.clock.every(TICK_MS, () => {
    void $.clock.now().then(now => update($, tick, () => now))
  })
}

// The status line's hooks: they keep the figures the band draws in $.state.
export function registerStatusLine(on: On): void {
  on('session.measure', async ($, e, next) => {
    const figures = toFigures(e.context)
    if (figures !== null) {
      await update($, context, () => figures)
    }

    return next(e)
  })

  // A main-thread step that read or wrote the prompt cache restarts the countdown.
  // Subagents and forks cache a different prefix, so they are ignored.
  on('turn.step', async function* ($, e, next) {
    const result = yield* next(e)

    if (e.agentId === undefined && hasCacheActivity(result.usage)) {
      const anchor = await $.clock.now()
      await update($, cache, (): CacheState => ({ kind: 'warm', anchor }))
    }

    return result
  })

  // Resume or fork: seed the countdown from the resumed transcript's last response.
  on('classic.SessionStart', async ($, e, next) => {
    if (e.prompt_cache_likely_expired === true) {
      await update($, cache, () => COLD)
    } else if (typeof e.seconds_since_last_response === 'number') {
      const anchor = (await $.clock.now()) - e.seconds_since_last_response * 1000
      await update($, cache, (): CacheState => ({ kind: 'warm', anchor }))
    }

    return next(e)
  }).catch(($, e, next) => next(e))

  on('session.end', async ($, e, next) => {
    if (e.reason === 'clear') {
      await update($, context, () => null)
      await update($, cache, (): CacheState => ({ kind: 'none' }))
    }

    return next(e)
  })

  // A compaction rewrites the prefix, so the old cache is useless. A precompute
  // installs nothing, and a subagent's compaction is not the main conversation's.
  on('session.compact', async ($, e, next) => {
    const result = await next(e)

    if (e.agentId === undefined && e.trigger !== 'precompute' && !('skip' in result && result.skip !== undefined)) {
      await update($, cache, () => COLD)
    }

    return result
  }).catch(($, e, next) => next(e))

  // Another model has no cache of this conversation.
  on('classic.PostModelSwitch', async ($, e, next) => {
    if (e.source !== 'resume' && e.from_model !== e.to_model) {
      await update($, cache, () => COLD)
    }

    return next(e)
  }).catch(($, e, next) => next(e))
}

/** The band's left side, `173k · ◷ 53m`, or null with nothing to show. Call while drawing. */
export async function statusSegments($: EngineInterface): Promise<Segment[]> {
  await read($, tick)

  const now = await $.clock.now()
  const segments = [
    contextSegment(await read($, context)),
    cacheSegment(await read($, cache), await read($, isCacheOff), now),
  ]

  return segments.filter((segment): segment is Segment => segment !== null)
}

function toFigures(usage: { tokens?: number; window: number }): ContextFigures | null {
  return usage.tokens === undefined ? null : { tokens: usage.tokens, window: usage.window }
}
