<template>
  <div class="request-logs-view">
    <div class="logs-intro">
      <el-alert
        title="日志查询工具"
        type="info"
        :closable="false"
        show-icon
      >
        <template #default>
          此页面专门用于查询和导出详细的请求日志。如需查看统计分析、趋势图表等，请访问 <router-link to="/statistics" style="color: #409eff;">统计分析</router-link> 页面。
        </template>
      </el-alert>
    </div>
    
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>日志查询</h2>
          <div class="header-actions">
            <el-button @click="refreshLogs" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button @click="exportLogs" :loading="exportLoading">
              <el-icon><Download /></el-icon>
              导出日志
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="logs-content">
        <!-- 筛选器 -->
        <div class="logs-filters">
          <el-form :model="filters" inline>
            <el-form-item label="状态码">
              <el-select v-model="filters.status_code" clearable placeholder="全部">
                <el-option label="200 成功" value="200" />
                <el-option label="400 错误请求" value="400" />
                <el-option label="401 未授权" value="401" />
                <el-option label="429 限流" value="429" />
                <el-option label="500 服务器错误" value="500" />
              </el-select>
            </el-form-item>
            
            <el-form-item label="服务商">
              <el-select v-model="filters.provider_type" clearable placeholder="全部">
                <el-option label="OpenAI" value="openai" />
                <el-option label="Google" value="google" />
                <el-option label="Anthropic" value="anthropic" />
              </el-select>
            </el-form-item>
            
            <el-form-item label="用户">
              <el-input 
                v-model="filters.user_id" 
                placeholder="用户ID" 
                clearable 
                style="width: 120px;"
              />
            </el-form-item>
            
            <el-form-item label="时间范围">
              <el-date-picker
                v-model="dateRange"
                type="datetimerange"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="YYYY-MM-DD HH:mm:ss"
                range-separator="至"
                start-placeholder="开始时间"
                end-placeholder="结束时间"
                @change="handleDateRangeChange"
                style="width: 350px;"
              />
            </el-form-item>
            
            <el-form-item>
              <el-button type="primary" @click="searchLogs">
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

        <!-- 请求日志表格 -->
        <el-table
          :data="logsList"
          v-loading="loading"
          stripe
          border
          style="width: 100%"
          @row-click="showLogDetail"
        >
          <el-table-column prop="timestamp" label="时间" width="180">
            <template #default="{ row }">
              {{ formatTime(row.timestamp) }}
            </template>
          </el-table-column>
          
          <el-table-column prop="method" label="方法" width="80">
            <template #default="{ row }">
              <el-tag :type="getMethodTagType(row.method)" size="small">
                {{ row.method }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="path" label="路径" width="200" show-overflow-tooltip />
          
          <el-table-column prop="status_code" label="状态码" width="100">
            <template #default="{ row }">
              <el-tag :type="getStatusTagType(row.status_code)" size="small">
                {{ row.status_code }}
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
          
          <el-table-column prop="provider_type" label="服务商" width="100">
            <template #default="{ row }">
              <el-tag size="small" v-if="row.provider_type">
                {{ row.provider_type }}
              </el-tag>
              <span v-else class="empty-value">-</span>
            </template>
          </el-table-column>
          
          <el-table-column prop="user_id" label="用户ID" width="100">
            <template #default="{ row }">
              <span v-if="row.user_id">{{ row.user_id }}</span>
              <span v-else class="empty-value">-</span>
            </template>
          </el-table-column>
          
          <el-table-column prop="prompt_tokens" label="输入Token" width="100">
            <template #default="{ row }">
              <span v-if="row.prompt_tokens">{{ formatNumber(row.prompt_tokens) }}</span>
              <span v-else class="empty-value">-</span>
            </template>
          </el-table-column>
          
          <el-table-column prop="completion_tokens" label="输出Token" width="100">
            <template #default="{ row }">
              <span v-if="row.completion_tokens">{{ formatNumber(row.completion_tokens) }}</span>
              <span v-else class="empty-value">-</span>
            </template>
          </el-table-column>
          
          <el-table-column prop="client_ip" label="客户端IP" width="130" show-overflow-tooltip />
          
          <el-table-column prop="user_agent" label="User Agent" show-overflow-tooltip>
            <template #default="{ row }">
              <span class="user-agent-text">{{ row.user_agent || '-' }}</span>
            </template>
          </el-table-column>
          
          <el-table-column label="操作" width="80" fixed="right">
            <template #default="{ row }">
              <el-button type="text" size="small" @click.stop="showLogDetail(row)">
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
            :page-sizes="[20, 50, 100, 200]"
            :total="pagination.total"
            layout="total, sizes, prev, pager, next, jumper"
            @size-change="handleSizeChange"
            @current-change="handleCurrentChange"
          />
        </div>
      </div>
    </el-card>

    <!-- 日志详情对话框 -->
    <el-dialog
      v-model="detailVisible"
      title="请求日志详情"
      width="80%"
      :max-width="1000"
    >
      <div v-if="selectedLog" class="log-detail">
        <el-tabs v-model="activeTab" type="border-card">
          <!-- 基本信息 -->
          <el-tab-pane label="基本信息" name="basic">
            <el-descriptions :column="2" border>
              <el-descriptions-item label="请求时间">
                {{ formatTime(selectedLog.timestamp) }}
              </el-descriptions-item>
              <el-descriptions-item label="请求方法">
                <el-tag :type="getMethodTagType(selectedLog.method)">
                  {{ selectedLog.method }}
                </el-tag>
              </el-descriptions-item>
              <el-descriptions-item label="请求路径">
                {{ selectedLog.path }}
              </el-descriptions-item>
              <el-descriptions-item label="状态码">
                <el-tag :type="getStatusTagType(selectedLog.status_code)">
                  {{ selectedLog.status_code }}
                </el-tag>
              </el-descriptions-item>
              <el-descriptions-item label="响应时间">
                <span :class="getResponseTimeClass(selectedLog.response_time)">
                  {{ selectedLog.response_time }}ms
                </span>
              </el-descriptions-item>
              <el-descriptions-item label="服务商">
                {{ selectedLog.provider_type || '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="用户ID">
                {{ selectedLog.user_id || '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="客户端IP">
                {{ selectedLog.client_ip }}
              </el-descriptions-item>
              <el-descriptions-item label="输入Token数" :span="1">
                {{ selectedLog.prompt_tokens || '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="输出Token数" :span="1">
                {{ selectedLog.completion_tokens || '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="User Agent" :span="2">
                <div class="user-agent-detail">
                  {{ selectedLog.user_agent || '-' }}
                </div>
              </el-descriptions-item>
            </el-descriptions>
          </el-tab-pane>

          <!-- 请求详情 -->
          <el-tab-pane label="请求详情" name="request">
            <div class="request-detail">
              <h4>请求头</h4>
              <pre class="json-content">{{ formatJson(selectedLog.request_headers) }}</pre>
              
              <h4>请求体</h4>
              <pre class="json-content">{{ formatJson(selectedLog.request_body) }}</pre>
            </div>
          </el-tab-pane>

          <!-- 响应详情 -->
          <el-tab-pane label="响应详情" name="response">
            <div class="response-detail">
              <h4>响应头</h4>
              <pre class="json-content">{{ formatJson(selectedLog.response_headers) }}</pre>
              
              <h4>响应体</h4>
              <pre class="json-content">{{ formatJson(selectedLog.response_body) }}</pre>
            </div>
          </el-tab-pane>

          <!-- 错误信息 -->
          <el-tab-pane label="错误信息" name="error" v-if="selectedLog.error_message">
            <div class="error-detail">
              <el-alert
                :title="selectedLog.error_type || '错误'"
                type="error"
                :description="selectedLog.error_message"
                show-icon
                :closable="false"
              />
              
              <div v-if="selectedLog.error_stack" class="error-stack">
                <h4>错误堆栈</h4>
                <pre class="stack-trace">{{ selectedLog.error_stack }}</pre>
              </div>
            </div>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-dialog>

    <!-- 导出对话框 -->
    <el-dialog
      v-model="exportVisible"
      title="导出日志"
      width="500px"
    >
      <el-form :model="exportForm" label-width="100px">
        <el-form-item label="导出格式">
          <el-radio-group v-model="exportForm.format">
            <el-radio label="csv">CSV</el-radio>
            <el-radio label="xlsx">Excel</el-radio>
          </el-radio-group>
        </el-form-item>
        
        <el-form-item label="时间范围">
          <el-date-picker
            v-model="exportDateRange"
            type="datetimerange"
            format="YYYY-MM-DD HH:mm:ss"
            value-format="YYYY-MM-DD HH:mm:ss"
            range-separator="至"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            @change="handleExportDateRangeChange"
            style="width: 100%;"
          />
        </el-form-item>
        
        <el-form-item label="包含字段">
          <el-checkbox-group v-model="exportForm.fields">
            <el-checkbox label="timestamp">时间</el-checkbox>
            <el-checkbox label="method">方法</el-checkbox>
            <el-checkbox label="path">路径</el-checkbox>
            <el-checkbox label="status_code">状态码</el-checkbox>
            <el-checkbox label="response_time">响应时间</el-checkbox>
            <el-checkbox label="provider_type">服务商</el-checkbox>
            <el-checkbox label="user_id">用户ID</el-checkbox>
            <el-checkbox label="client_ip">客户端IP</el-checkbox>
            <el-checkbox label="prompt_tokens">输入Token</el-checkbox>
            <el-checkbox label="completion_tokens">输出Token</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="exportVisible = false">取消</el-button>
          <el-button type="primary" @click="confirmExport" :loading="exportLoading">
            导出
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Refresh, Download, Search, RefreshLeft
} from '@element-plus/icons-vue'
import { StatisticsAPI } from '@/api'
import type { RequestLog, RequestLogParams } from '@/types'

// 状态
const loading = ref(false)
const exportLoading = ref(false)
const detailVisible = ref(false)
const exportVisible = ref(false)
const selectedLog = ref<RequestLog | null>(null)
const activeTab = ref('basic')

// 数据
const logsList = ref<RequestLog[]>([])
const dateRange = ref<[string, string] | null>(null)
const exportDateRange = ref<[string, string] | null>(null)

// 筛选器
const filters = reactive<RequestLogParams>({
  status_code: '',
  provider_type: '',
  user_id: '',
  start_time: '',
  end_time: ''
})

// 分页
const pagination = reactive({
  page: 1,
  size: 20,
  total: 0
})

// 导出表单
const exportForm = reactive({
  format: 'csv' as 'csv' | 'xlsx',
  start_time: '',
  end_time: '',
  fields: [
    'timestamp', 'method', 'path', 'status_code', 'response_time',
    'provider_type', 'user_id', 'client_ip', 'prompt_tokens', 'completion_tokens'
  ]
})

// 获取请求日志列表
const fetchLogs = async () => {
  try {
    loading.value = true
    const params: RequestLogParams = {
      ...filters,
      page: pagination.page,
      page_size: pagination.size,
      status_code: filters.status_code || undefined,
      provider_type: filters.provider_type || undefined,
      user_id: filters.user_id ? parseInt(filters.user_id) : undefined,
      start_time: filters.start_time || undefined,
      end_time: filters.end_time || undefined
    }
    
    const response = await StatisticsAPI.getRequestLogs(params)
    logsList.value = response.logs
    pagination.total = response.pagination?.total || 0
  } catch (error: any) {
    ElMessage.error(error.message || '获取请求日志失败')
    console.error('Failed to fetch logs:', error)
  } finally {
    loading.value = false
  }
}

// 刷新日志
const refreshLogs = () => {
  fetchLogs()
}

// 搜索日志
const searchLogs = () => {
  pagination.page = 1
  fetchLogs()
}

// 重置筛选器
const resetFilters = () => {
  Object.assign(filters, {
    status_code: '',
    provider_type: '',
    user_id: '',
    start_time: '',
    end_time: ''
  })
  dateRange.value = null
  pagination.page = 1
  fetchLogs()
}

// 处理时间范围变化
const handleDateRangeChange = (value: [string, string] | null) => {
  if (value) {
    filters.start_time = value[0]
    filters.end_time = value[1]
  } else {
    filters.start_time = ''
    filters.end_time = ''
  }
}

// 处理导出时间范围变化
const handleExportDateRangeChange = (value: [string, string] | null) => {
  if (value) {
    exportForm.start_time = value[0]
    exportForm.end_time = value[1]
  } else {
    exportForm.start_time = ''
    exportForm.end_time = ''
  }
}

// 显示日志详情
const showLogDetail = async (log: RequestLog) => {
  try {
    // 获取完整的日志详情
    selectedLog.value = await StatisticsAPI.getRequestLog(log.id)
    detailVisible.value = true
    activeTab.value = 'basic'
  } catch (error: any) {
    ElMessage.error(error.message || '获取日志详情失败')
  }
}

// 导出日志
const exportLogs = () => {
  // 设置默认导出时间范围（最近7天）
  if (!exportDateRange.value) {
    const endTime = new Date()
    const startTime = new Date(endTime.getTime() - 7 * 24 * 60 * 60 * 1000)
    exportDateRange.value = [
      startTime.toISOString().slice(0, 19).replace('T', ' '),
      endTime.toISOString().slice(0, 19).replace('T', ' ')
    ]
    exportForm.start_time = exportDateRange.value[0]
    exportForm.end_time = exportDateRange.value[1]
  }
  
  exportVisible.value = true
}

// 确认导出
const confirmExport = async () => {
  try {
    exportLoading.value = true
    
    const params = {
      format: exportForm.format,
      start_time: exportForm.start_time,
      end_time: exportForm.end_time,
      fields: exportForm.fields.join(','),
      ...filters
    }
    
    await StatisticsAPI.exportRequestLogs(params)
    ElMessage.success('导出请求已提交，请稍候下载')
    exportVisible.value = false
  } catch (error: any) {
    ElMessage.error(error.message || '导出失败')
  } finally {
    exportLoading.value = false
  }
}

// 分页处理
const handleSizeChange = (size: number) => {
  pagination.size = size
  pagination.page = 1
  fetchLogs()
}

const handleCurrentChange = (page: number) => {
  pagination.page = page
  fetchLogs()
}

// 工具函数
const formatTime = (timestamp: string) => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const formatNumber = (num: number) => {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toString()
}

const formatJson = (obj: any) => {
  if (!obj) return '-'
  if (typeof obj === 'string') {
    try {
      return JSON.stringify(JSON.parse(obj), null, 2)
    } catch {
      return obj
    }
  }
  return JSON.stringify(obj, null, 2)
}

const getMethodTagType = (method: string) => {
  const typeMap: Record<string, string> = {
    'GET': 'primary',
    'POST': 'success',
    'PUT': 'warning',
    'DELETE': 'danger',
    'PATCH': 'info'
  }
  return typeMap[method] || 'info'
}

const getStatusTagType = (status: number) => {
  if (status >= 200 && status < 300) return 'success'
  if (status >= 300 && status < 400) return 'info'
  if (status >= 400 && status < 500) return 'warning'
  if (status >= 500) return 'danger'
  return 'info'
}

const getResponseTimeClass = (responseTime: number) => {
  if (responseTime < 500) return 'response-time-good'
  if (responseTime < 2000) return 'response-time-normal'
  return 'response-time-slow'
}

onMounted(() => {
  fetchLogs()
})
</script>

<style scoped>
.request-logs-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.logs-intro {
  flex-shrink: 0;
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

.logs-content {
  height: calc(100% - 60px);
  display: flex;
  flex-direction: column;
}

.logs-filters {
  margin-bottom: 20px;
  padding: 16px;
  background: #f8f9fa;
  border-radius: 6px;
}

.empty-value {
  color: #c0c4cc;
  font-style: italic;
}

.user-agent-text {
  font-size: 12px;
  color: #666;
}

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

.pagination-wrapper {
  margin-top: 20px;
  display: flex;
  justify-content: center;
}

/* 日志详情样式 */
.log-detail {
  height: 600px;
  overflow-y: auto;
}

.user-agent-detail {
  word-break: break-all;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  background: #f5f5f5;
  padding: 8px;
  border-radius: 4px;
}

.request-detail,
.response-detail {
  height: 500px;
  overflow-y: auto;
}

.request-detail h4,
.response-detail h4 {
  margin: 20px 0 10px 0;
  color: #333;
  font-size: 14px;
}

.json-content {
  background: #f5f5f5;
  padding: 12px;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.4;
  margin: 0 0 20px 0;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 200px;
  overflow-y: auto;
}

.error-detail {
  height: 500px;
  overflow-y: auto;
}

.error-stack {
  margin-top: 20px;
}

.error-stack h4 {
  color: #333;
  font-size: 14px;
  margin-bottom: 10px;
}

.stack-trace {
  background: #f5f5f5;
  padding: 12px;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.4;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
  color: #f56c6c;
}

.dialog-footer {
  text-align: right;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .logs-filters .el-form {
    flex-direction: column;
  }
  
  .logs-filters .el-form-item {
    margin-bottom: 16px;
    margin-right: 0;
  }
}

/* Element Plus 样式覆盖 */
:deep(.el-table .cell) {
  padding: 8px 12px;
}

:deep(.el-table__row) {
  cursor: pointer;
}

:deep(.el-table__row:hover) {
  background-color: #f5f7fa;
}

:deep(.el-tabs__content) {
  height: 520px;
  overflow-y: auto;
}

:deep(.el-checkbox-group) {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}
</style>