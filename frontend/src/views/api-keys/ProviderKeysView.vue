<template>
  <div class="provider-keys-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>服务商密钥池</h2>
          <div class="header-actions">
            <el-button @click="refreshKeys" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button type="primary" @click="showCreateDialog">
              <el-icon><Plus /></el-icon>
              添加密钥
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="keys-content">
        <!-- 筛选器 -->
        <div class="keys-filters">
          <el-form :model="filters" inline>
            <el-form-item label="服务商类型">
              <el-select v-model="filters.provider_type" clearable placeholder="全部">
                <el-option 
                  v-for="type in providerTypes" 
                  :key="type.id" 
                  :label="type.display_name" 
                  :value="type.id" 
                />
              </el-select>
            </el-form-item>
            <el-form-item label="状态">
              <el-select v-model="filters.is_active" clearable placeholder="全部">
                <el-option label="启用" :value="true" />
                <el-option label="禁用" :value="false" />
              </el-select>
            </el-form-item>
            <el-form-item label="健康状态">
              <el-select v-model="filters.healthy" clearable placeholder="全部">
                <el-option label="健康" :value="true" />
                <el-option label="异常" :value="false" />
              </el-select>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="searchKeys">
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

        <!-- 密钥表格 -->
        <div class="table-container">
          <el-table
            :data="keysList"
            v-loading="loading"
            stripe
            border
            style="width: 100%"
            :height="tableHeight"
          >
          <el-table-column prop="name" label="密钥名称" width="150" show-overflow-tooltip />
          
          <el-table-column prop="description" label="描述" show-overflow-tooltip />
          
          <el-table-column prop="provider_type" label="服务商" width="120">
            <template #default="{ row }">
              <el-tag :type="getProviderTagType(row.provider_type)" size="small">
                {{ getProviderDisplayName(row.provider_type) }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="api_key" label="API密钥" width="200">
            <template #default="{ row }">
              <div class="api-key-cell">
                <span class="masked-key">{{ maskApiKey(row.api_key) }}</span>
                <el-button 
                  type="text" 
                  size="small" 
                  @click="copyApiKey(row.api_key)"
                >
                  <el-icon><CopyDocument /></el-icon>
                </el-button>
              </div>
            </template>
          </el-table-column>
          
          <el-table-column prop="weight" label="权重" width="80">
            <template #default="{ row }">
              <el-tag size="small">{{ row.weight }}</el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="max_requests_per_minute" label="请求限制/分钟" width="140">
            <template #default="{ row }">
              {{ row.max_requests_per_minute || '无限制' }}
            </template>
          </el-table-column>
          
          <el-table-column prop="max_tokens_per_day" label="Token限制/天" width="140">
            <template #default="{ row }">
              {{ formatNumber(row.max_tokens_per_day) || '无限制' }}
            </template>
          </el-table-column>
          
          <el-table-column prop="is_active" label="状态" width="80">
            <template #default="{ row }">
              <el-switch
                v-model="row.is_active"
                @change="toggleKeyStatus(row)"
                :loading="toggleLoading.includes(row.id)"
              />
            </template>
          </el-table-column>
          
          <el-table-column prop="health_status" label="健康状态" width="100">
            <template #default="{ row }">
              <el-tag 
                :type="row.health_status === 'healthy' ? 'success' : 'danger'" 
                size="small"
              >
                <el-icon>
                  <CircleCheck v-if="row.health_status === 'healthy'" />
                  <CircleClose v-else />
                </el-icon>
                {{ row.health_status === 'healthy' ? '健康' : '异常' }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="created_at" label="创建时间" width="180">
            <template #default="{ row }">
              {{ formatTime(row.created_at) }}
            </template>
          </el-table-column>
          
          <el-table-column label="操作" width="200" fixed="right">
            <template #default="{ row }">
              <el-button type="text" size="small" @click="showEditDialog(row)">
                编辑
              </el-button>
              <el-button type="text" size="small" @click="testKey(row)">
                测试
              </el-button>
              <el-button type="text" size="small" @click="showUsageStats(row)">
                统计
              </el-button>
              <el-popconfirm
                title="确定要删除这个密钥吗？"
                @confirm="deleteKey(row)"
              >
                <template #reference>
                  <el-button type="text" size="small" style="color: #f56c6c;">
                    删除
                  </el-button>
                </template>
              </el-popconfirm>
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
      </div>
    </el-card>

    <!-- 创建/编辑密钥对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑密钥' : '添加密钥'"
      width="600px"
      @close="resetForm"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="120px"
      >
        <el-form-item label="密钥名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入密钥名称" />
        </el-form-item>
        
        <el-form-item label="服务商类型" prop="provider_type">
          <el-select v-model="form.provider_type" placeholder="请选择服务商">
            <el-option 
              v-for="type in providerTypes" 
              :key="type.id" 
              :label="type.display_name" 
              :value="type.id" 
            />
          </el-select>
        </el-form-item>
        
        <el-form-item label="API密钥" prop="api_key">
          <el-input 
            v-model="form.api_key" 
            type="password" 
            placeholder="请输入API密钥"
            show-password
          />
        </el-form-item>
        
        <el-form-item label="权重" prop="weight">
          <el-input-number 
            v-model="form.weight" 
            :min="1" 
            :max="100" 
            placeholder="负载均衡权重"
          />
        </el-form-item>
        
        <el-form-item label="请求限制/分钟">
          <el-input-number 
            v-model="form.max_requests_per_minute" 
            :min="0" 
            placeholder="0表示无限制"
          />
        </el-form-item>
        
        <el-form-item label="Token限制/天">
          <el-input-number 
            v-model="form.max_tokens_per_day" 
            :min="0" 
            placeholder="0表示无限制"
          />
        </el-form-item>
        
        <el-form-item label="启用状态">
          <el-switch v-model="form.is_active" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" @click="submitForm" :loading="submitLoading">
            {{ isEdit ? '更新' : '创建' }}
          </el-button>
        </div>
      </template>
    </el-dialog>

    <!-- 使用统计对话框 -->
    <el-dialog
      v-model="statsVisible"
      title="使用统计"
      width="80%"
      :max-width="800"
    >
      <div v-if="selectedKeyStats" class="stats-content">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="总请求数">
            {{ selectedKeyStats.summary.total_requests }}
          </el-descriptions-item>
          <el-descriptions-item label="总Token数">
            {{ formatNumber(selectedKeyStats.summary.total_tokens) }}
          </el-descriptions-item>
          <el-descriptions-item label="平均响应时间">
            {{ selectedKeyStats.summary.avg_response_time }}ms
          </el-descriptions-item>
          <el-descriptions-item label="成功率">
            {{ (selectedKeyStats.summary.success_rate || 0).toFixed(2) }}%
          </el-descriptions-item>
        </el-descriptions>
        
        <div class="stats-chart" ref="statsChartRef"></div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import {
  Refresh, Plus, Search, RefreshLeft, CopyDocument,
  CircleCheck, CircleClose
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { ApiKeyAPI } from '@/api'
import type { UserProviderKey, CreateProviderKeyRequest, ProviderType } from '@/types'

// 状态
const loading = ref(false)
const submitLoading = ref(false)
const dialogVisible = ref(false)
const statsVisible = ref(false)
const isEdit = ref(false)
const toggleLoading = ref<number[]>([])

// 数据
const keysList = ref<UserProviderKey[]>([])
const providerTypes = ref<ProviderType[]>([])
const selectedKeyStats = ref<any>(null)

// 表单
const formRef = ref<FormInstance>()
const form = reactive<CreateProviderKeyRequest>({
  name: '',
  provider_type: '',
  api_key: '',
  weight: 1,
  max_requests_per_minute: 0,
  max_tokens_per_day: 0,
  is_active: true
})

// 筛选器
const filters = reactive({
  provider_type: '',
  is_active: null as boolean | null,
  healthy: null as boolean | null
})

// 分页
const pagination = reactive({
  page: 1,
  size: 20,
  total: 0
})

// 图表
const statsChartRef = ref<HTMLElement>()
const statsChart = ref<echarts.ECharts>()

// 表格高度计算
const tableHeight = ref<string | number>('auto')

const calculateTableHeight = () => {
  // 计算表格可用高度：页面高度 - 卡片头部 - 筛选器 - 分页 - 边距
  const windowHeight = window.innerHeight
  const headerHeight = 60  // 卡片头部高度
  const filtersHeight = 80  // 筛选器高度
  const paginationHeight = 60  // 分页高度
  const margins = 120  // 各种边距
  
  const availableHeight = windowHeight - headerHeight - filtersHeight - paginationHeight - margins
  tableHeight.value = Math.max(400, availableHeight)  // 最小400px高度
}

// 表单验证规则
const formRules: FormRules = {
  name: [
    { required: true, message: '请输入密钥名称', trigger: 'blur' },
    { min: 2, max: 50, message: '名称长度应为2-50个字符', trigger: 'blur' }
  ],
  provider_type: [
    { required: true, message: '请选择服务商类型', trigger: 'change' }
  ],
  api_key: [
    { required: true, message: '请输入API密钥', trigger: 'blur' },
    { min: 10, message: 'API密钥长度至少10个字符', trigger: 'blur' }
  ],
  weight: [
    { required: true, message: '请设置权重', trigger: 'blur' },
    { type: 'number', min: 1, max: 100, message: '权重应在1-100之间', trigger: 'blur' }
  ]
}

// 获取密钥列表
const fetchKeys = async () => {
  try {
    loading.value = true
    const params = {
      provider_type: filters.provider_type || undefined,
      status: filters.is_active === null ? undefined : (filters.is_active ? 'active' : 'inactive'),
      healthy: filters.healthy,
      page: pagination.page,
      limit: pagination.size
    }
    
    const response = await ApiKeyAPI.getProviderKeys(params)
    keysList.value = response.keys
    pagination.total = response.pagination?.total || 0
  } catch (error: any) {
    ElMessage.error(error.message || '获取密钥列表失败')
    console.error('Failed to fetch keys:', error)
  } finally {
    loading.value = false
  }
}

// 获取服务商类型
const fetchProviderTypes = async () => {
  try {
    providerTypes.value = await ApiKeyAPI.getProviderTypes()
  } catch (error: any) {
    console.error('Failed to fetch provider types:', error)
  }
}

// 刷新密钥列表
const refreshKeys = () => {
  fetchKeys()
}

// 搜索密钥
const searchKeys = () => {
  pagination.page = 1
  fetchKeys()
}

// 重置筛选器
const resetFilters = () => {
  filters.provider_type = ''
  filters.is_active = null
  filters.healthy = null
  pagination.page = 1
  fetchKeys()
}

// 显示创建对话框
const showCreateDialog = () => {
  isEdit.value = false
  resetForm()
  dialogVisible.value = true
}

// 显示编辑对话框
const showEditDialog = (key: UserProviderKey) => {
  isEdit.value = true
  form.name = key.name
  form.provider_type = key.provider_type
  form.api_key = key.api_key
  form.weight = key.weight
  form.max_requests_per_minute = key.max_requests_per_minute || 0
  form.max_tokens_per_day = key.max_tokens_per_day || 0
  form.is_active = key.is_active
  form.id = key.id
  dialogVisible.value = true
}

// 重置表单
const resetForm = () => {
  if (formRef.value) {
    formRef.value.resetFields()
  }
  Object.assign(form, {
    name: '',
    provider_type: '',
    api_key: '',
    weight: 1,
    max_requests_per_minute: 0,
    max_tokens_per_day: 0,
    is_active: true,
    id: undefined
  })
}

// 提交表单
const submitForm = async () => {
  if (!formRef.value) return
  
  try {
    const isValid = await formRef.value.validate()
    if (!isValid) return
    
    submitLoading.value = true
    
    if (isEdit.value && form.id) {
      await ApiKeyAPI.updateProviderKey(form.id, {
        name: form.name,
        api_key: form.api_key,
        weight: form.weight,
        max_requests_per_minute: form.max_requests_per_minute || undefined,
        max_tokens_per_day: form.max_tokens_per_day || undefined,
        is_active: form.is_active
      })
      ElMessage.success('密钥更新成功')
    } else {
      await ApiKeyAPI.createProviderKey(form)
      ElMessage.success('密钥创建成功')
    }
    
    dialogVisible.value = false
    fetchKeys()
  } catch (error: any) {
    ElMessage.error(error.message || '操作失败')
  } finally {
    submitLoading.value = false
  }
}

// 切换密钥状态
const toggleKeyStatus = async (key: UserProviderKey) => {
  try {
    toggleLoading.value.push(key.id)
    await ApiKeyAPI.toggleProviderKeyStatus(key.id, key.is_active)
    ElMessage.success(`密钥已${key.is_active ? '启用' : '禁用'}`)
  } catch (error: any) {
    key.is_active = !key.is_active // 回滚状态
    ElMessage.error(error.message || '状态切换失败')
  } finally {
    toggleLoading.value = toggleLoading.value.filter(id => id !== key.id)
  }
}

// 删除密钥
const deleteKey = async (key: UserProviderKey) => {
  try {
    await ApiKeyAPI.deleteProviderKey(key.id)
    ElMessage.success('密钥删除成功')
    fetchKeys()
  } catch (error: any) {
    ElMessage.error(error.message || '删除失败')
  }
}

// 测试密钥
const testKey = async (key: UserProviderKey) => {
  try {
    const result = await ApiKeyAPI.testProviderKey(key.id)
    if (result.success) {
      ElMessage.success(`测试成功，响应时间: ${result.response_time}ms`)
    } else {
      ElMessage.error(`测试失败: ${result.message}`)
    }
  } catch (error: any) {
    ElMessage.error(error.message || '测试失败')
  }
}

// 显示使用统计
const showUsageStats = async (key: UserProviderKey) => {
  try {
    selectedKeyStats.value = await ApiKeyAPI.getKeyUsageStats(key.id, {
      group_by: 'day'
    })
    statsVisible.value = true
    
    nextTick(() => {
      initStatsChart()
    })
  } catch (error: any) {
    ElMessage.error(error.message || '获取统计数据失败')
  }
}

// 初始化统计图表
const initStatsChart = () => {
  if (!statsChartRef.value || !selectedKeyStats.value) return
  
  if (statsChart.value) {
    statsChart.value.dispose()
  }
  
  statsChart.value = echarts.init(statsChartRef.value)
  
  const option = {
    title: {
      text: '使用统计趋势',
      textStyle: { fontSize: 14 }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['请求数', 'Token数', '成功率']
    },
    xAxis: {
      type: 'category',
      data: selectedKeyStats.value.usage.map((item: any) => item.timestamp)
    },
    yAxis: [
      {
        type: 'value',
        name: '请求数/Token数',
        position: 'left'
      },
      {
        type: 'value',
        name: '成功率(%)',
        position: 'right',
        min: 0,
        max: 100
      }
    ],
    series: [
      {
        name: '请求数',
        type: 'bar',
        data: selectedKeyStats.value.usage.map((item: any) => item.requests)
      },
      {
        name: 'Token数',
        type: 'line',
        data: selectedKeyStats.value.usage.map((item: any) => item.tokens)
      },
      {
        name: '成功率',
        type: 'line',
        yAxisIndex: 1,
        data: selectedKeyStats.value.usage.map((item: any) => item.success_rate)
      }
    ]
  }
  
  statsChart.value.setOption(option)
}

// 分页处理
const handleSizeChange = (size: number) => {
  pagination.size = size
  pagination.page = 1
  fetchKeys()
}

const handleCurrentChange = (page: number) => {
  pagination.page = page
  fetchKeys()
}

// 工具函数
const maskApiKey = (key: string) => {
  if (!key || key.length < 8) return key
  return key.substring(0, 4) + '*'.repeat(key.length - 8) + key.substring(key.length - 4)
}

const copyApiKey = async (key: string) => {
  try {
    await navigator.clipboard.writeText(key)
    ElMessage.success('API密钥已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败')
  }
}

const formatTime = (timestamp: string) => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const formatNumber = (num: number | null) => {
  if (!num) return 0
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

const getProviderTagType = (provider: string) => {
  const typeMap: Record<string, string> = {
    'openai': 'primary',
    'google': 'success',
    'anthropic': 'warning',
    'default': 'info'
  }
  return typeMap[provider] || typeMap.default
}

const getProviderDisplayName = (provider: string) => {
  const type = providerTypes.value.find(t => t.id === provider)
  return type?.display_name || provider
}

onMounted(() => {
  fetchProviderTypes()
  fetchKeys()
  calculateTableHeight()
  
  // 监听窗口大小变化
  window.addEventListener('resize', calculateTableHeight)
})

// 清理事件监听器
onUnmounted(() => {
  window.removeEventListener('resize', calculateTableHeight)
})
</script>

<style scoped>
.provider-keys-view {
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

.keys-content {
  height: calc(100% - 60px);
  display: flex;
  flex-direction: column;
}

.keys-filters {
  flex-shrink: 0;
  margin-bottom: 16px;
  padding: 20px;
  background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.02);
}

.keys-filters .el-form {
  margin-bottom: 0;
}

.keys-filters .el-form-item {
  margin-bottom: 0;
  margin-right: 24px;
}

.keys-filters .el-form-item:last-child {
  margin-right: 0;
}

.keys-filters .el-select,
.keys-filters .el-input {
  width: 160px;
}

.keys-filters .el-button {
  margin-left: 8px;
}

.keys-filters .el-button:first-child {
  margin-left: 0;
}

.table-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: #fff;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  overflow: hidden;
}

.table-container .el-table {
  flex: 1;
}

.table-container .el-table .el-table__header-wrapper {
  background: #fafafa;
}

.table-container .el-table th {
  background: #fafafa !important;
  color: #374151;
  font-weight: 600;
  font-size: 13px;
  padding: 12px 0;
  border-bottom: 2px solid #e5e7eb;
}

.table-container .el-table td {
  padding: 14px 0;
  border-bottom: 1px solid #f3f4f6;
}

.table-container .el-table .el-table__row:hover {
  background: #f9fafb;
}

.table-container .el-table .el-table__row:hover td {
  background: transparent;
}

/* 表格滚动优化 */
.table-container .el-table__body-wrapper {
  scrollbar-width: thin;
  scrollbar-color: #d1d5db #f3f4f6;
}

.table-container .el-table__body-wrapper::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.table-container .el-table__body-wrapper::-webkit-scrollbar-track {
  background: #f3f4f6;
  border-radius: 3px;
}

.table-container .el-table__body-wrapper::-webkit-scrollbar-thumb {
  background: #d1d5db;
  border-radius: 3px;
}

.table-container .el-table__body-wrapper::-webkit-scrollbar-thumb:hover {
  background: #9ca3af;
}

.api-key-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 200px;
}

.api-key-cell .masked-key {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.masked-key {
  font-family: 'Courier New', monospace;
  font-size: 12px;
}

.pagination-wrapper {
  flex-shrink: 0;
  padding: 16px 20px;
  background: #fafafa;
  border-top: 1px solid #e5e7eb;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.pagination-wrapper .el-pagination {
  margin: 0;
}

.pagination-wrapper .el-pagination .el-pagination__total {
  color: #6b7280;
  font-size: 13px;
}

.stats-content {
  height: 500px;
  display: flex;
  flex-direction: column;
}

.stats-chart {
  flex: 1;
  margin-top: 20px;
  min-height: 300px;
}

.dialog-footer {
  text-align: right;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .keys-filters {
    padding: 16px;
  }
  
  .keys-filters .el-form {
    flex-direction: column;
    gap: 16px;
  }
  
  .keys-filters .el-form-item {
    margin-bottom: 0;
    margin-right: 0;
    width: 100%;
  }
  
  .keys-filters .el-select,
  .keys-filters .el-input {
    width: 100%;
  }
  
  .pagination-wrapper {
    flex-direction: column;
    gap: 12px;
    padding: 12px 16px;
  }
  
  .pagination-wrapper .el-pagination {
    width: 100%;
    text-align: center;
  }
  
  .table-container .el-table .el-table__cell {
    padding: 8px 4px;
  }
  
  .header-actions {
    flex-direction: column;
    gap: 8px;
  }
  
  .header-actions .el-button {
    width: 100%;
  }
}

/* 中等屏幕优化 */
@media (max-width: 1024px) {
  .keys-filters .el-form {
    flex-wrap: wrap;
  }
  
  .keys-filters .el-form-item {
    margin-right: 16px;
    margin-bottom: 12px;
  }
  
  .table-container .el-table .el-table__cell {
    padding: 10px 6px;
  }
}
</style>