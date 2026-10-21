// The command key as the user's OS names it: ⌘ on macOS, Ctrl elsewhere.
export function modKey(userAgent: string = navigator.userAgent): string {
  return /Mac|iPhone|iPad/.test(userAgent) ? "⌘" : "Ctrl";
}

export const searchShortcut = (userAgent?: string) => `${modKey(userAgent)} K`;
