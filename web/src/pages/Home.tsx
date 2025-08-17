/**
 * Home.tsx
 * 主页占位：提供前往仪表板的入口，避免空白页面。
 */
import { Button } from '../components/ui/button'

export default function HomePage() {
  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-4">
      <div className="text-2xl font-bold">欢迎使用 Thinkmax 控制台</div>
      <a href="#/dashboard">
        <Button>进入仪表板</Button>
      </a>
    </div>
  )
}
