<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span class="card-title">号池密钥管理</span>
          <el-button type="primary" @click="handleAdd" class="add-button">
            <el-icon><Plus /></el-icon>
            新增密钥
          </el-button>
        </div>
      </template>
      
      <el-table v-loading="loading" :data="tableData" style="width: 100%" stripe>
        <el-table-column prop="name" label="名称" min-width="120" />
        <el-table-column prop="provider_display_name" label="服务商" min-width="100">
          <template #default="{ row }">
            <el-tag :type="getProviderType(row.provider_type || row.provider)" size="small">
              {{ row.provider_display_name || row.provider }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="api_key_prefix" label="API密钥" min-width="120">
          <template #default="{ row }">
            <code class="api-key-display">{{ row.api_key_prefix || row.apiKey }}</code>
          </template>
        </el-table-column>
        <el-table-column prop="weight" label="权重" width="80">
          <template #default="{ row }">
            {{ row.weight || 1 }}
          </template>
        </el-table-column>
        <el-table-column prop="status" label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 'active' ? 'success' : 'danger'" size="small">
              {{ row.status === 'active' ? '激活' : '禁用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" min-width="100">
          <template #default="{ row }">
            {{ formatDate(row.created_at) }}
          </template>
        </el-table-column>
        <el-table-column prop="last_used" label="最后使用" min-width="100">
          <template #default="{ row }">
            {{ row.last_used ? formatDate(row.last_used) : (row.lastUsed || '未使用') }}
          </template>
        </el-table-column>
        <el-table-column fixed="right" label="操作" width="150">
          <template #default="{ row }">
            <el-button type="primary" link size="small" @click="handleEdit(row)">
              编辑
            </el-button>
            <el-popconfirm
              title="确定要删除这个密钥吗？"
              confirm-button-text="确定删除"
              cancel-button-text="取消"
              icon="WarningFilled"
              icon-color="#f56c6c"
              @confirm="confirmDelete(row)"
            >
              <template #reference>
                <el-button type="danger" link size="small">
                  删除
                </el-button>
              </template>
            </el-popconfirm>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog 
      v-model="dialogVisible" 
      :title="dialogTitle" 
      width="600px" 
      @close="handleCloseDialog"
      :close-on-click-modal="false"
    >
      <el-form ref="formRef" :model="formData" :rules="formRules" label-width="100px">
        <el-form-item label="密钥名称" prop="name">
          <el-input v-model="formData.name" placeholder="请输入密钥名称" />
        </el-form-item>
        <el-form-item label="服务商" prop="provider">
          <el-select v-model="formData.provider" placeholder="请选择服务商" style="width: 100%">
            <el-option label="OpenAI" value="openai">
              <span class="provider-option">
                <el-tag type="primary" size="small">OpenAI</el-tag>
                <span style="margin-left: 8px;">ChatGPT、GPT-4等</span>
              </span>
            </el-option>
            <el-option label="Google Gemini" value="gemini">
              <span class="provider-option">
                <el-tag type="success" size="small">Gemini</el-tag>
                <span style="margin-left: 8px;">Gemini Pro、Gemini Ultra</span>
              </span>
            </el-option>
            <el-option label="Anthropic Claude" value="claude">
              <span class="provider-option">
                <el-tag type="warning" size="small">Claude</el-tag>
                <span style="margin-left: 8px;">Claude-3、Claude-2等</span>
              </span>
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item label="API密钥" prop="apiKey">
          <el-input 
            v-model="formData.apiKey" 
            type="password" 
            show-password
            placeholder="请输入API密钥"
          />
        </el-form-item>
        <el-form-item label="权重" prop="weight">
          <el-input-number 
            v-model="formData.weight" 
            :min="1" 
            :max="100"
            controls-position="right"
            style="width: 100%"
          />
          <div class="form-tip">权重越高，被选中的概率越大</div>
        </el-form-item>
        <el-form-item label="状态" prop="status">
          <el-radio-group v-model="formData.status">
            <el-radio label="active">激活</el-radio>
            <el-radio label="inactive">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" @click="handleSave" :loading="saveLoading">
            {{ isEdit ? '更新' : '创建' }}
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive, computed } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { getProviderKeys, addProviderKey, updateProviderKey, deleteProviderKey, type ProviderKey } from '@/api/apiKey'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'

// State
const loading = ref(true)
const saveLoading = ref(false)
const tableData = ref<ProviderKey[]>([])
const dialogVisible = ref(false)
const isEdit = ref(false)
const formRef = ref<FormInstance>()
let currentKeyId: number | null = null


const getInitialFormData = () => ({
  name: '',
  provider: 'openai' as 'openai' | 'gemini' | 'claude',
  apiKey: '',
  weight: 1,
  status: 'active' as 'active' | 'inactive',
})
const formData = reactive(getInitialFormData())

// Computed
const dialogTitle = computed(() => (isEdit.value ? '编辑密钥' : '新增密钥'))

// Form Rules
const formRules = reactive<FormRules>({
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  provider: [{ required: true, message: '请选择服务商', trigger: 'change' }],
  apiKey: [{ required: true, message: '请输入API密钥', trigger: 'blur' }],
})

// Methods
const fetchKeys = async () => {
  loading.value = true
  try {
    const response = await getProviderKeys()
    tableData.value = response.data
  } catch (error) {
    ElMessage.error('获取密钥列表失败')
  } finally {
    loading.value = false
  }
}

const handleAdd = () => {
  isEdit.value = false
  currentKeyId = null
  Object.assign(formData, getInitialFormData())
  dialogVisible.value = true
}

const handleEdit = (row: ProviderKey) => {
  isEdit.value = true
  currentKeyId = row.id
  Object.assign(formData, row)
  dialogVisible.value = true
}

const handleSave = async () => {
  if (!formRef.value) return
  await formRef.value.validate(async (valid) => {
    if (valid) {
      saveLoading.value = true
      try {
        if (isEdit.value && currentKeyId) {
          await updateProviderKey({ ...formData, id: currentKeyId } as ProviderKey)
          ElMessage.success('更新成功')
        } else {
          await addProviderKey(formData)
          ElMessage.success('新增成功')
        }
        dialogVisible.value = false
        fetchKeys()
      } catch (error) {
        console.error('Save failed:', error)
        ElMessage.error('操作失败')
      } finally {
        saveLoading.value = false
      }
    }
  })
}

// 删除确认方法
const confirmDelete = async (row: ProviderKey) => {
  try {
    await deleteProviderKey(row.id)
    ElMessage.success('删除成功')
    fetchKeys()
  } catch (error) {
    console.error('Delete failed:', error)
    ElMessage.error('删除失败')
  }
}

// 获取服务商类型样式
const getProviderType = (provider: string) => {
  const typeMap: Record<string, string> = {
    'openai': 'primary',
    'gemini': 'success', 
    'claude': 'warning'
  }
  return typeMap[provider] || 'info'
}

// 格式化日期
const formatDate = (dateStr: string | null) => {
  if (!dateStr) return ''
  return new Date(dateStr).toLocaleDateString('zh-CN')
}

const handleCloseDialog = () => {
  formRef.value?.clearValidate()
}

// Lifecycle
onMounted(() => {
  fetchKeys()
})
</script>

<style scoped>
.page-container {
  padding: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-title {
  font-size: 18px;
  font-weight: 600;
  color: #303133;
}

.add-button {
  margin-left: auto;
}

.api-key-display {
  background-color: #f5f7fa;
  padding: 2px 6px;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 12px;
  color: #606266;
}

.provider-option {
  display: flex;
  align-items: center;
}

.form-tip {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}

.dialog-footer {
  text-align: right;
}

.dialog-footer .el-button {
  margin-left: 10px;
}

/* 表格样式优化 */
:deep(.el-table th) {
  background-color: #fafafa;
  font-weight: 600;
}

:deep(.el-table--striped .el-table__body tr.el-table__row--striped td) {
  background-color: #fafafa;
}

/* 响应式处理 */
@media (max-width: 768px) {
  .page-container {
    padding: 10px;
  }
  
  .card-header {
    flex-direction: column;
    gap: 10px;
    align-items: stretch;
  }
  
  .add-button {
    margin-left: 0;
  }
}
</style>
