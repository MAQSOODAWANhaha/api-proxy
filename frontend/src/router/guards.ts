// 路由守卫配置

import type { Router, RouteLocationNormalized, NavigationGuardNext } from 'vue-router'
import { ElMessage, ElLoading } from 'element-plus'
import { useUserStore, useAppStore } from '@/stores'
import { getApiUrl } from '@/config'

// 白名单路由（不需要认证）
const whiteList = ['/login', '/register', '/403', '/404', '/500']

// loading实例
let loadingInstance: any = null

// 显示页面加载loading
const showPageLoading = () => {
  loadingInstance = ElLoading.service({
    lock: true,
    text: '页面加载中...',
    background: 'rgba(0, 0, 0, 0.7)'
  })
}

// 隐藏页面加载loading
const hidePageLoading = () => {
  if (loadingInstance) {
    loadingInstance.close()
    loadingInstance = null
  }
}

// 前置路由守卫
const beforeEachGuard = (
  to: RouteLocationNormalized,
  from: RouteLocationNormalized,
  next: NavigationGuardNext
) => {
  const userStore = useUserStore()
  const appStore = useAppStore()
  
  // 显示loading
  if (to.path !== from.path) {
    showPageLoading()
  }
  
  // 设置页面标题
  if (to.meta?.title) {
    appStore.setPageTitle(to.meta.title as string)
  }
  
  // 更新面包屑
  const breadcrumbs = getBreadcrumbs(to)
  appStore.setBreadcrumbs(breadcrumbs)
  
  // 检查是否在白名单中
  if (whiteList.includes(to.path)) {
    next()
    return
  }
  
  // 检查是否需要认证
  if (to.meta?.requiresAuth !== false) {
    // 检查是否已登录
    if (!userStore.isLoggedIn) {
      ElMessage.warning('请先登录')
      next({
        path: '/login',
        query: { redirect: to.fullPath }
      })
      return
    }
    
    // 检查token有效性
    if (!userStore.token) {
      ElMessage.error('登录状态已过期，请重新登录')
      userStore.logout(false)
      next({
        path: '/login',
        query: { redirect: to.fullPath }
      })
      return
    }
  }
  
  // 检查管理员权限
  if (to.meta?.requiresAdmin && !userStore.isAdmin) {
    ElMessage.error('权限不足，需要管理员权限')
    next('/403')
    return
  }
  
  // 检查特定权限
  if (to.meta?.permissions) {
    const permissions = Array.isArray(to.meta.permissions) 
      ? to.meta.permissions 
      : [to.meta.permissions]
    
    const hasPermission = permissions.some((permission: string) => 
      userStore.hasPermission(permission)
    )
    
    if (!hasPermission) {
      ElMessage.error('权限不足')
      next('/403')
      return
    }
  }
  
  // 检查角色权限
  if (to.meta?.roles) {
    const roles = Array.isArray(to.meta.roles) 
      ? to.meta.roles 
      : [to.meta.roles]
    
    const hasRole = roles.some((role: string) => 
      userStore.hasRole(role)
    )
    
    if (!hasRole) {
      ElMessage.error('角色权限不足')
      next('/403')
      return
    }
  }
  
  // 已登录用户不能访问登录注册页面
  if (userStore.isLoggedIn && ['/login', '/register'].includes(to.path)) {
    next('/')
    return
  }
  
  next()
}

// 后置路由守卫
const afterEachGuard = (
  to: RouteLocationNormalized,
  from: RouteLocationNormalized
) => {
  // 隐藏loading
  hidePageLoading()
  
  // 记录路由跳转日志（开发环境）
  if (import.meta.env.DEV) {
    console.log(`Route changed: ${from.path} -> ${to.path}`)
  }
}

// 路由错误处理
const onErrorGuard = (error: Error, to: RouteLocationNormalized) => {
  hidePageLoading()
  
  console.error('Route error:', error)
  ElMessage.error('页面加载失败')
  
  // 记录错误
  if (import.meta.env.PROD) {
    // 在生产环境中可以发送错误到监控服务
    console.error('Route navigation error:', {
      error: error.message,
      stack: error.stack,
      route: to.path,
      timestamp: new Date().toISOString()
    })
  }
}

// 获取面包屑导航
const getBreadcrumbs = (route: RouteLocationNormalized): Array<{ name: string; path?: string }> => {
  const breadcrumbs: Array<{ name: string; path?: string }> = []
  const matched = route.matched.filter(item => item.meta && item.meta.title)
  
  // 添加首页
  if (route.path !== '/dashboard' && matched.length > 0) {
    breadcrumbs.push({
      name: '首页',
      path: '/dashboard'
    })
  }
  
  matched.forEach((item, index) => {
    const isLast = index === matched.length - 1
    breadcrumbs.push({
      name: item.meta!.title as string,
      path: isLast ? undefined : item.path
    })
  })
  
  return breadcrumbs
}

// 设置路由守卫
export const setupRouterGuards = (router: Router) => {
  // 前置守卫
  router.beforeEach(beforeEachGuard)
  
  // 后置守卫
  router.afterEach(afterEachGuard)
  
  // 错误处理
  router.onError(onErrorGuard)
  
  // 全局解析守卫（在beforeEach和组件内守卫之后调用）
  router.beforeResolve(async (to, from, next) => {
    const userStore = useUserStore()
    
    // 在需要认证的页面，验证token有效性
    if (to.meta?.requiresAuth && userStore.isLoggedIn) {
      try {
        // 定期验证token（每10分钟验证一次）
        const lastValidation = localStorage.getItem('last_token_validation')
        const now = Date.now()
        const validationInterval = 10 * 60 * 1000 // 10分钟
        
        if (!lastValidation || now - parseInt(lastValidation) > validationInterval) {
          const isValid = await userStore.validateToken()
          if (!isValid) {
            ElMessage.error('登录状态已过期，请重新登录')
            next({
              path: '/login',
              query: { redirect: to.fullPath }
            })
            return
          }
          localStorage.setItem('last_token_validation', now.toString())
        }
      } catch (error) {
        console.error('Token validation error:', error)
        // 验证失败不阻止路由跳转，让用户继续使用
      }
    }
    
    next()
  })
}

// 导航失败处理
export const handleNavigationFailure = (failure: any) => {
  if (failure) {
    console.error('Navigation failure:', failure)
    
    // 根据失败原因进行不同处理
    switch (failure.type) {
      case 'aborted':
        // 导航被中止
        break
      case 'cancelled':
        // 导航被取消
        break
      case 'duplicated':
        // 重复导航
        break
      default:
        ElMessage.error('页面跳转失败')
    }
  }
}

// 动态路由添加
export const addDynamicRoutes = (router: Router, routes: any[]) => {
  routes.forEach(route => {
    router.addRoute(route)
  })
}

// 移除动态路由
export const removeDynamicRoutes = (router: Router, routeNames: string[]) => {
  routeNames.forEach(name => {
    if (router.hasRoute(name)) {
      router.removeRoute(name)
    }
  })
}

// 权限检查助手函数
export const checkPermission = (permission: string | string[]): boolean => {
  const userStore = useUserStore()
  
  if (Array.isArray(permission)) {
    return permission.some(p => userStore.hasPermission(p))
  }
  
  return userStore.hasPermission(permission)
}

// 角色检查助手函数
export const checkRole = (role: string | string[]): boolean => {
  const userStore = useUserStore()
  
  if (Array.isArray(role)) {
    return role.some(r => userStore.hasRole(r))
  }
  
  return userStore.hasRole(role)
}