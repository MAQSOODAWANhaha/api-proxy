<template>
  <div class="error-container">
    <div class="error-content">
      <div class="error-icon">
        <el-icon :size="120">
          <Warning />
        </el-icon>
      </div>
      <h1 class="error-title">500</h1>
      <p class="error-description">服务器内部错误，请稍后重试</p>
      <div class="error-actions">
        <el-button type="primary" @click="refresh">
          <el-icon><Refresh /></el-icon>
          刷新页面
        </el-button>
        <el-button @click="goHome">
          <el-icon><HomeFilled /></el-icon>
          返回首页
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { Warning, Refresh, HomeFilled } from '@element-plus/icons-vue'

const router = useRouter()

const refresh = () => {
  window.location.reload()
}

const goHome = () => {
  router.push('/')
}
</script>

<style scoped>
.error-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #ffecd2 0%, #fcb69f 100%);
  padding: 20px;
}

.error-content {
  text-align: center;
  background: white;
  padding: 60px 40px;
  border-radius: 16px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.1);
  max-width: 500px;
  width: 100%;
}

.error-icon {
  margin-bottom: 30px;
  color: #e6a23c;
}

.error-title {
  font-size: 72px;
  color: #e6a23c;
  margin: 0 0 20px 0;
  font-weight: bold;
}

.error-description {
  font-size: 18px;
  color: #666;
  margin-bottom: 40px;
  line-height: 1.6;
}

.error-actions {
  display: flex;
  gap: 16px;
  justify-content: center;
  flex-wrap: wrap;
}

@media (max-width: 768px) {
  .error-content {
    padding: 40px 20px;
  }
  
  .error-title {
    font-size: 56px;
  }
  
  .error-actions {
    flex-direction: column;
  }
}
</style>