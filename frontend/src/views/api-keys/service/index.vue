<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ $t('menu.serviceKeys') }}</span>
          <el-button type="primary" @click="handleAdd">
            <el-icon><Plus /></el-icon>
            创建服务
          </el-button>
        </div>
      </template>
      <el-table v-loading="loading" :data="tableData" style="width: 100%">
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
        <el-table-column label="操作" width="200">
          <template #default="{ row }">
            <el-button type="primary" link @click="handleEdit(row)">查看</el-button>
            <el-button type="danger" link @click="handleDelete(row)">撤销</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="500" @close="handleCloseDialog">
      <el-form ref="formRef" :model="formData" :rules="formRules" label-width="80px">
        <el-form-item label="名称" prop="name">
          <el-input v-model="formData.name" />
        </el-form-item>
        <el-form-item label="描述" prop="description">
          <el-input v-model="formData.description" type="textarea" rows="2" />
        </el-form-item>
        <el-form-item label="有效期(天)" prop="expires_in_days">
          <el-input-number v-model="formData.expires_in_days" :min="1" :max="365" />
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" @click="handleSave">确认</el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive, computed } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { getServiceKeys, addServiceKey, updateServiceKey, deleteServiceKey, type ServiceKey } from '@/api/serviceKey'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'

// State
const loading = ref(true)
const tableData = ref<ServiceKey[]>([])
const dialogVisible = ref(false)
const isEdit = ref(false)
const formRef = ref<FormInstance>()

const getInitialFormData = () => ({
  name: '',
  description: '',
  expires_in_days: 30,
})
const formData = reactive(getInitialFormData())

// Computed
const dialogTitle = computed(() => (isEdit.value ? '查看API密钥' : '创建API密钥'))

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
  Object.assign(formData, getInitialFormData())
  dialogVisible.value = true
}

const handleEdit = (row: ServiceKey) => {
  isEdit.value = true
  // For viewing purposes, show the key information
  formData.name = row.name
  formData.description = row.description || ''
  formData.expires_in_days = row.expires_at ? 
    Math.ceil((new Date(row.expires_at).getTime() - new Date().getTime()) / (1000 * 60 * 60 * 24)) : 
    30
  dialogVisible.value = true
}

const handleSave = async () => {
  if (!formRef.value) return
  await formRef.value.validate(async (valid) => {
    if (valid) {
      try {
        if (isEdit.value) {
          ElMessage.info('查看模式，无法编辑')
        } else {
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

const handleDelete = (row: ServiceKey) => {
  ElMessageBox.confirm(`确定要撤销API密钥 "${row.name}" 吗？撤销后将无法恢复！`, '警告', {
    confirmButtonText: '确定撤销',
    cancelButtonText: '取消',
    type: 'warning',
  }).then(async () => {
    try {
      await deleteServiceKey(row.id)
      ElMessage.success('撤销成功')
      fetchKeys()
    } catch (error) {
      console.error('Delete failed:', error)
      ElMessage.error('撤销失败')
    }
  })
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
  padding: 10px;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
</style>