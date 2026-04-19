/** Copy text to clipboard with fallback for non-HTTPS contexts. */
export async function copyToClipboard(text: string): Promise<boolean> {
  // Modern API (requires HTTPS or localhost)
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      // Fall through to legacy method
    }
  }

  // Legacy fallback using a temporary textarea
  try {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.position = 'fixed'
    textarea.style.left = '-9999px'
    textarea.style.top = '-9999px'
    document.body.appendChild(textarea)
    textarea.focus()
    textarea.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(textarea)
    return ok
  } catch {
    return false
  }
}
