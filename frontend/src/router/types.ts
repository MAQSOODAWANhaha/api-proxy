// 路由相关类型定义

import type { RouteRecordRaw } from 'vue-router'

// 扩展路由元信息类型
declare module 'vue-router' {
  interface RouteMeta {
    // 页面标题
    title?: string
    // 菜单图标
    icon?: string
    // 是否需要认证
    requiresAuth?: boolean
    // 是否需要管理员权限
    requiresAdmin?: boolean
    // 所需权限列表
    permissions?: string | string[]
    // 所需角色列表
    roles?: string | string[]
    // 是否在菜单中隐藏
    hideInMenu?: boolean
    // 是否缓存页面组件
    keepAlive?: boolean
    // 是否固定在标签页中
    affix?: boolean
    // 重定向地址
    redirect?: string
    // 是否为外部链接
    isExternal?: boolean
    // 外部链接地址
    externalUrl?: string
    // 页面排序
    sort?: number
    // 是否为子菜单
    isSubmenu?: boolean
    // 菜单分组
    group?: string
    // 页面布局类型
    layout?: 'default' | 'blank' | 'auth'
    // 页面加载状态
    loading?: boolean
    // 页面描述
    description?: string
    // 关键词（用于搜索）
    keywords?: string[]
    // 是否显示面包屑
    showBreadcrumb?: boolean
    // 是否显示标签页
    showTabs?: boolean
    // 页面背景色
    backgroundColor?: string
    // 自定义类名
    className?: string
    // 页面元数据
    metadata?: Record<string, any>
  }
}

// 菜单项类型
export interface MenuItem {
  id: string
  name: string
  title: string
  path: string
  icon?: string
  component?: any
  redirect?: string
  children?: MenuItem[]
  meta?: RouteMeta
  hidden?: boolean
  disabled?: boolean
  badge?: string | number
  tag?: {
    text: string
    type: 'primary' | 'success' | 'warning' | 'danger' | 'info'
  }
}

// 路由配置类型
export interface RouteConfig extends Omit<RouteRecordRaw, 'meta'> {
  meta?: RouteMeta
  children?: RouteConfig[]
}

// 面包屑项类型
export interface BreadcrumbItem {
  name: string
  path?: string
  icon?: string
  disabled?: boolean
}

// 标签页项类型
export interface TabItem {
  name: string
  title: string
  path: string
  closable?: boolean
  active?: boolean
  icon?: string
  meta?: RouteMeta
}

// 权限配置类型
export interface PermissionConfig {
  // 权限代码
  code: string
  // 权限名称
  name: string
  // 权限描述
  description?: string
  // 父权限
  parent?: string
  // 权限类型
  type: 'menu' | 'button' | 'api' | 'data'
  // 权限级别
  level: number
  // 是否启用
  enabled: boolean
  // 关联资源
  resources?: string[]
}

// 角色配置类型
export interface RoleConfig {
  // 角色ID
  id: string
  // 角色名称
  name: string
  // 角色描述
  description?: string
  // 角色权限
  permissions: string[]
  // 是否内置角色
  builtin: boolean
  // 是否启用
  enabled: boolean
  // 创建时间
  createdAt?: string
  // 更新时间
  updatedAt?: string
}

// 路由状态类型
export interface RouteState {
  // 当前路由
  currentRoute: RouteRecordRaw | null
  // 历史路由
  historyRoutes: RouteRecordRaw[]
  // 缓存的路由组件
  cachedRoutes: string[]
  // 标签页列表
  tabs: TabItem[]
  // 当前激活的标签页
  activeTab: string | null
}

// 导航选项类型
export interface NavigationOptions {
  // 是否替换当前历史记录
  replace?: boolean
  // 查询参数
  query?: Record<string, any>
  // Hash 值
  hash?: string
  // 状态数据
  state?: any
  // 是否强制刷新
  force?: boolean
}

// 路由守卫配置类型
export interface GuardConfig {
  // 是否启用权限检查
  enableAuth?: boolean
  // 是否启用角色检查
  enableRole?: boolean
  // 是否启用Token验证
  enableTokenValidation?: boolean
  // Token验证间隔（毫秒）
  tokenValidationInterval?: number
  // 白名单路由
  whitelist?: string[]
  // 登录页面路径
  loginPath?: string
  // 权限不足页面路径
  forbiddenPath?: string
  // 404页面路径
  notFoundPath?: string
}

// 动态路由配置类型
export interface DynamicRouteConfig {
  // 路由来源
  source: 'api' | 'config' | 'plugin'
  // 路由数据
  routes: RouteConfig[]
  // 是否自动注册
  autoRegister?: boolean
  // 注册前的处理函数
  beforeRegister?: (routes: RouteConfig[]) => RouteConfig[]
  // 注册后的回调函数
  afterRegister?: (routes: RouteConfig[]) => void
}

// 路由缓存配置类型
export interface RouteCacheConfig {
  // 最大缓存数量
  maxCacheCount?: number
  // 是否启用缓存
  enableCache?: boolean
  // 缓存策略
  cacheStrategy?: 'lru' | 'fifo' | 'custom'
  // 自定义缓存函数
  customCache?: (routes: string[]) => string[]
}

// 路由事件类型
export type RouteEvent = 
  | 'beforeRouteEnter'
  | 'beforeRouteUpdate' 
  | 'beforeRouteLeave'
  | 'routeChanged'
  | 'tabAdded'
  | 'tabRemoved'
  | 'tabActivated'

// 路由事件处理器类型
export type RouteEventHandler = (
  event: RouteEvent,
  data: any
) => void | Promise<void>

// 路由工具类型
export interface RouteUtils {
  // 检查权限
  checkPermission: (permission: string | string[]) => boolean
  // 检查角色
  checkRole: (role: string | string[]) => boolean
  // 获取路由标题
  getRouteTitle: (route: RouteRecordRaw) => string
  // 获取面包屑
  getBreadcrumbs: (route: RouteRecordRaw) => BreadcrumbItem[]
  // 过滤菜单路由
  filterMenuRoutes: (routes: RouteConfig[]) => RouteConfig[]
  // 查找路由
  findRoute: (routes: RouteConfig[], matcher: string | ((route: RouteConfig) => boolean)) => RouteConfig | null
  // 生成路由键
  generateRouteKey: (route: RouteRecordRaw) => string
}