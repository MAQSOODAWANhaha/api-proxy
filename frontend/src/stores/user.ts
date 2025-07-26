import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useUserStore = defineStore('user', () => {
  const token = ref(localStorage.getItem('token') || '')
  const lang = ref(localStorage.getItem('lang') || 'zh')
  const theme = ref(localStorage.getItem('theme') || 'light')

  function setToken(newToken: string) {
    token.value = newToken
    localStorage.setItem('token', newToken)
  }

  function removeToken() {
    token.value = ''
    localStorage.removeItem('token')
  }

  function setLang(newLang: 'en' | 'zh') {
    lang.value = newLang
    localStorage.setItem('lang', newLang)
  }

  function setTheme(newTheme: string) {
    theme.value = newTheme
    localStorage.setItem('theme', newTheme)
  }

  function logout() {
    removeToken()
    // 清除其他用户相关数据
    localStorage.removeItem('userInfo')
    localStorage.removeItem('permissions')
  }

  return { 
    token, 
    lang, 
    theme,
    setToken, 
    removeToken, 
    setLang, 
    setTheme,
    logout 
  }
})