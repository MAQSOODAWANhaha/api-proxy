import React, { useState, useCallback } from 'react'
import { toast } from 'sonner'
import ModernSelect from '../../../../components/common/ModernSelect'
import OAuthHandler, { OAuthResult, OAuthStatus } from '../../../../components/common/OAuthHandler'
import { LoadingSpinner } from '../../../../components/ui/loading'
import { api, ProviderType } from '../../../../lib/api'
import { ProviderKeyEditFormState } from '../types'

/** 编辑对话框 */
const EditDialog: React.FC<{
  item: ProviderKeyEditFormState
  onClose: () => void
  onSubmit: (item: ProviderKeyEditFormState) => void
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState<ProviderKeyEditFormState>({
    id: Number(item.id),
    provider: item.provider_type_id ? String(item.provider_type_id) : item.provider,
    provider_type_id: item.provider_type_id || 0,
    keyName: item.keyName,
    keyValue: item.keyValue,
    auth_type: item.auth_type || 'api_key',
    weight: item.weight,
    requestLimitPerMinute: item.requestLimitPerMinute,
    tokenLimitPromptPerMinute: item.tokenLimitPromptPerMinute,
    requestLimitPerDay: item.requestLimitPerDay,
    status: item.status,
    project_id: item.project_id || '',
  })

  // 服务商类型状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(true)
  const [selectedProviderType, setSelectedProviderType] = useState<ProviderType | null>(null)

  // OAuth相关状态
  const [oauthStatus, setOAuthStatus] = useState<OAuthStatus>('idle')
  const [oauthExtraParams, setOAuthExtraParams] = useState<{ [key: string]: string }>({})

  // 获取服务商类型列表
  const fetchProviderTypes = useCallback(async () => {
    setLoadingProviderTypes(true)
    try {
      const response = await api.auth.getProviderTypes({ is_active: true })

      if (response.success && response.data) {
        setProviderTypes(response.data.provider_types || [])
        // 优先按 provider_type_id 匹配（避免 display_name 重名）
        const currentProvider =
          response.data.provider_types?.find((type) => type.id === item.provider_type_id) ||
          response.data.provider_types?.find((type) => type.display_name === item.provider)
        if (currentProvider) {
          setSelectedProviderType(currentProvider)
          setFormData((prev) => ({
            ...prev,
            provider: String(currentProvider.id),
            provider_type_id: currentProvider.id,
            auth_type: currentProvider.auth_type || prev.auth_type,
          }))
        }
      } else {
        console.error('[EditDialog] 获取服务商类型失败:', response.message)
      }
    } catch (err) {
      console.error('[EditDialog] 获取服务商类型异常:', err)
    } finally {
      setLoadingProviderTypes(false)
    }
  }, [item.provider, item.provider_type_id])

  // 初始化：获取服务商类型
  React.useEffect(() => {
    fetchProviderTypes()
  }, [fetchProviderTypes])

  // OAuth处理函数
  const handleOAuthComplete = async (result: OAuthResult) => {
    if (result.success && result.data) {
      // OAuth成功完成，将获取到的token填充到表单
      setOAuthStatus('success')

      // 将OAuth返回的session_id填入表单的API密钥字段 (OAuth类型需要session_id而不是access_token)
      const newKeyValue = result.data.session_id

      setFormData((prev) => ({
        ...prev,
        keyValue: newKeyValue,
      }))

      // 显示成功消息，提示用户可以看到token并决定是否提交
      toast.success('OAuth授权成功！', {
        description: 'OAuth会话ID已填充到API密钥字段，请检查并完善其他信息后点击"保存修改"按钮提交。',
        duration: 5000,
      })
    } else {
      setOAuthStatus('error')
      toast.error('OAuth授权失败', {
        description: result.error || 'OAuth授权过程中发生错误，请重试',
        duration: 5000,
      })
    }
  }

  const handleProviderTypeChange = (value: string) => {
    const selectedProvider = providerTypes.find((type) => String(type.id) === value)
    if (selectedProvider) {
      setFormData((prev) => ({
        ...prev,
        provider: String(selectedProvider.id),
        provider_type_id: selectedProvider.id,
        auth_type: selectedProvider.auth_type || 'api_key',
        keyValue: '',
      }))
      setSelectedProviderType(selectedProvider)
      // 重置 OAuth 状态
      setOAuthStatus('idle')
      setOAuthExtraParams({})
    }
  }

  const getAuthConfig = (): any | null => {
    const authConfigs = selectedProviderType?.auth_configs
    if (!authConfigs || typeof authConfigs !== 'object' || Array.isArray(authConfigs)) return null
    return authConfigs
  }

  // 获取当前认证类型的额外参数配置
  const getCurrentAuthExtraParams = (): Array<{
    key: string
    label: string
    default: string
    required: boolean
    type: string
    placeholder?: string
    description?: string
  }> => {
    const cfg = getAuthConfig()
    return cfg?.extra_params || []
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    // OAuth类型的密钥需要先完成OAuth流程
    if (formData.auth_type === 'oauth' && oauthStatus !== 'success') {
      toast.info('请先完成OAuth授权流程')
      return
    }
    onSubmit(formData)
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData((prev) => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta),
    }))
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑账号密钥</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 密钥名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 密钥名称
          </label>
          <input
            type="text"
            required
            value={formData.keyName}
            onChange={(e) => setFormData({ ...formData, keyName: e.target.value })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 服务商类型 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 服务商类型
          </label>

          {loadingProviderTypes ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <LoadingSpinner size="sm" tone="primary" />
              <span className="text-sm text-neutral-600">加载服务商类型...</span>
            </div>
          ) : (
            <ModernSelect
              value={formData.provider}
              onValueChange={handleProviderTypeChange}
              options={providerTypes.map((type) => ({
                value: String(type.id),
                label: `${type.display_name} (${type.name}) / ${type.auth_type || ''}`,
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>

        {/* 认证类型（由服务商类型行决定） */}
        {selectedProviderType && (
          <div className="rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2 text-sm text-neutral-700">
            <span className="font-medium">认证类型：</span>
            <span>{selectedProviderType.auth_type || formData.auth_type}</span>
          </div>
        )}

        {/* 动态额外参数字段 */}
        {selectedProviderType && formData.auth_type === 'oauth' && getCurrentAuthExtraParams().length > 0 && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-2">OAuth额外参数</label>
            {getCurrentAuthExtraParams().map((param) => (
              <div key={param.key} className="mb-3">
                <label className="block text-sm font-medium text-neutral-700 mb-1">
                  {param.required && <span className="text-red-500">*</span>} {param.label}
                </label>
                <input
                  type={param.type === 'number' ? 'number' : 'text'}
                  required={param.required}
                  value={oauthExtraParams[param.key] || param.default || ''}
                  onChange={(e) =>
                    setOAuthExtraParams((prev) => ({
                      ...prev,
                      [param.key]: e.target.value,
                    }))
                  }
                  placeholder={param.placeholder}
                  className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                {param.description && (
                  <p className="text-xs text-neutral-600 mt-1">{param.description}</p>
                )}
              </div>
            ))}
          </div>
        )}

        {/* OAuth Handler */}
        {selectedProviderType && formData.auth_type === 'oauth' && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              OAuth授权
            </label>
            <OAuthHandler
              request={{
                provider_name: `${selectedProviderType.name}:${selectedProviderType.auth_type || 'oauth'}`,
                name: formData.keyName || 'Provider Key',
                description: `${selectedProviderType.display_name} OAuth Key`,
                extra_params: oauthExtraParams,
              }}
              status={oauthStatus}
              onStatusChange={setOAuthStatus}
              onComplete={handleOAuthComplete}
            />
          </div>
        )}

        {/* API密钥 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> API密钥
          </label>
          <input
            type="text"
            required={formData.auth_type !== 'oauth'}
            value={formData.keyValue}
            onChange={(e) => setFormData({ ...formData, keyValue: e.target.value })}
            placeholder={formData.auth_type === 'oauth' ? 'OAuth授权完成后自动填入' : '请输入API密钥'}
            disabled={formData.auth_type === 'oauth'}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 disabled:bg-neutral-50 disabled:text-neutral-500"
          />
        </div>

        {/* Gemini 项目ID - 仅在选择 Gemini 时显示 */}
        {selectedProviderType && selectedProviderType.name === 'gemini' && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              项目ID
              <span className="text-xs text-neutral-500 ml-2">（可选，用于 Google Cloud Code Assist）</span>
            </label>
            <input
              type="text"
              value={formData.project_id || ''}
              onChange={(e) => setFormData({ ...formData, project_id: e.target.value })}
              placeholder="请输入 Google Cloud 项目ID"
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <p className="text-xs text-neutral-600 mt-1">留空使用标准 Gemini API，填写则使用 Code Assist API</p>
          </div>
        )}

        {/* 权重 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">权重</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('weight', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.weight}
              onChange={(e) => setFormData({ ...formData, weight: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('weight', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 请求限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">请求限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.requestLimitPerMinute}
              onChange={(e) => setFormData({ ...formData, requestLimitPerMinute: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* Token限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', -10)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.tokenLimitPromptPerMinute}
              onChange={(e) =>
                setFormData({ ...formData, tokenLimitPromptPerMinute: parseInt(e.target.value) || 0 })
              }
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', 10)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 请求限制/天 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">请求限制/天</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerDay', -100)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.requestLimitPerDay}
              onChange={(e) => setFormData({ ...formData, requestLimitPerDay: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerDay', 100)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 启用状态 */}
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-neutral-700">启用状态</label>
          <button
            type="button"
            onClick={() => setFormData({ ...formData, status: formData.status === 'active' ? 'disabled' : 'active' })}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              formData.status === 'active' ? 'bg-violet-600' : 'bg-neutral-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                formData.status === 'active' ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
          <span className="text-sm text-neutral-600">
            {formData.status === 'active' ? '启用' : '停用'}
          </span>
        </div>

        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
          >
            取消
          </button>
          <button
            type="submit"
            className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700"
          >
            保存
          </button>
        </div>
      </form>
    </div>
  )
}

export default EditDialog
