<template>
  <div class="configuration-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>系统配置</h2>
          <div class="header-actions">
            <el-button @click="refreshConfig" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button type="primary" @click="saveConfig" :loading="saving">
              <el-icon><Check /></el-icon>
              保存配置
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="config-content">
        <el-alert
          title="配置修改注意事项"
          type="warning"
          description="修改系统配置可能影响系统运行，请谨慎操作。建议在修改前创建备份。"
          show-icon
          :closable="false"
          style="margin-bottom: 20px;"
        />
        
        <el-skeleton :loading="loading" animated>
          <template #template>
            <div v-for="i in 5" :key="i" style="margin-bottom: 20px;">
              <el-skeleton-item variant="text" style="width: 200px; margin-bottom: 10px;" />
              <el-skeleton-item variant="rect" style="width: 100%; height: 40px;" />
            </div>
          </template>
          
          <template #default>
            <div class="config-form">
              <div class="config-section">
                <h3>数据库配置</h3>
                <el-form :model="config" label-width="150px">
                  <el-form-item label="数据库路径">
                    <el-input v-model="config.database_url" placeholder="SQLite数据库文件路径" />
                  </el-form-item>
                  <el-form-item label="连接池大小">
                    <el-input-number v-model="config.max_connections" :min="1" :max="100" />
                  </el-form-item>
                </el-form>
              </div>
              
              <el-divider />
              
              <div class="config-section">
                <h3>Redis配置</h3>
                <el-form :model="config" label-width="150px">
                  <el-form-item label="Redis地址">
                    <el-input v-model="config.redis_url" placeholder="redis://localhost:6379" />
                  </el-form-item>
                  <el-form-item label="连接超时(秒)">
                    <el-input-number v-model="config.redis_timeout" :min="1" :max="300" />
                  </el-form-item>
                </el-form>
              </div>
              
              <el-divider />
              
              <div class="config-section">
                <h3>日志配置</h3>
                <el-form :model="config" label-width="150px">
                  <el-form-item label="日志级别">
                    <el-select v-model="config.log_level">
                      <el-option label="DEBUG" value="debug" />
                      <el-option label="INFO" value="info" />
                      <el-option label="WARN" value="warn" />
                      <el-option label="ERROR" value="error" />
                    </el-select>
                  </el-form-item>
                  <el-form-item label="日志文件大小(MB)">
                    <el-input-number v-model="config.max_log_size" :min="1" :max="1000" />
                  </el-form-item>
                </el-form>
              </div>
            </div>
          </template>
        </el-skeleton>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh, Check } from '@element-plus/icons-vue'
import { SystemAPI } from '@/api'

const loading = ref(false)
const saving = ref(false)

const config = reactive({
  database_url: '',
  max_connections: 10,
  redis_url: '',
  redis_timeout: 30,
  log_level: 'info',
  max_log_size: 100
})

const fetchConfig = async () => {
  try {
    loading.value = true
    const data = await SystemAPI.getConfig()
    Object.assign(config, data)
  } catch (error: any) {
    ElMessage.error(error.message || '获取配置失败')
    console.error('Failed to fetch config:', error)
  } finally {
    loading.value = false
  }
}

const saveConfig = async () => {
  try {
    saving.value = true
    await SystemAPI.updateConfig(config)
    ElMessage.success('配置保存成功')
  } catch (error: any) {
    ElMessage.error(error.message || '保存配置失败')
    console.error('Failed to save config:', error)
  } finally {
    saving.value = false
  }
}

const refreshConfig = () => {
  fetchConfig()
}

onMounted(() => {
  fetchConfig()
})
</script>

<style scoped>
.configuration-view {
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

.header-actions {
  display: flex;
  gap: 12px;
}

.config-content {
  min-height: 400px;
}

.config-section h3 {
  color: #333;
  margin-bottom: 20px;
  font-size: 16px;
  font-weight: 600;
}

.config-section {
  margin-bottom: 30px;
}

:deep(.el-divider) {
  margin: 30px 0;
}
</style>