import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useUserStore = defineStore('user', () => {
  const token = ref(localStorage.getItem('token') || '')
  const lang = ref(localStorage.getItem('lang') || 'en')

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

  return { token, lang, setToken, removeToken, setLang }
})