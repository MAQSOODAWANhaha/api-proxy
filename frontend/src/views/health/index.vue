<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ $t('menu.healthCheck') }}</span>
          <el-button type="primary" @click="fetchHealthStatus" :loading="loading" icon-class="el-icon-refresh">
            <el-icon><Refresh /></el-icon>
            刷新
          </el-button>
        </div>
      </template>

      <el-table v-loading="loading" :data="tableData" style="width: 100%">
        <el-table-column prop="name" label="密钥名称" />
        <el-table-column prop="provider" label="服务商" />
        <el-table-column prop="isHealthy" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.isHealthy ? 'success' : 'danger'">
              {{ row.isHealthy ? '健康' : '不健康' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="responseTime" label="响应时间 (ms)" />
        <el-table-column prop="lastSuccess" label="上次成功" width="180" />
        <el-table-column prop="lastFailure" label="上次失败" width="180" />
        <el-table-column prop="errorMessage" label="错误信息" />
      </el-table>
    </el-card>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { getHealthStatuses, type HealthStatus } from '@/api/health'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'

const loading = ref(true)
const tableData = ref<HealthStatus[]>([])

const fetchHealthStatus = async () => {
  loading.value = true
  try {
    const response = await getHealthStatuses()
    tableData.value = response.data
  } catch (error) {
    ElMessage.error('获取健康状态失败')
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  fetchHealthStatus()
})
</script>

<style scoped>
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
</style>
