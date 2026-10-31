/**
 * Every keyboard shortcut the board has, as the shortcut map (`?`) lists them. None approves
 * anything: an approval card takes only its own answers, on the card, once it is armed — and
 * while it is up, no shortcut opens anything over it.
 */

export interface Shortcut {
  /** The keys, each one a `<kbd>`: `['Ctrl', 'K']`. */
  keys: readonly string[];
  /** What they do. */
  does: string;
}

export interface ShortcutGroup {
  title: string;
  items: readonly Shortcut[];
}

/** The modifier the palette's shortcut names: ⌘ on a Mac, Ctrl elsewhere (both work). */
export function mod(platform: string = navigator.platform): string {
  return /^Mac|^iP/.test(platform) ? '⌘' : 'Ctrl';
}

export function shortcuts(m: string = mod()): ShortcutGroup[] {
  return [
    {
      title: 'Anywhere',
      items: [
        {
          keys: [m, 'K'],
          does: 'Open the command palette: jump to a world, a page, a chat or a run',
        },
        { keys: ['?'], does: 'Show this map of shortcuts' },
        { keys: ['Esc'], does: 'Close a dialog, the menu or (at phone width) the side pane' },
        { keys: ['Tab'], does: 'Move to the next control; Shift+Tab to the one before' },
      ],
    },
    {
      title: 'The rail',
      items: [
        { keys: ['↓'], does: 'The next item in the rail' },
        { keys: ['↑'], does: 'The item before' },
        { keys: ['Home'], does: 'The first item' },
        { keys: ['End'], does: 'The last item' },
      ],
    },
    {
      title: 'The command palette',
      items: [
        { keys: ['↓'], does: 'The next match' },
        { keys: ['↑'], does: 'The match before' },
        { keys: ['Enter'], does: 'Go to the match' },
        { keys: ['Esc'], does: 'Close the palette' },
      ],
    },
    {
      title: 'A chat',
      items: [
        { keys: ['Enter'], does: 'Send the line, or queue it behind the running turn' },
        { keys: ['Shift', 'Enter'], does: 'A new line in the composer' },
        { keys: ['Esc'], does: 'Stop the running turn (the controlling tab only)' },
      ],
    },
    {
      title: 'The colony net',
      items: [{ keys: ['Enter'], does: "Open the focused body's card (Space too)" }],
    },
  ];
}
