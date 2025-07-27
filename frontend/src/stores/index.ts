// Pinia stores统一导出

import { createPinia } from 'pinia'

// 创建pinia实例
export const pinia = createPinia()

// 导出stores
export { useUserStore } from './user'
export { useAppStore } from './app'

// 默认导出pinia实例
export default pinia