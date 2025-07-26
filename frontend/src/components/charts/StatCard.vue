<template>
  <Card 
    :class="cardClasses"
    :variant="variant"
    :hoverable="hoverable"
    :clickable="clickable"
    :loading="loading"
    @click="handleClick"
  >
    <div class="stat-card">
      <!-- 图标区域 -->
      <div class="stat-icon" :style="iconStyle">
        <component 
          v-if="icon" 
          :is="icon" 
          :class="iconClasses" 
        />
        <div 
          v-else-if="iconText" 
          class="stat-icon-text"
        >
          {{ iconText }}
        </div>
      </div>
      
      <!-- 内容区域 -->
      <div class="stat-content">
        <div class="stat-title">{{ title }}</div>
        <div class="stat-value" :style="valueStyle">
          {{ formattedValue }}
        </div>
        
        <!-- 描述信息 -->
        <div v-if="description" class="stat-description">
          {{ description }}
        </div>
        
        <!-- 变化趋势 -->
        <div v-if="change !== undefined" class="stat-change">
          <component 
            :is="changeIcon" 
            :class="changeIconClasses"
          />
          <span :class="changeTextClasses">
            {{ changeText }}
          </span>
        </div>
      </div>
      
      <!-- 额外内容 -->
      <div v-if="slots.extra" class="stat-extra">
        <slot name="extra" />
      </div>
    </div>
  </Card>
</template>

<script lang="ts">
import { defineComponent, computed, useSlots } from 'vue'
import { Card } from '@/components/ui'
import { 
  ArrowUp, 
  ArrowDown, 
  Minus
} from '@element-plus/icons-vue'
import { useDesignSystem } from '@/composables/useDesignSystem'

// 统计类型
type StatType = 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'default'
type ChangeType = 'increase' | 'decrease' | 'neutral'

export default defineComponent({
  name: 'StatCard',
  components: {
    Card
  },
  props: {
    /** 标题 */
    title: {
      type: String,
      required: true
    },
    /** 数值 */
    value: {
      type: [String, Number],
      required: true
    },
    /** 描述 */
    description: {
      type: String,
      default: ''
    },
    /** 图标组件 */
    icon: {
      type: Object,
      default: null
    },
    /** 图标文本 */
    iconText: {
      type: String,
      default: ''
    },
    /** 统计类型 */
    type: {
      type: String as () => StatType,
      default: 'default'
    },
    /** 卡片变种 */
    variant: {
      type: String as () => 'default' | 'outlined' | 'elevated' | 'filled',
      default: 'default'
    },
    /** 是否可悬停 */
    hoverable: {
      type: Boolean,
      default: false
    },
    /** 是否可点击 */
    clickable: {
      type: Boolean,
      default: false
    },
    /** 加载状态 */
    loading: {
      type: Boolean,
      default: false
    },
    /** 变化值 */
    change: {
      type: Number,
      default: undefined
    },
    /** 变化类型 */
    changeType: {
      type: String as () => ChangeType,
      default: 'neutral'
    },
    /** 变化单位 */
    changeUnit: {
      type: String,
      default: '%'
    },
    /** 数值格式化函数 */
    formatter: {
      type: Function,
      default: null
    },
    /** 自定义样式 */
    customColor: {
      type: String,
      default: ''
    }
  },
  emits: ['click'],
  setup(props, { emit }) {
    // 获取插槽
    const slots = useSlots()

    // 使用设计系统
    const { colors } = useDesignSystem()

    // 获取类型颜色配置
    const getTypeColors = () => {
      const colorMap = {
        primary: {
          bg: colors.primary[500] + '15',
          icon: colors.primary[500],
          value: colors.primary[500]
        },
        success: {
          bg: colors.success[500] + '15',
          icon: colors.success[500],
          value: colors.success[500]
        },
        warning: {
          bg: colors.warning[500] + '15',
          icon: colors.warning[500],
          value: colors.warning[500]
        },
        danger: {
          bg: colors.error[500] + '15',
          icon: colors.error[500],
          value: colors.error[500]
        },
        info: {
          bg: colors.info[500] + '15',
          icon: colors.info[500],
          value: colors.info[500]
        },
        default: {
          bg: colors.neutral[100],
          icon: colors.neutral[600],
          value: colors.neutral[900]
        }
      }
      
      return colorMap[props.type] || colorMap.default
    }

    // 计算样式类
    const cardClasses = computed(() => [
      'ui-stat-card',
      `ui-stat-card--${props.type}`,
      {
        'ui-stat-card--clickable': props.clickable
      }
    ])

    const iconClasses = computed(() => [
      'stat-icon-component',
      `stat-icon-component--${props.type}`
    ])

    // 计算图标样式
    const iconStyle = computed(() => {
      const typeColors = getTypeColors()
      return {
        backgroundColor: props.customColor ? props.customColor + '15' : typeColors.bg,
        color: props.customColor || typeColors.icon
      }
    })

    // 计算数值样式
    const valueStyle = computed(() => {
      const typeColors = getTypeColors()
      return {
        color: props.customColor || typeColors.value
      }
    })

    // 格式化数值
    const formattedValue = computed(() => {
      if (props.formatter) {
        return props.formatter(props.value)
      }
      
      const numValue = typeof props.value === 'number' ? props.value : parseFloat(props.value.toString())
      
      if (isNaN(numValue)) {
        return props.value.toString()
      }
      
      // 格式化大数值
      if (numValue >= 1000000000) {
        return (numValue / 1000000000).toFixed(1) + 'B'
      } else if (numValue >= 1000000) {
        return (numValue / 1000000).toFixed(1) + 'M'
      } else if (numValue >= 1000) {
        return (numValue / 1000).toFixed(1) + 'K'
      }
      
      return numValue.toLocaleString()
    })

    // 计算变化图标
    const changeIcon = computed(() => {
      if (props.changeType === 'increase') return ArrowUp
      if (props.changeType === 'decrease') return ArrowDown
      return Minus
    })

    // 计算变化图标样式类
    const changeIconClasses = computed(() => [
      'stat-change-icon',
      {
        'stat-change-icon--increase': props.changeType === 'increase',
        'stat-change-icon--decrease': props.changeType === 'decrease',
        'stat-change-icon--neutral': props.changeType === 'neutral'
      }
    ])

    // 计算变化文本样式类
    const changeTextClasses = computed(() => [
      'stat-change-text',
      {
        'stat-change-text--increase': props.changeType === 'increase',
        'stat-change-text--decrease': props.changeType === 'decrease',
        'stat-change-text--neutral': props.changeType === 'neutral'
      }
    ])

    // 计算变化文本
    const changeText = computed(() => {
      if (props.change === undefined) return ''
      
      const absChange = Math.abs(props.change)
      const formattedChange = absChange < 1 ? absChange.toFixed(2) : absChange.toFixed(1)
      
      return `${formattedChange}${props.changeUnit}`
    })

    // 处理点击事件
    const handleClick = (event: MouseEvent) => {
      if (props.clickable) {
        emit('click', event)
      }
    }

    return {
      slots,
      cardClasses,
      iconClasses,
      iconStyle,
      valueStyle,
      formattedValue,
      changeIcon,
      changeIconClasses,
      changeTextClasses,
      changeText,
      handleClick
    }
  }
})
</script>

<style scoped>
.ui-stat-card {
  --stat-card-padding: var(--spacing-6);
}

.ui-stat-card--clickable {
  cursor: pointer;
  transition: all var(--transition-normal);
}

.ui-stat-card--clickable:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-lg);
}

.stat-card {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--stat-card-padding);
}

/* 图标区域 */
.stat-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  border-radius: var(--border-radius-lg);
  font-size: 24px;
  flex-shrink: 0;
  transition: all var(--transition-normal);
}

.stat-icon-component {
  font-size: inherit;
}

.stat-icon-text {
  font-size: 20px;
  font-weight: 600;
}

/* 内容区域 */
.stat-content {
  flex: 1;
  min-width: 0;
}

.stat-title {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-1);
  font-weight: 500;
}

.stat-value {
  font-size: var(--font-size-2xl);
  font-weight: 700;
  line-height: 1.2;
  margin-bottom: var(--spacing-1);
}

.stat-description {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  margin-bottom: var(--spacing-2);
}

.stat-change {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  font-size: var(--font-size-xs);
}

.stat-change-icon {
  font-size: 14px;
}

.stat-change-icon--increase {
  color: var(--color-status-success);
}

.stat-change-icon--decrease {
  color: var(--color-status-danger);
}

.stat-change-icon--neutral {
  color: var(--color-text-secondary);
}

.stat-change-text--increase {
  color: var(--color-status-success);
}

.stat-change-text--decrease {
  color: var(--color-status-danger);
}

.stat-change-text--neutral {
  color: var(--color-text-secondary);
}

/* 额外内容区域 */
.stat-extra {
  flex-shrink: 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .ui-stat-card {
    --stat-card-padding: var(--spacing-4);
  }
  
  .stat-card {
    gap: var(--spacing-3);
  }
  
  .stat-icon {
    width: 48px;
    height: 48px;
    font-size: 20px;
  }
  
  .stat-value {
    font-size: var(--font-size-xl);
  }
}

@media (max-width: 480px) {
  .stat-card {
    flex-direction: column;
    text-align: center;
    gap: var(--spacing-3);
  }
  
  .stat-content {
    text-align: center;
  }
}

/* 深色主题适配 */
.theme-dark .ui-stat-card--clickable:hover {
  box-shadow: var(--shadow-dark-lg);
}
</style>