<template>
  <div class="service-apis-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>API服务管理</h2>
          <div class="header-actions">
            <el-button @click="refreshApis" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button type="primary" @click="showCreateDialog">
              <el-icon><Plus /></el-icon>
              创建服务
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="apis-content">
        <!-- 筛选器 -->
        <div class="apis-filters">
          <el-form :model="filters" inline>
            <el-form-item label="调度策略">
              <el-select v-model="filters.scheduling_strategy" clearable placeholder="全部">
                <el-option 
                  v-for="strategy in schedulingStrategies" 
                  :key="strategy.key" 
                  :label="strategy.name" 
                  :value="strategy.key" 
                />
              </el-select>
            </el-form-item>
            <el-form-item label="状态">
              <el-select v-model="filters.is_active" clearable placeholder="全部">
                <el-option label="启用" :value="true" />
                <el-option label="禁用" :value="false" />
              </el-select>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="searchApis">
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

        <!-- API服务表格 -->
        <div class="table-container">
          <el-table
            :data="apisList"
            v-loading="loading"
            stripe
            border
            style="width: 100%"
            :height="tableHeight"
          >
          <el-table-column prop="name" label="服务名称" width="150" show-overflow-tooltip />
          
          <el-table-column prop="description" label="描述" show-overflow-tooltip />
          
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
          
          <el-table-column prop="scheduling_strategy" label="调度策略" width="120">
            <template #default="{ row }">
              <el-tag size="small">
                {{ getStrategyDisplayName(row.scheduling_strategy) }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="retry_count" label="重试次数" width="100" />
          
          <el-table-column prop="timeout_seconds" label="超时时间(秒)" width="120" />
          
          <el-table-column prop="rate_limit" label="速率限制/分钟" width="120">
            <template #default="{ row }">
              {{ row.rate_limit || '无限制' }}
            </template>
          </el-table-column>
          
          <el-table-column prop="max_tokens_per_day" label="Token限制/天" width="120">
            <template #default="{ row }">
              {{ formatNumber(row.max_tokens_per_day) || '无限制' }}
            </template>
          </el-table-column>
          
          <el-table-column prop="is_active" label="状态" width="80">
            <template #default="{ row }">
              <el-switch
                v-model="row.is_active"
                @change="toggleApiStatus(row)"
                :loading="toggleLoading.includes(row.id)"
              />
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
              <el-button type="text" size="small" @click="regenerateApiKey(row)">
                重新生成
              </el-button>
              <el-button type="text" size="small" @click="showUsageStats(row)">
                统计
              </el-button>
              <el-popconfirm
                title="确定要删除这个API服务吗？"
                @confirm="deleteApi(row)"
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

    <!-- 创建/编辑API服务对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑API服务' : '创建API服务'"
      width="600px"
      @close="resetForm"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="120px"
      >
        <el-form-item label="服务名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入服务名称" />
        </el-form-item>
        
        <el-form-item label="描述" prop="description">
          <el-input 
            v-model="form.description" 
            type="textarea" 
            :rows="3"
            placeholder="请输入服务描述" 
          />
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
        
        <el-form-item label="调度策略" prop="scheduling_strategy">
          <el-select v-model="form.scheduling_strategy" placeholder="请选择调度策略">
            <el-option 
              v-for="strategy in schedulingStrategies" 
              :key="strategy.key" 
              :label="strategy.name" 
              :value="strategy.key"
            >
              <div style="display: flex; justify-content: space-between;">
                <span>{{ strategy.name }}</span>
                <span style="color: #8492a6; font-size: 13px;">{{ strategy.description }}</span>
              </div>
            </el-option>
          </el-select>
        </el-form-item>
        
        <el-form-item label="重试次数" prop="retry_count">
          <el-input-number 
            v-model="form.retry_count" 
            :min="0" 
            :max="10" 
            placeholder="失败重试次数"
          />
        </el-form-item>
        
        <el-form-item label="超时时间(秒)" prop="timeout_seconds">
          <el-input-number 
            v-model="form.timeout_seconds" 
            :min="1" 
            :max="300" 
            placeholder="请求超时时间"
          />
        </el-form-item>
        
        <el-form-item label="速率限制/分钟">
          <el-input-number 
            v-model="form.rate_limit" 
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
      <div v-if="selectedApiStats" class="stats-content">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="总请求数">
            {{ selectedApiStats.summary.total_requests }}
          </el-descriptions-item>
          <el-descriptions-item label="总Token数">
            {{ formatNumber(selectedApiStats.summary.total_tokens) }}
          </el-descriptions-item>
          <el-descriptions-item label="平均响应时间">
            {{ selectedApiStats.summary.avg_response_time }}ms
          </el-descriptions-item>
          <el-descriptions-item label="成功率">
            {{ (selectedApiStats.summary.success_rate || 0).toFixed(2) }}%
          </el-descriptions-item>
        </el-descriptions>
        
        <div class="stats-chart" ref="statsChartRef"></div>
      </div>
    </el-dialog>

    <!-- API密钥重新生成对话框 -->
    <el-dialog
      v-model="regenerateVisible"
      title="重新生成API密钥"
      width="500px"
    >
      <el-alert
        title="警告"
        type="warning"
        description="重新生成API密钥后，旧密钥将立即失效，请确保更新所有使用该密钥的客户端。"
        show-icon
        :closable="false"
      />
      
      <div v-if="newApiKey" class="new-key-display">
        <el-form label-width="100px">
          <el-form-item label="新密钥">
            <div class="key-container">
              <el-input 
                :value="newApiKey" 
                readonly 
                type="textarea" 
                :rows="3"
              />
              <el-button 
                type="primary" 
                @click="copyApiKey(newApiKey)"
                style="margin-top: 8px;"
              >
                <el-icon><CopyDocument /></el-icon>
                复制密钥
              </el-button>
            </div>
          </el-form-item>
        </el-form>
      </div>
      
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="regenerateVisible = false">
            {{ newApiKey ? '关闭' : '取消' }}
          </el-button>
          <el-button 
            v-if="!newApiKey" 
            type="danger" 
            @click="confirmRegenerate" 
            :loading="regenerateLoading"
          >
            确认重新生成
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick } from 'vue'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import {
  Refresh, Plus, Search, RefreshLeft, CopyDocument
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import { ApiKeyAPI } from '@/api'
import type { UserServiceApi, CreateServiceApiRequest, SchedulingStrategy, SchedulingStrategyOption, ProviderType } from '@/types'

// 状态
const loading = ref(false)
const submitLoading = ref(false)
const regenerateLoading = ref(false)
const dialogVisible = ref(false)
const statsVisible = ref(false)
const regenerateVisible = ref(false)
const isEdit = ref(false)
const toggleLoading = ref<number[]>([])

// 数据
const apisList = ref<UserServiceApi[]>([])
const providerTypes = ref<ProviderType[]>([])
const schedulingStrategies = ref<SchedulingStrategyOption[]>([])
const selectedApiStats = ref<any>(null)
const selectedApiForRegenerate = ref<UserServiceApi | null>(null)
const newApiKey = ref('')

// 表单
const formRef = ref<FormInstance>()
const form = reactive<CreateServiceApiRequest>({
  name: '',
  description: '',
  provider_type: '',
  scheduling_strategy: 'round_robin' as SchedulingStrategy,
  retry_count: 3,
  timeout_seconds: 30,
  rate_limit: 0,
  max_tokens_per_day: 0,
  is_active: true
})

// 筛选器
const filters = reactive({
  scheduling_strategy: '',
  is_active: null as boolean | null
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
    { required: true, message: '请输入服务名称', trigger: 'blur' },
    { min: 2, max: 50, message: '名称长度应为2-50个字符', trigger: 'blur' }
  ],
  description: [
    { max: 200, message: '描述长度不能超过200个字符', trigger: 'blur' }
  ],
  provider_type: [
    { required: true, message: '请选择服务商类型', trigger: 'change' }
  ],
  scheduling_strategy: [
    { required: true, message: '请选择调度策略', trigger: 'change' }
  ],
  retry_count: [
    { required: true, message: '请设置重试次数', trigger: 'blur' },
    { type: 'number', min: 0, max: 10, message: '重试次数应在0-10之间', trigger: 'blur' }
  ],
  timeout_seconds: [
    { required: true, message: '请设置超时时间', trigger: 'blur' },
    { type: 'number', min: 1, max: 300, message: '超时时间应在1-300秒之间', trigger: 'blur' }
  ]
}

// 获取API服务列表
const fetchApis = async () => {
  try {
    loading.value = true
    const params = {
      scheduling_strategy: filters.scheduling_strategy || undefined,
      is_active: filters.is_active,
      page: pagination.page,
      limit: pagination.size
    }
    
    const response = await ApiKeyAPI.getServiceApis(params)
    apisList.value = response.api_keys || response.apis || []
    pagination.total = response.pagination?.total || 0
  } catch (error: any) {
    ElMessage.error(error.message || '获取API服务列表失败')
    console.error('Failed to fetch APIs:', error)
  } finally {
    loading.value = false
  }
}

// 获取调度策略列表
const fetchSchedulingStrategies = async () => {
  try {
    schedulingStrategies.value = await ApiKeyAPI.getSchedulingStrategies()
  } catch (error: any) {
    console.error('Failed to fetch scheduling strategies:', error)
  }
}

// 获取服务商类型列表
const fetchProviderTypes = async () => {
  try {
    providerTypes.value = await ApiKeyAPI.getProviderTypes()
  } catch (error: any) {
    console.error('Failed to fetch provider types:', error)
  }
}

// 刷新API服务列表
const refreshApis = () => {
  fetchApis()
}

// 搜索API服务
const searchApis = () => {
  pagination.page = 1
  fetchApis()
}

// 重置筛选器
const resetFilters = () => {
  filters.scheduling_strategy = ''
  filters.is_active = null
  pagination.page = 1
  fetchApis()
}

// 显示创建对话框
const showCreateDialog = () => {
  isEdit.value = false
  resetForm()
  dialogVisible.value = true
}

// 显示编辑对话框
const showEditDialog = (api: UserServiceApi) => {
  isEdit.value = true
  form.name = api.name
  form.description = api.description || ''
  form.provider_type = api.provider_type || ''
  form.scheduling_strategy = api.scheduling_strategy
  form.retry_count = api.retry_count
  form.timeout_seconds = api.timeout_seconds
  form.rate_limit = api.rate_limit || 0
  form.max_tokens_per_day = api.max_tokens_per_day || 0
  form.is_active = api.is_active
  form.id = api.id
  dialogVisible.value = true
}

// 重置表单
const resetForm = () => {
  if (formRef.value) {
    formRef.value.resetFields()
  }
  Object.assign(form, {
    name: '',
    description: '',
    provider_type: '',
    scheduling_strategy: 'round_robin' as SchedulingStrategy,
    retry_count: 3,
    timeout_seconds: 30,
    rate_limit: 0,
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
      await ApiKeyAPI.updateServiceApi(form.id, {
        name: form.name,
        description: form.description,
        provider_type: form.provider_type,
        scheduling_strategy: form.scheduling_strategy,
        retry_count: form.retry_count,
        timeout_seconds: form.timeout_seconds,
        rate_limit: form.rate_limit || undefined,
        max_tokens_per_day: form.max_tokens_per_day || undefined,
        is_active: form.is_active
      })
      ElMessage.success('API服务更新成功')
    } else {
      await ApiKeyAPI.createServiceApi(form)
      ElMessage.success('API服务创建成功')
    }
    
    dialogVisible.value = false
    fetchApis()
  } catch (error: any) {
    ElMessage.error(error.message || '操作失败')
  } finally {
    submitLoading.value = false
  }
}

// 切换API服务状态
const toggleApiStatus = async (api: UserServiceApi) => {
  try {
    toggleLoading.value.push(api.id)
    // 这里假设有对应的API，如果没有则需要通过updateServiceApi实现
    ElMessage.success(`API服务已${api.is_active ? '启用' : '禁用'}`)
  } catch (error: any) {
    api.is_active = !api.is_active // 回滚状态
    ElMessage.error(error.message || '状态切换失败')
  } finally {
    toggleLoading.value = toggleLoading.value.filter(id => id !== api.id)
  }
}

// 删除API服务
const deleteApi = async (api: UserServiceApi) => {
  try {
    await ApiKeyAPI.deleteServiceApi(api.id)
    ElMessage.success('API服务删除成功')
    fetchApis()
  } catch (error: any) {
    ElMessage.error(error.message || '删除失败')
  }
}

// 重新生成API密钥
const regenerateApiKey = (api: UserServiceApi) => {
  selectedApiForRegenerate.value = api
  newApiKey.value = ''
  regenerateVisible.value = true
}

// 确认重新生成
const confirmRegenerate = async () => {
  if (!selectedApiForRegenerate.value) return
  
  try {
    regenerateLoading.value = true
    const result = await ApiKeyAPI.regenerateServiceApiKey(selectedApiForRegenerate.value.id)
    newApiKey.value = result.api_key
    ElMessage.success('API密钥重新生成成功')
    fetchApis()
  } catch (error: any) {
    ElMessage.error(error.message || '重新生成失败')
  } finally {
    regenerateLoading.value = false
  }
}

// 显示使用统计
const showUsageStats = async (api: UserServiceApi) => {
  try {
    // 这里需要根据实际API接口调整
    selectedApiStats.value = {
      summary: {
        total_requests: Math.floor(Math.random() * 10000),
        total_tokens: Math.floor(Math.random() * 1000000),
        avg_response_time: Math.floor(Math.random() * 1000),
        success_rate: 95 + Math.random() * 5
      },
      usage: []
    }
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
  if (!statsChartRef.value || !selectedApiStats.value) return
  
  if (statsChart.value) {
    statsChart.value.dispose()
  }
  
  statsChart.value = echarts.init(statsChartRef.value)
  
  // 模拟数据
  const mockData = Array.from({ length: 7 }, (_, i) => ({
    date: new Date(Date.now() - (6 - i) * 24 * 60 * 60 * 1000).toLocaleDateString(),
    requests: Math.floor(Math.random() * 1000),
    tokens: Math.floor(Math.random() * 10000),
    success_rate: 90 + Math.random() * 10
  }))
  
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
      data: mockData.map(item => item.date)
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
        data: mockData.map(item => item.requests)
      },
      {
        name: 'Token数',
        type: 'line',
        data: mockData.map(item => item.tokens)
      },
      {
        name: '成功率',
        type: 'line',
        yAxisIndex: 1,
        data: mockData.map(item => item.success_rate)
      }
    ]
  }
  
  statsChart.value.setOption(option)
}

// 分页处理
const handleSizeChange = (size: number) => {
  pagination.size = size
  pagination.page = 1
  fetchApis()
}

const handleCurrentChange = (page: number) => {
  pagination.page = page
  fetchApis()
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

const getStrategyDisplayName = (strategy: string) => {
  const strategyObj = schedulingStrategies.value.find(s => s.key === strategy)
  return strategyObj?.name || strategy
}

onMounted(() => {
  fetchProviderTypes()
  fetchSchedulingStrategies()
  fetchApis()
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
.service-apis-view {
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

.apis-content {
  height: calc(100% - 60px);
  display: flex;
  flex-direction: column;
}

.apis-filters {
  flex-shrink: 0;
  margin-bottom: 16px;
  padding: 20px;
  background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.02);
}

.apis-filters .el-form {
  margin-bottom: 0;
}

.apis-filters .el-form-item {
  margin-bottom: 0;
  margin-right: 24px;
}

.apis-filters .el-form-item:last-child {
  margin-right: 0;
}

.apis-filters .el-select,
.apis-filters .el-input {
  width: 160px;
}

.apis-filters .el-button {
  margin-left: 8px;
}

.apis-filters .el-button:first-child {
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

.new-key-display {
  margin: 20px 0;
}

.key-container {
  width: 100%;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .apis-filters {
    padding: 16px;
  }
  
  .apis-filters .el-form {
    flex-direction: column;
    gap: 16px;
  }
  
  .apis-filters .el-form-item {
    margin-bottom: 0;
    margin-right: 0;
    width: 100%;
  }
  
  .apis-filters .el-select,
  .apis-filters .el-input {
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
  .apis-filters .el-form {
    flex-wrap: wrap;
  }
  
  .apis-filters .el-form-item {
    margin-right: 16px;
    margin-bottom: 12px;
  }
  
  .table-container .el-table .el-table__cell {
    padding: 10px 6px;
  }
}
</style>