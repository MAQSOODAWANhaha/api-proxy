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
.layout-container {
  height: 100vh;
  width: 100vw;
}
.sidebar {
  background: #001529;
  transition: width 0.3s ease-in-out;
  display: flex;
  flex-direction: column;
}
.logo-container {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  height: 64px;
  box-sizing: border-box;
  flex-shrink: 0;
  overflow: hidden;
}
.logo-img {
  height: 32px;
  width: 32px;
  margin-right: 12px;
  flex-shrink: 0;
}
.logo-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: white;
  white-space: nowrap;
}
.menu-wrapper {
  flex-grow: 1;
  overflow-y: auto;
  &::-webkit-scrollbar { display: none; }
  -ms-overflow-style: none;
  scrollbar-width: none;
}
.el-menu-vertical:not(.el-menu--collapse) {
  width: 220px;
}
.el-menu {
  border-right: none;
  background: #001529;
}
.el-menu-item,
:deep(.el-sub-menu__title) {
  color: rgba(255, 255, 255, 0.65) !important;
}
.el-menu-item:hover,
:deep(.el-sub-menu__title:hover) {
  background-color: #000c17 !important;
  color: #fff !important;
}
.el-menu-item.is-active {
  background-color: var(--color-primary) !important;
  color: #fff !important;
}
:deep(.el-menu--inline) {
  background-color: #000c17 !important;
}
.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: #fff;
  padding: 0 24px;
  height: 64px;
  box-shadow: 0 1px 4px rgba(0,21,41,.08);
}
.collapse-icon {
  font-size: 22px;
  cursor: pointer;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 24px; /* Use gap for consistent spacing */
}
.action-item {
  cursor: pointer;
  display: flex;
  align-items: center;
  font-size: 20px;
}
.user-info {
  gap: 8px; /* Space between avatar and name */
  font-size: 14px;
}
.main-content {
  background-color: var(--bg-color);
  overflow-y: auto;
}
</style>