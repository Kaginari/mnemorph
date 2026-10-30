/** Links and images from model or tool text: what the board lets through. */

/** The only schemes a link may carry. */
export const SCHEMES: readonly string[] = ['http:', 'https:', 'mailto:'];

/**
 * The URL a link may point at, or `undefined`: an absolute http, https or mailto URL with no
 * credentials in it (`https://bank.example@evil.example` reads as a bank and is not).
 */
export function safeHref(raw: string): string | undefined {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return undefined;
  }
  if (!SCHEMES.includes(url.protocol)) return undefined;
  if (url.username || url.password) return undefined;
  return url.href;
}

/** The `rel` every outbound link carries. */
export const REL = 'noopener noreferrer';
