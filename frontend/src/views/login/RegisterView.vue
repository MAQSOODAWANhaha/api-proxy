<template>
  <div class="register-container">
    <div class="register-wrapper">
      <!-- 左侧背景 -->
      <div class="register-banner">
        <div class="banner-content">
          <h1 class="banner-title">加入我们</h1>
          <p class="banner-subtitle">开始您的AI代理之旅</p>
          <div class="banner-features">
            <div class="feature-item">
              <el-icon><Star /></el-icon>
              <span>免费注册</span>
            </div>
            <div class="feature-item">
              <el-icon><Lightning /></el-icon>
              <span>快速部署</span>
            </div>
            <div class="feature-item">
              <el-icon><Trophy /></el-icon>
              <span>专业服务</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 右侧注册表单 -->
      <div class="register-form-wrapper">
        <div class="register-form">
          <div class="form-header">
            <h2>创建新账号</h2>
            <p>请填写以下信息完成注册</p>
          </div>

          <el-form
            ref="registerFormRef"
            :model="registerForm"
            :rules="registerRules"
            @keyup.enter="handleRegister"
            size="large"
          >
            <el-form-item prop="username">
              <el-input
                v-model="registerForm.username"
                placeholder="请输入用户名"
                prefix-icon="User"
                clearable
                :disabled="loading"
              />
            </el-form-item>

            <el-form-item prop="email">
              <el-input
                v-model="registerForm.email"
                type="email"
                placeholder="请输入邮箱地址"
                prefix-icon="Message"
                clearable
                :disabled="loading"
              />
            </el-form-item>

            <el-form-item prop="password">
              <el-input
                v-model="registerForm.password"
                type="password"
                placeholder="请输入密码"
                prefix-icon="Lock"
                show-password
                clearable
                :disabled="loading"
              />
            </el-form-item>

            <el-form-item prop="confirmPassword">
              <el-input
                v-model="registerForm.confirmPassword"
                type="password"
                placeholder="请确认密码"
                prefix-icon="Lock"
                show-password
                clearable
                :disabled="loading"
              />
            </el-form-item>

            <el-form-item prop="agreed">
              <el-checkbox v-model="registerForm.agreed" :disabled="loading">
                我已阅读并同意
                <el-link type="primary" @click="showTerms">《服务条款》</el-link>
                和
                <el-link type="primary" @click="showPrivacy">《隐私政策》</el-link>
              </el-checkbox>
            </el-form-item>

            <el-form-item>
              <el-button
                type="primary"
                size="large"
                :loading="loading"
                @click="handleRegister"
                class="register-button"
              >
                {{ loading ? '注册中...' : '立即注册' }}
              </el-button>
            </el-form-item>

            <el-form-item>
              <div class="login-link">
                <span>已有账号？</span>
                <el-link type="primary" @click="$router.push('/login')">
                  立即登录
                </el-link>
              </div>
            </el-form-item>
          </el-form>
        </div>
      </div>
    </div>

    <!-- 服务条款对话框 -->
    <el-dialog
      v-model="termsVisible"
      title="服务条款"
      width="80%"
      :max-width="600"
    >
      <div class="terms-content">
        <h3>1. 服务说明</h3>
        <p>本平台为企业级AI服务代理平台，为用户提供多种AI服务的统一接入和管理功能。</p>
        
        <h3>2. 用户责任</h3>
        <p>用户应合法合规使用本服务，不得利用本服务进行任何违法违规活动。</p>
        
        <h3>3. 隐私保护</h3>
        <p>我们严格保护用户隐私，不会泄露用户的个人信息和业务数据。</p>
        
        <h3>4. 服务条款修改</h3>
        <p>本服务条款可能会根据业务发展需要进行修改，修改后将在平台公布。</p>
      </div>
      <template #footer>
        <el-button @click="termsVisible = false">关闭</el-button>
      </template>
    </el-dialog>

    <!-- 隐私政策对话框 -->
    <el-dialog
      v-model="privacyVisible"
      title="隐私政策"
      width="80%"
      :max-width="600"
    >
      <div class="privacy-content">
        <h3>1. 信息收集</h3>
        <p>我们仅收集为提供服务所必需的用户信息，包括账号信息、使用数据等。</p>
        
        <h3>2. 信息使用</h3>
        <p>收集的信息仅用于提供和改进服务，不会用于其他商业目的。</p>
        
        <h3>3. 信息保护</h3>
        <p>我们采用行业标准的安全措施保护用户信息安全。</p>
        
        <h3>4. 信息共享</h3>
        <p>未经用户同意，我们不会与第三方共享用户的个人信息。</p>
      </div>
      <template #footer>
        <el-button @click="privacyVisible = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, type FormInstance, type FormRules } from 'element-plus'
import { User, Lock, Message, Star, Lightning, Trophy } from '@element-plus/icons-vue'
import { useUserStore, useAppStore } from '@/stores'
import type { RegisterRequest } from '@/types'

const router = useRouter()
const userStore = useUserStore()
const appStore = useAppStore()

// 表单引用
const registerFormRef = ref<FormInstance>()

// 加载状态
const loading = ref(false)

// 对话框状态
const termsVisible = ref(false)
const privacyVisible = ref(false)

// 注册表单数据
const registerForm = reactive<RegisterRequest & { 
  confirmPassword: string
  agreed: boolean 
}>({
  username: '',
  email: '',
  password: '',
  confirmPassword: '',
  agreed: false
})

// 确认密码验证
const validateConfirmPassword = (rule: any, value: string, callback: any) => {
  if (value === '') {
    callback(new Error('请确认密码'))
  } else if (value !== registerForm.password) {
    callback(new Error('两次输入的密码不一致'))
  } else {
    callback()
  }
}

// 同意条款验证
const validateAgreed = (rule: any, value: boolean, callback: any) => {
  if (!value) {
    callback(new Error('请阅读并同意服务条款和隐私政策'))
  } else {
    callback()
  }
}

// 表单验证规则
const registerRules: FormRules = {
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { min: 3, max: 20, message: '用户名长度应为3-20个字符', trigger: 'blur' },
    { pattern: /^[a-zA-Z0-9_]+$/, message: '用户名只能包含字母、数字和下划线', trigger: 'blur' }
  ],
  email: [
    { required: true, message: '请输入邮箱地址', trigger: 'blur' },
    { type: 'email', message: '请输入正确的邮箱地址', trigger: 'blur' }
  ],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    { min: 6, max: 20, message: '密码长度应为6-20个字符', trigger: 'blur' },
    { pattern: /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)/, message: '密码必须包含大小写字母和数字', trigger: 'blur' }
  ],
  confirmPassword: [
    { required: true, validator: validateConfirmPassword, trigger: 'blur' }
  ],
  agreed: [
    { required: true, validator: validateAgreed, trigger: 'change' }
  ]
}

// 处理注册
const handleRegister = async () => {
  if (!registerFormRef.value) return
  
  try {
    const isValid = await registerFormRef.value.validate()
    if (!isValid) return
    
    loading.value = true
    
    // 准备注册数据
    const registerData: RegisterRequest = {
      username: registerForm.username,
      email: registerForm.email,
      password: registerForm.password
    }
    
    const success = await userStore.register(registerData)
    
    if (success) {
      ElMessage.success('注册成功！请登录您的账号')
      // 注册成功后跳转到登录页面
      await router.push('/login')
    }
  } catch (error: any) {
    console.error('Register error:', error)
    ElMessage.error(error.message || '注册失败，请重试')
  } finally {
    loading.value = false
  }
}

// 显示服务条款
const showTerms = () => {
  termsVisible.value = true
}

// 显示隐私政策
const showPrivacy = () => {
  privacyVisible.value = true
}

// 页面挂载时的处理
onMounted(() => {
  // 设置页面标题
  appStore.setPageTitle('用户注册')
  
  // 如果已经登录，直接跳转到首页
  if (userStore.isLoggedIn) {
    router.push('/')
  }
})
</script>

<style scoped>
.register-container {
  min-height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.register-wrapper {
  width: 100%;
  max-width: 1000px;
  background: white;
  border-radius: 20px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.1);
  overflow: hidden;
  display: flex;
  min-height: 700px;
}

.register-banner {
  flex: 1;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  padding: 60px 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  position: relative;
}

.register-banner::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: url('data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><pattern id="grain" width="100" height="100" patternUnits="userSpaceOnUse"><circle cx="50" cy="50" r="1" fill="white" opacity="0.1"/></pattern></defs><rect width="100" height="100" fill="url(%23grain)"/></svg>');
  opacity: 0.1;
}

.banner-content {
  position: relative;
  z-index: 1;
  text-align: center;
}

.banner-title {
  font-size: 48px;
  font-weight: bold;
  margin-bottom: 20px;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

.banner-subtitle {
  font-size: 18px;
  margin-bottom: 40px;
  opacity: 0.9;
}

.banner-features {
  display: flex;
  flex-direction: column;
  gap: 20px;
  align-items: center;
}

.feature-item {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 16px;
  opacity: 0.9;
}

.feature-item .el-icon {
  font-size: 20px;
}

.register-form-wrapper {
  flex: 1;
  padding: 60px 40px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.register-form {
  width: 100%;
  max-width: 400px;
}

.form-header {
  text-align: center;
  margin-bottom: 40px;
}

.form-header h2 {
  font-size: 28px;
  color: #333;
  margin-bottom: 10px;
  font-weight: 600;
}

.form-header p {
  color: #666;
  font-size: 14px;
}

.register-button {
  width: 100%;
  height: 50px;
  font-size: 16px;
  font-weight: 600;
}

.login-link {
  text-align: center;
  color: #666;
  font-size: 14px;
}

.login-link span {
  margin-right: 8px;
}

.terms-content,
.privacy-content {
  max-height: 400px;
  overflow-y: auto;
  line-height: 1.6;
}

.terms-content h3,
.privacy-content h3 {
  color: #333;
  margin: 20px 0 10px 0;
  font-size: 16px;
}

.terms-content p,
.privacy-content p {
  color: #666;
  margin-bottom: 15px;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .register-wrapper {
    flex-direction: column;
    max-width: 400px;
  }
  
  .register-banner {
    padding: 40px 20px;
    min-height: 300px;
  }
  
  .banner-title {
    font-size: 32px;
  }
  
  .banner-subtitle {
    font-size: 16px;
  }
  
  .banner-features {
    flex-direction: row;
    justify-content: space-around;
    flex-wrap: wrap;
  }
  
  .register-form-wrapper {
    padding: 40px 20px;
  }
}

@media (max-width: 480px) {
  .register-container {
    padding: 10px;
  }
  
  .register-banner {
    padding: 30px 15px;
  }
  
  .banner-title {
    font-size: 24px;
  }
  
  .register-form-wrapper {
    padding: 30px 15px;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-form-item) {
  margin-bottom: 24px;
}

:deep(.el-input__wrapper) {
  border-radius: 8px;
  box-shadow: 0 0 0 1px #dcdfe6 inset;
  transition: all 0.3s;
}

:deep(.el-input__wrapper:hover) {
  box-shadow: 0 0 0 1px #c0c4cc inset;
}

:deep(.el-input__wrapper.is-focus) {
  box-shadow: 0 0 0 1px #409eff inset;
}

:deep(.el-button--primary) {
  border-radius: 8px;
  transition: all 0.3s;
}

:deep(.el-button--primary:hover) {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(64, 158, 255, 0.4);
}

:deep(.el-checkbox__input.is-checked .el-checkbox__inner) {
  background-color: #409eff;
  border-color: #409eff;
}

:deep(.el-dialog__body) {
  padding: 20px 25px 10px 25px;
}
</style>