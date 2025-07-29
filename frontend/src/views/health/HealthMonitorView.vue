<template>
  <div class="health-monitor-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>健康监控</h2>
          <div class="header-actions">
            <el-button @click="refreshHealth" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button type="primary" @click="triggerBatchHealthCheck" :loading="batchChecking">
              <el-icon><CircleCheck /></el-icon>
              批量检查
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="health-content">
        <!-- 健康状态概览 -->
        <div class="health-summary">
          <el-row :gutter="24">
            <el-col :span="6">
              <div class="summary-item total">
                <div class="summary-number">{{ healthSummary.total }}</div>
                <div class="summary-label">总密钥数</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="summary-item healthy">
                <div class="summary-number">{{ healthSummary.healthy }}</div>
                <div class="summary-label">健康密钥</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="summary-item unhealthy">
                <div class="summary-number">{{ healthSummary.unhealthy }}</div>
                <div class="summary-label">异常密钥</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="summary-item rate">
                <div class="summary-number">{{ healthRate }}%</div>
                <div class="summary-label">健康率</div>
              </div>
            </el-col>
          </el-row>
        </div>

        <!-- 筛选器 -->
        <div class="health-filters">
          <el-form :model="filters" inline>
            <el-form-item label="关键词搜索">
              <el-input
                v-model="filters.keyword"
                placeholder="输入密钥名称"
                clearable
                style="width: 200px"
                @keyup.enter="searchHealth"
              >
                <template #prefix>
                  <el-icon><Search /></el-icon>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="服务商类型">
              <el-select v-model="filters.provider_type" clearable placeholder="全部">
                <el-option 
                  v-for="provider in providerTypes" 
                  :key="provider.id"
                  :label="provider.display_name" 
                  :value="provider.name" 
                />
              </el-select>
            </el-form-item>
            <el-form-item label="健康状态">
              <el-select v-model="filters.healthy" clearable placeholder="全部">
                <el-option label="健康" :value="true" />
                <el-option label="异常" :value="false" />
              </el-select>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="searchHealth">
                <el-icon><Search /></el-icon>
                查询
              </el-button>
              <el-button @click="resetFilters">
                <el-icon><RefreshLeft /></el-icon>
                重置
              </el-button>
            </el-form-item>
          </el-form>
        </div>

        <!-- 健康状态表格 -->
        <el-table
          :data="healthList"
          v-loading="loading"
          stripe
          border
          style="width: 100%"
        >
          <el-table-column prop="key_name" label="密钥名称" width="150" show-overflow-tooltip />
          
          <el-table-column prop="provider_name" label="服务商" width="120">
            <template #default="{ row }">
              <el-tag :type="getProviderTagType(row.provider_name)" size="small">
                {{ row.provider_name }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="is_healthy" label="健康状态" width="100">
            <template #default="{ row }">
              <el-tag :type="row.is_healthy ? 'success' : 'danger'" size="small">
                <el-icon>
                  <CircleCheck v-if="row.is_healthy" />
                  <CircleClose v-else />
                </el-icon>
                {{ row.is_healthy ? '健康' : '异常' }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="response_time" label="响应时间" width="120">
            <template #default="{ row }">
              <span :class="getResponseTimeClass(row.response_time)">
                {{ row.response_time }}ms
              </span>
            </template>
          </el-table-column>
          
          <el-table-column prop="success_rate" label="成功率" width="100">
            <template #default="{ row }">
              <el-progress
                :percentage="row.success_rate"
                :color="getSuccessRateColor(row.success_rate)"
                :stroke-width="6"
                text-inside
                :format="() => `${row.success_rate}%`"
              />
            </template>
          </el-table-column>
          
          <el-table-column prop="last_check_time" label="最后检查时间" width="180">
            <template #default="{ row }">
              {{ formatTime(row.last_check_time) }}
            </template>
          </el-table-column>
          
          <el-table-column prop="error_message" label="错误信息" show-overflow-tooltip>
            <template #default="{ row }">
              <span v-if="row.error_message" class="error-message">
                {{ row.error_message }}
              </span>
              <span v-else class="success-message">正常</span>
            </template>
          </el-table-column>
          
          <el-table-column label="操作" width="150" fixed="right">
            <template #default="{ row }">
              <el-button
                type="text"
                size="small"
                @click="triggerHealthCheck(row.key_id)"
                :loading="checkingKeys.includes(row.key_id)"
              >
                立即检查
              </el-button>
              <el-button
                type="text"
                size="small"
                @click="showHealthDetail(row)"
              >
                详情
              </el-button>
            </template>
          </el-table-column>
        </el-table>

        <!-- 分页 -->
        <div class="pagination-wrapper">
          <el-pagination
            v-model:current-page="pagination.page"
            v-model:page-size="pagination.size"
            :page-sizes="[20, 50, 100]"
            :total="pagination.total"
            layout="total, sizes, prev, pager, next, jumper"
            @size-change="handleSizeChange"
            @current-change="handleCurrentChange"
          />
        </div>
      </div>
    </el-card>

    <!-- 健康检查详情对话框 -->
    <el-dialog
      v-model="detailVisible"
      title="健康检查详情"
      width="80%"
      :max-width="800"
    >
      <div v-if="selectedHealth" class="health-detail">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="密钥名称">
            {{ selectedHealth.key_name }}
          </el-descriptions-item>
          <el-descriptions-item label="服务商">
            {{ selectedHealth.provider_name }}
          </el-descriptions-item>
          <el-descriptions-item label="健康状态">
            <el-tag :type="selectedHealth.is_healthy ? 'success' : 'danger'">
              {{ selectedHealth.is_healthy ? '健康' : '异常' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="响应时间">
            {{ selectedHealth.response_time }}ms
          </el-descriptions-item>
          <el-descriptions-item label="成功率">
            {{ selectedHealth.success_rate }}%
          </el-descriptions-item>
          <el-descriptions-item label="最后检查时间">
            {{ formatTime(selectedHealth.last_check_time) }}
          </el-descriptions-item>
          <el-descriptions-item label="错误信息" :span="2">
            <pre v-if="selectedHealth.error_message" class="error-details">{{ selectedHealth.error_message }}</pre>
            <span v-else class="success-message">无错误</span>
          </el-descriptions-item>
        </el-descriptions>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Refresh, CircleCheck, CircleClose, Search, RefreshLeft
} from '@element-plus/icons-vue'
import { ApiKeyAPI } from '@/api'
import type { ProviderType } from '@/types'

const loading = ref(false)
const batchChecking = ref(false)
const detailVisible = ref(false)
const selectedHealth = ref<any>(null)
const checkingKeys = ref<number[]>([])

const healthList = ref<any[]>([])
const healthSummary = reactive({
  total: 0,
  healthy: 0,
  unhealthy: 0
})

// 动态获取的服务商类型列表
const providerTypes = ref<ProviderType[]>([])

const filters = reactive({
  provider_type: '',
  healthy: null as boolean | null,
  keyword: ''
})

const pagination = reactive({
  page: 1,
  size: 20,
  total: 0
})

// 计算属性
const healthRate = computed(() => {
  if (healthSummary.total === 0) return 0
  return Math.round((healthSummary.healthy / healthSummary.total) * 100)
})

// 获取服务商类型列表
const fetchProviderTypes = async () => {
  try {
    const types = await ApiKeyAPI.getProviderTypes()
    providerTypes.value = types
  } catch (error: any) {
    console.error('Failed to fetch provider types:', error)
    // 失败时使用默认选项
    providerTypes.value = [
      { id: '1', name: 'openai', display_name: 'OpenAI', base_url: '', supported_features: [] },
      { id: '2', name: 'gemini', display_name: 'Google Gemini', base_url: '', supported_features: [] },
      { id: '3', name: 'claude', display_name: 'Anthropic Claude', base_url: '', supported_features: [] }
    ]
  }
}

// 获取健康状态数据
const fetchHealthData = async () => {
  try {
    loading.value = true
    const params = {
      provider_type: filters.provider_type || undefined,
      healthy: filters.healthy,
      keyword: filters.keyword || undefined,
      page: pagination.page,
      limit: pagination.size
    }
    
    const response = await ApiKeyAPI.getHealthStatus(params)
    healthList.value = response.statuses
    
    // 更新概览数据
    healthSummary.total = response.summary.total
    healthSummary.healthy = response.summary.healthy
    healthSummary.unhealthy = response.summary.unhealthy
    
    // 更新分页信息
    if (response.pagination) {
      pagination.total = response.pagination.total
    }
    
  } catch (error: any) {
    ElMessage.error(error.message || '获取健康状态失败')
    console.error('Failed to fetch health data:', error)
  } finally {
    loading.value = false
  }
}

// 刷新健康状态
const refreshHealth = () => {
  fetchHealthData()
}

// 搜索健康状态
const searchHealth = () => {
  pagination.page = 1
  fetchHealthData()
}

// 重置筛选器
const resetFilters = () => {
  filters.provider_type = ''
  filters.healthy = null
  filters.keyword = ''
  pagination.page = 1
  fetchHealthData()
}

// 触发单个健康检查
const triggerHealthCheck = async (keyId: number) => {
  try {
    checkingKeys.value.push(keyId)
    await ApiKeyAPI.triggerHealthCheck(keyId)
    ElMessage.success('健康检查已触发')
    
    // 延迟刷新数据
    setTimeout(() => {
      fetchHealthData()
    }, 2000)
  } catch (error: any) {
    ElMessage.error(error.message || '触发健康检查失败')
  } finally {
    checkingKeys.value = checkingKeys.value.filter(id => id !== keyId)
  }
}

// 批量健康检查
const triggerBatchHealthCheck = async () => {
  try {
    batchChecking.value = true
    
    // 为所有密钥触发健康检查
    const promises = healthList.value.map(item => 
      ApiKeyAPI.triggerHealthCheck(item.key_id).catch(err => {
        console.error(`Health check failed for key ${item.key_id}:`, err)
      })
    )
    
    await Promise.all(promises)
    ElMessage.success('批量健康检查已触发')
    
    // 延迟刷新数据
    setTimeout(() => {
      fetchHealthData()
    }, 3000)
  } catch (error: any) {
    ElMessage.error('批量健康检查失败')
  } finally {
    batchChecking.value = false
  }
}

// 显示健康检查详情
const showHealthDetail = (health: any) => {
  selectedHealth.value = health
  detailVisible.value = true
}

// 分页处理
const handleSizeChange = (size: number) => {
  pagination.size = size
  pagination.page = 1
  fetchHealthData()
}

const handleCurrentChange = (page: number) => {
  pagination.page = page
  fetchHealthData()
}

// 工具函数
const formatTime = (timestamp: string) => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const getProviderTagType = (provider: string) => {
  const typeMap: Record<string, string> = {
    'OpenAI': 'primary',
    'Google': 'success',
    'Anthropic': 'warning',
    'default': 'info'
  }
  return typeMap[provider] || typeMap.default
}

const getResponseTimeClass = (responseTime: number) => {
  if (responseTime < 1000) return 'response-time-good'
  if (responseTime < 3000) return 'response-time-normal'
  return 'response-time-slow'
}

const getSuccessRateColor = (rate: number) => {
  if (rate >= 95) return '#67c23a'
  if (rate >= 85) return '#e6a23c'
  return '#f56c6c'
}

onMounted(async () => {
  await fetchProviderTypes()
  fetchHealthData()
})
</script>

<style scoped>
.health-monitor-view {
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

.health-content {
  height: calc(100% - 60px);
  display: flex;
  flex-direction: column;
}

/* 健康状态概览 */
.health-summary {
  margin-bottom: 24px;
  padding: 20px;
  background: #f8f9fa;
  border-radius: 8px;
}

.summary-item {
  text-align: center;
  padding: 16px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  transition: transform 0.3s;
}

.summary-item:hover {
  transform: translateY(-2px);
}

.summary-number {
  font-size: 32px;
  font-weight: bold;
  margin-bottom: 8px;
}

.summary-item.total .summary-number {
  color: #409eff;
}

.summary-item.healthy .summary-number {
  color: #67c23a;
}

.summary-item.unhealthy .summary-number {
  color: #f56c6c;
}

.summary-item.rate .summary-number {
  color: #e6a23c;
}

.summary-label {
  font-size: 14px;
  color: #666;
}

/* 筛选器 */
.health-filters {
  margin-bottom: 20px;
  padding: 16px;
  background: #f8f9fa;
  border-radius: 6px;
}

/* 表格样式 */
.response-time-good {
  color: #67c23a;
  font-weight: 500;
}

.response-time-normal {
  color: #e6a23c;
  font-weight: 500;
}

.response-time-slow {
  color: #f56c6c;
  font-weight: 500;
}

.error-message {
  color: #f56c6c;
  font-size: 12px;
}

.success-message {
  color: #67c23a;
  font-size: 12px;
}

.pagination-wrapper {
  margin-top: 20px;
  display: flex;
  justify-content: center;
}

/* 健康检查详情 */
.health-detail {
  max-height: 500px;
  overflow-y: auto;
}

.error-details {
  background: #f5f5f5;
  padding: 12px;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.4;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .summary-number {
    font-size: 24px;
  }
  
  .health-filters .el-form {
    flex-direction: column;
  }
  
  .health-filters .el-form-item {
    margin-bottom: 16px;
    margin-right: 0;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-table .cell) {
  padding: 8px 12px;
}

:deep(.el-progress-bar__inner) {
  border-radius: 3px;
}

:deep(.el-progress__text) {
  font-size: 12px !important;
  color: white !important;
}
</style>