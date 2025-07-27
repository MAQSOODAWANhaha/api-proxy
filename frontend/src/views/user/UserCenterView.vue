<template>
  <div class="user-center-view">
    <el-row :gutter="24">
      <!-- 个人信息卡片 -->
      <el-col :xs="24" :sm="24" :md="16" :lg="16">
        <el-card class="profile-card">
          <template #header>
            <div class="card-header">
              <h3>个人信息</h3>
              <el-button 
                v-if="!editingProfile" 
                type="text" 
                @click="startEditProfile"
              >
                <el-icon><Edit /></el-icon>
                编辑
              </el-button>
              <div v-else class="edit-actions">
                <el-button size="small" @click="cancelEditProfile">取消</el-button>
                <el-button size="small" type="primary" @click="saveProfile" :loading="profileLoading">
                  保存
                </el-button>
              </div>
            </div>
          </template>
          
          <div class="profile-content">
            <!-- 头像区域 -->
            <div class="avatar-section">
              <div class="avatar-container">
                <el-avatar 
                  :size="80" 
                  :src="userInfo.avatar" 
                  class="user-avatar"
                >
                  <el-icon><User /></el-icon>
                </el-avatar>
                <div v-if="editingProfile" class="avatar-upload">
                  <el-upload
                    class="avatar-uploader"
                    action="#"
                    :show-file-list="false"
                    :before-upload="beforeAvatarUpload"
                    :http-request="handleAvatarUpload"
                  >
                    <el-button size="small" type="text">
                      <el-icon><Camera /></el-icon>
                      更换头像
                    </el-button>
                  </el-upload>
                </div>
              </div>
            </div>
            
            <!-- 信息表单 -->
            <div class="info-section">
              <el-form
                ref="profileFormRef"
                :model="profileForm"
                :rules="profileRules"
                label-width="100px"
                :disabled="!editingProfile"
              >
                <el-form-item label="用户名" prop="username">
                  <el-input v-model="profileForm.username" disabled />
                </el-form-item>
                
                <el-form-item label="邮箱地址" prop="email">
                  <el-input v-model="profileForm.email" />
                </el-form-item>
                
                <el-form-item label="昵称" prop="nickname">
                  <el-input v-model="profileForm.nickname" placeholder="请输入昵称" />
                </el-form-item>
                
                <el-form-item label="手机号码" prop="phone">
                  <el-input v-model="profileForm.phone" placeholder="请输入手机号码" />
                </el-form-item>
                
                <el-form-item label="公司/组织">
                  <el-input v-model="profileForm.organization" placeholder="请输入公司或组织名称" />
                </el-form-item>
                
                <el-form-item label="个人简介">
                  <el-input 
                    v-model="profileForm.bio" 
                    type="textarea" 
                    :rows="3"
                    placeholder="请输入个人简介"
                  />
                </el-form-item>
              </el-form>
            </div>
          </div>
        </el-card>
      </el-col>
      
      <!-- 账户设置和统计信息 -->
      <el-col :xs="24" :sm="24" :md="8" :lg="8">
        <!-- 密码修改 -->
        <el-card class="password-card">
          <template #header>
            <h3>修改密码</h3>
          </template>
          
          <el-form
            ref="passwordFormRef"
            :model="passwordForm"
            :rules="passwordRules"
            label-width="100px"
            @submit.prevent="changePassword"
          >
            <el-form-item label="当前密码" prop="currentPassword">
              <el-input 
                v-model="passwordForm.currentPassword" 
                type="password" 
                placeholder="请输入当前密码"
                show-password
              />
            </el-form-item>
            
            <el-form-item label="新密码" prop="newPassword">
              <el-input 
                v-model="passwordForm.newPassword" 
                type="password" 
                placeholder="请输入新密码"
                show-password
              />
            </el-form-item>
            
            <el-form-item label="确认密码" prop="confirmPassword">
              <el-input 
                v-model="passwordForm.confirmPassword" 
                type="password" 
                placeholder="请再次输入新密码"
                show-password
              />
            </el-form-item>
            
            <el-form-item>
              <el-button 
                type="primary" 
                @click="changePassword" 
                :loading="passwordLoading"
                style="width: 100%;"
              >
                修改密码
              </el-button>
            </el-form-item>
          </el-form>
        </el-card>
        
        <!-- 账户统计 -->
        <el-card class="stats-card">
          <template #header>
            <h3>账户统计</h3>
          </template>
          
          <div class="stats-list">
            <div class="stats-item">
              <div class="stats-label">注册时间</div>
              <div class="stats-value">{{ formatTime(userInfo.created_at) }}</div>
            </div>
            
            <div class="stats-item">
              <div class="stats-label">最后登录</div>
              <div class="stats-value">{{ formatTime(userInfo.last_login_at) }}</div>
            </div>
            
            <div class="stats-item">
              <div class="stats-label">总请求数</div>
              <div class="stats-value">{{ formatNumber(userStats.total_requests) }}</div>
            </div>
            
            <div class="stats-item">
              <div class="stats-label">Token消耗</div>
              <div class="stats-value">{{ formatNumber(userStats.total_tokens) }}</div>
            </div>
            
            <div class="stats-item">
              <div class="stats-label">API密钥数</div>
              <div class="stats-value">{{ userStats.api_keys_count }}</div>
            </div>

            <div class="stats-item">
              <div class="stats-label">账户状态</div>
              <div class="stats-value">
                <el-tag :type="userInfo.is_active ? 'success' : 'danger'" size="small">
                  {{ userInfo.is_active ? '正常' : '禁用' }}
                </el-tag>
              </div>
            </div>
          </div>
        </el-card>
        
        <!-- 安全设置 -->
        <el-card class="security-card">
          <template #header>
            <h3>安全设置</h3>
          </template>
          
          <div class="security-list">
            <div class="security-item">
              <div class="security-info">
                <div class="security-title">两步验证</div>
                <div class="security-desc">为账户添加额外的安全保护</div>
              </div>
              <el-switch 
                v-model="securitySettings.two_factor_enabled"
                @change="toggleTwoFactor"
                :loading="twoFactorLoading"
              />
            </div>
            
            <div class="security-item">
              <div class="security-info">
                <div class="security-title">登录通知</div>
                <div class="security-desc">有新设备登录时发送邮件通知</div>
              </div>
              <el-switch 
                v-model="securitySettings.login_notification"
                @change="toggleLoginNotification"
                :loading="notificationLoading"
              />
            </div>
            
            <div class="security-item">
              <div class="security-info">
                <div class="security-title">API访问日志</div>
                <div class="security-desc">记录所有API访问活动</div>
              </div>
              <el-switch 
                v-model="securitySettings.api_logging"
                @change="toggleApiLogging"
                :loading="loggingLoading"
              />
            </div>
          </div>
          
          <div class="security-actions">
            <el-button @click="viewLoginHistory" style="width: 100%;">
              <el-icon><Clock /></el-icon>
              查看登录历史
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 登录历史对话框 -->
    <el-dialog
      v-model="loginHistoryVisible"
      title="登录历史"
      width="80%"
      :max-width="800"
    >
      <el-table
        :data="loginHistory"
        v-loading="historyLoading"
        stripe
        style="width: 100%"
      >
        <el-table-column prop="login_time" label="登录时间" width="180">
          <template #default="{ row }">
            {{ formatTime(row.login_time) }}
          </template>
        </el-table-column>
        
        <el-table-column prop="ip_address" label="IP地址" width="150" />
        
        <el-table-column prop="location" label="位置" width="150" />
        
        <el-table-column prop="device" label="设备" show-overflow-tooltip />
        
        <el-table-column prop="status" label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="row.status === 'success' ? 'success' : 'danger'" size="small">
              {{ row.status === 'success' ? '成功' : '失败' }}
            </el-tag>
          </template>
        </el-table-column>
      </el-table>
      
      <div class="pagination-wrapper">
        <el-pagination
          v-model:current-page="historyPagination.page"
          v-model:page-size="historyPagination.size"
          :page-sizes="[10, 20, 50]"
          :total="historyPagination.total"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleHistorySizeChange"
          @current-change="handleHistoryCurrentChange"
        />
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules, type UploadRequestOptions } from 'element-plus'
import {
  Edit, User, Camera, Clock
} from '@element-plus/icons-vue'
import { UserAPI } from '@/api'
import { useUserStore, useAppStore } from '@/stores'
import type { User as UserType } from '@/types'

const userStore = useUserStore()
const appStore = useAppStore()

// 状态
const editingProfile = ref(false)
const profileLoading = ref(false)
const passwordLoading = ref(false)
const twoFactorLoading = ref(false)
const notificationLoading = ref(false)
const loggingLoading = ref(false)
const historyLoading = ref(false)
const loginHistoryVisible = ref(false)

// 数据
const userInfo = ref<UserType>({} as UserType)
const userStats = ref<any>({})
const loginHistory = ref<any[]>([])

// 表单引用
const profileFormRef = ref<FormInstance>()
const passwordFormRef = ref<FormInstance>()

// 个人信息表单
const profileForm = reactive({
  username: '',
  email: '',
  nickname: '',
  phone: '',
  organization: '',
  bio: ''
})

const originalProfileForm = reactive({
  username: '',
  email: '',
  nickname: '',
  phone: '',
  organization: '',
  bio: ''
})

// 密码修改表单
const passwordForm = reactive({
  currentPassword: '',
  newPassword: '',
  confirmPassword: ''
})

// 安全设置
const securitySettings = reactive({
  two_factor_enabled: false,
  login_notification: true,
  api_logging: true
})

// 分页
const historyPagination = reactive({
  page: 1,
  size: 20,
  total: 0
})

// 表单验证规则
const profileRules: FormRules = {
  email: [
    { required: true, message: '请输入邮箱地址', trigger: 'blur' },
    { type: 'email', message: '请输入有效的邮箱地址', trigger: 'blur' }
  ],
  phone: [
    { pattern: /^1[3-9]\d{9}$/, message: '请输入有效的手机号码', trigger: 'blur' }
  ]
}

const passwordRules: FormRules = {
  currentPassword: [
    { required: true, message: '请输入当前密码', trigger: 'blur' }
  ],
  newPassword: [
    { required: true, message: '请输入新密码', trigger: 'blur' },
    { min: 8, message: '密码长度至少8位', trigger: 'blur' },
    { 
      pattern: /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]/, 
      message: '密码必须包含大小写字母、数字和特殊字符', 
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

// 获取用户信息和统计数据
const fetchUserData = async () => {
  try {
    const [profile, stats, security] = await Promise.all([
      UserAPI.getProfile(),
      UserAPI.getUserStats(),
      UserAPI.getSecuritySettings()
    ])
    
    userInfo.value = profile
    userStats.value = stats
    Object.assign(securitySettings, security)
    
    // 更新表单数据
    Object.assign(profileForm, {
      username: profile.username,
      email: profile.email,
      nickname: profile.nickname || '',
      phone: profile.phone || '',
      organization: profile.organization || '',
      bio: profile.bio || ''
    })
    
    Object.assign(originalProfileForm, profileForm)
  } catch (error: any) {
    ElMessage.error('获取用户信息失败')
    console.error('Failed to fetch user data:', error)
  }
}

// 开始编辑个人信息
const startEditProfile = () => {
  editingProfile.value = true
}

// 取消编辑
const cancelEditProfile = () => {
  editingProfile.value = false
  Object.assign(profileForm, originalProfileForm)
}

// 保存个人信息
const saveProfile = async () => {
  if (!profileFormRef.value) return
  
  try {
    const isValid = await profileFormRef.value.validate()
    if (!isValid) return
    
    profileLoading.value = true
    
    await UserAPI.updateProfile({
      email: profileForm.email,
      nickname: profileForm.nickname,
      phone: profileForm.phone,
      organization: profileForm.organization,
      bio: profileForm.bio
    })
    
    ElMessage.success('个人信息更新成功')
    editingProfile.value = false
    Object.assign(originalProfileForm, profileForm)
    
    // 更新用户信息
    await fetchUserData()
  } catch (error: any) {
    ElMessage.error(error.message || '更新失败')
  } finally {
    profileLoading.value = false
  }
}

// 修改密码
const changePassword = async () => {
  if (!passwordFormRef.value) return
  
  try {
    const isValid = await passwordFormRef.value.validate()
    if (!isValid) return
    
    passwordLoading.value = true
    
    await UserAPI.changePassword({
      current_password: passwordForm.currentPassword,
      new_password: passwordForm.newPassword
    })
    
    ElMessage.success('密码修改成功，请重新登录')
    
    // 重置表单
    passwordFormRef.value.resetFields()
    Object.assign(passwordForm, {
      currentPassword: '',
      newPassword: '',
      confirmPassword: ''
    })
    
    // 可以选择自动登出
    // userStore.logout()
  } catch (error: any) {
    ElMessage.error(error.message || '密码修改失败')
  } finally {
    passwordLoading.value = false
  }
}

// 头像上传前检查
const beforeAvatarUpload = (file: File) => {
  const isImage = file.type.startsWith('image/')
  const isLt2M = file.size / 1024 / 1024 < 2

  if (!isImage) {
    ElMessage.error('只能上传图片文件!')
    return false
  }
  if (!isLt2M) {
    ElMessage.error('图片大小不能超过 2MB!')
    return false
  }
  return true
}

// 处理头像上传
const handleAvatarUpload = async (options: UploadRequestOptions) => {
  const formData = new FormData()
  formData.append('avatar', options.file)
  
  try {
    const result = await UserAPI.uploadAvatar(formData)
    userInfo.value.avatar = result.avatar_url
    ElMessage.success('头像更新成功')
  } catch (error: any) {
    ElMessage.error(error.message || '头像上传失败')
  }
}

// 切换两步验证
const toggleTwoFactor = async (enabled: boolean) => {
  try {
    twoFactorLoading.value = true
    
    if (enabled) {
      // 启用两步验证需要额外确认
      const result = await ElMessageBox.confirm(
        '启用两步验证需要绑定手机号或邮箱，是否继续？',
        '确认启用',
        {
          confirmButtonText: '确认',
          cancelButtonText: '取消',
          type: 'warning'
        }
      )
      
      if (result === 'confirm') {
        await UserAPI.enableTwoFactor()
        ElMessage.success('两步验证已启用')
      } else {
        securitySettings.two_factor_enabled = false
      }
    } else {
      await UserAPI.disableTwoFactor()
      ElMessage.success('两步验证已关闭')
    }
  } catch (error: any) {
    securitySettings.two_factor_enabled = !enabled
    if (error !== 'cancel') {
      ElMessage.error(error.message || '设置失败')
    }
  } finally {
    twoFactorLoading.value = false
  }
}

// 切换登录通知
const toggleLoginNotification = async (enabled: boolean) => {
  try {
    notificationLoading.value = true
    await UserAPI.updateSecuritySettings({ login_notification: enabled })
    ElMessage.success(`登录通知已${enabled ? '开启' : '关闭'}`)
  } catch (error: any) {
    securitySettings.login_notification = !enabled
    ElMessage.error(error.message || '设置失败')
  } finally {
    notificationLoading.value = false
  }
}

// 切换API日志
const toggleApiLogging = async (enabled: boolean) => {
  try {
    loggingLoading.value = true
    await UserAPI.updateSecuritySettings({ api_logging: enabled })
    ElMessage.success(`API访问日志已${enabled ? '开启' : '关闭'}`)
  } catch (error: any) {
    securitySettings.api_logging = !enabled
    ElMessage.error(error.message || '设置失败')
  } finally {
    loggingLoading.value = false
  }
}

// 查看登录历史
const viewLoginHistory = async () => {
  try {
    loginHistoryVisible.value = true
    await fetchLoginHistory()
  } catch (error: any) {
    ElMessage.error('获取登录历史失败')
  }
}

// 获取登录历史
const fetchLoginHistory = async () => {
  try {
    historyLoading.value = true
    const response = await UserAPI.getLoginHistory({
      page: historyPagination.page,
      page_size: historyPagination.size
    })
    
    loginHistory.value = response.history
    historyPagination.total = response.pagination?.total || 0
  } catch (error: any) {
    ElMessage.error('获取登录历史失败')
  } finally {
    historyLoading.value = false
  }
}

// 分页处理
const handleHistorySizeChange = (size: number) => {
  historyPagination.size = size
  historyPagination.page = 1
  fetchLoginHistory()
}

const handleHistoryCurrentChange = (page: number) => {
  historyPagination.page = page
  fetchLoginHistory()
}

// 工具函数
const formatTime = (timestamp: string) => {
  if (!timestamp) return '-'
  return new Date(timestamp).toLocaleString('zh-CN')
}

const formatNumber = (num: number) => {
  if (!num) return 0
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

// 生命周期
onMounted(() => {
  appStore.setPageTitle('用户中心')
  fetchUserData()
})
</script>

<style scoped>
.user-center-view {
  height: 100%;
  padding: 24px;
  overflow-y: auto;
}

/* 卡片样式 */
.profile-card,
.password-card,
.stats-card,
.security-card {
  margin-bottom: 24px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-header h3 {
  margin: 0;
  color: #333;
  font-size: 16px;
  font-weight: 600;
}

.edit-actions {
  display: flex;
  gap: 8px;
}

/* 个人信息样式 */
.profile-content {
  display: flex;
  gap: 24px;
}

.avatar-section {
  flex-shrink: 0;
}

.avatar-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.user-avatar {
  border: 3px solid #f0f0f0;
}

.avatar-upload {
  text-align: center;
}

.avatar-uploader :deep(.el-upload) {
  border: none;
  background: none;
}

.info-section {
  flex: 1;
}

/* 统计卡片样式 */
.stats-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.stats-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.stats-item:last-child {
  border-bottom: none;
}

.stats-label {
  color: #666;
  font-size: 14px;
}

.stats-value {
  color: #333;
  font-weight: 500;
}

/* 安全设置样式 */
.security-list {
  margin-bottom: 20px;
}

.security-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 0;
  border-bottom: 1px solid #f0f0f0;
}

.security-item:last-child {
  border-bottom: none;
}

.security-info {
  flex: 1;
}

.security-title {
  color: #333;
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 4px;
}

.security-desc {
  color: #666;
  font-size: 12px;
}

.security-actions {
  margin-top: 16px;
}

/* 分页样式 */
.pagination-wrapper {
  margin-top: 20px;
  display: flex;
  justify-content: center;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .user-center-view {
    padding: 16px;
  }
  
  .profile-content {
    flex-direction: column;
    gap: 16px;
  }
  
  .avatar-section {
    text-align: center;
  }
  
  .stats-item,
  .security-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  
  .stats-value {
    align-self: flex-end;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-card__header) {
  padding: 16px 20px;
  border-bottom: 1px solid #f0f0f0;
}

:deep(.el-card__body) {
  padding: 20px;
}

:deep(.el-form-item) {
  margin-bottom: 20px;
}

:deep(.el-form-item__label) {
  color: #333;
  font-weight: 500;
}

:deep(.el-input[disabled] .el-input__inner) {
  background-color: #f8f9fa;
  color: #666;
}

:deep(.el-switch) {
  height: 20px;
}

:deep(.el-avatar) {
  font-size: 28px;
}
</style>