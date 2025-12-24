/**
 * ThemeToggle - switch between light and dark using next-themes.
 */

import { Moon, Sun } from 'lucide-react'
import { useTheme } from 'next-themes'
import { Button } from '@/components/ui/button'
import { useEffect, useState } from 'react'

export default function ThemeToggle() {
  const { theme, setTheme, systemTheme } = useTheme()
  const [mounted, setMounted] = useState(false)

  useEffect(() => setMounted(true), [])

  const current = theme === 'system' ? systemTheme : theme
  const isDark = current === 'dark'

  if (!mounted) {
    return null
  }

  return (
    <Button
      variant="outline"
      className="bg-transparent size-9 p-0 rounded-full"
      onClick={() => setTheme(isDark ? 'light' : 'dark')}
      aria-label="切换主题"
      title="切换主题"
    >
      {isDark ? <Sun className="size-5 text-yellow-500" /> : <Moon className="size-5 text-indigo-600" />}
    </Button>
  )
}
