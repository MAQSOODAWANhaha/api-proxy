/**
 * API基础服务 - 使用新的错误处理系统
 * @deprecated 请使用 @/utils/request 中的新HTTP客户端
 */

import { http } from '@/utils/request'

// 导出新的HTTP客户端
export default http

// 为了向后兼容，保留原有的service导出
export const service = http
