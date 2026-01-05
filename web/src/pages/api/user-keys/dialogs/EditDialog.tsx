import React, { useEffect, useState, useCallback } from 'react';
import ModernSelect from '../../../../components/common/ModernSelect';
import MultiSelect from '../../../../components/common/MultiSelect';
import { LoadingSpinner } from '../../../../components/ui/loading';
import { api, ProviderType, SchedulingStrategy } from '../../../../lib/api';
import { ApiKey, UserProviderKey } from '../types';

const EditDialog: React.FC<{
  item: ApiKey;
  onClose: () => void;
  onSubmit: (item: ApiKey) => void;
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState({ ...item });

  // 编辑弹窗独有的状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([]);
  const [schedulingStrategies, setSchedulingStrategies] = useState<
    SchedulingStrategy[]
  >([]);
  const [userProviderKeys, setUserProviderKeys] = useState<UserProviderKey[]>(
    []
  );
  const [loadingKeys, setLoadingKeys] = useState(false);
  const [loadingDetail, setLoadingDetail] = useState(true);
  const [submitting, setSubmitting] = useState(false);

  // 获取API Key完整详情数据
  const fetchDetailData = useCallback(async () => {
    setLoadingDetail(true);
    try {
      const response = await api.userService.getKeyDetail(item.id);
      if (response.success && response.data) {
        const detail = response.data;
        // 使用完整的详情数据更新formData
        setFormData((prev) => ({
          ...prev, // 保留原有的字段
          ...detail,
          // 确保数字字段有默认值
          retry_count: detail.retry_count || 0,
          timeout_seconds: detail.timeout_seconds || 0,
          max_request_per_min: detail.max_request_per_min || 0,
          max_requests_per_day: detail.max_requests_per_day || 0,
          max_tokens_per_day: detail.max_tokens_per_day || 0,
          max_cost_per_day: detail.max_cost_per_day || 0,
          // 确保数组字段有默认值
          user_provider_keys_ids: detail.user_provider_keys_ids || [],
        }));
      } else {
        console.error("获取API Key详情失败:", response.message);
      }
    } catch (err) {
      console.error("获取API Key详情异常:", err);
    } finally {
      setLoadingDetail(false);
    }
  }, [item.id]);

  // 获取服务商类型列表
  const fetchProviderTypesLocal = async () => {
    try {
      const response = await api.auth.getProviderTypes({ is_active: true });

      if (response.success && response.data) {
        setProviderTypes(response.data.provider_types || []);
      } else {
        console.error("[EditDialog] 获取服务商类型失败:", response.message);
      }
    } catch (err) {
      console.error("[EditDialog] 获取服务商类型异常:", err);
    }
  };

  // 获取调度策略列表
  const fetchSchedulingStrategiesLocal = async () => {
    try {
      const response = await api.auth.getSchedulingStrategies();
      if (response.success && response.data) {
        setSchedulingStrategies(response.data.scheduling_strategies || []);
      }
    } catch (err) {
      console.error("获取调度策略失败:", err);
    }
  };

  // 获取用户提供商密钥列表的本地函数
  const fetchUserProviderKeysLocal = async (providerTypeId: number) => {
    if (!providerTypeId) {
      setUserProviderKeys([]);
      return;
    }

    setLoadingKeys(true);
    try {
      const response = await api.providerKeys.getSimpleList({
        is_active: true,
        provider_type_id: providerTypeId,
      });
      if (response.success && response.data) {
        setUserProviderKeys(
          response.data.provider_keys.map((key) => ({
            id: key.id,
            name: key.name,
            display_name: key.display_name,
          })) || []
        );
      } else {
        setUserProviderKeys([]);
      }
    } catch (err) {
      console.error("获取用户提供商密钥失败:", err);
      setUserProviderKeys([]);
    } finally {
      setLoadingKeys(false);
    }
  };

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData((prev) => ({
      ...prev,
      [field]: Math.max(
        0,
        (prev[field as keyof typeof prev] as number) + delta
      ),
    }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (submitting) return;

    setSubmitting(true);
    try {
      await onSubmit(formData);
    } catch (err) {
      console.error("提交失败:", err);
    } finally {
      setSubmitting(false);
    }
  };

  // 当服务商类型更改时，重新获取对应的用户提供商密钥
  useEffect(() => {
    if (formData.provider_type_id) {
      fetchUserProviderKeysLocal(formData.provider_type_id);
    }
  }, [formData.provider_type_id]);

  // 初始化：获取完整详情数据、服务商类型、调度策略
  useEffect(() => {
    const initializeEditDialog = async () => {
      // 首先获取完整的详情数据
      await fetchDetailData();

      // 同时获取服务商类型和调度策略
      await Promise.all([
        fetchProviderTypesLocal(),
        fetchSchedulingStrategiesLocal(),
      ]);
    };
    initializeEditDialog();
  }, [fetchDetailData]);

  // 当详情数据加载完成且provider_type_id确定后，获取对应的用户提供商密钥
  useEffect(() => {
    if (!loadingDetail && formData.provider_type_id) {
      fetchUserProviderKeysLocal(formData.provider_type_id);
    }
  }, [loadingDetail, formData.provider_type_id]);

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">
        编辑 API Key
      </h3>

      {loadingDetail ? (
        <div className="flex items-center justify-center py-8">
          <LoadingSpinner size="lg" tone="primary" />
          <span className="ml-2 text-neutral-600">正在加载详情数据...</span>
        </div>
      ) : (
        <form onSubmit={handleSubmit} className="space-y-4">
          {/* 服务名称 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              服务名称 *
            </label>
            <input
              type="text"
              required
              value={formData.name}
              onChange={(e) =>
                setFormData({ ...formData, name: e.target.value })
              }
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
          </div>

          {/* 描述 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              描述
            </label>
            <textarea
              value={formData.description}
              onChange={(e) =>
                setFormData({ ...formData, description: e.target.value })
              }
              rows={3}
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 resize-none"
            />
          </div>

          {/* 服务商类型 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              服务商类型 *
            </label>
            <ModernSelect
              value={formData.provider_type_id.toString()}
              onValueChange={(value) => {
                const selectedProvider = providerTypes.find(
                  (type) => type.id.toString() === value
                );
                setFormData({
                  ...formData,
                  provider_type_id: parseInt(value),
                  provider: selectedProvider
                    ? selectedProvider.name
                    : formData.provider,
                });
              }}
              options={providerTypes.map((type) => ({
                value: type.id.toString(),
                label: `${type.display_name} (${type.name}) / ${type.auth_type || ""}`,
              }))}
              placeholder="请选择服务商类型"
            />
          </div>

          {/* 调度策略 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              调度策略
            </label>
            <ModernSelect
              value={formData.scheduling_strategy || ""}
              onValueChange={(value) =>
                setFormData({ ...formData, scheduling_strategy: value })
              }
              options={schedulingStrategies.map((option) => ({
                value: option.value,
                label: option.label,
              }))}
              placeholder="请选择调度策略"
            />
          </div>

          {/* 账号API Keys（多选） */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-2">
              账号API Keys *
            </label>
            {loadingKeys ? (
              <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
                <LoadingSpinner size="sm" tone="primary" />
                <span className="text-sm text-neutral-600">
                  加载密钥列表...
                </span>
              </div>
            ) : (
              <MultiSelect
                value={(formData.user_provider_keys_ids || []).map((id) =>
                  id.toString()
                )}
                onValueChange={(value) =>
                  setFormData((prev) => ({
                    ...prev,
                    user_provider_keys_ids: value.map((v) => parseInt(v)),
                  }))
                }
                options={userProviderKeys.map((key) => ({
                  value: key.id.toString(),
                  label: key.display_name || key.name,
                }))}
                placeholder="请选择账号API Keys"
                searchPlaceholder="搜索API Keys..."
                maxDisplay={3}
              />
            )}
            {!loadingKeys && userProviderKeys.length === 0 && (
              <p className="text-xs text-yellow-600 mt-1">
                当前服务商类型下没有可用的账号API Keys
              </p>
            )}
          </div>

          {/* 数字配置选项 */}
          <div className="grid grid-cols-2 gap-4">
            {/* 重试次数 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                重试次数
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() => handleNumberChange("retry_count", -1)}
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  min="0"
                  value={formData.retry_count}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      retry_count: parseInt(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() => handleNumberChange("retry_count", 1)}
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>

            {/* 超时时间 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                超时时间(秒)
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() => handleNumberChange("timeout_seconds", -5)}
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  min="0"
                  value={formData.timeout_seconds}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      timeout_seconds: parseInt(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() => handleNumberChange("timeout_seconds", 5)}
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>

            {/* 速率限制 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                速率限制/分钟
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() => handleNumberChange("max_request_per_min", -10)}
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  min="0"
                  value={formData.max_request_per_min}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      max_request_per_min: parseInt(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() => handleNumberChange("max_request_per_min", 10)}
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>

            {/* 速率限制/天 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                速率限制/天
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() =>
                    handleNumberChange("max_requests_per_day", -1000)
                  }
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  min="0"
                  value={formData.max_requests_per_day}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      max_requests_per_day: parseInt(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() =>
                    handleNumberChange("max_requests_per_day", 1000)
                  }
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>

            {/* Token限制 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                Token限制/天
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() =>
                    handleNumberChange("max_tokens_per_day", -1000)
                  }
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  min="0"
                  value={formData.max_tokens_per_day}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      max_tokens_per_day: parseInt(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() => handleNumberChange("max_tokens_per_day", 1000)}
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>

            {/* 费用限制 */}
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">
                费用限制/天 (USD)
              </label>
              <div className="flex items-center">
                <button
                  type="button"
                  onClick={() => handleNumberChange("max_cost_per_day", -1)}
                  className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
                >
                  −
                </button>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  value={formData.max_cost_per_day}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      max_cost_per_day: parseFloat(e.target.value) || 0,
                    })
                  }
                  className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                <button
                  type="button"
                  onClick={() => handleNumberChange("max_cost_per_day", 1)}
                  className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {/* 过期时间 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              过期时间
            </label>
            <input
              type="datetime-local"
              value={formData.expires_at || ""}
              onChange={(e) =>
                setFormData({ ...formData, expires_at: e.target.value || null })
              }
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
          </div>

          {/* 启用状态 */}
          <div className="flex items-center gap-3">
            <label className="text-sm font-medium text-neutral-700">
              启用状态
            </label>
            <button
              type="button"
              onClick={() =>
                setFormData({ ...formData, is_active: !formData.is_active })
              }
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                formData.is_active ? "bg-violet-600" : "bg-neutral-200"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  formData.is_active ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
            <span className="text-sm text-neutral-600">
              {formData.is_active ? "启用" : "停用"}
            </span>
          </div>

          {/* 日志模式 */}
          <div className="flex items-center gap-3">
            <label className="text-sm font-medium text-neutral-700">
              日志模式
            </label>
            <button
              type="button"
              onClick={() =>
                setFormData({ ...formData, log_mode: !formData.log_mode })
              }
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                formData.log_mode ? "bg-violet-600" : "bg-neutral-200"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  formData.log_mode ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
            <span className="text-sm text-neutral-600">
              {formData.log_mode ? "开启" : "关闭"}
            </span>
          </div>

          <div className="flex gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              disabled={submitting}
              className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={submitting || loadingKeys}
              className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {submitting && (
                <LoadingSpinner size="sm" tone="inverse" />
              )}
              {submitting ? "保存中..." : "保存"}
            </button>
          </div>
        </form>
      )}
    </div>
  );
};


export default EditDialog;
