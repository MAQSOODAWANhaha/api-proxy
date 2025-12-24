import { toast } from 'sonner'

const fallbackCopyText = (text: string): boolean => {
  if (typeof document === 'undefined') return false

  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  textarea.style.pointerEvents = 'none'
  textarea.style.left = '-9999px'
  document.body.appendChild(textarea)
  textarea.select()

  let success = false
  try {
    success = document.execCommand('copy')
  } catch {
    success = false
  } finally {
    document.body.removeChild(textarea)
  }

  return success
}

export const copyWithFeedback = async (text: string, label: string): Promise<boolean> => {
  if (!text) {
    toast.error('复制失败，请手动复制')
    return false
  }

  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      toast.success(`${label}已复制到剪贴板`)
      return true
    }
  } catch {
    // ignore and fallback
  }

  const success = fallbackCopyText(text)
  if (success) {
    toast.success(`${label}已复制到剪贴板`)
  } else {
    toast.error('复制失败，请手动复制')
  }

  return success
}
