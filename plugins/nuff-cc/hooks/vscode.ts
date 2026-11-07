import type { EngineInterface } from 'claude-code'

export type OpenResult = { isOpened: true } | { isOpened: false; reason: string }

const NOT_FOUND =
  "the `code` command was not found on PATH. In VS Code run 'Shell Command: Install 'code' command in PATH' (macOS/Linux) or reinstall VS Code with 'Add to PATH' (Windows), then restart Claude."

// `code -r <folder>`: focuses a window that already has the folder, otherwise
// reuses the most recent window, or starts VS Code when none is open.
export async function openInVsCode(
  $: EngineInterface,
  folder: string,
): Promise<OpenResult> {
  // On Windows `code` is `code.cmd`, which cannot start without a shell.
  const comSpec = await $.env.get('ComSpec')
  const argv = comSpec
    ? [comSpec, '/d', '/c', 'code', '-r', folder]
    : ['code', '-r', folder]

  let ran
  try {
    ran = await $.process.run(argv)
  } catch {
    return { isOpened: false, reason: NOT_FOUND }
  }

  if (ran.exitCode === 0) {
    return { isOpened: true }
  }

  // cmd.exe /c exits 1 for an unknown command and says so on stderr.
  const detail = (ran.stderr || ran.stdout).trim()
  if (/is not recognized as an internal or external command/i.test(detail)) {
    return { isOpened: false, reason: NOT_FOUND }
  }

  return {
    isOpened: false,
    reason: `\`code\` exited with ${ran.exitCode}${detail ? `: ${detail}` : '.'}`,
  }
}
