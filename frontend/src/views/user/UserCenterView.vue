<template>
  <div class="user-center-view">
    <div class="container">
      <!-- 页面标题 -->
      <div class="page-header">
        <h2>用户中心</h2>
        <p class="page-description">管理您的账户安全设置</p>
      </div>

      <!-- 修改密码卡片 -->
      <el-card class="password-card">
        <template #header>
          <div class="card-header">
            <h3>修改密码</h3>
            <el-icon class="header-icon"><Lock /></el-icon>
          </div>
        </template>
        
        <el-form
          ref="passwordFormRef"
          :model="passwordForm"
          :rules="passwordRules"
          label-width="120px"
          @submit.prevent="changePassword"
          class="password-form"
        >
          <el-form-item label="当前密码" prop="currentPassword">
            <el-input 
              v-model="passwordForm.currentPassword" 
              type="password" 
              placeholder="请输入当前密码"
              show-password
              size="large"
              clearable
            />
          </el-form-item>
          
          <el-form-item label="新密码" prop="newPassword">
            <el-input 
              v-model="passwordForm.newPassword" 
              type="password" 
              placeholder="请输入新密码"
              show-password
              size="large"
              clearable
            />
            <div class="password-tips">
              <p class="tip-title">密码要求：</p>
              <ul class="tip-list">
                <li :class="{ valid: passwordChecks.length }">至少8位字符</li>
                <li :class="{ valid: passwordChecks.lowercase }">包含小写字母</li>
                <li :class="{ valid: passwordChecks.uppercase }">包含大写字母</li>
                <li :class="{ valid: passwordChecks.number }">包含数字</li>
                <li :class="{ valid: passwordChecks.special }">包含特殊字符</li>
              </ul>
            </div>
          </el-form-item>
          
          <el-form-item label="确认新密码" prop="confirmPassword">
            <el-input 
              v-model="passwordForm.confirmPassword" 
              type="password" 
              placeholder="请再次输入新密码确认"
              show-password
              size="large"
              clearable
            />
          </el-form-item>
          
          <el-form-item class="submit-item">
            <el-button 
              type="primary" 
              @click="changePassword" 
              :loading="passwordLoading"
              size="large"
              class="submit-btn"
            >
              <el-icon><Key /></el-icon>
              修改密码
            </el-button>
            <el-button 
              @click="resetForm" 
              size="large"
              class="reset-btn"
            >
              <el-icon><Refresh /></el-icon>
              重置表单
            </el-button>
          </el-form-item>
        </el-form>
      </el-card>

      <!-- 安全提示卡片 -->
      <el-card class="security-tips-card">
        <template #header>
          <div class="card-header">
            <h3>安全提示</h3>
            <el-icon class="header-icon"><Warning /></el-icon>
          </div>
        </template>
        
        <div class="tips-content">
          <div class="tip-item">
            <el-icon class="tip-icon success"><CircleCheck /></el-icon>
            <div class="tip-text">
              <h4>定期更换密码</h4>
              <p>建议每3-6个月更换一次密码，提升账户安全性</p>
            </div>
          </div>
          
          <div class="tip-item">
            <el-icon class="tip-icon info"><InfoFilled /></el-icon>
            <div class="tip-text">
              <h4>使用强密码</h4>
              <p>密码应包含大小写字母、数字和特殊字符，避免使用个人信息</p>
            </div>
          </div>
          
          <div class="tip-item">
            <el-icon class="tip-icon warning"><WarningFilled /></el-icon>
            <div class="tip-text">
              <h4>妥善保管密码</h4>
              <p>不要与他人分享密码，不要在不安全的地方保存密码</p>
            </div>
          </div>
        </div>
      </el-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { ElMessage, type FormInstance, type FormRules } from 'element-plus'
import {
  Lock, Key, Refresh, Warning, CircleCheck, InfoFilled, WarningFilled
} from '@element-plus/icons-vue'
import { UserAPI } from '@/api'
import { useAppStore } from '@/stores'

const appStore = useAppStore()

// 状态
const passwordLoading = ref(false)

// 表单引用
const passwordFormRef = ref<FormInstance>()

// 密码修改表单
const passwordForm = reactive({
  currentPassword: '',
  newPassword: '',
  confirmPassword: ''
})

// 密码强度检查
const passwordChecks = computed(() => {
  const password = passwordForm.newPassword
  return {
    length: password.length >= 8,
    lowercase: /[a-z]/.test(password),
    uppercase: /[A-Z]/.test(password),
    number: /\d/.test(password),
    special: /[@$!%*?&]/.test(password)
  }
})

// 密码强度等级
const passwordStrength = computed(() => {
  const checks = Object.values(passwordChecks.value)
  const validCount = checks.filter(Boolean).length
  
  if (validCount <= 1) return { level: 'weak', text: '弱', color: '#f56c6c' }
  if (validCount <= 3) return { level: 'medium', text: '中', color: '#e6a23c' }
  if (validCount <= 4) return { level: 'strong', text: '强', color: '#67c23a' }
  return { level: 'very-strong', text: '很强', color: '#409eff' }
})

// 表单验证规则
const passwordRules: FormRules = {
  currentPassword: [
    { required: true, message: '请输入当前密码', trigger: 'blur' }
  ],
  newPassword: [
    { required: true, message: '请输入新密码', trigger: 'blur' },
    { min: 8, message: '密码长度至少8位', trigger: 'blur' },
    { 
      validator: (rule, value, callback) => {
        const checks = passwordChecks.value
        if (!checks.lowercase || !checks.uppercase || !checks.number || !checks.special) {
          callback(new Error('密码必须包含大小写字母、数字和特殊字符'))
        } else {
          callback()
        }
      }, 
      trigger: 'blur' 
    }
  ],
  confirmPassword: [
    { required: true, message: '请确认新密码', trigger: 'blur' },
    {
      validator: (rule, value, callback) => {
        if (value !== passwordForm.newPassword) {
          callback(new Error('两次密码输入不一致'))
        } else {
          callback()
        }
      },
      trigger: 'blur'
    }
  ]
}

// 修改密码
const changePassword = async () => {
  if (!passwordFormRef.value) return
  
  try {
    const isValid = await passwordFormRef.value.validate()
    if (!isValid) return
    
    passwordLoading.value = true
    
    const response = await UserAPI.changePassword({
      current_password: passwordForm.currentPassword,
      new_password: passwordForm.newPassword
    })
    
    if (response.success) {
      ElMessage.success('密码修改成功！')
      resetForm()
    } else {
      ElMessage.error(response.message || '密码修改失败')
    }
  } catch (error: any) {
    ElMessage.error(error.message || '密码修改失败')
  } finally {
    passwordLoading.value = false
  }
}

// 重置表单
const resetForm = () => {
  if (passwordFormRef.value) {
    passwordFormRef.value.resetFields()
  }
  Object.assign(passwordForm, {
    currentPassword: '',
    newPassword: '',
    confirmPassword: ''
  })
}

// 监听新密码变化，自动验证确认密码
watch(
  () => passwordForm.newPassword,
  () => {
    if (passwordForm.confirmPassword && passwordFormRef.value) {
      passwordFormRef.value.validateField('confirmPassword')
    }
  }
)

// 生命周期
appStore.setPageTitle('用户中心')
</script>

<style scoped>
.user-center-view {
  min-height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  padding: 40px 20px;
}

.container {
  max-width: 600px;
  margin: 0 auto;
}

/* 页面标题 */
.page-header {
  text-align: center;
  margin-bottom: 40px;
  color: white;
}

.page-header h2 {
  font-size: 32px;
  font-weight: 600;
  margin: 0 0 12px 0;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.page-description {
  font-size: 16px;
  opacity: 0.9;
  margin: 0;
}

/* 卡片样式 */
.password-card,
.security-tips-card {
  margin-bottom: 24px;
  border-radius: 16px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  background: rgba(255, 255, 255, 0.95);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.card-header h3 {
  margin: 0;
  color: #333;
  font-size: 18px;
  font-weight: 600;
}

.header-icon {
  font-size: 20px;
  color: #409eff;
}

/* 表单样式 */
.password-form {
  padding: 8px 0;
}

.password-tips {
  margin-top: 12px;
  padding: 16px;
  background: #f8f9fa;
  border-radius: 8px;
  border-left: 4px solid #409eff;
}

.tip-title {
  margin: 0 0 8px 0;
  font-size: 14px;
  font-weight: 600;
  color: #333;
}

.tip-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.tip-list li {
  padding: 4px 0;
  font-size: 13px;
  color: #666;
  display: flex;
  align-items: center;
  gap: 8px;
}

.tip-list li::before {
  content: '○';
  color: #ddd;
  font-weight: bold;
}

.tip-list li.valid {
  color: #67c23a;
}

.tip-list li.valid::before {
  content: '●';
  color: #67c23a;
}

.submit-item {
  margin-top: 32px;
  text-align: center;
}

.submit-btn {
  min-width: 140px;
  margin-right: 16px;
}

.reset-btn {
  min-width: 120px;
}

/* 安全提示样式 */
.tips-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.tip-item {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  padding: 16px;
  background: #f8f9fa;
  border-radius: 12px;
  transition: all 0.3s ease;
}

.tip-item:hover {
  background: #f0f2f5;
  transform: translateY(-2px);
}

.tip-icon {
  font-size: 24px;
  margin-top: 2px;
  flex-shrink: 0;
}

.tip-icon.success {
  color: #67c23a;
}

.tip-icon.info {
  color: #409eff;
}

.tip-icon.warning {
  color: #e6a23c;
}

.tip-text h4 {
  margin: 0 0 8px 0;
  font-size: 16px;
  font-weight: 600;
  color: #333;
}

.tip-text p {
  margin: 0;
  font-size: 14px;
  color: #666;
  line-height: 1.5;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .user-center-view {
    padding: 20px 16px;
  }
  
  .container {
    max-width: 100%;
  }
  
  .page-header h2 {
    font-size: 28px;
  }
  
  .page-description {
    font-size: 14px;
  }
  
  .password-form {
    padding: 4px 0;
  }
  
  .submit-item {
    margin-top: 24px;
  }
  
  .submit-btn,
  .reset-btn {
    width: 100%;
    margin: 8px 0;
  }
  
  .tip-item {
    padding: 12px;
  }
  
  .tip-text h4 {
    font-size: 15px;
  }
  
  .tip-text p {
    font-size: 13px;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-card__header) {
  padding: 20px 24px;
  border-bottom: 1px solid #f0f0f0;
  background: rgba(255, 255, 255, 0.8);
  border-radius: 16px 16px 0 0;
}

:deep(.el-card__body) {
  padding: 24px;
}

:deep(.el-form-item) {
  margin-bottom: 24px;
}

:deep(.el-form-item__label) {
  color: #333;
  font-weight: 500;
  font-size: 15px;
}

:deep(.el-input__wrapper) {
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transition: all 0.3s ease;
}

:deep(.el-input__wrapper:hover) {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

:deep(.el-input.is-focus .el-input__wrapper) {
  box-shadow: 0 4px 12px rgba(64, 158, 255, 0.3);
}

:deep(.el-button) {
  border-radius: 8px;
  font-weight: 500;
  transition: all 0.3s ease;
}

:deep(.el-button:hover) {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

:deep(.el-button--primary) {
  background: linear-gradient(45deg, #409eff, #667eea);
  border: none;
}

:deep(.el-button--primary:hover) {
  background: linear-gradient(45deg, #66b1ff, #7c8ceb);
}
</style>