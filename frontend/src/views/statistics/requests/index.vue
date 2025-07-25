<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ $t('menu.requestLogs') }}</span>
        </div>
      </template>

      <!-- Filter Section -->
      <el-form :inline="true" :model="filters" class="filter-form">
        <el-form-item label="状态码">
          <el-select v-model="filters.statusCode" placeholder="任意状态码" clearable>
            <el-option label="Success (200)" :value="200" />
            <el-option label="Unauthorized (401)" :value="401" />
            <el-option label="Error (500)" :value="500" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleFilter">查询</el-button>
        </el-form-item>
      </el-form>

      <!-- Table Section -->
      <el-table v-loading="loading" :data="tableData" style="width: 100%">
        <el-table-column prop="id" label="请求ID" width="180" />
        <el-table-column prop="path" label="路径" />
        <el-table-column prop="statusCode" label="状态码">
           <template #default="{ row }">
            <el-tag :type="row.statusCode === 200 ? 'success' : 'danger'">
              {{ row.statusCode }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="responseTime" label="响应时间 (ms)" />
        <el-table-column prop="totalTokens" label="Tokens" />
        <el-table-column prop="modelUsed" label="模型" />
        <el-table-column prop="createdAt" label="创建时间" width="180" />
      </el-table>

      <!-- Pagination Section -->
      <el-pagination
        background
        layout="prev, pager, next, total"
        :total="total"
        :page-size="pagination.limit"
        :current-page="pagination.page"
        @current-change="handlePageChange"
        class="pagination-container"
      />
    </el-card>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive } from 'vue'
import { getRequestLogs, type RequestLog } from '@/api/requestLog'
import { ElMessage } from 'element-plus'

// State
const loading = ref(true)
const tableData = ref<RequestLog[]>([])
const total = ref(0)

const filters = reactive({
  statusCode: undefined,
})

const pagination = reactive({
  page: 1,
  limit: 10,
})

// Methods
const fetchLogs = async () => {
  loading.value = true
  try {
    const params = {
      page: pagination.page,
      limit: pagination.limit,
      statusCode: filters.statusCode,
    }
    const response = await getRequestLogs(params)
    tableData.value = response.data.logs
    total.value = response.data.total
  } catch (error) {
    ElMessage.error('获取日志失败')
  } finally {
    loading.value = false
  }
}

const handleFilter = () => {
  pagination.page = 1
  fetchLogs()
}

const handlePageChange = (page: number) => {
  pagination.page = page
  fetchLogs()
}

// Lifecycle
onMounted(() => {
  fetchLogs()
})
</script>

<style scoped>
.page-container {
  padding: 10px;
}
.filter-form {
  background-color: #f5f7fa;
  padding: 16px;
  border-radius: 4px;
  margin-bottom: 20px;
}
.pagination-container {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}
</style>
