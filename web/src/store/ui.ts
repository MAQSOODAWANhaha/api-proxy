/**
 * ui.ts
 * 全局 UI 状态：侧栏的折叠状态与移动端抽屉开合。
 */

import { create } from 'zustand'

/** UI 状态接口 */
interface UiState {
  /** 桌面端侧栏是否折叠（仅影响 md 及以上断点） */
  sidebarCollapsed: boolean
  /** 移动端抽屉是否打开（仅影响 md 以下断点） */
  mobileSidebarOpen: boolean
  /** 切换折叠状态 */
  toggleSidebar: () => void
  /** 打开移动端抽屉 */
  openMobileSidebar: () => void
  /** 关闭移动端抽屉 */
  closeMobileSidebar: () => void
}

/** UI 状态全局 store */
export const useUiStore = create<UiState>((set) => ({
  sidebarCollapsed: false,
  mobileSidebarOpen: false,
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  openMobileSidebar: () => set({ mobileSidebarOpen: true }),
  closeMobileSidebar: () => set({ mobileSidebarOpen: false }),
}))
