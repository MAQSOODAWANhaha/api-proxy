<template>
  <div class="system-info-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>系统信息</h2>
          <el-button type="primary" @click="refreshSystemInfo" :loading="loading">
            <el-icon><Refresh /></el-icon>
            刷新
          </el-button>
        </div>
      </template>
      
      <div class="system-info-content">
        <el-skeleton :loading="loading" animated>
          <template #template>
            <el-skeleton-item variant="h1" style="width: 240px; margin-bottom: 20px;" />
            <el-skeleton-item variant="text" style="width: 100%; margin-bottom: 10px;" />
            <el-skeleton-item variant="text" style="width: 80%; margin-bottom: 10px;" />
            <el-skeleton-item variant="text" style="width: 60%;" />
          </template>
          
          <template #default>
            <el-descriptions title="系统基本信息" :column="2" border>
              <el-descriptions-item label="系统版本">{{ systemInfo?.version || 'N/A' }}</el-descriptions-item>
              <el-descriptions-item label="构建时间">{{ systemInfo?.build_time || 'N/A' }}</el-descriptions-item>
              <el-descriptions-item label="运行时间">{{ systemInfo?.uptime || 'N/A' }}</el-descriptions-item>
              <el-descriptions-item label="Rust版本">{{ systemInfo?.rust_version || 'N/A' }}</el-descriptions-item>
              <el-descriptions-item label="操作系统">{{ systemInfo?.os_info || 'N/A' }}</el-descriptions-item>
              <el-descriptions-item label="CPU架构">{{ systemInfo?.arch || 'N/A' }}</el-descriptions-item>
            </el-descriptions>
          </template>
        </el-skeleton>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { SystemAPI } from '@/api'
import type { SystemInfo } from '@/types'

const loading = ref(false)
const systemInfo = ref<SystemInfo | null>(null)

const fetchSystemInfo = async () => {
  try {
    loading.value = true
    systemInfo.value = await SystemAPI.getSystemInfo()
  } catch (error: any) {
    ElMessage.error(error.message || '获取系统信息失败')
    console.error('Failed to fetch system info:', error)
  } finally {
    loading.value = false
  }
}

const refreshSystemInfo = () => {
  fetchSystemInfo()
}

onMounted(() => {
  fetchSystemInfo()
})
</script>

<style scoped>
.system-info-view {
  height: 100%;
}

.page-card {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-header h2 {
  margin: 0;
  color: #333;
}

.system-info-content {
  min-height: 400px;
}
</style>