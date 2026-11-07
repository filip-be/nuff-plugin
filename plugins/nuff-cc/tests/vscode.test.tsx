import { expect, test } from 'claude-code/testing'
import type { On } from 'claude-code'

const BAND = {
  plugin: 'nuff-cc',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 10,
    bodyColumns: 80,
    scroll: { offset: 0, bodyRows: 10 },
    view: {},
  },
} as const

const FOLDER = 'C:\\DEV\\project'

const RUN = {
  command: 'vscode',
  args: '',
  origin: { kind: 'composer' },
  presentation: { isFullscreen: false, columns: 80 },
} as const

// The engine beneath the mod: a Windows host whose `code` exits `exitCode`, writing `stderr`.
function host(on: On, exitCode: number, ran: string[][], toasts: string[], stderr = '') {
  on('env.get', (_, e) => ({
    value: e.name === 'ComSpec' ? 'C:\\Windows\\system32\\cmd.exe' : undefined,
  }))
  on('session.cwd', () => ({ value: FOLDER }))
  on('process.run', (_, e) => {
    ran.push([...e.argv])
    return {
      value: {
        exitCode,
        stdout: '',
        stderr,
        isStdoutTruncated: false,
        isStderrTruncated: false,
      },
    }
  })
  on('ui.toast', (_, e) => {
    toasts.push(e.text)
    return { value: undefined }
  })
}

test('/vscode opens the session folder through cmd.exe on Windows', async ($, on) => {
  const ran: string[][] = []
  host(on, 0, ran, [])

  const result = await $.command.run(RUN)

  expect(ran).toEqual([
    ['C:\\Windows\\system32\\cmd.exe', '/d', '/c', 'code', '-r', FOLDER],
  ])
  expect(result.text).toBe(`Opened ${FOLDER} in VS Code.`)
})

test('/vscode says how to fix a missing code command', async ($, on) => {
  // What cmd.exe really answers for a command it cannot find.
  host(
    on,
    1,
    [],
    [],
    "'code' is not recognized as an internal or external command,\r\noperable program or batch file.\r\n",
  )

  const result = await $.command.run(RUN)

  expect(result.text).toMatch(/not found on PATH/)
})

test('the desktop band button opens VS Code and toasts', async ($, on) => {
  const ran: string[][] = []
  const toasts: string[] = []
  host(on, 0, ran, toasts)

  const ui = await $.ui.mount({ ...BAND, surface: 'desktop' })
  await ui.press({ key: 'open-in-vscode' })
  await ui.unmount()

  expect(ran).toHaveLength(1)
  expect(toasts).toEqual([`Opened ${FOLDER} in VS Code.`])
})

test('the band button stays off the terminal and VS Code', async ($, on) => {
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>engine</Text>
  })

  for (const surface of ['terminal', 'vscode'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect(await ui.find({ type: 'Button', key: 'open-in-vscode' })).toBeUndefined()
    await ui.unmount()
  }
})

test('/vscode passes on any other failure of code', async ($, on) => {
  host(on, 2, [], [], 'boom\n')

  const result = await $.command.run(RUN)

  expect(result.text).toBe('Could not open VS Code: `code` exited with 2: boom')
})
