<template>
  <div class="login-wrapper">
    <!-- 语言选择器 -->
    <div class="language-selector-container">
      <LanguageSelector size="small" variant="button" />
    </div>
    
    <div class="login-container">
      <div class="login-left">
        <div class="logo-area">
          <img src="@/assets/logo.svg" alt="Logo" />
          <h1>{{ $t('login.subtitle') }}</h1>
          <p>{{ $t('login.welcome') }}</p>
        </div>
      </div>
      <div class="login-right">
        <el-card class="login-card">
          <template #header>
            <div class="card-header">
              <h2>{{ $t('login.title') }}</h2>
            </div>
          </template>
          <el-form ref="loginFormRef" :model="loginForm" size="large">
            <el-form-item prop="username">
              <el-input v-model="loginForm.username" :placeholder="$t('login.username')" />
            </el-form-item>
            <el-form-item prop="password">
              <el-input
                v-model="loginForm.password"
                type="password"
                :placeholder="$t('login.password')"
                show-password
                @keyup.enter="handleLogin"
              />
            </el-form-item>
            <el-form-item>
              <el-button
                type="primary"
                class="login-btn"
                :loading="loading"
                @click="handleLogin"
              >
                {{ $t('login.loginBtn') }}
              </el-button>
            </el-form-item>
          </el-form>
        </el-card>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { login } from '@/api/auth'
import { ElMessage } from 'element-plus'
import { LanguageSelector } from '@/components/ui'
import { useI18n } from '@/locales'

const router = useRouter()
const userStore = useUserStore()
const { t } = useI18n()
const loading = ref(false)

const loginForm = reactive({
  username: 'admin',
  password: 'admin123',
})

const handleLogin = async () => {
  loading.value = true
  try {
    const response = await login(loginForm)
    // Store the actual JWT token from the API response
    userStore.setToken(response.data.data.token)
    ElMessage.success(t('login.loginSuccess'))
    router.push('/')
  } catch (error: any) {
    console.error('Login failed:', error)
    // Handle different error scenarios
    if (error.response?.status === 401) {
      ElMessage.error(t('login.invalidCredentials'))
    } else if (error.response?.status === 400) {
      ElMessage.error(t('login.usernameRequired'))
    } else {
      ElMessage.error(t('login.loginFailed'))
    }
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login-wrapper {
  height: 100vh;
  width: 100vw;
  display: flex;
  justify-content: center;
  align-items: center;
  background-image: url('@/assets/background.svg');
  background-size: cover;
  position: relative;
}

.language-selector-container {
  position: absolute;
  top: var(--spacing-6);
  right: var(--spacing-6);
  z-index: 10;
}
.login-container {
  display: flex;
  width: 900px;
  height: 550px;
  background: #fff;
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0,0,0,0.1);
  overflow: hidden;
}
.login-left {
  width: 50%;
  background: linear-gradient(135deg, #4a90e2, #50e3c2);
  color: white;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  padding: 40px;
  text-align: center;
}
.logo-area img {
  width: 80px;
  height: 80px;
  margin-bottom: 20px;
}
.logo-area h1 {
  font-size: 28px;
  margin-bottom: 10px;
}
.logo-area p {
  font-size: 16px;
  opacity: 0.8;
}
.login-right {
  width: 50%;
  display: flex;
  justify-content: center;
  align-items: center;
}
.login-card {
  width: 350px;
  border: none;
  box-shadow: none;
}
.card-header h2 {
  margin: 0;
  font-size: 24px;
  text-align: center;
  color: #333;
}
.login-btn {
  width: 100%;
}
</style>
