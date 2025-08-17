/**
 * ThemeToggle.tsx
 * 深浅色主题切换按钮，符合现代平面风格。
 */
import { Moon, Sun } from 'lucide-react'
import { useTheme } from 'next-themes'
import { Button } from './ui/button'

/** 主题切换按钮 */
export default function ThemeToggle() {
  const { theme, setTheme } = useTheme()

  const toggle = () => {
    setTheme(theme === 'dark' ? 'light' : 'dark')
  }

  return (
    <Button variant="outline" className="bg-transparent h-8 w-8 p-0" onClick={toggle} title="主题切换">
      <Sun className="h-4 w-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
      <Moon className="absolute h-4 w-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
      <span className="sr-only">Toggle theme</span>
    </Button>
  )
}
