/**
 * Sidebar.tsx
 * 左侧导航栏（桌面侧栏 + 移动抽屉），基于 shadcn/ui Sheet 组件优化。
 * 说明：
 * - 使用 lucide-react 图标与 TailwindCSS 进行视觉设计。
 * - 使用 zustand 的 ui store 管理折叠与移动端抽屉的可见性。
 * - 使用 react-router 的 useLocation/useNavigate 进行路由跳转。
 * - 移动端使用 shadcn/ui Sheet 组件提供更好的用户体验。
 */

import React, { useMemo } from 'react'
import { useLocation, useNavigate } from 'react-router'
import {
  LayoutDashboard,
  KeyRound,
  KeySquare,
  FileText,
  Building2,
  Users as UsersIcon,
  Settings as SettingsIcon,
} from 'lucide-react'
import { Sheet, SheetContent } from '@/components/ui/sheet'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
// 说明：为避免构建器未配置别名导致报错，这里使用相对路径替代 "@/store/ui"
import { useUiStore } from '../../store/ui'

/** 主色（violet-600） */
const PRIMARY = '#7c3aed'

/**
 * 导航项接口：用于描述每一个菜单项
 */
interface NavItem {
  /** 唯一 key */
  key: string
  /** 菜单显示文本 */
  label: string
  /** 路由路径 */
  path: string
  /** 菜单图标 */
  icon: React.ReactNode
}

/**
 * 判断目标路由是否处于激活状态（严格匹配：自身或以其为前缀）
 */
function isActive(pathname: string, target: string): boolean {
  return pathname === target || pathname.startsWith(target + '/')
}

/**
 * 单个导航按钮（使用 shadcn/ui Button 组件，深紫色填充选中效果）
 */
const NavButton: React.FC<{
  active: boolean
  collapsed: boolean
  icon: React.ReactNode
  label: string
  onClick: () => void
  title?: string
}> = ({ active, collapsed, icon, label, onClick, title }) => {
  const iconBox = cn(
    'flex h-8 w-8 items-center justify-center rounded-lg transition-all duration-200',
    active ? 'bg-white/20 text-white' : 'bg-neutral-100 text-neutral-600 group-hover:bg-neutral-200'
  )

  return (
    <div className="relative">
      {active && (
        <span
          aria-hidden
          className="absolute left-0 top-1 bottom-1 w-[4px] rounded-r-full bg-white/90"
        />
      )}
      <Button
        variant="ghost"
        onClick={onClick}
        className={cn(
          'relative flex items-center gap-3 rounded-none min-h-10 text-sm font-medium transition-all duration-200 select-none group w-full',
          collapsed ? 'justify-center px-0' : 'justify-start pl-6 pr-3',
          active
            ? 'bg-violet-600 text-white outline outline-1 outline-violet-500/50 outline-offset-0 hover:bg-violet-600'
            : 'text-neutral-700 hover:bg-neutral-100'
        )}
        title={title ?? label}
        aria-current={active ? 'page' : undefined}
      >
        <span className={iconBox}>{icon}</span>
        {!collapsed && <span className="truncate">{label}</span>}
      </Button>
    </div>
  )
}

/**
 * 品牌区（圆角 Logo + 文案“Proxy”）
 */
const Brand: React.FC<{ collapsed: boolean }> = ({ collapsed }) => {
  return (
    <div
      className={[
        'flex h-14 items-center border-b',
        'border-neutral-200',
        collapsed ? 'justify-center px-0 gap-0' : 'justify-start px-3 gap-3',
      ].join(' ')}
    >
      <div
        className="flex h-10 w-10 items-center justify-center rounded-xl border border-black/5 text-white"
        style={{ backgroundColor: PRIMARY }}
        aria-label="品牌"
        title="品牌"
      >
        <span className="text-base font-bold leading-none">P</span>
      </div>
      {!collapsed && (
        <div className="text-sm font-semibold text-neutral-900">Proxy</div>
      )}
    </div>
  )
}

/**
 * 侧栏主组件：白色背景 + 扁平化菜单（桌面+移动端）
 * 复杂逻辑说明：
 * - 桌面端：固定侧栏，可折叠。
 * - 移动端：抽屉式侧栏，点击遮罩或关闭按钮关闭。
 */
const Sidebar: React.FC = () => {
  const location = useLocation()
  const navigate = useNavigate()

  // 从 UI Store 获取侧栏状态
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed)
  const mobileSidebarOpen = useUiStore((s) => s.mobileSidebarOpen)
  const closeMobileSidebar = useUiStore((s) => s.closeMobileSidebar)

  /** 扁平化菜单列表（与需求顺序保持一致） */
  const navItems: NavItem[] = useMemo(
    () => [
      { key: 'dashboard', label: '仪表板', path: '/dashboard', icon: <LayoutDashboard size={18} /> },
      { key: 'user-keys', label: '用户API Keys', path: '/api/user-keys', icon: <KeyRound size={18} /> },
      { key: 'provider-keys', label: '账号API Keys', path: '/api/provider-keys', icon: <KeySquare size={18} /> },
      { key: 'providers', label: '服务商', path: '/providers', icon: <Building2 size={18} /> },
      { key: 'logs', label: '请求记录', path: '/logs', icon: <FileText size={18} /> },
      { key: 'users', label: '用户管理', path: '/users', icon: <UsersIcon size={18} /> },
      { key: 'settings', label: '系统设置', path: '/settings', icon: <SettingsIcon size={18} /> },
    ],
    []
  )

  // 个人中心已迁移到右上角头像菜单中

  /**
   * 点击导航：前往并在移动端关闭抽屉
   * 复杂逻辑说明：移动端抽屉打开时点击导航需关闭抽屉，桌面端不受影响
   */
  function handleNav(path: string) {
    navigate(path)
    if (mobileSidebarOpen) closeMobileSidebar()
  }

  /**
   * 桌面版侧栏（白色风格）
   */
  const DesktopAside = (
    <aside
      className={[
        'hidden md:flex md:flex-col md:h-screen shrink-0',
        'bg-white text-neutral-800',
        'border-r border-neutral-200',
        sidebarCollapsed ? 'w-[76px]' : 'w-64',
        'transition-[width] duration-300 ease-in-out',
      ].join(' ')}
      aria-label="侧边导航"
    >
      {/* 品牌区 */}
      <Brand collapsed={sidebarCollapsed} />

      {/* 菜单区域 */}
      <nav className="flex flex-1 flex-col gap-1 py-2">
        {navItems.map((item) => {
          const active = isActive(location.pathname, item.path)
          return (
            <NavButton
              key={item.key}
              active={active}
              collapsed={sidebarCollapsed}
              icon={item.icon}
              label={item.label}
              onClick={() => handleNav(item.path)}
              title={item.label}
            />
          )
        })}

        {/* 底部空间保留 */}
        <div className="mt-auto" />
      </nav>
    </aside>
  )

  /**
   * 移动端抽屉侧栏（使用 shadcn/ui Sheet 组件）
   */
  const MobileDrawer = (
    <Sheet open={mobileSidebarOpen} onOpenChange={(open) => !open && closeMobileSidebar()}>
      <SheetContent 
        side="left" 
        className={cn(
          'w-[82vw] max-w-xs p-0',
          'bg-white text-neutral-800',
          'flex flex-col'
        )}
      >
        {/* 品牌区 */}
        <Brand collapsed={false} />

        {/* 菜单 */}
        <nav className="flex flex-1 flex-col gap-1 py-2">
          {navItems.map((item) => {
            const active = isActive(location.pathname, item.path)
            return (
              <NavButton
                key={item.key}
                active={active}
                collapsed={false}
                icon={item.icon}
                label={item.label}
                onClick={() => handleNav(item.path)}
                title={item.label}
              />
            )
          })}
          {/* 底部空间保留 */}
          <div className="mt-auto" />
        </nav>
      </SheetContent>
    </Sheet>
  )

  return (
    <>
      {DesktopAside}
      {MobileDrawer}
    </>
  )
}

export default Sidebar
