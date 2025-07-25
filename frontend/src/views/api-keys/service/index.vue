<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span class="card-title">服务密钥管理</span>
          <el-button type="primary" @click="handleAdd">
            <el-icon><Plus /></el-icon>
            创建API密钥
          </el-button>
        </div>
      </template>
      <el-table v-loading="loading" :data="tableData" style="width: 100%" stripe>
        <el-table-column prop="name" label="名称" />
        <el-table-column prop="key_prefix" label="API密钥" />
        <el-table-column prop="scopes" label="权限范围">
          <template #default="{ row }">
            <el-tag v-for="scope in row.scopes.slice(0, 2)" :key="scope" size="small" style="margin-right: 4px;">
              {{ scope }}
            </el-tag>
            <span v-if="row.scopes.length > 2">+{{ row.scopes.length - 2 }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="usage_count" label="使用次数" />
        <el-table-column prop="status" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.status === 'active' ? 'success' : 'info'">
              {{ row.status }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间">
          <template #default="{ row }">
            {{ new Date(row.created_at).toLocaleDateString() }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <div class="action-buttons">
              <el-button type="primary" link size="small" @click="handleView(row)">
                <el-icon><View /></el-icon>
                查看
              </el-button>
              <el-button type="warning" link size="small" @click="handleEdit(row)">
                <el-icon><Edit /></el-icon>
                编辑
              </el-button>
              <el-popconfirm
                title="确定要撤销这个API密钥吗？撤销后将无法恢复！"
                confirm-button-text="确定撤销"
                cancel-button-text="取消"
                icon="WarningFilled"
                icon-color="#f56c6c"
                @confirm="handleDelete(row)"
              >
                <template #reference>
                  <el-button type="danger" link size="small">
                    <el-icon><Delete /></el-icon>
                    撤销
                  </el-button>
                </template>
              </el-popconfirm>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="500" @close="handleCloseDialog">
      <el-form ref="formRef" :model="formData" :rules="formRules" label-width="80px">
        <el-form-item label="名称" prop="name">
          <el-input v-model="formData.name" :disabled="isView" />
        </el-form-item>
        <el-form-item label="描述" prop="description">
          <el-input v-model="formData.description" type="textarea" rows="2" :disabled="isView" />
        </el-form-item>
        <el-form-item label="有效期(天)" prop="expires_in_days" v-if="!isEdit">
          <el-input-number v-model="formData.expires_in_days" :min="1" :max="365" />
        </el-form-item>
        <el-form-item label="状态" prop="status" v-if="isEdit">
          <el-radio-group v-model="formData.status" :disabled="isView">
            <el-radio label="active">激活</el-radio>
            <el-radio label="inactive">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="dialogVisible = false">{{ isView ? '关闭' : '取消' }}</el-button>
          <el-button v-if="!isView" type="primary" @click="handleSave">
            {{ isEdit ? '更新' : '创建' }}
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive, computed } from 'vue'
import { Plus, View, Edit, Delete } from '@element-plus/icons-vue'
import { getServiceKeys, addServiceKey, updateServiceKey, deleteServiceKey, type ServiceKey } from '@/api/serviceKey'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'

// State
const loading = ref(true)
const tableData = ref<ServiceKey[]>([])
const dialogVisible = ref(false)
const isEdit = ref(false)
const isView = ref(false)
const formRef = ref<FormInstance>()
let currentServiceKey: ServiceKey | null = null

const getInitialFormData = () => ({
  name: '',
  description: '',
  expires_in_days: 30,
  status: 'active' as 'active' | 'inactive'
})
const formData = reactive(getInitialFormData())

// Computed
const dialogTitle = computed(() => {
  if (isView.value) return '查看API密钥'
  if (isEdit.value) return '编辑API密钥'
  return '创建API密钥'
})

// Form Rules
const formRules = reactive<FormRules>({
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
})

// Methods
const fetchKeys = async () => {
  loading.value = true
  try {
    const response = await getServiceKeys()
    tableData.value = response.data
  } catch (error) {
    ElMessage.error('获取服务列表失败')
  } finally {
    loading.value = false
  }
}

const handleAdd = () => {
  isEdit.value = false
  isView.value = false
  currentServiceKey = null
  Object.assign(formData, getInitialFormData())
  dialogVisible.value = true
}

const handleView = (row: ServiceKey) => {
  isEdit.value = false
  isView.value = true
  currentServiceKey = row
  formData.name = row.name
  formData.description = row.description || ''
  formData.status = row.status
  dialogVisible.value = true
}

const handleEdit = (row: ServiceKey) => {
  isEdit.value = true
  isView.value = false
  currentServiceKey = row
  formData.name = row.name
  formData.description = row.description || ''
  formData.status = row.status
  dialogVisible.value = true
}

const handleSave = async () => {
  if (!formRef.value) return
  if (isView.value) {
    dialogVisible.value = false
    return
  }
  
  await formRef.value.validate(async (valid) => {
    if (valid) {
      try {
        if (isEdit.value && currentServiceKey) {
          // 更新操作
          const updatedKey: ServiceKey = {
            ...currentServiceKey,
            name: formData.name,
            description: formData.description,
            status: formData.status
          }
          await updateServiceKey(updatedKey)
          ElMessage.success('更新成功')
        } else {
          // 创建操作
          const response = await addServiceKey(formData)
          if (response.data?.key) {
            ElMessageBox.alert(`API密钥创建成功！请立即保存，创建后将无法再次查看：\n\n${response.data.key}`, '重要提示', {
              confirmButtonText: '已保存',
              type: 'success',
            })
          }
          ElMessage.success('创建成功')
        }
        dialogVisible.value = false
        fetchKeys()
      } catch (error) {
        console.error('Operation failed:', error)
        ElMessage.error('操作失败')
      }
    }
  })
}

const handleDelete = async (row: ServiceKey) => {
  try {
    await deleteServiceKey(row.id)
    ElMessage.success('撤销成功')
    fetchKeys()
  } catch (error) {
    console.error('Delete failed:', error)
    ElMessage.error('撤销失败')
  }
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

.action-buttons {
  display: flex;
  gap: 8px;
  align-items: center;
}

.action-buttons .el-button {
  margin: 0;
  padding: 4px 8px;
}

.action-buttons .el-button .el-icon {
  margin-right: 4px;
}

/* 表格样式优化 */
:deep(.el-table) {
  border-radius: 8px;
  overflow: hidden;
}

:deep(.el-table th) {
  background-color: #fafafa;
  font-weight: 600;
  color: #303133;
}

:deep(.el-table--striped .el-table__body tr.el-table__row--striped td) {
  background-color: #fafafa;
}

:deep(.el-table .el-table__cell) {
  padding: 12px 0;
}

/* 状态标签样式 */
:deep(.el-tag) {
  border-radius: 4px;
}

/* 对话框样式 */
:deep(.el-dialog) {
  border-radius: 8px;
}

:deep(.el-dialog__header) {
  padding: 20px 20px 0;
  border-bottom: 1px solid #ebeef5;
  margin-bottom: 20px;
}

:deep(.el-dialog__title) {
  font-size: 16px;
  font-weight: 600;
}

:deep(.el-dialog__body) {
  padding: 20px;
}

:deep(.el-form-item__label) {
  font-weight: 600;
  color: #303133;
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
  
  .action-buttons {
    flex-wrap: wrap;
    gap: 4px;
  }
  
  .action-buttons .el-button {
    font-size: 12px;
    padding: 2px 6px;
  }
  
  :deep(.el-table .el-table__cell) {
    padding: 8px 0;
  }
}

@media (max-width: 480px) {
  .action-buttons {
    flex-direction: column;
    align-items: flex-start;
  }
  
  .action-buttons .el-button {
    width: 100%;
    justify-content: flex-start;
  }
}
</style>