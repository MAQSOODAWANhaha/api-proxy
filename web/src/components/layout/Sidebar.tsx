/**
 * Sidebar.tsx
 * 左侧导航栏（桌面侧栏 + 移动抽屉），白色背景、紫色点缀，支持折叠与移动端抽屉。
 * 说明：
 * - 使用 lucide-react 图标与 TailwindCSS 进行视觉设计。
 * - 使用 zustand 的 ui store 管理折叠与移动端抽屉的可见性。
 * - 使用 react-router 的 useLocation/useNavigate 进行路由跳转。
 */

import React, { useMemo } from 'react'
import { useLocation, useNavigate } from 'react-router'
import {
  LayoutDashboard,
  KeyRound,
  KeySquare,
  FileText,
  Users as UsersIcon,
  Settings as SettingsIcon,
  User,
  X,
} from 'lucide-react'
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
 * 单个导航按钮（深紫色填充选中效果）
 */
const NavButton: React.FC<{
  active: boolean
  collapsed: boolean
  icon: React.ReactNode
  label: string
  onClick: () => void
  title?: string
}> = ({ active, collapsed, icon, label, onClick, title }) => {
  // 样式说明：
  // - 未激活：透明底，文本 neutral-700，图标容器 neutral-100；悬停出现 neutral-100。
  // - 激活：violet-600 深色背景 + 白色文字 + 微妙阴影，完全填充的选中效果。
  const base =
    'relative flex items-center gap-3 rounded-none min-h-10 text-sm font-medium transition-all duration-200 select-none'
  const layout = collapsed ? 'justify-center px-0' : 'justify-start pl-6 pr-3'
  const state = active
    ? 'bg-violet-600 text-white shadow-lg shadow-violet-600/25 ring-1 ring-violet-500'
    : 'text-neutral-700 hover:bg-neutral-100 hover:shadow-sm hover:scale-[1.02]'
  const focus =
    'focus:outline-none focus-visible:ring-2 focus-visible:ring-violet-500/40 focus-visible:ring-offset-0'

  const iconBox = [
    'flex h-8 w-8 items-center justify-center rounded-lg transition-all duration-200',
    active ? 'bg-white/20 text-white' : 'bg-neutral-100 text-neutral-600 group-hover:bg-neutral-200',
  ].join(' ')

  return (
    <div className="relative">
      {active && (
        <span
          aria-hidden
          className="absolute left-0 top-1 bottom-1 w-[4px] rounded-r-full bg-white/90 shadow-sm"
        />
      )}
      <button
        type="button"
        onClick={onClick}
        className={[base, layout, state, focus, 'group w-full'].join(' ')}
        title={title ?? label}
        aria-current={active ? 'page' : undefined}
      >
        <span className={iconBox}>{icon}</span>
        {!collapsed && <span className="truncate">{label}</span>}
      </button>
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
        className="flex h-10 w-10 items-center justify-center rounded-xl text-white shadow-[0_0_0_1px_rgba(0,0,0,0.04)_inset]"
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
      { key: 'logs', label: '请求记录', path: '/logs', icon: <FileText size={18} /> },
      { key: 'users', label: '用户管理', path: '/users', icon: <UsersIcon size={18} /> },
      { key: 'settings', label: '系统设置', path: '/settings', icon: <SettingsIcon size={18} /> },
    ],
    []
  )

  /** 底部固定项（个人中心） */
  const bottomItem: NavItem = useMemo(
    () => ({ key: 'profile', label: '个人中心', path: '/profile', icon: <User size={18} /> }),
    []
  )

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

        {/* 底部分隔与个人中心 */}
        <div className="mt-auto pt-2">
          <div className={`${sidebarCollapsed ? 'mx-3' : 'mx-6'} mb-2 h-px bg-neutral-200`} />
          <NavButton
            active={isActive(location.pathname, bottomItem.path)}
            collapsed={sidebarCollapsed}
            icon={bottomItem.icon}
            label={bottomItem.label}
            onClick={() => handleNav(bottomItem.path)}
            title={bottomItem.label}
          />
        </div>
      </nav>
    </aside>
  )

  /**
   * 移动端抽屉侧栏（白色风格）
   */
  const MobileDrawer = mobileSidebarOpen ? (
    <div className="fixed inset-0 z-50 md:hidden" aria-modal="true" role="dialog">
      {/* 遮罩 */}
      <div
        className="absolute inset-0 bg-black/40"
        onClick={closeMobileSidebar}
        aria-label="关闭侧边栏"
      />
      {/* 抽屉面板 */}
      <div
        className={[
          'absolute left-0 top-0 h-full w-[82vw] max-w-xs',
          'bg-white text-neutral-800',
          'border-r border-neutral-200',
          'shadow-2xl',
          'animate-in slide-in-from-left duration-200',
          'flex flex-col',
        ].join(' ')}
      >
        {/* 顶部栏（包含关闭按钮与品牌） */}
        <div className="relative">
          <Brand collapsed={false} />
          <button
            type="button"
            onClick={closeMobileSidebar}
            className="absolute right-3 top-2 inline-flex h-9 w-9 items-center justify-center rounded-lg text-neutral-600 hover:bg-neutral-100"
            aria-label="关闭侧边栏"
            title="关闭"
          >
            <X size={18} />
          </button>
        </div>

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
          <div className="mt-auto pt-2">
            <div className="mx-6 mb-2 h-px bg-neutral-200" />
            <NavButton
              active={isActive(location.pathname, bottomItem.path)}
              collapsed={false}
              icon={bottomItem.icon}
              label={bottomItem.label}
              onClick={() => handleNav(bottomItem.path)}
              title={bottomItem.label}
            />
          </div>
        </nav>
      </div>
    </div>
  ) : null

  return (
    <>
      {DesktopAside}
      {MobileDrawer}
    </>
  )
}

export default Sidebar
