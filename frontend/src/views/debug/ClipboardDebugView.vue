<template>
  <div class="clipboard-debug-view">
    <el-card class="page-card">
      <template #header>
        <div class="card-header">
          <h2>剪贴板功能诊断</h2>
          <el-button @click="refreshInfo" :loading="refreshing">
            <el-icon><Refresh /></el-icon>
            刷新信息
          </el-button>
        </div>
      </template>

      <div class="debug-content">
        <!-- 环境信息 -->
        <el-card class="section-card" shadow="never">
          <template #header>
            <h3>环境信息</h3>
          </template>
          <el-descriptions :column="2" border>
            <el-descriptions-item label="协议">
              <el-tag :type="envInfo.protocol === 'https:' ? 'success' : 'warning'">
                {{ envInfo.protocol }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="主机">
              {{ envInfo.host }}
            </el-descriptions-item>
            <el-descriptions-item label="安全上下文">
              <el-tag :type="envInfo.isSecureContext ? 'success' : 'danger'">
                {{ envInfo.isSecureContext ? '是' : '否' }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="用户代理">
              <div class="user-agent">{{ envInfo.userAgent }}</div>
            </el-descriptions-item>
          </el-descriptions>
        </el-card>

        <!-- API 支持检查 -->
        <el-card class="section-card" shadow="never">
          <template #header>
            <h3>API 支持情况</h3>
          </template>
          <div class="api-support-grid">
            <div v-for="(support, api) in apiSupport" :key="api" class="support-item">
              <el-tag :type="support ? 'success' : 'danger'" size="large">
                <el-icon>
                  <component :is="support ? Check : Close" />
                </el-icon>
                {{ api }}
              </el-tag>
            </div>
          </div>
        </el-card>

        <!-- 权限状态 -->
        <el-card class="section-card" shadow="never">
          <template #header>
            <h3>剪贴板权限</h3>
          </template>
          <el-descriptions :column="2" border>
            <el-descriptions-item label="写入权限">
              <el-tag :type="getPermissionTagType(permissions.write)">
                {{ permissions.write }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="读取权限">
              <el-tag :type="getPermissionTagType(permissions.read)">
                {{ permissions.read }}
              </el-tag>
            </el-descriptions-item>
          </el-descriptions>
          <div class="permission-actions">
            <el-button @click="requestPermissions" :loading="requestingPermissions">
              请求权限
            </el-button>
            <el-button @click="refreshPermissions">
              刷新权限状态
            </el-button>
          </div>
        </el-card>

        <!-- 功能测试 -->
        <el-card class="section-card" shadow="never">
          <template #header>
            <h3>功能测试</h3>
          </template>
          <div class="test-section">
            <h4>基础复制测试</h4>
            <div class="test-item">
              <el-input
                v-model="testText"
                placeholder="输入要测试复制的文本"
                style="width: 300px; margin-right: 12px;"
              />
              <el-button @click="testBasicCopy" :loading="testing.basic">
                测试复制
              </el-button>
            </div>
            <div v-if="testResults.basic" class="test-result">
              <el-alert
                :type="testResults.basic.success ? 'success' : 'error'"
                :title="testResults.basic.message"
                :description="testResults.basic.details"
                show-icon
                :closable="false"
              />
            </div>
          </div>

          <el-divider />

          <div class="test-section">
            <h4>API密钥复制测试</h4>
            <div class="test-item">
              <ApiKeyCopyCell
                :api-key="sampleApiKey"
                @copy-success="onApiKeyCopySuccess"
                @copy-error="onApiKeyCopyError"
              />
            </div>
            <div v-if="testResults.apiKey" class="test-result">
              <el-alert
                :type="testResults.apiKey.success ? 'success' : 'error'"
                :title="testResults.apiKey.message"
                :description="testResults.apiKey.details"
                show-icon
                :closable="false"
              />
            </div>
          </div>

          <el-divider />

          <div class="test-section">
            <h4>备用方案测试</h4>
            <div class="test-item">
              <el-button @click="testFallbackCopy" :loading="testing.fallback">
                测试备用复制方案
              </el-button>
            </div>
            <div v-if="testResults.fallback" class="test-result">
              <el-alert
                :type="testResults.fallback.success ? 'success' : 'error'"
                :title="testResults.fallback.message"
                :description="testResults.fallback.details"
                show-icon
                :closable="false"
              />
            </div>
          </div>
        </el-card>

        <!-- 建议和解决方案 -->
        <el-card class="section-card" shadow="never">
          <template #header>
            <h3>建议和解决方案</h3>
          </template>
          <div class="suggestions">
            <div v-for="suggestion in suggestions" :key="suggestion.type" class="suggestion-item">
              <el-alert
                :type="suggestion.type"
                :title="suggestion.title"
                :description="suggestion.description"
                show-icon
                :closable="false"
              />
            </div>
          </div>
        </el-card>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { Refresh, Check, Close } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { copyToClipboard, copyApiKey, isClipboardSupported, checkClipboardPermissions } from '@/utils/clipboard'
import ApiKeyCopyCell from '@/components/ui/ApiKeyCopyCell.vue'

// 响应式数据
const refreshing = ref(false)
const requestingPermissions = ref(false)
const testText = ref('测试复制文本 - Hello World!')
const sampleApiKey = ref('sk-1234567890abcdefghijklmnopqrstuvwxyz1234567890')

const testing = reactive({
  basic: false,
  fallback: false
})

const testResults = reactive({
  basic: null as { success: boolean; message: string; details?: string } | null,
  apiKey: null as { success: boolean; message: string; details?: string } | null,
  fallback: null as { success: boolean; message: string; details?: string } | null
})

const envInfo = reactive({
  protocol: '',
  host: '',
  isSecureContext: false,
  userAgent: ''
})

const apiSupport = reactive({
  'navigator.clipboard': false,
  'navigator.clipboard.writeText': false,
  'navigator.clipboard.readText': false,
  'document.execCommand': false,
  'navigator.permissions': false
})

const permissions = reactive({
  write: 'unsupported' as string,
  read: 'unsupported' as string
})

// 计算建议
const suggestions = computed(() => {
  const suggestions = []

  if (!envInfo.isSecureContext) {
    suggestions.push({
      type: 'warning',
      title: '需要安全上下文',
      description: '现代剪贴板 API 需要 HTTPS 协议或 localhost 环境。请使用 HTTPS 访问或在本地环境测试。'
    })
  }

  if (!apiSupport['navigator.clipboard']) {
    suggestions.push({
      type: 'error',
      title: '不支持剪贴板 API',
      description: '您的浏览器不支持现代剪贴板 API。建议升级到较新版本的浏览器。'
    })
  }

  if (!apiSupport['document.execCommand']) {
    suggestions.push({
      type: 'error',
      title: '备用方案不可用',
      description: '浏览器不支持 document.execCommand，这意味着备用复制方案也无法使用。'
    })
  }

  if (permissions.write === 'denied') {
    suggestions.push({
      type: 'warning',
      title: '剪贴板权限被拒绝',
      description: '用户拒绝了剪贴板写入权限。可以尝试重新请求权限或指导用户手动复制。'
    })
  }

  if (suggestions.length === 0) {
    suggestions.push({
      type: 'success',
      title: '环境正常',
      description: '剪贴板功能应该可以正常工作。如果仍有问题，请检查具体的错误信息。'
    })
  }

  return suggestions
})

// 初始化环境信息
const initEnvironmentInfo = () => {
  envInfo.protocol = location.protocol
  envInfo.host = location.host
  envInfo.isSecureContext = window.isSecureContext
  envInfo.userAgent = navigator.userAgent

  // 检查 API 支持
  apiSupport['navigator.clipboard'] = !!navigator.clipboard
  apiSupport['navigator.clipboard.writeText'] = !!(navigator.clipboard && navigator.clipboard.writeText)
  apiSupport['navigator.clipboard.readText'] = !!(navigator.clipboard && navigator.clipboard.readText)
  // eslint-disable-next-line @typescript-eslint/no-deprecated
  apiSupport['document.execCommand'] = !!document.execCommand
  apiSupport['navigator.permissions'] = !!navigator.permissions
}

// 刷新权限状态
const refreshPermissions = async () => {
  const perms = await checkClipboardPermissions()
  permissions.write = perms.write
  permissions.read = perms.read
}

// 请求权限
const requestPermissions = async () => {
  requestingPermissions.value = true
  
  try {
    // 尝试写入一个测试文本来触发权限请求
    await navigator.clipboard.writeText('permission test')
    ElMessage.success('权限请求成功')
    await refreshPermissions()
  } catch (error) {
    ElMessage.error('权限请求失败')
    console.error('Permission request failed:', error)
  } finally {
    requestingPermissions.value = false
  }
}

// 测试基础复制
const testBasicCopy = async () => {
  testing.basic = true
  testResults.basic = null
  
  try {
    const success = await copyToClipboard(testText.value, '测试复制成功')
    testResults.basic = {
      success,
      message: success ? '基础复制测试成功' : '基础复制测试失败',
      details: success ? '文本已成功复制到剪贴板' : '复制操作返回失败'
    }
  } catch (error) {
    testResults.basic = {
      success: false,
      message: '基础复制测试出错',
      details: error instanceof Error ? error.message : '未知错误'
    }
  } finally {
    testing.basic = false
  }
}

// 测试备用复制方案
const testFallbackCopy = async () => {
  testing.fallback = true
  testResults.fallback = null
  
  try {
    // 创建临时输入框测试备用方案
    const textArea = document.createElement('textarea')
    textArea.value = testText.value
    textArea.style.position = 'fixed'
    textArea.style.top = '0'
    textArea.style.left = '0'
    textArea.style.opacity = '0'
    
    document.body.appendChild(textArea)
    textArea.select()
    
    // eslint-disable-next-line @typescript-eslint/no-deprecated
    const success = document.execCommand('copy')
    document.body.removeChild(textArea)
    
    testResults.fallback = {
      success,
      message: success ? '备用方案测试成功' : '备用方案测试失败',
      details: success ? '使用 execCommand 成功复制' : 'execCommand 返回失败'
    }
    
    if (success) {
      ElMessage.success('备用方案复制成功')
    }
  } catch (error) {
    testResults.fallback = {
      success: false,
      message: '备用方案测试出错',
      details: error instanceof Error ? error.message : '未知错误'
    }
  } finally {
    testing.fallback = false
  }
}

// API密钥复制成功回调
const onApiKeyCopySuccess = (key: string) => {
  testResults.apiKey = {
    success: true,
    message: 'API密钥复制成功',
    details: `成功复制密钥: ${key.substring(0, 10)}...`
  }
}

// API密钥复制失败回调
const onApiKeyCopyError = (error: Error) => {
  testResults.apiKey = {
    success: false,
    message: 'API密钥复制失败',
    details: error.message
  }
}

// 获取权限标签类型
const getPermissionTagType = (permission: string) => {
  switch (permission) {
    case 'granted': return 'success'
    case 'denied': return 'danger'
    case 'prompt': return 'warning'
    default: return 'info'
  }
}

// 刷新所有信息
const refreshInfo = async () => {
  refreshing.value = true
  
  try {
    initEnvironmentInfo()
    await refreshPermissions()
    ElMessage.success('信息已刷新')
  } catch (error) {
    ElMessage.error('刷新信息失败')
    console.error('Refresh failed:', error)
  } finally {
    refreshing.value = false
  }
}

onMounted(() => {
  initEnvironmentInfo()
  refreshPermissions()
})
</script>

<style scoped>
.clipboard-debug-view {
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

.debug-content {
  max-height: calc(100vh - 200px);
  overflow-y: auto;
}

.section-card {
  margin-bottom: 24px;
  border: 1px solid #e5e7eb;
}

.section-card:last-child {
  margin-bottom: 0;
}

.section-card .el-card__header {
  background: #fafafa;
  border-bottom: 1px solid #e5e7eb;
}

.section-card h3 {
  margin: 0;
  color: #374151;
  font-size: 16px;
  font-weight: 600;
}

.user-agent {
  max-width: 400px;
  word-break: break-all;
  font-size: 12px;
  color: #6b7280;
}

.api-support-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 12px;
}

.support-item {
  display: flex;
  justify-content: center;
}

.permission-actions {
  margin-top: 16px;
  display: flex;
  gap: 12px;
}

.test-section {
  margin-bottom: 24px;
}

.test-section:last-child {
  margin-bottom: 0;
}

.test-section h4 {
  margin: 0 0 12px 0;
  color: #374151;
  font-size: 14px;
  font-weight: 600;
}

.test-item {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
}

.test-result {
  margin-top: 12px;
}

.suggestions {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.suggestion-item {
  margin: 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .api-support-grid {
    grid-template-columns: 1fr;
  }
  
  .test-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  
  .test-item .el-input {
    width: 100% !important;
  }
  
  .permission-actions {
    flex-direction: column;
  }
  
  .permission-actions .el-button {
    width: 100%;
  }
}
</style>