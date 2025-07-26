import { createApp } from 'vue'
import { createPinia } from 'pinia'
import router from './router'
import i18n from './locales'
import { validateConfig, appConfig } from '@/config'
import App from './App.vue'

// Import global styles
import './styles/globals.css'
import './styles/index.css'
import './assets/main.css'

// Initialize theme system
import { themeManager } from './styles/theme'

// 验证配置
validateConfig()

// 初始化主题系统
themeManager

// 设置应用标题
document.title = appConfig.title

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.use(i18n)

app.mount('#app')