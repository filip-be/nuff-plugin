// Dumb swimmer: keeps the agent aware of its context size and nudges it to
// hand off before its work degrades. Pure logic first, then the hooks.
import { atom, read, update } from 'claude-code'
import type { EngineInterface, On, PromptSubmitInput } from 'claude-code'

import type { SwimmerState, Zone } from '../types'
import { DRIFT_TOKENS, DUMB_TOKENS, formatTokens } from './status'

/** In the Dumb zone, every this many main-conversation tool calls of a turn carries a Nudge. */
export const NUDGE_EVERY = 10

export const CLEAR: SwimmerState = { zone: 'clear', calls: 0 }

/** The zone of a context size; an unknown size counts as clear. */
export function zoneOf(tokens: number | null): Zone {
  if (tokens === null) {
    return 'clear'
  }
  if (tokens >= DUMB_TOKENS) {
    return 'dumb'
  }
  if (tokens >= DRIFT_TOKENS) {
    return 'drift'
  }

  return 'clear'
}

const HANDOFF =
  'Write a Handoff so a fresh session can carry on: if you are working from a plan file ' +
  "(such as .agents/PLAN.md), add a Handoff section at its top with what's done, what's left " +
  'and the next step; otherwise give the user a self-contained prompt in a code block to paste ' +
  'into a fresh session. Then stop and offer the user to run /clear and continue in a fresh ' +
  'session (you cannot run /clear yourself).'

export function driftNudge(tokens: number): string {
  return (
    `nuff-cc: your context size is ${formatTokens(tokens)} tokens, past ${formatTokens(DRIFT_TOKENS)}: ` +
    `you are in the Drift zone, heading toward the Dumb zone at ${formatTokens(DUMB_TOKENS)}, ` +
    'where your work degrades. Steer toward a stopping point: finish what you are doing and do ' +
    'not start large new work in this session.'
  )
}

export function dumbNudge(tokens: number): string {
  return (
    `nuff-cc: your context size is ${formatTokens(tokens)} tokens: you are in the Dumb zone ` +
    `(${formatTokens(DUMB_TOKENS)} and over), where your work degrades. Stop at the next safe ` +
    'point: one where the work is consistent, with no change half-made and nothing left broken ' +
    'that worked before. Finish only what it takes to get there, and do not start a new task. ' +
    HANDOFF
  )
}

export function promptReminder(tokens: number): string {
  return (
    `nuff-cc: your context size is ${formatTokens(tokens)} tokens: you are still in the Dumb zone ` +
    `(${formatTokens(DUMB_TOKENS)} and over). Do what the user asks, keeping it small. At the end, ` +
    'write or refresh the Handoff. ' +
    HANDOFF
  )
}

/**
 * One main-conversation tool call at `tokens`: the next state and the Nudge it
 * carries, if any. A Drift Nudge on crossing up from clear; in the Dumb zone the
 * first call and every NUDGE_EVERY-th after it, counted within the turn.
 */
export function swim(prev: SwimmerState, tokens: number | null): { state: SwimmerState; nudge: string | null } {
  const zone = zoneOf(tokens)
  const calls = zone === 'dumb' ? (prev.zone === 'dumb' ? prev.calls : 0) + 1 : 0
  const state = { zone, calls }

  if (tokens !== null && zone === 'drift' && prev.zone === 'clear') {
    return { state, nudge: driftNudge(tokens) }
  }
  if (tokens !== null && zone === 'dumb' && calls % NUDGE_EVERY === 1) {
    return { state, nudge: dumbNudge(tokens) }
  }

  return { state, nudge: null }
}

/** A prompt the user typed (at the prompt or through Remote Control): no command, no meta prompt. */
export function isUsersOwn(e: Pick<PromptSubmitInput, 'command' | 'isMeta' | 'origin'>): boolean {
  return (
    e.command === undefined &&
    e.isMeta !== true &&
    (e.origin.kind === 'composer' || e.origin.kind === 'bridge')
  )
}

/** The reminder a prompt the user typed at `tokens` carries, if any. */
export function reminderFor(tokens: number | null): string | null {
  return tokens !== null && zoneOf(tokens) === 'dumb' ? promptReminder(tokens) : null
}

// Change the tag when SwimmerState's fields change.
const swimmer = atom({ plugin: 'nuff-cc', key: 'swimmer' } as const, CLEAR, { shape: 'swimmer-1' })

async function tokensNow($: EngineInterface): Promise<number | null> {
  const usage = await $.session.usage()

  return usage.context.tokens ?? null
}

// The swimmer's hooks. Subagents have contexts of their own, so only the main
// conversation's calls count. Nudges are read by the model alone and never block.
export function registerSwimmer(on: On): void {
  on('turn.start', async ($, e, next) => {
    if (e.agentId === undefined) {
      await update($, swimmer, prev => ({ ...prev, calls: 0 }))
    }

    return next(e)
  }).catch(($, e, next) => next(e))

  on('tool.call', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined || result.deny !== undefined) {
      return result
    }

    const tokens = await tokensNow($)
    const { state, nudge } = swim(await read($, swimmer), tokens)
    await update($, swimmer, () => state)

    return nudge === null ? result : { ...result, context: [...(result.context ?? []), nudge] }
  }).catch(($, e, next) => next(e))

  on('prompt.submit', async ($, e, next) => {
    const reminder = isUsersOwn(e) ? reminderFor(await tokensNow($)) : null

    return next(reminder === null ? e : { ...e, context: [...(e.context ?? []), reminder] })
  }).catch(($, e, next) => next(e))
}
