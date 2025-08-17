/**
 * DashboardLayout.tsx
 * 主布局：左侧固定侧栏 + 顶部工具栏 + 右侧内容区。
 * 重点修复：为内容区添加 overflow-x-hidden，并保证右侧列 min-w-0，避免出现横向滚动条（底部拖拽条）。
 */

import React from 'react'
import { Outlet } from 'react-router'
import Sidebar from '../components/layout/Sidebar'
import Topbar from '../components/layout/Topbar'

/**
 * DashboardLayout
 * - 左侧 Sidebar 固定宽度，不收缩
 * - 右侧内容列 min-w-0，主内容区 overflow-x-hidden，仅允许纵向滚动
 */
const DashboardLayout: React.FC = () => {
  return (
    <div
      className="
        flex h-screen w-full overflow-hidden
        bg-white text-foreground
      "
    >
      {/* 左侧固定侧栏 */}
      <Sidebar />

      {/* 右侧内容区：min-w-0 防止子元素造成横向溢出 */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* 顶部工具栏 */}
        <Topbar />

        {/* 主内容：仅允许纵向滚动；禁止横向滚动（移除底部拖拽条） */}
        <main className="flex-1 overflow-y-auto overflow-x-hidden py-4 px-2 md:py-6 md:px-3">
          <Outlet />
        </main>
      </div>
    </div>
  )
}

export default DashboardLayout
