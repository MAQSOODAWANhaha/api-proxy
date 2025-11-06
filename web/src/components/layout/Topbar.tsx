/**
 * Topbar.tsx
 * 顶部栏组件：基于 shadcn/ui 组件，提供侧栏开关与当前页面标题展示。
 */

import React from 'react'
import { useLocation, useNavigate } from 'react-router'
import { useAuthStore } from '../../store/auth'
import { Menu, PanelsTopLeft, Bell, User, LogOut } from 'lucide-react'
import userAvatar from '../../assets/image.png'
import { Button } from '@/components/ui/button'
import { Avatar, AvatarImage, AvatarFallback } from '@/components/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { useUiStore } from '../../store/ui'

/**
 * 获取当前路由对应的页面标题
 */
function usePageTitle(): string {
  const location = useLocation()
  const path = location.pathname

  /** 基础路由与标题映射（最长前缀匹配） */
  const titleMap: Record<string, string> = {
    '/dashboard': '仪表板',
    '/api/user-keys': '用户API Keys',
    '/api/provider-keys': '账号API Keys',
    '/api': 'API 密钥管理',
    // '/stats' 已移除
    '/logs': '请求记录',
    '/users': '用户管理',
    '/settings': '系统设置',
    '/profile': '个人信息',
    '/login': '登录',
  }

  // 复杂逻辑：做最长匹配，确保诸如 /api/user-keys 优先于 /api
  let best = '仪表板'
  let bestLen = -1
  Object.keys(titleMap).forEach((key) => {
    if (path.startsWith(key) && key.length > bestLen) {
      best = titleMap[key]
      bestLen = key.length
    }
  })
  return best
}

/**
 * Topbar - 基于 shadcn/ui 组件的顶部栏
 * - 左侧：移动端抽屉开关、桌面端折叠/展开侧栏
 * - 中部：页面标题
 * - 右侧：通知按钮和用户菜单
 */
const Topbar: React.FC = () => {
  const title = usePageTitle()
  const navigate = useNavigate()
  const { logout } = useAuthStore()
  const toggleSidebar = useUiStore((s) => s.toggleSidebar)
  const openMobileSidebar = useUiStore((s) => s.openMobileSidebar)

  /** 处理退出登录 */
  const handleLogout = async () => {
    await logout()
    navigate('/login', { replace: true })
  }

  return (
    <header
      className={cn(
        'sticky top-0 z-40',
        'flex h-14 w-full items-center justify-between',
        'border-b border-neutral-200 bg-white/80 backdrop-blur supports-[backdrop-filter]:bg-white/60',
        'px-3 md:px-4'
      )}
      aria-label="应用顶部栏"
    >
      {/* 左侧：菜单开关区（移动端与桌面端分离） */}
      <div className="flex items-center gap-2">
        {/* 移动端打开抽屉 */}
        <Button
          variant="outline"
          size="icon"
          onClick={openMobileSidebar}
          className="h-9 w-9 md:hidden"
          aria-label="打开菜单"
        >
          <Menu size={18} />
        </Button>

        {/* 桌面端折叠/展开侧栏 */}
        <Button
          variant="outline"
          onClick={toggleSidebar}
          className="hidden gap-2 md:inline-flex"
          aria-label="折叠/展开侧栏"
        >
          <PanelsTopLeft size={18} />
          <span className="hidden text-sm md:inline">侧栏</span>
        </Button>

        {/* 页面标题 */}
        <h1 className="ml-1 text-base font-semibold text-neutral-900 md:ml-3" aria-live="polite">
          {title}
        </h1>
      </div>

      {/* 右侧：操作区域 */}
      <div className="flex items-center gap-2 md:gap-3">
        {/* 通知按钮 */}
        <Button
          variant="outline"
          size="icon"
          className="h-9 w-9"
          aria-label="通知"
        >
          <Bell size={18} />
        </Button>

        {/* 用户菜单 */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" className="h-9 w-9 rounded-full p-0">
              <Avatar className="h-9 w-9">
                <AvatarImage
                  src={userAvatar}
                  alt="用户头像"
                />
                <AvatarFallback>
                  <User size={18} />
                </AvatarFallback>
              </Avatar>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => navigate('/profile')}>
              <User className="mr-2 h-4 w-4" />
              个人信息
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={handleLogout}>
              <LogOut className="mr-2 h-4 w-4" />
              退出登录
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  )
}

export default Topbar
