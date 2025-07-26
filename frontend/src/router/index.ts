import { createRouter, createWebHistory } from 'vue-router'
import { routes } from './routes'
import { useUserStore } from '@/stores/user'
import { handleError } from '@/utils/error'

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 路由白名单（不需要认证的页面）
const whiteList = ['/login', '/error']

router.beforeEach((to, from, next) => {
  try {
    const userStore = useUserStore()
    
    // 检查路由是否需要认证
    const requiresAuth = to.meta?.requiresAuth !== false
    
    // 错误页面和登录页面直接放行
    if (to.path.startsWith('/error') || to.path === '/login') {
      next()
      return
    }
    
    if (userStore.token) {
      if (to.path === '/login') {
        next({ path: '/' })
      } else {
        next()
      }
    } else {
      if (!requiresAuth || whiteList.some(path => to.path.startsWith(path))) {
        next()
      } else {
        next(`/login?redirect=${encodeURIComponent(to.fullPath)}`)
      }
    }
  } catch (error) {
    console.error('路由守卫错误:', error)
    handleError(error, 'router-guard', {
      showNotification: false,
      showMessage: false,
      logError: true
    })
    
    // 路由守卫出错时，重定向到错误页面
    next('/error/500?message=' + encodeURIComponent('路由导航失败'))
  }
})

// 全局路由错误处理
router.onError((error) => {
  console.error('路由错误:', error)
  handleError(error, 'router-error', {
    showNotification: true,
    showMessage: false,
    logError: true
  })
})

export default router