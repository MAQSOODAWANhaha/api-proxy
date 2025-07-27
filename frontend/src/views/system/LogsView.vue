<template>
  <div class="logs-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>系统日志</h2>
          <div class="header-actions">
            <el-button @click="refreshLogs" :loading="loading">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button @click="downloadLogs">
              <el-icon><Download /></el-icon>
              下载日志
            </el-button>
          </div>
        </div>
      </template>
      
      <div class="logs-content">
        <!-- 筛选器 -->
        <div class="logs-filters">
          <el-form :model="filters" inline>
            <el-form-item label="日志级别">
              <el-select v-model="filters.level" clearable placeholder="全部">
                <el-option label="DEBUG" value="debug" />
                <el-option label="INFO" value="info" />
                <el-option label="WARN" value="warn" />
                <el-option label="ERROR" value="error" />
              </el-select>
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
        
        <!-- 日志表格 -->
        <el-table
          :data="logs"
          v-loading="loading"
          height="500"
          stripe
          border
          style="width: 100%"
        >
          <el-table-column prop="timestamp" label="时间" width="180">
            <template #default="{ row }">
              {{ formatTimestamp(row.timestamp) }}
            </template>
          </el-table-column>
          
          <el-table-column prop="level" label="级别" width="80">
            <template #default="{ row }">
              <el-tag :type="getLevelTagType(row.level)" size="small">
                {{ row.level.toUpperCase() }}
              </el-tag>
            </template>
          </el-table-column>
          
          <el-table-column prop="module" label="模块" width="120" />
          
          <el-table-column prop="message" label="日志内容" show-overflow-tooltip>
            <template #default="{ row }">
              <span :class="getMessageClass(row.level)">{{ row.message }}</span>
            </template>
          </el-table-column>
          
          <el-table-column label="操作" width="100">
            <template #default="{ row }">
              <el-button
                type="text"
                size="small"
                @click="showLogDetail(row)"
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
    
    <!-- 日志详情对话框 -->
    <el-dialog
      v-model="detailVisible"
      title="日志详情"
      width="80%"
      :max-width="800"
    >
      <div v-if="selectedLog" class="log-detail">
        <el-descriptions :column="1" border>
          <el-descriptions-item label="时间">
            {{ formatTimestamp(selectedLog.timestamp) }}
          </el-descriptions-item>
          <el-descriptions-item label="级别">
            <el-tag :type="getLevelTagType(selectedLog.level)">
              {{ selectedLog.level.toUpperCase() }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="模块">
            {{ selectedLog.module }}
          </el-descriptions-item>
          <el-descriptions-item label="消息">
            <pre class="log-message">{{ selectedLog.message }}</pre>
          </el-descriptions-item>
          <el-descriptions-item v-if="selectedLog.details" label="详细信息">
            <pre class="log-details">{{ JSON.stringify(selectedLog.details, null, 2) }}</pre>
          </el-descriptions-item>
        </el-descriptions>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh, Download, Search, RefreshLeft } from '@element-plus/icons-vue'
import { SystemAPI } from '@/api'

const loading = ref(false)
const detailVisible = ref(false)
const selectedLog = ref<any>(null)
const dateRange = ref<[string, string] | null>(null)

const logs = ref<any[]>([])

const filters = reactive({
  level: '',
  start_time: '',
  end_time: ''
})

const pagination = reactive({
  page: 1,
  size: 20,
  total: 0
})

const fetchLogs = async () => {
  try {
    loading.value = true
    const params = {
      level: filters.level || undefined,
      start_time: filters.start_time || undefined,
      end_time: filters.end_time || undefined,
      limit: pagination.size,
      offset: (pagination.page - 1) * pagination.size
    }
    
    const response = await SystemAPI.getSystemLogs(params)
    logs.value = response.logs
    pagination.total = response.total
  } catch (error: any) {
    ElMessage.error(error.message || '获取日志失败')
    console.error('Failed to fetch logs:', error)
  } finally {
    loading.value = false
  }
}

const refreshLogs = () => {
  fetchLogs()
}

const searchLogs = () => {
  pagination.page = 1
  fetchLogs()
}

const resetFilters = () => {
  filters.level = ''
  filters.start_time = ''
  filters.end_time = ''
  dateRange.value = null
  pagination.page = 1
  fetchLogs()
}

const handleDateRangeChange = (value: [string, string] | null) => {
  if (value) {
    filters.start_time = value[0]
    filters.end_time = value[1]
  } else {
    filters.start_time = ''
    filters.end_time = ''
  }
}

const handleSizeChange = (size: number) => {
  pagination.size = size
  pagination.page = 1
  fetchLogs()
}

const handleCurrentChange = (page: number) => {
  pagination.page = page
  fetchLogs()
}

const showLogDetail = (log: any) => {
  selectedLog.value = log
  detailVisible.value = true
}

const downloadLogs = async () => {
  try {
    await SystemAPI.downloadSystemLogs({
      level: filters.level || undefined,
      start_time: filters.start_time || undefined,
      end_time: filters.end_time || undefined
    })
    ElMessage.success('日志下载已开始')
  } catch (error: any) {
    ElMessage.error(error.message || '下载日志失败')
  }
}

const formatTimestamp = (timestamp: string) => {
  return new Date(timestamp).toLocaleString('zh-CN')
}

const getLevelTagType = (level: string) => {
  const levelMap: Record<string, string> = {
    debug: 'info',
    info: 'success',
    warn: 'warning',
    error: 'danger'
  }
  return levelMap[level.toLowerCase()] || 'info'
}

const getMessageClass = (level: string) => {
  return `log-message-${level.toLowerCase()}`
}

onMounted(() => {
  fetchLogs()
})
</script>

<style scoped>
.logs-view {
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

.pagination-wrapper {
  margin-top: 20px;
  display: flex;
  justify-content: center;
}

.log-detail {
  max-height: 500px;
  overflow-y: auto;
}

.log-message,
.log-details {
  white-space: pre-wrap;
  word-break: break-all;
  background: #f5f5f5;
  padding: 12px;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.4;
  margin: 0;
}

.log-message-debug {
  color: #909399;
}

.log-message-info {
  color: #67c23a;
}

.log-message-warn {
  color: #e6a23c;
}

.log-message-error {
  color: #f56c6c;
  font-weight: 500;
}

:deep(.el-table .cell) {
  word-break: break-all;
}
</style>