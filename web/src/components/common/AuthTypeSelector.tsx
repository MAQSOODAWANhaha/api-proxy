/**
 * AuthTypeSelector.tsx
 * 认证类型选择器组件 - 根据服务商类型显示支持的认证方式
 */

import React, { useEffect, useState } from 'react'
import { Shield, Key, User, Zap } from 'lucide-react'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { ProviderType } from '@/lib/api'

/** 认证类型选项 */
export interface AuthTypeOption {
  value: string
  label: string
  description: string
  icon: React.ComponentType<{ className?: string }>
  recommended?: boolean
}

/** 认证类型映射配置 */
const AUTH_TYPE_CONFIG: Record<string, AuthTypeOption> = {
  api_key: {
    value: 'api_key',
    label: 'API Key',
    description: '使用API密钥进行认证',
    icon: Key,
  },
  oauth: {
    value: 'oauth',
    label: 'OAuth 2.0',
    description: '使用OAuth 2.0进行安全授权',
    icon: Shield,
    recommended: true,
  },
  service_account: {
    value: 'service_account',
    label: 'Service Account',
    description: '使用Google服务账户密钥认证',
    icon: Zap,
  },
  adc: {
    value: 'adc',
    label: 'ADC',
    description: '应用默认凭据（Application Default Credentials）',
    icon: Shield,
  },
}

/** 组件Props */
export interface AuthTypeSelectorProps {
  /** 当前选中的认证类型 */
  value?: string
  /** 选择变更回调 */
  onValueChange: (authType: string) => void
  /** 服务商类型信息 */
  providerType?: ProviderType | null
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义样式类名 */
  className?: string
  /** 占位符文本 */
  placeholder?: string
  /** 是否显示推荐标签 */
  showRecommended?: boolean
}

/**
 * AuthTypeSelector 认证类型选择器
 * - 根据服务商类型自动过滤支持的认证方式
 * - 显示认证类型的图标和描述
 * - 支持推荐标签显示
 * - 自动选择默认认证类型
 */
const AuthTypeSelector: React.FC<AuthTypeSelectorProps> = ({
  value,
  onValueChange,
  providerType,
  disabled = false,
  className,
  placeholder = '选择认证类型',
  showRecommended = true,
}) => {
  const [availableAuthTypes, setAvailableAuthTypes] = useState<AuthTypeOption[]>([])

  // 根据服务商类型更新可用的认证类型
  useEffect(() => {
    if (!providerType?.supported_auth_types) {
      setAvailableAuthTypes([])
      return
    }

    const supportedTypes = providerType.supported_auth_types
      .map(type => AUTH_TYPE_CONFIG[type])
      .filter(Boolean)

    setAvailableAuthTypes(supportedTypes)

    // 如果当前没有选中值，自动选择第一个支持的认证类型
    if (!value && supportedTypes.length > 0) {
      // 优先选择推荐的认证类型
      const recommendedType = supportedTypes.find(type => type.recommended)
      const defaultType = recommendedType || supportedTypes[0]
      onValueChange(defaultType.value)
    }
  }, [providerType, value, onValueChange])

  // 获取当前选中的认证类型配置
  const selectedAuthType = value ? AUTH_TYPE_CONFIG[value] : null

  return (
    <div className={cn('space-y-2', className)}>
      <Select
        value={value || ''}
        onValueChange={onValueChange}
        disabled={disabled || availableAuthTypes.length === 0}
      >
        <SelectTrigger className="w-full">
          <SelectValue placeholder={placeholder}>
            {selectedAuthType && (
              <div className="flex items-center gap-2">
                <selectedAuthType.icon className="h-4 w-4" />
                <span>{selectedAuthType.label}</span>
                {showRecommended && selectedAuthType.recommended && (
                  <Badge variant="secondary" className="text-xs">
                    推荐
                  </Badge>
                )}
              </div>
            )}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {availableAuthTypes.length === 0 ? (
            <SelectItem value="none" disabled>
              <div className="flex items-center gap-2 text-muted-foreground">
                <Shield className="h-4 w-4" />
                <span>暂无可用的认证类型</span>
              </div>
            </SelectItem>
          ) : (
            availableAuthTypes.map((authType) => (
              <SelectItem key={authType.value} value={authType.value}>
                <div className="flex items-center justify-between w-full">
                  <div className="flex items-center gap-2">
                    <authType.icon className="h-4 w-4" />
                    <div className="flex flex-col">
                      <span className="font-medium">{authType.label}</span>
                      <span className="text-xs text-muted-foreground">
                        {authType.description}
                      </span>
                    </div>
                  </div>
                  {showRecommended && authType.recommended && (
                    <Badge variant="secondary" className="text-xs ml-2">
                      推荐
                    </Badge>
                  )}
                </div>
              </SelectItem>
            ))
          )}
        </SelectContent>
      </Select>

      {/* 当前选择的认证类型的详细说明 */}
      {selectedAuthType && (
        <div className="text-sm text-muted-foreground p-3 bg-muted/50 rounded-md">
          <div className="flex items-center gap-2 mb-1">
            <selectedAuthType.icon className="h-4 w-4" />
            <span className="font-medium">{selectedAuthType.label}</span>
            {showRecommended && selectedAuthType.recommended && (
              <Badge variant="secondary" className="text-xs">
                推荐
              </Badge>
            )}
          </div>
          <p>{selectedAuthType.description}</p>
          
          {/* OAuth特殊说明 */}
          {value === 'oauth' && (
            <p className="mt-2 text-xs text-blue-600 dark:text-blue-400">
              💡 OAuth 2.0 提供更安全的授权方式，无需直接暴露API密钥
            </p>
          )}
        </div>
      )}
    </div>
  )
}

export default AuthTypeSelector