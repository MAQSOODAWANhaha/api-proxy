// Vue Router 配置

import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { setupRouterGuards } from './guards'
import { useUserStore } from '@/stores'

// 定义路由
const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/login/LoginView.vue'),
    meta: {
      title: '用户登录',
      requiresAuth: false,
      hideInMenu: true
    }
  },
  {
    path: '/register',
    name: 'Register',
    component: () => import('@/views/login/RegisterView.vue'),
    meta: {
      title: '用户注册',
      requiresAuth: false,
      hideInMenu: true
    }
  },
  {
    path: '/',
    component: () => import('@/layouts/MainLayout.vue'),
    redirect: '/dashboard',
    meta: {
      requiresAuth: true
    },
    children: [
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/dashboard/DashboardView.vue'),
        meta: {
          title: '仪表盘',
          icon: 'dashboard',
          requiresAuth: true,
          keepAlive: true
        }
      },
      {
        path: 'api-keys',
        name: 'ApiKeys',
        redirect: '/api-keys/provider',
        meta: {
          title: 'API密钥管理',
          icon: 'key',
          requiresAuth: true
        },
        children: [
          {
            path: 'provider',
            name: 'ProviderKeys',
            component: () => import('@/views/api-keys/ProviderKeysView.vue'),
            meta: {
              title: '服务商密钥池',
              requiresAuth: true,
              keepAlive: true
            }
          },
          {
            path: 'service',
            name: 'ServiceApis',
            component: () => import('@/views/api-keys/ServiceApisView.vue'),
            meta: {
              title: 'API服务管理',
              requiresAuth: true,
              keepAlive: true
            }
          }
        ]
      },
      {
        path: 'statistics',
        name: 'Statistics',
        redirect: '/statistics/overview',
        meta: {
          title: '统计分析',
          icon: 'chart',
          requiresAuth: true
        },
        children: [
          {
            path: 'overview',
            name: 'StatisticsOverview',
            component: () => import('@/views/statistics/OverviewView.vue'),
            meta: {
              title: '数据概览',
              requiresAuth: true,
              keepAlive: true
            }
          },
          {
            path: 'logs',
            name: 'RequestLogs',
            component: () => import('@/views/statistics/RequestLogsView.vue'),
            meta: {
              title: '请求日志',
              requiresAuth: true,
              keepAlive: false
            }
          },
          {
            path: 'analytics',
            name: 'Analytics',
            component: () => import('@/views/statistics/AnalyticsView.vue'),
            meta: {
              title: '深度分析',
              requiresAuth: true,
              keepAlive: true
            }
          }
        ]
      },
      {
        path: 'health',
        name: 'Health',
        component: () => import('@/views/health/HealthMonitorView.vue'),
        meta: {
          title: '健康监控',
          icon: 'monitor',
          requiresAuth: true,
          keepAlive: true
        }
      },
      {
        path: 'system',
        name: 'System',
        redirect: '/system/info',
        meta: {
          title: '系统管理',
          icon: 'setting',
          requiresAuth: true,
          requiresAdmin: true
        },
        children: [
          {
            path: 'info',
            name: 'SystemInfo',
            component: () => import('@/views/system/SystemInfoView.vue'),
            meta: {
              title: '系统信息',
              requiresAuth: true,
              requiresAdmin: true
            }
          },
          {
            path: 'config',
            name: 'SystemConfig',
            component: () => import('@/views/system/ConfigurationView.vue'),
            meta: {
              title: '系统配置',
              requiresAuth: true,
              requiresAdmin: true
            }
          },
          {
            path: 'logs',
            name: 'SystemLogs',
            component: () => import('@/views/system/LogsView.vue'),
            meta: {
              title: '系统日志',
              requiresAuth: true,
              requiresAdmin: true
            }
          }
        ]
      },
      {
        path: 'user-center',
        name: 'UserCenter',
        component: () => import('@/views/user/UserCenterView.vue'),
        meta: {
          title: '用户中心',
          icon: 'user',
          requiresAuth: true,
          hideInMenu: true,
          keepAlive: false
        }
      }
    ]
  },
  {
    path: '/403',
    name: 'Forbidden',
    component: () => import('@/views/error/403View.vue'),
    meta: {
      title: '权限不足',
      hideInMenu: true
    }
  },
  {
    path: '/404',
    name: 'NotFound',
    component: () => import('@/views/error/404View.vue'),
    meta: {
      title: '页面不存在',
      hideInMenu: true
    }
  },
  {
    path: '/500',
    name: 'ServerError',
    component: () => import('@/views/error/500View.vue'),
    meta: {
      title: '服务器错误',
      hideInMenu: true
    }
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/404'
  }
]

// 创建路由实例
const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
  scrollBehavior(to, from, savedPosition) {
    if (savedPosition) {
      return savedPosition
    } else {
      return { top: 0 }
    }
  }
})

// 设置路由守卫
setupRouterGuards(router)

// 路由工具函数
export const getRouteTitle = (route: any): string => {
  return route.meta?.title || route.name || '未知页面'
}

// 获取面包屑导航
export const getBreadcrumbs = (route: any): Array<{ name: string; path?: string }> => {
  const breadcrumbs: Array<{ name: string; path?: string }> = []
  const matched = route.matched.filter((item: any) => item.meta && item.meta.title)
  
  matched.forEach((item: any) => {
    breadcrumbs.push({
      name: item.meta.title,
      path: item.path === route.path ? undefined : item.path
    })
  })
  
  return breadcrumbs
}

// 检查路由权限
export const checkRoutePermission = (route: any): boolean => {
  const userStore = useUserStore()
  
  // 不需要认证的页面
  if (!route.meta?.requiresAuth) {
    return true
  }
  
  // 需要登录
  if (!userStore.isLoggedIn) {
    return false
  }
  
  // 需要管理员权限
  if (route.meta?.requiresAdmin && !userStore.isAdmin) {
    return false
  }
  
  // 检查特定权限
  if (route.meta?.permissions) {
    const permissions = Array.isArray(route.meta.permissions) 
      ? route.meta.permissions 
      : [route.meta.permissions]
    
    return permissions.some((permission: string) => userStore.hasPermission(permission))
  }
  
  return true
}

// 获取菜单路由（排除隐藏的路由）
export const getMenuRoutes = (routes: RouteRecordRaw[]): RouteRecordRaw[] => {
  return routes.filter(route => {
    if (route.meta?.hideInMenu) {
      return false
    }
    
    if (route.children) {
      route.children = getMenuRoutes(route.children)
      return route.children.length > 0
    }
    
    return true
  })
}

// 查找路由
export const findRoute = (routes: RouteRecordRaw[], name: string): RouteRecordRaw | null => {
  for (const route of routes) {
    if (route.name === name) {
      return route
    }
    
    if (route.children) {
      const found = findRoute(route.children, name)
      if (found) {
        return found
      }
    }
  }
  
  return null
}

export { routes }
export default router