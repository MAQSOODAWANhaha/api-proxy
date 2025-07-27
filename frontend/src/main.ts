import { createApp } from 'vue'
import { pinia } from '@/stores'
import router from '@/router'
import ElementPlus from 'element-plus'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import '@/assets/css/main.css'
import { initializeMockData } from '@/utils/mockData'

import App from './App.vue'

const app = createApp(App)

// 注册Element Plus图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

// 使用插件
app.use(pinia)
app.use(router)  
app.use(ElementPlus, {
  // Element Plus 全局配置
  size: 'default',
  zIndex: 3000,
})

// 全局错误处理
app.config.errorHandler = (err, vm, info) => {
  console.error('Vue Error:', err)
  console.error('Error Info:', info)
  
  // 在生产环境中，可以将错误发送到错误监控服务
  if (import.meta.env.PROD) {
    // TODO: 发送错误到监控服务
  }
}

// 全局警告处理
app.config.warnHandler = (msg, vm, trace) => {
  if (import.meta.env.DEV) {
    console.warn('Vue Warning:', msg)
    console.warn('Trace:', trace)
  }
}

// 初始化模拟数据检测并启动应用
initializeMockData().then(() => {
  app.mount('#app')
}).catch(error => {
  console.error('Failed to initialize app:', error)
  app.mount('#app')
})