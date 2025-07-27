// 通用类型定义

// API响应基础结构
export interface ApiResponse<T = any> {
  success?: boolean
  data?: T
  message?: string
  error?: string
  code?: number
}

// 错误响应
export interface ErrorResponse {
  error: string
  message: string
  code?: number
}

// 分页参数
export interface PaginationParams {
  page?: number
  limit?: number
}

// 分页响应信息
export interface PaginationInfo {
  page: number
  limit: number
  total: number
  pages: number
}

// 带分页的列表响应
export interface PaginatedResponse<T> {
  data: T[]
  pagination: PaginationInfo
}

// 时间范围查询参数
export interface TimeRangeParams {
  start_time?: string
  end_time?: string
  hours?: number
  days?: number
}

// 排序参数
export interface SortParams {
  sort_by?: string
  sort_order?: 'asc' | 'desc'
}

// 搜索参数
export interface SearchParams {
  keyword?: string
  search_fields?: string[]
}

// 通用CRUD操作响应
export interface CrudResponse<T = any> {
  success: boolean
  message: string
  data?: T
}

// 状态选项
export type StatusType = 'active' | 'inactive' | 'pending' | 'disabled'

// 请求方法类型
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH'

// 表格列配置
export interface TableColumn {
  key: string
  label: string
  width?: number | string
  sortable?: boolean
  filterable?: boolean
  type?: 'text' | 'number' | 'date' | 'status' | 'actions'
}

// 表单字段配置
export interface FormField {
  key: string
  label: string
  type: 'input' | 'select' | 'textarea' | 'number' | 'password' | 'switch' | 'date'
  required?: boolean
  placeholder?: string
  options?: Array<{ label: string; value: any }>
  rules?: any[]
}

// 操作按钮配置
export interface ActionButton {
  label: string
  type?: 'primary' | 'success' | 'warning' | 'danger' | 'info'
  icon?: string
  action: string
  disabled?: boolean
  loading?: boolean
}

// 菜单项
export interface MenuItem {
  id: string
  title: string
  path?: string
  icon?: string
  children?: MenuItem[]
  roles?: string[]
  hidden?: boolean
}

// 面包屑项
export interface BreadcrumbItem {
  title: string
  path?: string
}

// 通知消息类型
export type NotificationType = 'success' | 'warning' | 'info' | 'error'

// 图表数据点
export interface ChartDataPoint {
  name: string
  value: number
  timestamp?: string
}

// 图表配置
export interface ChartConfig {
  type: 'line' | 'bar' | 'pie' | 'area'
  title?: string
  xAxisLabel?: string
  yAxisLabel?: string
  color?: string | string[]
  height?: number
}