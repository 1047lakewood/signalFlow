/**
 * Strip the Windows verbatim-path prefix (`\\?\`) that Tauri dialog and
 * std::fs::canonicalize add on Windows.  Returns the path unchanged on
 * other platforms or when the prefix is absent.
 */
export function cleanPath(p: string): string {
  if (p.startsWith("\\\\?\\")) {
    return p.slice(4);
  }
  return p;
}
