import { describe, expect, test } from 'claude-code/testing'

import {
  CACHE_TTL_MS,
  cacheSegment,
  contextColor,
  contextSegment,
  costSegment,
  formatCost,
  formatTokens,
  hasCacheActivity,
  isFlagSet,
} from '../hooks/status'

const MIN = 60_000

describe('formatTokens', () => {
  test('rounds thousands down', async () => {
    expect(formatTokens(173_999)).toBe('173k')
    expect(formatTokens(1000)).toBe('1k')
    expect(formatTokens(1_234_567)).toBe('1234k')
  })

  test('shows the plain number under 1k', async () => {
    expect(formatTokens(999)).toBe('999')
    expect(formatTokens(0)).toBe('0')
  })
})

describe('contextColor', () => {
  test('default text below the thresholds', async () => {
    expect(contextColor(299_999, 1_000_000)).toBeUndefined()
    expect(contextColor(119_999, 200_000)).toBeUndefined()
  })

  test('warning at 300k or 60% of the window', async () => {
    expect(contextColor(300_000, 1_000_000)).toBe('warning')
    expect(contextColor(120_000, 200_000)).toBe('warning')
  })

  test('error at 500k or 80% of the window', async () => {
    expect(contextColor(500_000, 1_000_000)).toBe('error')
    expect(contextColor(160_000, 200_000)).toBe('error')
  })

  test('a window of 0 falls back to the absolute thresholds', async () => {
    expect(contextColor(100, 0)).toBeUndefined()
    expect(contextColor(300_000, 0)).toBe('warning')
  })
})

test('contextSegment is hidden without figures', async () => {
  expect(contextSegment(null)).toBeNull()
  expect(contextSegment({ tokens: 173_000, window: 1_000_000 })).toEqual({
    text: '173k',
    color: undefined,
  })
})

describe('cacheSegment', () => {
  const anchor = 1_000_000

  test('hidden before the first anchor', async () => {
    expect(cacheSegment({ kind: 'none' }, false, anchor)).toBeNull()
  })

  test('counts whole minutes down, success above 15', async () => {
    const warm = { kind: 'warm', anchor } as const
    expect(cacheSegment(warm, false, anchor)).toEqual({ text: '◷ 60m', color: 'success' })
    expect(cacheSegment(warm, false, anchor + 7 * MIN - 1)).toEqual({ text: '◷ 53m', color: 'success' })
    expect(cacheSegment(warm, false, anchor + 44 * MIN - 1)).toEqual({ text: '◷ 16m', color: 'success' })
  })

  test('warning from 15m down to <1m', async () => {
    const warm = { kind: 'warm', anchor } as const
    expect(cacheSegment(warm, false, anchor + 44 * MIN + 1)).toEqual({ text: '◷ 15m', color: 'warning' })
    expect(cacheSegment(warm, false, anchor + 59 * MIN)).toEqual({ text: '◷ 1m', color: 'warning' })
    expect(cacheSegment(warm, false, anchor + 59 * MIN + 1)).toEqual({ text: '◷ <1m', color: 'warning' })
  })

  test('cold at 0 and after', async () => {
    const warm = { kind: 'warm', anchor } as const
    expect(cacheSegment(warm, false, anchor + CACHE_TTL_MS)).toEqual({ text: 'cold', color: 'error' })
    expect(cacheSegment({ kind: 'cold' }, false, anchor)).toEqual({ text: 'cold', color: 'error' })
  })

  test('an anchor in the future shows at most 60m', async () => {
    expect(cacheSegment({ kind: 'warm', anchor }, false, anchor - 10 * MIN)?.text).toBe('◷ 60m')
  })

  test('cache off wins and is dim', async () => {
    const off = { text: 'cache off', isDim: true }
    expect(cacheSegment({ kind: 'none' }, true, anchor)).toEqual(off)
    expect(cacheSegment({ kind: 'warm', anchor }, true, anchor)).toEqual(off)
  })
})

test('hasCacheActivity needs a cache read or write', async () => {
  const usage = { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 }
  expect(hasCacheActivity(null)).toBe(false)
  expect(hasCacheActivity(usage)).toBe(false)
  expect(hasCacheActivity({ ...usage, cache_read_input_tokens: 1 })).toBe(true)
  expect(hasCacheActivity({ ...usage, cache_creation_input_tokens: 1 })).toBe(true)
})

test('isFlagSet reads DISABLE_PROMPT_CACHING', async () => {
  expect(isFlagSet(undefined)).toBe(false)
  expect(isFlagSet('')).toBe(false)
  expect(isFlagSet('0')).toBe(false)
  expect(isFlagSet('false')).toBe(false)
  expect(isFlagSet('1')).toBe(true)
  expect(isFlagSet('true')).toBe(true)
})

describe('costSegment', () => {
  test('formats dollars to the cent', async () => {
    expect(formatCost(1.234)).toBe('$1.23')
    expect(formatCost(0.006)).toBe('$0.01')
    expect(formatCost(12)).toBe('$12.00')
  })

  test('hides until the session has cost anything', async () => {
    expect(costSegment(null)).toBeNull()
    expect(costSegment(0)).toBeNull()
    expect(costSegment(0.42)).toEqual({ text: '$0.42' })
  })
})
