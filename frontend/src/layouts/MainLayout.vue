<template>
  <div class="main-layout">
    <!-- 侧边栏 -->
    <aside 
      class="sidebar"
      :class="{ 'sidebar-collapsed': appStore.sidebarCollapsed }"
    >
      <div class="sidebar-header">
        <div class="logo">
          <el-icon v-if="!appStore.sidebarCollapsed" class="logo-icon">
            <Cpu />
          </el-icon>
          <span v-if="!appStore.sidebarCollapsed" class="logo-text">AI代理平台</span>
        </div>
      </div>
      
      <nav class="sidebar-nav">
        <el-menu
          :default-active="activeMenu"
          :collapse="appStore.sidebarCollapsed"
          :unique-opened="true"
          router
          class="sidebar-menu"
        >
          <!-- 统计分析（首页） -->
          <el-menu-item index="/statistics">
            <el-icon><DataAnalysis /></el-icon>
            <template #title>统计分析</template>
          </el-menu-item>

          <!-- API密钥管理 -->
          <el-sub-menu index="/api-keys">
            <template #title>
              <el-icon><Key /></el-icon>
              <span>API密钥管理</span>
            </template>
            <el-menu-item index="/api-keys/provider">
              <el-icon><Collection /></el-icon>
              <template #title>服务商密钥池</template>
            </el-menu-item>
            <el-menu-item index="/api-keys/service">
              <el-icon><Setting /></el-icon>
              <template #title>API服务管理</template>
            </el-menu-item>
          </el-sub-menu>

          
          <!-- 日志查询 -->
          <el-menu-item index="/logs">
            <el-icon><Document /></el-icon>
            <template #title>日志查询</template>
          </el-menu-item>

          <!-- 健康监控 -->
          <el-menu-item index="/health">
            <el-icon><Monitor /></el-icon>
            <template #title>健康监控</template>
          </el-menu-item>

          <!-- 系统管理 -->
          <el-sub-menu v-if="userStore.isAdmin" index="/system">
            <template #title>
              <el-icon><Tools /></el-icon>
              <span>系统管理</span>
            </template>
            <el-menu-item index="/system/info">
              <el-icon><InfoFilled /></el-icon>
              <template #title>系统信息</template>
            </el-menu-item>
            <el-menu-item index="/system/config">
              <el-icon><Setting /></el-icon>
              <template #title>系统配置</template>
            </el-menu-item>
            <el-menu-item index="/system/logs">
              <el-icon><Files /></el-icon>
              <template #title>系统日志</template>
            </el-menu-item>
          </el-sub-menu>
        </el-menu>
      </nav>
    </aside>

    <!-- 主内容区域 -->
    <div class="main-content">
      <!-- 顶部导航栏 -->
      <header class="header">
        <!-- 左侧操作 -->
        <div class="header-left">
          <el-button
            type="text"
            @click="appStore.toggleSidebar"
            class="sidebar-toggle"
          >
            <el-icon><Fold v-if="!appStore.sidebarCollapsed" /><Expand v-else /></el-icon>
          </el-button>

          <!-- 面包屑导航 -->
          <el-breadcrumb separator="/" class="breadcrumb">
            <el-breadcrumb-item
              v-for="crumb in appStore.breadcrumbs"
              :key="crumb.name"
              :to="crumb.path"
            >
              {{ crumb.name }}
            </el-breadcrumb-item>
          </el-breadcrumb>
        </div>

        <!-- 右侧操作 -->
        <div class="header-right">
          <!-- 刷新按钮 -->
          <el-tooltip content="刷新页面" placement="bottom">
            <el-button type="text" @click="refreshPage">
              <el-icon><Refresh /></el-icon>
            </el-button>
          </el-tooltip>

          <!-- 全屏按钮 -->
          <el-tooltip content="全屏显示" placement="bottom">
            <el-button type="text" @click="toggleFullscreen">
              <el-icon><FullScreen /></el-icon>
            </el-button>
          </el-tooltip>

          <!-- 通知中心 -->
          <el-dropdown trigger="click" class="notification-dropdown">
            <el-button type="text" class="notification-button">
              <el-badge :value="notificationCount" :hidden="notificationCount === 0">
                <el-icon><Bell /></el-icon>
              </el-badge>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <div class="notification-header">
                  <span>通知中心</span>
                  <el-button type="text" size="small" @click="clearAllNotifications">
                    全部清除
                  </el-button>
                </div>
                <div class="notification-list">
                  <div
                    v-for="notification in notifications"
                    :key="notification.id"
                    class="notification-item"
                    @click="markAsRead(notification.id)"
                  >
                    <div class="notification-content">
                      <div class="notification-title">{{ notification.title }}</div>
                      <div class="notification-time">{{ formatTime(notification.time) }}</div>
                    </div>
                    <el-tag
                      v-if="!notification.read"
                      type="danger"
                      size="small"
                      effect="plain"
                    >
                      新
                    </el-tag>
                  </div>
                </div>
                <el-dropdown-item v-if="notifications.length === 0" disabled>
                  暂无通知
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <!-- 用户菜单 -->
          <el-dropdown trigger="click" class="user-dropdown">
            <div class="user-info">
              <el-avatar :size="32" class="user-avatar">
                {{ userStore.userAvatar }}
              </el-avatar>
              <span v-if="!appStore.isMobile" class="username">
                {{ userStore.userName }}
              </span>
              <el-icon class="dropdown-icon"><ArrowDown /></el-icon>
            </div>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="goToUserCenter">
                  <el-icon><User /></el-icon>
                  用户中心
                </el-dropdown-item>
                <el-dropdown-item divided @click="handleLogout">
                  <el-icon><SwitchButton /></el-icon>
                  退出登录
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </header>

      <!-- 页面内容 -->
      <main class="page-content">
        <router-view v-slot="{ Component, route }">
          <transition name="page-fade" mode="out-in">
            <keep-alive v-if="route.meta?.keepAlive">
              <component :is="Component" :key="route.fullPath" />
            </keep-alive>
            <component v-else :is="Component" :key="route.fullPath" />
          </transition>
        </router-view>
      </main>
    </div>

    <!-- 全局加载遮罩 -->
    <div v-if="appStore.isLoading" class="global-loading">
      <el-loading
        element-loading-text="加载中..."
        element-loading-background="rgba(0, 0, 0, 0.8)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Cpu, Key, Collection, Setting, DataAnalysis, 
  Document, Monitor, Tools, InfoFilled, Files,
  Fold, Expand, Refresh, FullScreen, Bell, User, ArrowDown,
  SwitchButton
} from '@element-plus/icons-vue'
import { useUserStore, useAppStore } from '@/stores'

const router = useRouter()
const route = useRoute()
const userStore = useUserStore()
const appStore = useAppStore()

// 通知相关
const notifications = ref([
  {
    id: 1,
    title: 'API密钥即将过期',
    time: new Date(Date.now() - 5 * 60 * 1000),
    read: false
  },
  {
    id: 2,
    title: '系统维护通知',
    time: new Date(Date.now() - 30 * 60 * 1000),
    read: true
  },
  {
    id: 3,
    title: '新用户注册',
    time: new Date(Date.now() - 2 * 60 * 60 * 1000),
    read: false
  }
])

// 计算属性
const activeMenu = computed(() => {
  const path = route.path
  // 处理子路由的激活状态
  if (path.startsWith('/api-keys')) {
    return path
  } else if (path.startsWith('/statistics')) {
    return path
  } else if (path.startsWith('/system')) {
    return path
  }
  return path
})

const notificationCount = computed(() => {
  return notifications.value.filter(n => !n.read).length
})

// 方法
const refreshPage = () => {
  window.location.reload()
}

const toggleFullscreen = () => {
  if (!document.fullscreenElement) {
    document.documentElement.requestFullscreen()
  } else {
    document.exitFullscreen()
  }
}

const goToUserCenter = () => {
  router.push('/user-center')
}

const handleLogout = async () => {
  try {
    await ElMessageBox.confirm(
      '确定要退出登录吗？',
      '退出确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    await userStore.logout()
    router.push('/login')
  } catch {
    // 用户取消操作
  }
}

const markAsRead = (id: number) => {
  const notification = notifications.value.find(n => n.id === id)
  if (notification) {
    notification.read = true
  }
}

const clearAllNotifications = () => {
  notifications.value = []
}

const formatTime = (time: Date) => {
  const now = new Date()
  const diff = now.getTime() - time.getTime()
  const minutes = Math.floor(diff / (1000 * 60))
  const hours = Math.floor(diff / (1000 * 60 * 60))
  const days = Math.floor(diff / (1000 * 60 * 60 * 24))

  if (minutes < 1) {
    return '刚刚'
  } else if (minutes < 60) {
    return `${minutes}分钟前`
  } else if (hours < 24) {
    return `${hours}小时前`
  } else {
    return `${days}天前`
  }
}

// 监听窗口大小变化
const handleResize = () => {
  appStore.updateDeviceType()
}

// 生命周期
onMounted(() => {
  window.addEventListener('resize', handleResize)
  
  // 移动端自动折叠侧边栏
  if (appStore.isMobile) {
    appStore.setSidebarCollapsed(true)
  }
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
})
</script>

<style scoped>
.main-layout {
  display: flex;
  height: 100vh;
  background: #f5f5f5;
}

/* 侧边栏样式 */
.sidebar {
  width: 250px;
  background: #001529;
  transition: width 0.3s ease;
  box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  z-index: 1000;
}

.sidebar-collapsed {
  width: 64px;
}

.sidebar-header {
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid #1f1f1f;
}

.logo {
  display: flex;
  align-items: center;
  gap: 12px;
  color: white;
  font-size: 18px;
  font-weight: bold;
}

.logo-icon {
  font-size: 24px;
  color: #1890ff;
}

.sidebar-nav {
  height: calc(100vh - 60px);
  overflow-y: auto;
}

.sidebar-menu {
  border: none;
  background: transparent;
}

/* 主内容区域 */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 顶部导航栏 */
.header {
  height: 60px;
  background: white;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  z-index: 999;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
}

.sidebar-toggle {
  font-size: 18px;
  color: #666;
}

.breadcrumb {
  font-size: 14px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.notification-dropdown,
.user-dropdown {
  cursor: pointer;
}

.notification-button {
  font-size: 18px;
  color: #666;
}

.user-info {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.username {
  font-size: 14px;
  color: #333;
}

.dropdown-icon {
  font-size: 12px;
  color: #666;
  transition: transform 0.3s;
}

.user-dropdown:hover .dropdown-icon {
  transform: rotate(180deg);
}

/* 通知相关样式 */
.notification-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid #f0f0f0;
  font-weight: 500;
}

.notification-list {
  max-height: 300px;
  overflow-y: auto;
}

.notification-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid #f0f0f0;
  cursor: pointer;
  transition: background-color 0.3s;
}

.notification-item:hover {
  background-color: #f5f5f5;
}

.notification-content {
  flex: 1;
}

.notification-title {
  font-size: 14px;
  color: #333;
  margin-bottom: 4px;
}

.notification-time {
  font-size: 12px;
  color: #999;
}

/* 页面内容 */
.page-content {
  flex: 1;
  padding: 24px;
  overflow-y: auto;
  background: #f5f5f5;
}

/* 全局加载遮罩 */
.global-loading {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 9999;
}

/* 页面切换动画 */
.page-fade-enter-active,
.page-fade-leave-active {
  transition: opacity 0.3s ease;
}

.page-fade-enter-from,
.page-fade-leave-to {
  opacity: 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    z-index: 1001;
    transform: translateX(-100%);
    transition: transform 0.3s ease;
  }

  .sidebar:not(.sidebar-collapsed) {
    transform: translateX(0);
  }

  .main-content {
    width: 100%;
    margin-left: 0;
  }

  .header-left .breadcrumb {
    display: none;
  }

  .username {
    display: none;
  }

  .page-content {
    padding: 16px;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-menu) {
  background-color: transparent;
}

:deep(.el-menu-item) {
  color: rgba(255, 255, 255, 0.65);
  border-right: none !important;
}

:deep(.el-menu-item:hover) {
  background-color: #1f1f1f;
  color: #1890ff;
}

:deep(.el-menu-item.is-active) {
  background-color: #1890ff;
  color: white;
}

:deep(.el-sub-menu__title) {
  color: rgba(255, 255, 255, 0.65);
  border-right: none !important;
}

:deep(.el-sub-menu__title:hover) {
  background-color: #1f1f1f;
  color: #1890ff;
}

:deep(.el-sub-menu .el-menu-item) {
  background-color: #000c17;
}

:deep(.el-breadcrumb__item:last-child .el-breadcrumb__inner) {
  color: #1890ff;
}

:deep(.el-dropdown-menu) {
  min-width: 200px;
}

:deep(.el-badge__content) {
  border: 1px solid white;
}
</style>