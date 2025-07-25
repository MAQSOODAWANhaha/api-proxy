<template>
  <div class="page-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ $t('menu.providerKeys') }}</span>
          <el-button type="primary" @click="handleAdd">
            <el-icon><Plus /></el-icon>
            新增密钥
          </el-button>
        </div>
      </template>
      <el-table v-loading="loading" :data="tableData" style="width: 100%">
        <el-table-column prop="name" label="名称" />
        <el-table-column prop="provider" label="服务商" />
        <el-table-column prop="weight" label="权重" />
        <el-table-column prop="status" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.status === 'active' ? 'success' : 'info'">
              {{ row.status }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200">
          <template #default="{ row }">
            <el-button type="primary" link @click="handleEdit(row)">编辑</el-button>
            <el-button type="danger" link @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="500" @close="handleCloseDialog">
      <el-form ref="formRef" :model="formData" :rules="formRules" label-width="80px">
        <el-form-item label="名称" prop="name">
          <el-input v-model="formData.name" />
        </el-form-item>
        <el-form-item label="服务商" prop="provider">
          <el-select v-model="formData.provider" placeholder="请选择服务商">
            <el-option label="OpenAI" value="openai" />
            <el-option label="Gemini" value="gemini" />
            <el-option label="Claude" value="claude" />
          </el-select>
        </el-form-item>
        <el-form-item label="API密钥" prop="api_key">
          <el-input v-model="formData.api_key" />
        </el-form-item>
        <el-form-item label="权重" prop="weight">
          <el-input-number v-model="formData.weight" :min="1" />
        </el-form-item>
        <el-form-item label="状态" prop="status">
          <el-switch v-model="formData.status" active-value="active" inactive-value="inactive" />
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
import { getProviderKeys, addProviderKey, updateProviderKey, deleteProviderKey, type ProviderKey } from '@/api/apiKey'
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus'

// State
const loading = ref(true)
const tableData = ref<ProviderKey[]>([])
const dialogVisible = ref(false)
const isEdit = ref(false)
const formRef = ref<FormInstance>()
let currentKeyId: number | null = null;


const getInitialFormData = (): Omit<ProviderKey, 'id' | 'lastUsed'> => ({
  name: '',
  provider: 'openai',
  api_key: '',
  weight: 1,
  status: 'active',
})
const formData = reactive(getInitialFormData())

// Computed
const dialogTitle = computed(() => (isEdit.value ? '编辑密钥' : '新增密钥'))

// Form Rules
const formRules = reactive<FormRules>({
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  provider: [{ required: true, message: '请选择服务商', trigger: 'change' }],
  api_key: [{ required: true, message: '请输入API密钥', trigger: 'blur' }],
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
      try {
        if (isEdit.value && currentKeyId) {
          await updateProviderKey({ ...formData, id: currentKeyId, lastUsed: '' })
          ElMessage.success('更新成功')
        } else {
          await addProviderKey(formData)
          ElMessage.success('新增成功')
        }
        dialogVisible.value = false
        fetchKeys()
      } catch (error) {
        ElMessage.error('操作失败')
      }
    }
  })
}

const handleDelete = (row: ProviderKey) => {
  ElMessageBox.confirm(`确定要删除密钥 "${row.name}" 吗？`, '警告', {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
  }).then(async () => {
    try {
      await deleteProviderKey(row.id)
      ElMessage.success('删除成功')
      fetchKeys()
    } catch (error) {
      ElMessage.error('删除失败')
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
