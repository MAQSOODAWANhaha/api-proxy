/**
 * App.tsx
 * 应用路由入口：提供主题（next-themes）、HashRouter、受保护路由与主布局。
 */
import { HashRouter, Routes, Route, Navigate } from 'react-router'
import { ThemeProvider } from 'next-themes'
import { Toaster } from 'sonner'

/** 布局与路由守卫 */
import DashboardLayout from './layouts/DashboardLayout'
import ProtectedRoute from './components/ProtectedRoute'

/** 各页面 */
import DashboardPage from './pages/dashboard/Dashboard'
import ApiUserKeysPage from './pages/api/ApiUserKeys'
import ProviderKeysPage from './pages/api/ProviderKeys'
import LogsPage from './pages/Logs'
import UsersPage from './pages/Users'
import SettingsPage from './pages/Settings'
import ProfilePage from './pages/Profile'
import LoginPage from './pages/Login'

/**
 * 应用根组件
 */
export default function App() {
  return (
    <ThemeProvider attribute="class" defaultTheme="light" enableSystem={false}>
      <HashRouter>
        <Routes>
          {/* 默认重定向到仪表板 */}
          <Route path="/" element={<Navigate to="/dashboard" replace />} />

          {/* 登录页（公开） */}
          <Route path="/login" element={<LoginPage />} />

          {/* 受保护区域：登录后访问 */}
          <Route element={<ProtectedRoute />}>
            <Route element={<DashboardLayout />}>
              <Route path="/dashboard" element={<DashboardPage />} />
              <Route path="/api" element={<ApiUserKeysPage />} />
              <Route path="/api/user-keys" element={<ApiUserKeysPage />} />
              <Route path="/api/provider-keys" element={<ProviderKeysPage />} />
              {/* 统计分析页已移除 */}
              <Route path="/logs" element={<LogsPage />} />
              <Route path="/users" element={<UsersPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/profile" element={<ProfilePage />} />
            </Route>
          </Route>

          {/* 兜底重定向 */}
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </HashRouter>
      <Toaster richColors />
    </ThemeProvider>
  )
}
