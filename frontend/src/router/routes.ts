import type { RouteRecordRaw } from 'vue-router'

export const routes: Array<RouteRecordRaw> = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/login/index.vue'),
  },
  {
    path: '/',
    name: 'Layout',
    component: () => import('@/layouts/index.vue'),
    redirect: '/dashboard',
    children: [
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/dashboard/index.vue'),
        meta: { title: 'Dashboard', icon: 'el-icon-odometer' },
      },
      {
        path: 'api-keys',
        name: 'ApiKeys',
        meta: { title: 'API Keys', icon: 'el-icon-key' },
        children: [
          {
            path: 'provider',
            name: 'ProviderKeys',
            component: () => import('@/views/api-keys/provider/index.vue'),
            meta: { title: 'Provider Keys' },
          },
          {
            path: 'service',
            name: 'ServiceKeys',
            component: () => import('@/views/api-keys/service/index.vue'),
            meta: { title: 'Service Keys' },
          },
        ],
      },
      {
        path: 'statistics',
        name: 'Statistics',
        meta: { title: 'Statistics', icon: 'el-icon-data-line' },
        children: [
          {
            path: 'requests',
            name: 'RequestLogs',
            component: () => import('@/views/statistics/requests/index.vue'),
            meta: { title: 'Request Logs' },
          },
          {
            path: 'daily',
            name: 'DailyStats',
            component: () => import('@/views/statistics/daily/index.vue'),
            meta: { title: 'Daily Stats' },
          },
        ],
      },
      {
        path: 'health',
        name: 'Health',
        component: () => import('@/views/health/index.vue'),
        meta: { title: 'Health Check', icon: 'el-icon-first-aid-kit' },
      },
      {
        path: 'user-center',
        name: 'UserCenter',
        component: () => import('@/views/user-center/index.vue'),
        meta: { title: 'User Center', icon: 'el-icon-user' },
      },
    ],
  },
]
