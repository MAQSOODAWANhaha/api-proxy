<template>
  <div class="page-container">
    <el-row :gutter="20">
      <el-col :span="8">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>个人信息</span>
            </div>
          </template>
          <div class="user-profile">
            <el-avatar :size="100" />
            <h2 class="username">{{ profile.username }}</h2>
            <p class="user-detail">
              <el-icon><Message /></el-icon>
              {{ profile.email }}
            </p>
            <p class="user-detail">
              <el-icon><Clock /></el-icon>
              上次登录: {{ profile.lastLogin }}
            </p>
          </div>
        </el-card>
      </el-col>
      <el-col :span="16">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>修改密码</span>
            </div>
          </template>
          <el-form
            ref="formRef"
            :model="passwordForm"
            :rules="passwordRules"
            label-width="100px"
            style="max-width: 400px"
          >
            <el-form-item label="旧密码" prop="oldPassword">
              <el-input v-model="passwordForm.oldPassword" type="password" show-password />
            </el-form-item>
            <el-form-item label="新密码" prop="newPassword">
              <el-input v-model="passwordForm.newPassword" type="password" show-password />
            </el-form-item>
            <el-form-item label="确认密码" prop="confirmPassword">
              <el-input v-model="passwordForm.confirmPassword" type="password" show-password />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="handleUpdatePassword">更新密码</el-button>
            </el-form-item>
          </el-form>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive } from 'vue'
import { getUserProfile, updatePassword } from '@/api/user'
import type { UserProfile } from '@/api/user'
import { ElMessage, type FormInstance, type FormRules } from 'element-plus'
import { Message, Clock } from '@element-plus/icons-vue'

// State
const profile = ref<UserProfile>({ username: '', email: '', lastLogin: '' })
const formRef = ref<FormInstance>()

const passwordForm = reactive({
  oldPassword: '',
  newPassword: '',
  confirmPassword: '',
})

// Password validation rules
const validatePass = (rule: any, value: any, callback: any) => {
  if (value === '') {
    callback(new Error('请输入新密码'))
  } else {
    if (passwordForm.confirmPassword !== '') {
      if (!formRef.value) return
      formRef.value.validateField('confirmPassword', () => null)
    }
    callback()
  }
}
const validatePass2 = (rule: any, value: any, callback: any) => {
  if (value === '') {
    callback(new Error('请再次输入密码'))
  } else if (value !== passwordForm.newPassword) {
    callback(new Error("两次输入不一致!"))
  } else {
    callback()
  }
}

const passwordRules = reactive<FormRules>({
  oldPassword: [{ required: true, message: '请输入旧密码', trigger: 'blur' }],
  newPassword: [{ required: true, validator: validatePass, trigger: 'blur' }],
  confirmPassword: [{ required: true, validator: validatePass2, trigger: 'blur' }],
})

// Methods
const fetchProfile = async () => {
  try {
    const response = await getUserProfile()
    profile.value = response.data
  } catch (error) {
    ElMessage.error('获取用户信息失败')
  }
}

const handleUpdatePassword = async () => {
  if (!formRef.value) return
  await formRef.value.validate(async (valid) => {
    if (valid) {
      try {
        await updatePassword({
          oldPassword: passwordForm.oldPassword,
          newPassword: passwordForm.newPassword,
        })
        ElMessage.success('密码更新成功')
        formRef.value?.resetFields()
      } catch (error) {
        ElMessage.error('密码更新失败')
      }
    }
  })
}

// Lifecycle
onMounted(() => {
  fetchProfile()
})
</script>

<style scoped>
.page-container {
  padding: 10px;
}
.card-header {
  font-weight: 600;
}
.user-profile {
  text-align: center;
}
.username {
  margin-top: 20px;
  font-size: 24px;
}
.user-detail {
  margin-top: 10px;
  color: #606266;
  display: flex;
  align-items: center;
  justify-content: center;
}
.user-detail .el-icon {
  margin-right: 8px;
}
</style>