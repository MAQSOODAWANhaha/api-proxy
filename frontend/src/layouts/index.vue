<template>
  <el-container class="layout-container">
    <!-- Sidebar -->
    <el-aside :width="appStore.isSidebarCollapsed ? '64px' : '220px'" class="sidebar">
      <div class="logo-container">
        <img src="@/assets/logo.svg" alt="Logo" class="logo-img" />
        <h1 v-if="!appStore.isSidebarCollapsed" class="logo-title">AI Proxy</h1>
      </div>
      <div class="menu-wrapper">
        <el-menu
          :router="true"
          :default-active="$route.path"
          :collapse="appStore.isSidebarCollapsed"
          class="el-menu-vertical"
        >
          <el-menu-item index="/dashboard">
            <el-icon><Odometer /></el-icon>
            <template #title><span>{{ $t('menu.dashboard') }}</span></template>
          </el-menu-item>
          <el-sub-menu index="/api-keys">
            <template #title>
              <el-icon><Key /></el-icon>
              <span>{{ $t('menu.apiKeys') }}</span>
            </template>
            <el-menu-item index="/api-keys/provider">{{ $t('menu.providerKeys') }}</el-menu-item>
            <el-menu-item index="/api-keys/service">{{ $t('menu.serviceKeys') }}</el-menu-item>
          </el-sub-menu>
          <el-sub-menu index="/statistics">
            <template #title>
              <el-icon><DataLine /></el-icon>
              <span>{{ $t('menu.statistics') }}</span>
            </template>
            <el-menu-item index="/statistics/requests">{{ $t('menu.requestLogs') }}</el-menu-item>
            <el-menu-item index="/statistics/daily">{{ $t('menu.dailyStats') }}</el-menu-item>
          </el-sub-menu>
          <el-menu-item index="/health">
            <el-icon><FirstAidKit /></el-icon>
            <template #title><span>{{ $t('menu.healthCheck') }}</span></template>
          </el-menu-item>
          <el-menu-item index="/user-center">
            <el-icon><User /></el-icon>
            <template #title><span>{{ $t('menu.userCenter') }}</span></template>
          </el-menu-item>
        </el-menu>
      </div>
    </el-aside>

    <!-- Main Content -->
    <el-container class="content-wrapper">
      <el-header class="header">
        <div class="header-left">
          <el-icon class="collapse-icon" @click="appStore.toggleSidebar">
            <Expand v-if="appStore.isSidebarCollapsed" />
            <Fold v-else />
          </el-icon>
        </div>
        <div class="toolbar">
          <!-- 主题切换器 -->
          <ThemeToggle mode="dropdown" />
          
          <el-dropdown @command="handleLanguageChange">
            <span class="action-item">
              <el-icon><Switch /></el-icon>
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="en" :disabled="currentLanguage === 'en'">English</el-dropdown-item>
                <el-dropdown-item command="zh" :disabled="currentLanguage === 'zh'">中文</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          <el-dropdown @command="handleUserCommand">
            <span class="action-item user-info">
              <el-avatar :size="28" />
              <span>Tom</span>
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="profile">{{ $t('header.user') }}</el-dropdown-item>
                <el-dropdown-item command="logout" divided>{{ $t('header.logout') }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </el-header>

      <el-main class="main-content">
        <router-view />
      </el-main>
    </el-container>
  </el-container>
</template>

<script lang="ts" setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { useAppStore } from '@/stores/app'
import { logout } from '@/api/auth'
import { Odometer, Key, DataLine, FirstAidKit, User, Switch, Fold, Expand } from '@element-plus/icons-vue'
import ThemeToggle from '@/components/ui/ThemeToggle.vue'

const router = useRouter()
const userStore = useUserStore()
const appStore = useAppStore()
const currentLanguage = computed(() => userStore.lang)

const handleLanguageChange = (lang: 'en' | 'zh') => {
  userStore.setLang(lang)
}

const handleUserCommand = async (command: string) => {
  if (command === 'logout') {
    await logout()
    userStore.removeToken()
    router.push('/login')
  } else if (command === 'profile') {
    router.push('/user-center')
  }
}
</script>

<style scoped>
/* 使用设计系统变量 */
.layout-container {
  height: 100vh;
  width: 100vw;
  font-family: var(--font-family-sans);
}

.sidebar {
  background: var(--color-neutral-900);
  transition: width var(--transition-normal);
  display: flex;
  flex-direction: column;
  border-right: var(--border-width-1) solid var(--color-border-primary);
}

.theme-dark .sidebar {
  background: var(--color-neutral-800);
}

.logo-container {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-4);
  height: 64px;
  box-sizing: border-box;
  flex-shrink: 0;
  overflow: hidden;
  border-bottom: var(--border-width-1) solid var(--color-border-primary);
}

.logo-img {
  height: var(--spacing-8);
  width: var(--spacing-8);
  margin-right: var(--spacing-3);
  flex-shrink: 0;
}

.logo-title {
  margin: 0;
  font-size: var(--font-size-xl);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-inverse);
  white-space: nowrap;
}

.menu-wrapper {
  flex-grow: 1;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--color-border-secondary) transparent;
}

.menu-wrapper::-webkit-scrollbar {
  width: 4px;
}

.menu-wrapper::-webkit-scrollbar-track {
  background: transparent;
}

.menu-wrapper::-webkit-scrollbar-thumb {
  background-color: var(--color-border-secondary);
  border-radius: var(--border-radius-full);
}

.el-menu-vertical:not(.el-menu--collapse) {
  width: 220px;
}

:deep(.el-menu) {
  border-right: none;
  background: transparent;
}

:deep(.el-menu-item),
:deep(.el-sub-menu__title) {
  color: rgba(255, 255, 255, 0.75) !important;
  transition: all var(--transition-fast) !important;
  margin: var(--spacing-1);
  border-radius: var(--border-radius-md);
}

:deep(.el-menu-item:hover),
:deep(.el-sub-menu__title:hover) {
  background-color: var(--color-interactive-hover) !important;
  color: var(--color-text-inverse) !important;
  transform: translateX(2px);
}

:deep(.el-menu-item.is-active) {
  background-color: var(--color-brand-primary) !important;
  color: var(--color-text-inverse) !important;
  box-shadow: var(--box-shadow-sm);
}

:deep(.el-menu--inline) {
  background-color: var(--color-bg-tertiary) !important;
}

:deep(.el-sub-menu .el-menu-item) {
  background-color: transparent !important;
  color: rgba(255, 255, 255, 0.65) !important;
}

:deep(.el-sub-menu .el-menu-item:hover) {
  background-color: var(--color-interactive-hover) !important;
  color: var(--color-text-inverse) !important;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: var(--color-bg-elevated);
  padding: 0 var(--spacing-6);
  height: 64px;
  box-shadow: var(--box-shadow-sm);
  border-bottom: var(--border-width-1) solid var(--color-border-primary);
  transition: all var(--transition-normal);
}

.header-left {
  display: flex;
  align-items: center;
}

.collapse-icon {
  font-size: var(--font-size-xl);
  cursor: pointer;
  color: var(--color-text-secondary);
  transition: color var(--transition-fast);
  padding: var(--spacing-2);
  border-radius: var(--border-radius-md);
}

.collapse-icon:hover {
  color: var(--color-brand-primary);
  background-color: var(--color-interactive-hover);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
}

.action-item {
  cursor: pointer;
  display: flex;
  align-items: center;
  font-size: var(--font-size-lg);
  color: var(--color-text-secondary);
  transition: color var(--transition-fast);
  padding: var(--spacing-2);
  border-radius: var(--border-radius-md);
}

.action-item:hover {
  color: var(--color-brand-primary);
  background-color: var(--color-interactive-hover);
}

.user-info {
  gap: var(--spacing-2);
  font-size: var(--font-size-sm);
  color: var(--color-text-primary);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--border-radius-lg);
  border: var(--border-width-1) solid var(--color-border-primary);
  transition: all var(--transition-fast);
}

.user-info:hover {
  border-color: var(--color-brand-primary);
  box-shadow: var(--box-shadow-sm);
}

.main-content {
  background-color: var(--color-bg-secondary);
  overflow-y: auto;
  transition: background-color var(--transition-normal);
}

/* 响应式设计 */
@media (max-width: 768px) {
  .sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    z-index: var(--z-index-fixed);
  }
  
  .header {
    padding: 0 var(--spacing-4);
  }
  
  .toolbar {
    gap: var(--spacing-2);
  }
  
  .user-info span {
    display: none;
  }
}

/* 深色主题特殊样式 */
.theme-dark :deep(.el-menu-item),
.theme-dark :deep(.el-sub-menu__title) {
  color: var(--color-text-secondary) !important;
}

.theme-dark :deep(.el-menu-item:hover),
.theme-dark :deep(.el-sub-menu__title:hover) {
  color: var(--color-text-primary) !important;
}

.theme-dark :deep(.el-menu-item.is-active) {
  color: var(--color-text-inverse) !important;
}
</style>