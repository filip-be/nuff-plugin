import type { EngineInterface, Register } from 'claude-code'

import { registerStatusLine, startStatusLine, statusSegments } from './statusline'
import { openInVsCode } from './vscode'

// Entry point: every mod of nuff-cc registers its hooks here.
export const register: Register = on => {
  registerStatusLine(on)

  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'vscode',
      description: "Open this session's folder in VS Code",
    })
    await startStatusLine($)

    return next(e)
  })

  on('command.run', { command: 'vscode' }, async $ => ({
    text: await openAndReport($),
  }))

  // The band sits above the prompt on the desktop app only: the status line on
  // the left, the button on the right. The terminal has its own statusLine and
  // /vscode; inside VS Code the extension shows the cache and the button would
  // open VS Code.
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    if (e.surface !== 'desktop' || e.props.hasSurvey) {
      return next(e)
    }

    const { Box, Button, Text } = $.ui.resolve(e)
    // A status line that cannot be read must not take the button down with it.
    const segments = await statusSegments($).catch(() => [])
    const parts = segments.flatMap((segment, index) => [
      ...(index > 0 ? [<Text dimColor>{' · '}</Text>] : []),
      <Text color={segment.color} dimColor={segment.isDim}>
        {segment.text}
      </Text>,
    ])

    return (
      <Box flexDirection="row" justifyContent="space-between" alignItems="center" width="100%">
        <Box flexGrow={1} flexShrink={1}>
          <Text>{parts}</Text>
        </Box>
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
