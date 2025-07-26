import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useConfigStore = defineStore('config', () => {
  const apiBaseUrl = ref(import.meta.env.VITE_API_BASE_URL || '/api')
  const appName = ref(import.meta.env.VITE_APP_NAME || 'AI服务代理管理平台')
  const version = ref(import.meta.env.VITE_APP_VERSION || '1.0.0')

  function getApiBaseUrl(): string {
    return apiBaseUrl.value
  }

  function setApiBaseUrl(url: string) {
    apiBaseUrl.value = url
  }

  function getAppName(): string {
    return appName.value
  }

  function getVersion(): string {
    return version.value
  }

  return { 
    apiBaseUrl, 
    appName, 
    version,
    getApiBaseUrl, 
    setApiBaseUrl, 
    getAppName, 
    getVersion 
  }
})