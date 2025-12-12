/**
 * App.tsx
 * 应用路由入口：提供主题（next-themes）、HashRouter、受保护路由与主布局。
 */
import { HashRouter, Routes, Route, Navigate } from 'react-router'
import { ThemeProvider } from 'next-themes'
import { Toaster } from 'sonner'
import { useTimezoneInit } from './store/timezone'

/** 布局与路由守卫 */
import DashboardLayout from './layouts/DashboardLayout'
import ProtectedRoute from './components/ProtectedRoute'

/** 各页面 */
import DashboardPage from './pages/dashboard/Dashboard'
import ApiUserKeysPage from './pages/api/ApiUserKeys'
import ProviderKeysPage from './pages/api/ProviderKeys'
import ProvidersPage from './pages/Providers'
import LogsPage from './pages/Logs'
import UsersPage from './pages/Users'
import SettingsPage from './pages/Settings'
import ProfilePage from './pages/Profile'
import LoginPage from './pages/Login'
import OAuthCallbackPage from './pages/OAuthCallbackPage'
import StatsStandalonePage from './pages/stats/StatsStandalonePage'

/**
 * 应用根组件
 */
export default function App() {
  // 初始化时区设置
  useTimezoneInit()

  return (
    <ThemeProvider attribute="class" defaultTheme="light" enableSystem={false}>
      <HashRouter>
        <Routes>
          {/* 默认展示统计页面 */}
          <Route path="/" element={<Navigate to="/stats" replace />} />

          {/* 登录页（公开） */}
          <Route path="/login" element={<LoginPage />} />

          {/* OAuth回调页（公开） */}
          <Route path="/auth/callback" element={<OAuthCallbackPage />} />

          {/* 独立统计页面（公开） */}
          <Route path="/stats" element={<StatsStandalonePage />} />

          {/* 受保护区域：登录后访问 */}
          <Route element={<ProtectedRoute />}>
            <Route element={<DashboardLayout />}>
              <Route path="/dashboard" element={<DashboardPage />} />
              <Route path="/api" element={<ApiUserKeysPage />} />
              <Route path="/api/user-keys" element={<ApiUserKeysPage />} />
              <Route path="/api/provider-keys" element={<ProviderKeysPage />} />
              <Route path="/providers" element={<ProvidersPage />} />
              {/* 统计分析页已移除 */}
              <Route path="/logs" element={<LogsPage />} />
              <Route path="/users" element={<UsersPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/profile" element={<ProfilePage />} />
            </Route>
          </Route>

          {/* 兜底重定向 */}
          <Route path="*" element={<Navigate to="/stats" replace />} />
        </Routes>
      </HashRouter>
      <Toaster richColors />
    </ThemeProvider>
  )
}
