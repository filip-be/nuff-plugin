import { expect, mock, test } from 'claude-code/testing'
import type { On, TurnUsage } from 'claude-code'
import type { Engine } from 'claude-code/testing'

const BAND = {
  plugin: 'nuff-cc',
  component: 'AbovePrompt',
  surface: 'desktop',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 10,
    bodyColumns: 80,
    scroll: { offset: 0, bodyRows: 10 },
    view: {},
  },
} as const

const MIN = 60_000
const T0 = 1_700_000_000_000

const CACHED: TurnUsage = {
  model: 'claude-opus-5-5',
  input_tokens: 10,
  output_tokens: 20,
  cache_read_input_tokens: 170_000,
  cache_creation_input_tokens: 3_000,
}
const UNCACHED: TurnUsage = { ...CACHED, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 }

type World = { tokens?: number; window?: number; usd?: number; env?: Record<string, string> }

// The engine beneath the plugin: a session whose usage reads `tokens`, a clock
// at T0, and each event the plugin hooks answered at the bottom.
function engine(on: On, world: World = {}) {
  const clock = mock.clock(on, { now: T0 })
  mock.env(on, world.env ?? {})
  let usage: TurnUsage | null = CACHED

  on('command.register', () => ({ value: { command: 'vscode' } }))
  on('session.usage', () => ({
    value: {
      startedAt: T0,
      context: { tokens: world.tokens, window: world.window ?? 1_000_000 },
      rateLimits: [],
      ...(world.usd === undefined ? {} : { cost: { usd: world.usd } }),
    },
  }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.measure', (_, e) => ({ changed: e.changed }))
  on('session.end', (_, e) => ({ sessionId: e.sessionId }))
  on('session.compact', (_, e) => ({ messages: e.messages }))
  on('classic.SessionStart', () => ({}))
  on('classic.PostModelSwitch', () => ({}))
  on('turn.step', async function* (_, e) {
    return {
      turnId: e.turnId,
      index: e.index,
      answer: '',
      toolUses: [],
      stopReason: 'end_turn',
      usage,
    }
  })

  return {
    clock,
    setUsage: (next: TurnUsage | null) => {
      usage = next
    },
  }
}

async function start($: Engine) {
  await $.session.start({ cwd: '/', surface: 'desktop', isInteractive: true })
}

async function step($: Engine, agentId?: string) {
  const stream = $.turn.step({
    turnId: 't1',
    index: 0,
    model: 'claude-opus-5-5',
    messageCount: 1,
    ...(agentId === undefined ? {} : { agentId }),
  })
  for await (const _ of stream) {
    // drain
  }
  await stream.result
}

async function measure($: Engine, tokens: number, window = 1_000_000) {
  await $.session.measure({ context: { tokens, window }, rateLimits: [], changed: ['context'] })
}

// The band's status texts as drawn: each segment and separator, in order.
async function status($: Engine): Promise<{ text: string; color: unknown; dim: unknown }[]> {
  const ui = await $.ui.mount(BAND)
  const texts = await ui.findAll({ type: 'Text' })
  await ui.unmount()

  return texts
    .filter(t => t.children.every(c => typeof c === 'string'))
    .map(t => ({ text: t.text, color: t.props.color, dim: t.props.dimColor }))
}

async function line($: Engine): Promise<string> {
  return (await status($)).map(s => s.text).join('')
}

test('a fresh session shows nothing until figures arrive', async ($, on) => {
  engine(on)
  await start($)

  expect(await line($)).toBe('')

  const ui = await $.ui.mount(BAND)
  expect(await ui.find({ type: 'Button', key: 'open-in-vscode' })).toBeDefined()
  await ui.unmount()
})

test('the context seeds from usage at start and follows session.measure', async ($, on) => {
  engine(on, { tokens: 173_456 })
  await start($)

  expect(await line($)).toBe('173k')

  await measure($, 650_000)
  const [segment] = await status($)
  expect(segment).toEqual({ text: '650k', color: 'error', dim: undefined })
})

test('the context turns warning at 60% of a small window', async ($, on) => {
  engine(on)
  await start($)
  await measure($, 130_000, 200_000)

  expect((await status($))[0]?.color).toBe('warning')
})

test('a cached main-thread step anchors the countdown, which ticks each minute', async ($, on) => {
  const { clock } = engine(on, { tokens: 173_000 })
  await start($)
  await step($)

  expect(await status($)).toEqual([
    { text: '173k', color: undefined, dim: undefined },
    { text: ' · ', color: undefined, dim: true },
    { text: '◷ 60m', color: 'success', dim: undefined },
  ])

  await clock.advance(7 * MIN)
  expect(await line($)).toBe('173k · ◷ 53m')

  await clock.advance(38 * MIN)
  expect((await status($))[2]).toEqual({ text: '◷ 15m', color: 'warning', dim: undefined })

  await clock.advance(14 * MIN + 30_000)
  expect(await line($)).toBe('173k · ◷ <1m')

  await clock.advance(MIN)
  expect((await status($))[2]).toEqual({ text: 'cold', color: 'error', dim: undefined })
})

test('the band redraws on its own each minute while mounted', async ($, on) => {
  const { clock } = engine(on)
  await start($)
  await step($)

  const ui = await $.ui.mount(BAND)
  expect(await ui.find({ type: 'Text', text: '◷ 60m' })).toBeDefined()
  await clock.advance(MIN)
  expect(await ui.find({ type: 'Text', text: '◷ 59m' })).toBeDefined()
  await ui.unmount()
})

test('subagent steps and steps without cache usage do not anchor', async ($, on) => {
  const { setUsage } = engine(on)
  await start($)

  await step($, 'agent-1')
  expect(await line($)).toBe('')

  setUsage(null)
  await step($)
  expect(await line($)).toBe('')

  setUsage(UNCACHED)
  await step($)
  expect(await line($)).toBe('')
})

test('a resume seeds the countdown from the last response', async ($, on) => {
  engine(on)
  await start($)
  await $.classic.SessionStart({ source: 'resume', seconds_since_last_response: 600 })

  expect(await line($)).toBe('◷ 50m')
})

test('a resume past the TTL is cold', async ($, on) => {
  engine(on)
  await start($)
  await $.classic.SessionStart({
    source: 'resume',
    seconds_since_last_response: 120,
    prompt_cache_likely_expired: true,
  })

  expect(await line($)).toBe('cold')
})

test('startup and compact SessionStart leave the countdown alone', async ($, on) => {
  engine(on)
  await start($)
  await step($)
  await $.classic.SessionStart({ source: 'compact' })

  expect(await line($)).toBe('◷ 60m')
})

test('/clear hides both segments until new figures arrive', async ($, on) => {
  engine(on, { tokens: 50_000 })
  await start($)
  await step($)
  expect(await line($)).toBe('50k · ◷ 60m')

  await $.session.end({ reason: 'clear', sessionId: 's1', resume: { id: 's1' } })
  expect(await line($)).toBe('')

  await measure($, 20_000)
  await step($)
  expect(await line($)).toBe('20k · ◷ 60m')
})

test('/compact hides the context and makes the cache cold until the next response', async ($, on) => {
  const { clock } = engine(on, { tokens: 450_000 })
  await start($)
  await step($)
  await $.session.compact({
    trigger: 'manual',
    messages: [{ role: 'user', text: 'summary', toolUses: [] }],
  })

  expect(await line($)).toBe('cold')

  // The first measure after a compaction has no tokens yet.
  await $.session.measure({ context: { window: 1_000_000 }, rateLimits: [], changed: ['context'] })
  expect(await line($)).toBe('cold')

  await clock.advance(MIN)
  await measure($, 30_000)
  await step($)
  expect(await line($)).toBe('30k · ◷ 60m')
})

test('a model switch makes the cache cold; a resume restore does not', async ($, on) => {
  engine(on)
  await start($)
  await step($)

  const sw = {
    requested_model: null,
    from_model: 'claude-opus-5-5',
    to_model: 'claude-opus-5-5',
    context_tokens: 0,
    prompt_cache_warm: true,
    cache_ttl: '1h',
    estimated_cache_write_usd: 0,
    pricing: 'default',
  } as const
  await $.classic.PostModelSwitch({ ...sw, source: 'command' })
  expect(await line($)).toBe('◷ 60m')

  await $.classic.PostModelSwitch({ ...sw, to_model: 'claude-sonnet-5-5', source: 'resume' })
  expect(await line($)).toBe('◷ 60m')

  await $.classic.PostModelSwitch({ ...sw, to_model: 'claude-sonnet-5-5', source: 'picker' })
  expect(await line($)).toBe('cold')
})

test('the cost seeds from usage, follows session.measure and sits after the cache', async ($, on) => {
  engine(on, { tokens: 173_000, usd: 1.5 })
  await start($)
  await step($)

  expect(await status($)).toEqual([
    { text: '173k', color: undefined, dim: undefined },
    { text: ' · ', color: undefined, dim: true },
    { text: '◷ 60m', color: 'success', dim: undefined },
    { text: ' · ', color: undefined, dim: true },
    { text: '$1.50', color: undefined, dim: undefined },
  ])

  await $.session.measure({
    context: { tokens: 180_000, window: 1_000_000 },
    rateLimits: [],
    cost: { usd: 2.346 },
    changed: ['context', 'cost'],
  })
  expect(await line($)).toBe('180k · ◷ 60m · $2.35')
})

test('the cost survives a compaction and hides after /clear', async ($, on) => {
  engine(on, { tokens: 50_000, usd: 0.75 })
  await start($)
  await step($)
  await $.session.compact({
    trigger: 'manual',
    messages: [{ role: 'user', text: 'summary', toolUses: [] }],
  })
  expect(await line($)).toBe('cold · $0.75')

  await $.session.end({ reason: 'clear', sessionId: 's1', resume: { id: 's1' } })
  expect(await line($)).toBe('')
})

test('DISABLE_PROMPT_CACHING shows a dim cache off', async ($, on) => {
  engine(on, { tokens: 1_000, env: { DISABLE_PROMPT_CACHING: '1' } })
  await start($)
  await step($)

  expect((await status($)).at(-1)).toEqual({ text: 'cache off', color: undefined, dim: true })
  expect(await line($)).toBe('1k · cache off')
})

test('the status line stays off the terminal, VS Code and a survey', async ($, on) => {
  engine(on, { tokens: 1_000 })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>engine</Text>
  })
  await start($)

  for (const surface of ['terminal', 'vscode'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect(await ui.find({ type: 'Text', text: '1k' })).toBeUndefined()
    await ui.unmount()
  }

  const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, hasSurvey: true } })
  expect(await ui.find({ type: 'Text', text: '1k' })).toBeUndefined()
  await ui.unmount()
})

test('a value an earlier version stored under another shape is declined', async ($, on) => {
  engine(on)
  on('state.get', (_, e) => ({
    value: e.key === 'context' ? { value: { value: 42 }, version: 1 } : { value: undefined, version: 0 },
  }))

  expect(await line($)).toBe('')
})
