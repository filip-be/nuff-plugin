import type { EngineInterface, Register } from 'claude-code'

import { openInVsCode } from './vscode'

// Entry point: every mod of nuff-cc registers its hooks here.
export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'vscode',
      description: "Open this session's folder in VS Code",
    })

    return next(e)
  })

  on('command.run', { command: 'vscode' }, async $ => ({
    text: await openAndReport($),
  }))

  // The button sits above the prompt on the desktop app only: in the terminal
  // /vscode is enough, and inside VS Code the button would open VS Code.
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    if (e.surface !== 'desktop' || e.props.hasSurvey) {
      return next(e)
    }

    const { Box, Button } = $.ui.resolve(e)

    return (
      <Box>
        <Button
          key="open-in-vscode"
          label="Open in VS Code"
          onPress={async () => {
            $.ui.toast(await openAndReport($))
          }}
        />
      </Box>
    )
  })
}

async function openAndReport($: EngineInterface): Promise<string> {
  const folder = await $.session.cwd()
  const opened = await openInVsCode($, folder)

  return opened.isOpened
    ? `Opened ${folder} in VS Code.`
    : `Could not open VS Code: ${opened.reason}`
}
