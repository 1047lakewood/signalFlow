/**
 * Strip the Windows verbatim-path prefix (`\\?\`) that Tauri dialog and
 * std::fs::canonicalize add on Windows.  Also handles the verbatim UNC
 * form (`\\?\UNC\host\share` → `\\host\share`).  Returns the path
 * unchanged on other platforms or when the prefix is absent.
 */
export function cleanPath(p: string): string {
  // Verbatim UNC: \\?\UNC\host\share\... → \\host\share\...
  if (p.startsWith("\\\\?\\UNC\\")) {
    return "\\\\" + p.slice(8);
  }
  // Verbatim local: \\?\C:\... → C:\...
  if (p.startsWith("\\\\?\\")) {
    return p.slice(4);
  }
  return p;
}
