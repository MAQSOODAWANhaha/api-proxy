/**
 * Topbar.tsx
 * 顶部栏组件：提供侧栏开关与当前页面标题展示。移除搜索框与 Export 按钮。
 */

import React from 'react'
import { useLocation } from 'react-router'
import { Menu, PanelsTopLeft, Bell } from 'lucide-react'
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
 * Topbar
 * - 左侧：移动端抽屉开关、桌面端折叠/展开侧栏
 * - 中部：页面标题
 * - 右侧：占位操作（去除搜索与 Export，仅保留通知/头像位）
 */
const Topbar: React.FC = () => {
  const title = usePageTitle()
  const toggleSidebar = useUiStore((s) => s.toggleSidebar)
  const openMobileSidebar = useUiStore((s) => s.openMobileSidebar)

  return (
    <header
      className={[
        'sticky top-0 z-40',
        'flex h-14 w-full items-center justify-between',
        'border-b border-neutral-200 bg-white/80 backdrop-blur supports-[backdrop-filter]:bg-white/60',
        'px-3 md:px-4',
      ].join(' ')}
      aria-label="应用顶部栏"
    >
      {/* 左侧：菜单开关区（移动端与桌面端分离） */}
      <div className="flex items-center gap-2">
        {/* 移动端打开抽屉 */}
        <button
          type="button"
          onClick={openMobileSidebar}
          className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-neutral-200 bg-white text-neutral-700 hover:bg-neutral-50 md:hidden"
          aria-label="打开菜单"
          title="打开菜单"
        >
          <Menu size={18} />
        </button>

        {/* 桌面端折叠/展开侧栏 */}
        <button
          type="button"
          onClick={toggleSidebar}
          className="hidden h-9 items-center justify-center gap-2 rounded-lg border border-neutral-200 bg-white px-2 text-neutral-700 hover:bg-neutral-50 md:inline-flex"
          aria-label="折叠/展开侧栏"
          title="折叠/展开侧栏"
        >
          <PanelsTopLeft size={18} />
          <span className="hidden text-sm md:inline">侧栏</span>
        </button>

        {/* 页面标题 */}
        <h1 className="ml-1 text-base font-semibold text-neutral-900 md:ml-3" aria-live="polite">
          {title}
        </h1>
      </div>

      {/* 右侧：简单操作区域（去除搜索与 Export） */}
      <div className="flex items-center gap-2 md:gap-3">
        {/* 通知按钮（示例，可按需替换/移除） */}
        <button
          type="button"
          className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-neutral-200 bg-white text-neutral-700 hover:bg-neutral-50"
          aria-label="通知"
          title="通知"
        >
          <Bell size={18} />
        </button>

        {/* 用户头像占位（可接入真实头像） */}
        <div
          className="flex h-9 w-9 items-center justify-center overflow-hidden rounded-full border border-neutral-200 bg-neutral-100"
          aria-label="用户"
          title="用户"
        >
          <img
            src="https://pub-cdn.sider.ai/u/U024HX2V46R/web-coder/689c58b6f5303283889f5c38/resource/72cd5825-101c-47c5-b053-9bbcec64f97f.jpg"
            alt="User"
            className="h-full w-full object-cover"
          />
        </div>
      </div>
    </header>
  )
}

export default Topbar
