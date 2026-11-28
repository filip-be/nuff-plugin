import { describe, expect, test } from 'claude-code/testing'
import type { On, PromptOrigin } from 'claude-code'
import type { Engine } from 'claude-code/testing'

import { CLEAR, isUsersOwn, reminderFor, swim, zoneOf } from '../hooks/swimmer'
import type { SwimmerState } from '../types'

describe('zoneOf', () => {
  test('clear below 300k and when unknown, drift from 300k, dumb from 500k', async () => {
    expect(zoneOf(null)).toBe('clear')
    expect(zoneOf(299_999)).toBe('clear')
    expect(zoneOf(300_000)).toBe('drift')
    expect(zoneOf(499_999)).toBe('drift')
    expect(zoneOf(500_000)).toBe('dumb')
  })
})

// Runs calls at the given sizes from `from`; answers which of them carried a Nudge.
function run(sizes: (number | null)[], from: SwimmerState = CLEAR): { state: SwimmerState; nudged: number[] } {
  let state = from
  const nudged: number[] = []
  sizes.forEach((tokens, index) => {
    const out = swim(state, tokens)
    state = out.state
    if (out.nudge !== null) {
      nudged.push(index)
    }
  })

  return { state, nudged }
}

describe('swim', () => {
  test('nothing below 300k or while the size is unknown', async () => {
    expect(run([0, 100_000, null, 299_999]).nudged).toEqual([])
  })

  test('one Drift Nudge per crossing, again after a drop', async () => {
    const first = swim(CLEAR, 310_000)
    expect(first.nudge).toContain('Drift zone')
    expect(first.nudge).toContain('310k')

    expect(run([310_000, 320_000, 400_000]).nudged).toEqual([0])
    expect(run([310_000, 320_000, 50_000, 330_000]).nudged).toEqual([0, 3])
    expect(run([310_000, null, 330_000]).nudged).toEqual([0, 2])
  })

  test('falling from the Dumb zone into the Drift zone sends no Drift Nudge', async () => {
    expect(run([510_000, 450_000]).nudged).toEqual([0])
  })

  test('the Dumb zone nudges on its first call and every 10th after it', async () => {
    const first = swim(CLEAR, 520_000)
    expect(first.nudge).toContain('Dumb zone')
    expect(first.nudge).toContain('no change half-made')
    expect(first.nudge).toContain('Handoff')
    expect(first.nudge).toContain('.agents/PLAN.md')
    expect(first.nudge).toContain('/clear')

    const sizes = Array.from({ length: 25 }, () => 520_000)
    expect(run(sizes).nudged).toEqual([0, 10, 20])
  })

  test('the Nudges stop below 500k and start over on re-entry', async () => {
    expect(run([510_000, 510_000, 100_000, 510_000]).nudged).toEqual([0, 3])
  })

  test('a turn reset of the counter nudges the next call again', async () => {
    const before = run([510_000, 510_000, 510_000]).state
    expect(before).toEqual({ zone: 'dumb', calls: 3 })
    expect(swim({ ...before, calls: 0 }, 510_000).nudge).not.toBeNull()
  })
})

describe('prompt reminders', () => {
  const composer: PromptOrigin = { kind: 'composer' }

  test('only prompts the user typed count', async () => {
    expect(isUsersOwn({ origin: composer })).toBe(true)
    expect(isUsersOwn({ origin: { kind: 'bridge' } })).toBe(true)
    expect(isUsersOwn({ origin: composer, isMeta: true })).toBe(false)
    expect(isUsersOwn({ origin: composer, command: { name: 'compact' } as never })).toBe(false)
    expect(isUsersOwn({ origin: { kind: 'sdk' } })).toBe(false)
    expect(isUsersOwn({ origin: { kind: 'peer' } as never })).toBe(false)
  })

  test('a reminder in the Dumb zone only', async () => {
    expect(reminderFor(null)).toBeNull()
    expect(reminderFor(450_000)).toBeNull()
    expect(reminderFor(500_000)).toContain('still in the Dumb zone')
  })
})

// Through the engine: the hooks attach Nudges to main-conversation tool results
// and to prompts the user typed.
function engine(on: On, size: { tokens?: number }) {
  on('session.usage', () => ({
    value: { startedAt: 0, context: { tokens: size.tokens, window: 1_000_000 }, rateLimits: [] },
  }))
  on('tool.call', () => ({ result: { stdout: 'ok', stderr: '', interrupted: false } }))
  on('prompt.submit', (_, e) => ({ text: e.text, context: e.context }))
  on('turn.start', (_, e) => ({ turnId: e.turnId }))
}

async function bash($: Engine, agentId?: string) {
  const result = await $.tool.call({
    tool: 'Bash',
    input: { command: 'ls' },
    command: 'ls',
    ...(agentId === undefined ? {} : { agentId }),
  } as never)

  return result.deny === undefined ? (result.context ?? []) : []
}

describe('hooks', () => {
  test('main-conversation tool calls carry the Nudges; subagent calls neither carry nor count', async ($, on) => {
    const size: { tokens?: number } = { tokens: 100_000 }
    engine(on, size)

    expect(await bash($)).toEqual([])

    size.tokens = 350_000
    expect(await bash($, 'agent-1')).toEqual([])
    const drift = await bash($)
    expect(drift).toHaveLength(1)
    expect(drift[0]).toContain('Drift zone')
    expect(await bash($)).toEqual([])

    size.tokens = 600_000
    expect((await bash($))[0]).toContain('Dumb zone')
    for (let i = 0; i < 9; i++) {
      expect(await bash($)).toEqual([])
    }
    expect((await bash($))[0]).toContain('Dumb zone')

    await $.turn.start({ text: 'next', turnId: 't2' })
    expect((await bash($))[0]).toContain('Dumb zone')
  })

  test('a typed prompt in the Dumb zone carries a reminder; peers and prompts below 500k do not', async ($, on) => {
    const size: { tokens?: number } = { tokens: 600_000 }
    engine(on, size)

    const typed = await $.prompt.submit({ text: 'go on', wait: false, origin: { kind: 'composer' } } as never)
    expect(typed.context?.[0]).toContain('still in the Dumb zone')

    const peer = await $.prompt.submit({ text: 'hi', wait: false, origin: { kind: 'peer' } } as never)
    expect(peer.context ?? []).toEqual([])

    size.tokens = 200_000
    const below = await $.prompt.submit({ text: 'go on', wait: false, origin: { kind: 'composer' } } as never)
    expect(below.context ?? []).toEqual([])
  })
})
