<template>
  <div class="dashboard-skeleton">
    <!-- 顶部统计卡片 -->
    <div class="dashboard-skeleton-stats">
      <div 
        v-for="stat in statsArray" 
        :key="stat"
        class="dashboard-skeleton-stat-card"
      >
        <Card class="stat-card-skeleton">
          <div class="stat-card-content">
            <div class="stat-card-icon">
              <Skeleton shape="circle" width="48px" height="48px" />
            </div>
            <div class="stat-card-text">
              <Skeleton width="80px" height="14px" />
              <Skeleton width="120px" height="28px" />
              <Skeleton width="60px" height="12px" />
            </div>
          </div>
        </Card>
      </div>
    </div>
    
    <!-- 图表区域 -->
    <div class="dashboard-skeleton-charts">
      <!-- 主要图表 -->
      <Card class="main-chart-skeleton">
        <div class="chart-header">
          <Skeleton width="150px" height="24px" />
          <Skeleton width="100px" height="32px" />
        </div>
        <div class="chart-content">
          <Skeleton height="300px" />
        </div>
      </Card>
      
      <!-- 次要图表 -->
      <Card class="secondary-chart-skeleton">
        <div class="chart-header">
          <Skeleton width="120px" height="24px" />
        </div>
        <div class="chart-content">
          <Skeleton height="300px" />
        </div>
      </Card>
    </div>
    
    <!-- 数据表格 */
    <div class="dashboard-skeleton-table">
      <Card>
        <div class="table-header">
          <Skeleton width="100px" height="24px" />
          <div class="table-actions">
            <Skeleton width="80px" height="32px" />
            <Skeleton width="32px" height="32px" shape="circle" />
          </div>
        </div>
        <TableSkeleton :rows="tableRows" :columns="4" />
      </Card>
    </div>
    
    <!-- 底部活动列表 */
    <div class="dashboard-skeleton-activities">
      <Card>
        <div class="activities-header">
          <Skeleton width="80px" height="24px" />
          <Skeleton width="60px" height="20px" />
        </div>
        <div class="activities-list">
          <div 
            v-for="activity in activitiesArray" 
            :key="activity"
            class="activity-item"
          >
            <Skeleton shape="circle" width="32px" height="32px" />
            <div class="activity-content">
              <Skeleton width="200px" height="16px" />
              <Skeleton width="100px" height="14px" />
            </div>
            <Skeleton width="60px" height="14px" />
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent, computed } from 'vue'
import Card from '@/components/ui/Card.vue'
import Skeleton from '@/components/ui/Skeleton.vue'
import TableSkeleton from './TableSkeleton.vue'

export default defineComponent({
  name: 'DashboardSkeleton',
  components: {
    Card,
    Skeleton,
    TableSkeleton
  },
  props: {
    /** 统计卡片数量 */
    statsCount: {
      type: Number,
      default: 4
    },
    /** 表格行数 */
    tableRows: {
      type: Number,
      default: 5
    },
    /** 活动数量 */
    activitiesCount: {
      type: Number,
      default: 6
    },
    /** 动画类型 */
    animation: {
      type: String as () => 'pulse' | 'wave' | 'none',
      default: 'pulse'
    }
  },
  setup(props) {
    return {
      // 计算属性
      statsArray: computed<number[]>(() => {
        return Array.from({ length: props.statsCount }, (_, i) => i)
      }),
      activitiesArray: computed<number[]>(() => {
        return Array.from({ length: props.activitiesCount }, (_, i) => i)
      })
    }
  }
})
</script>

<style scoped>
.dashboard-skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
  padding: var(--spacing-6);
}

/* 统计卡片区域 */
.dashboard-skeleton-stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: var(--spacing-4);
}

.stat-card-skeleton {
  padding: var(--spacing-5);
}

.stat-card-content {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
}

.stat-card-text {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

/* 图表区域 */
.dashboard-skeleton-charts {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: var(--spacing-4);
}

.main-chart-skeleton,
.secondary-chart-skeleton {
  padding: var(--spacing-5);
}

.chart-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-4);
}

.chart-content {
  border-radius: var(--border-radius-md);
  overflow: hidden;
}

/* 表格区域 */
.dashboard-skeleton-table {
  width: 100%;
}

.table-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-5);
  border-bottom: 1px solid var(--color-border-primary);
}

.table-actions {
  display: flex;
  gap: var(--spacing-2);
  align-items: center;
}

/* 活动列表 */
.dashboard-skeleton-activities {
  width: 100%;
}

.activities-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-5);
  border-bottom: 1px solid var(--color-border-primary);
}

.activities-list {
  padding: var(--spacing-5);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.activity-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.activity-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

/* 响应式设计 */
@media (max-width: 1200px) {
  .dashboard-skeleton-charts {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .dashboard-skeleton {
    padding: var(--spacing-4);
    gap: var(--spacing-4);
  }
  
  .dashboard-skeleton-stats {
    grid-template-columns: 1fr;
  }
  
  .stat-card-skeleton,
  .main-chart-skeleton,
  .secondary-chart-skeleton {
    padding: var(--spacing-4);
  }
  
  .table-header,
  .activities-header,
  .activities-list {
    padding: var(--spacing-4);
  }
  
  .stat-card-content {
    gap: var(--spacing-3);
  }
  
  .activity-item {
    gap: var(--spacing-2);
  }
}

@media (max-width: 480px) {
  .dashboard-skeleton-stats {
    grid-template-columns: 1fr;
  }
  
  .chart-header,
  .table-header,
  .activities-header {
    flex-direction: column;
    gap: var(--spacing-2);
    align-items: flex-start;
  }
  
  .table-actions {
    align-self: stretch;
    justify-content: flex-end;
  }
}
</style>