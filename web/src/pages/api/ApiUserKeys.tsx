/**
 * ApiUserKeys.tsx
 * 用户 API Keys 管理页：完整的增删改查和统计功能
 */

import React, { useState, useEffect, useCallback } from "react";
import {
  Search,
  Plus,
  Edit,
  Trash2,
  BarChart3,
  Eye,
  EyeOff,
  RefreshCw,
  Copy,
  ChevronLeft,
  ChevronRight,
  Key,
  Activity,
  Users,
} from "lucide-react";
import { StatCard } from "../../components/common/StatCard";
import FilterSelect from "../../components/common/FilterSelect";
import ModernSelect from "../../components/common/ModernSelect";
import DataTableShell from "@/components/common/DataTableShell";
import { api } from "../../lib/api";
import DialogPortal from "./user-keys/dialogs/DialogPortal";
import { ApiKey, DialogType } from "./user-keys/types";
import { copyWithFeedback } from "../../lib/clipboard";
import { LoadingSpinner, LoadingState } from "@/components/ui/loading";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

/** 页面主组件 */
const ApiUserKeysPage: React.FC = () => {
  const [data, setData] = useState<ApiKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [statusFilter, setStatusFilter] = useState<
    "all" | "active" | "disabled"
  >("all");
  const [selectedItem, setSelectedItem] = useState<ApiKey | null>(null);
  const [dialogType, setDialogType] = useState<DialogType>(null);
  const [showKeyValues, setShowKeyValues] = useState<{
    [key: string]: boolean;
  }>({});

  // 分页状态
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [totalItems, setTotalItems] = useState(0);

  // 获取API Keys列表
  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await api.userService.getKeys({
        page: currentPage,
        limit: pageSize,
        name: searchTerm || undefined,
        is_active:
          statusFilter === "all" ? undefined : statusFilter === "active",
      });

      if (response.success && response.data) {
        setData(response.data.service_api_keys || []);
        setTotalItems(response.data.pagination?.total || 0);
      } else {
        setError(response.message || "获取API Keys失败");
      }
    } catch (err) {
      setError("获取API Keys时发生错误");
      console.error("获取API Keys失败:", err);
    } finally {
      setLoading(false);
    }
  }, [currentPage, pageSize, searchTerm, statusFilter]);

  useEffect(() => {
    setCurrentPage(1);
  }, [searchTerm, statusFilter]);

  // 服务端分页 + 服务端过滤
  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const totalPages = Math.ceil(totalItems / pageSize);

  const paginatedData = data;

  // 添加新API Key
  const handleAdd = async (
    newKey: Omit<
      ApiKey,
      "id" | "usage" | "created_at" | "last_used_at" | "api_key"
    >
  ) => {
    try {
      const response = await api.userService.createKey({
        name: newKey.name,
        description: newKey.description,
        provider_type_id: newKey.provider_type_id,
        user_provider_keys_ids: newKey.user_provider_keys_ids || [],
        log_mode: newKey.log_mode,
        scheduling_strategy: newKey.scheduling_strategy,
        retry_count: newKey.retry_count,
        timeout_seconds: newKey.timeout_seconds,
        max_request_per_min: newKey.max_request_per_min,
        max_requests_per_day: newKey.max_requests_per_day,
        max_tokens_per_day: newKey.max_tokens_per_day,
        max_cost_per_day: newKey.max_cost_per_day,
        expires_at: newKey.expires_at || undefined,
        is_active: newKey.is_active,
      });

      if (response.success) {
        // 重新加载数据
        fetchData();
        setDialogType(null);
      } else {
        setError(response.message || "创建API Key失败");
      }
    } catch (err) {
      setError("创建API Key时发生错误");
      console.error("创建API Key失败:", err);
    }
  };

  // 编辑API Key
  const handleEdit = async (updatedKey: ApiKey) => {
    try {
      const response = await api.userService.updateKey(updatedKey.id, {
        name: updatedKey.name,
        description: updatedKey.description,
        user_provider_keys_ids: updatedKey.user_provider_keys_ids,
        log_mode: updatedKey.log_mode,
        scheduling_strategy: updatedKey.scheduling_strategy,
        retry_count: updatedKey.retry_count,
        timeout_seconds: updatedKey.timeout_seconds,
        max_request_per_min: updatedKey.max_request_per_min,
        max_requests_per_day: updatedKey.max_requests_per_day,
        max_tokens_per_day: updatedKey.max_tokens_per_day,
        max_cost_per_day: updatedKey.max_cost_per_day,
        expires_at: updatedKey.expires_at || undefined,
      });

      if (response.success) {
        // 重新加载数据
        fetchData();
        setDialogType(null);
        setSelectedItem(null);
      } else {
        setError(response.message || "更新API Key失败");
      }
    } catch (err) {
      setError("更新API Key时发生错误");
      console.error("更新API Key失败:", err);
    }
  };

  // 删除API Key
  const handleDelete = async () => {
    if (selectedItem) {
      try {
        const response = await api.userService.deleteKey(selectedItem.id);

        if (response.success) {
          // 重新加载数据
          fetchData();
          setDialogType(null);
          setSelectedItem(null);
        } else {
          setError(response.message || "删除API Key失败");
        }
      } catch (err) {
        setError("删除API Key时发生错误");
        console.error("删除API Key失败:", err);
      }
    }
  };

  // 切换API Key可见性
  const toggleKeyVisibility = (id: number) => {
    setShowKeyValues((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  // 渲染遮罩的API Key
  const renderMaskedKey = (key: string, id: number) => {
    const isVisible = showKeyValues[id];
    return (
      <div className="flex items-center gap-2">
        <code className="table-code">
          {isVisible
            ? key
            : `${key.substring(0, 8)}...${key.substring(key.length - 4)}`}
        </code>
        <button
          onClick={() => toggleKeyVisibility(id)}
          className="text-neutral-500 hover:text-neutral-700"
          title={isVisible ? "隐藏" : "显示"}
        >
          {isVisible ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
        <button
          onClick={() => void copyWithFeedback(key, "API Key")}
          className="text-neutral-500 hover:text-neutral-700"
          title="复制 API Key"
          aria-label="复制 API Key"
        >
          <Copy size={14} />
        </button>
      </div>
    );
  };

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">
            用户 API Keys
          </h2>
          <p className="text-sm text-neutral-600 mt-1">管理用户的API访问密钥</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={fetchData}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50"
            title="刷新数据"
          >
            {loading ? (
              <LoadingSpinner size="sm" tone="muted" />
            ) : (
              <RefreshCw size={16} />
            )}
            刷新
          </button>
          <button
            onClick={() => setDialogType("add")}
            className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700"
          >
            <Plus size={16} />
            新增密钥
          </button>
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="mb-4 p-4 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
          {error}
        </div>
      )}

      {/* 统计信息 */}
      {loading ? (
        <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="rounded-2xl border border-neutral-200 bg-white p-4"
            >
              <div className="flex items-center gap-3">
                <Skeleton className="h-10 w-10 rounded-xl" />
                <div className="flex-1">
                  <Skeleton className="h-4 w-20 mb-2" />
                  <Skeleton className="h-6 w-24" />
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
          <StatCard
            icon={<Key size={18} />}
            value={totalItems.toString()}
            label="总密钥数"
            color="#7c3aed"
          />
          <StatCard
            icon={<Activity size={18} />}
            value={data.filter((item) => item.is_active).length.toString()}
            label="活跃密钥"
            color="#10b981"
          />
          <StatCard
            icon={<Users size={18} />}
            value={data
              .reduce(
                (sum, item) => sum + (item.usage?.successful_requests || 0),
                0
              )
              .toLocaleString()}
            label="总使用次数"
            color="#0ea5e9"
          />
        </div>
      )}

      {/* 搜索和过滤 */}
      <div className="flex items-center gap-4 mb-4">
        <div className="relative flex-1 max-w-md">
          <Search
            className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400"
            size={16}
          />
          <input
            type="text"
            placeholder="搜索密钥名称..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-2">
          <FilterSelect
            value={statusFilter}
            onValueChange={(value) =>
              setStatusFilter(value as "all" | "active" | "disabled")
            }
            options={[
              { value: "all", label: "全部状态" },
              { value: "active", label: "启用" },
              { value: "disabled", label: "停用" },
            ]}
            placeholder="全部状态"
          />
        </div>
      </div>

      {/* 加载指示器 */}
      {loading && (
        <div className="flex justify-center py-8">
          <LoadingState text="加载中..." />
        </div>
      )}

      {/* 数据表格 */}
      {!loading && (
        <DataTableShell>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>密钥名称</TableHead>
                <TableHead>描述</TableHead>
                <TableHead>服务商</TableHead>
                <TableHead>API Key</TableHead>
                <TableHead>使用情况</TableHead>
                <TableHead>状态</TableHead>
                <TableHead>日志</TableHead>
                <TableHead>最后使用</TableHead>
                <TableHead>操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {paginatedData.map((item) => (
                <TableRow key={item.id}>
                  <TableCell>
                    <div>
                      <div className="font-medium">{item.name}</div>
                      <div className="table-subtext">
                        创建于{" "}
                        {new Date(item.created_at).toLocaleDateString()}
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div
                      className="table-subtext max-w-xs truncate"
                      title={item.description || ""}
                    >
                      {item.description || "无描述"}
                    </div>
                  </TableCell>
                  <TableCell>
                    <span className="table-tag">
                      {item.provider || `服务商 ${item.provider_type_id}`}
                    </span>
                  </TableCell>
                  <TableCell>{renderMaskedKey(item.api_key, item.id)}</TableCell>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <span className="text-sm">
                        {(item.usage?.successful_requests || 0).toLocaleString()}{" "}
                        /{" "}
                        {(item.usage?.failed_requests || 0).toLocaleString()}
                      </span>
                      <button
                        onClick={() => {
                          setSelectedItem(item);
                          setDialogType("stats");
                        }}
                        className="text-violet-600 hover:text-violet-700"
                        title="查看统计"
                      >
                        <BarChart3 size={14} />
                      </button>
                    </div>
                    <div className="mt-1 h-1.5 w-full rounded-full bg-neutral-200">
                      <div
                        className="h-1.5 rounded-full bg-violet-600"
                        style={{
                          width: `${Math.min(
                            ((item.usage?.successful_requests || 0) /
                              Math.max(
                                1,
                                (item.usage?.successful_requests || 0) +
                                  (item.usage?.failed_requests || 0)
                              )) *
                              100,
                            100
                          )}%`,
                        }}
                      />
                    </div>
                    <div className="mt-1 table-subtext">
                      速率限制/分: {(item.max_request_per_min || 0) > 0 ? `${item.max_request_per_min!.toLocaleString()}` : "无"}
                    </div>
                    <div className="table-subtext">
                      速率限制/天:{" "}
                      {(item.max_requests_per_day || 0) > 0 ? `${item.max_requests_per_day!.toLocaleString()}` : "无"}
                    </div>
                    <div className="table-subtext">
                      Token/天:{" "}
                      {(item.max_tokens_per_day || 0) > 0 ? `${item.max_tokens_per_day!.toLocaleString()}` : "无"}
                    </div>
                    <div className="table-subtext">
                      费用/天:{" "}
                      {Number(item.max_cost_per_day || 0) > 0
                        ? `$${Number(item.max_cost_per_day || 0).toFixed(2)}`
                        : "无"}
                    </div>
                  </TableCell>
                  <TableCell>
                    <span className={item.is_active ? "table-status-success" : "table-status-muted"}>
                      {item.is_active ? "启用" : "停用"}
                    </span>
                  </TableCell>
                  <TableCell>
                    <span className={item.log_mode ? "table-status-success" : "table-status-muted"}>
                      {item.log_mode ? "开启" : "关闭"}
                    </span>
                  </TableCell>
                  <TableCell className="table-subtext">
                    {item.last_used_at
                      ? new Date(item.last_used_at).toLocaleString()
                      : "从未使用"}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => {
                          setSelectedItem(item);
                          setDialogType("edit");
                        }}
                        className="p-1 text-neutral-500 hover:text-violet-600"
                        title="编辑"
                      >
                        <Edit size={16} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedItem(item);
                          setDialogType("delete");
                        }}
                        className="p-1 text-neutral-500 hover:text-red-600"
                        title="删除"
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          {/* 分页组件 */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between px-4 py-3 border-t border-neutral-200">
              <div className="text-sm text-neutral-600">
                显示 {(currentPage - 1) * pageSize + 1} -{" "}
                {Math.min(currentPage * pageSize, totalItems)} 条， 共{" "}
                {totalItems} 条记录
              </div>
              <div className="flex items-center gap-4">
                {/* 每页数量选择 */}
                <div className="flex items-center gap-2">
                  <span className="text-sm text-neutral-600">每页</span>
                  <ModernSelect
                    value={pageSize.toString()}
                    onValueChange={(value) => {
                      const newSize = Number(value);
                      setPageSize(newSize);
                      setCurrentPage(1); // 重置到第一页
                    }}
                    options={[
                      { value: "10", label: "10" },
                      { value: "20", label: "20" },
                      { value: "50", label: "50" },
                      { value: "100", label: "100" },
                    ]}
                    triggerClassName="h-8 w-16"
                  />
                  <span className="text-sm text-neutral-600">条</span>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    onClick={() =>
                      setCurrentPage((prev) => Math.max(prev - 1, 1))
                    }
                    disabled={currentPage === 1}
                    className={`flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg border ${
                      currentPage === 1
                        ? "bg-neutral-50 text-neutral-400 border-neutral-200 cursor-not-allowed"
                        : "bg-white text-neutral-700 border-neutral-200 hover:bg-neutral-50"
                    }`}
                  >
                    <ChevronLeft size={16} />
                    上一页
                  </button>

                  <div className="flex items-center gap-1">
                    {Array.from({ length: Math.min(5, totalPages) }, (_, i) => {
                      // 显示当前页附近5个页码
                      const start = Math.max(
                        1,
                        Math.min(currentPage - 2, totalPages - 4)
                      );
                      const page = start + i;
                      return (
                        <button
                          key={page}
                          onClick={() => setCurrentPage(page)}
                          className={`px-3 py-1.5 text-sm rounded-lg ${
                            page === currentPage
                              ? "bg-violet-600 text-white"
                              : "bg-white text-neutral-700 border border-neutral-200 hover:bg-neutral-50"
                          }`}
                        >
                          {page}
                        </button>
                      );
                    })}
                  </div>

                  <button
                    onClick={() =>
                      setCurrentPage((prev) => Math.min(prev + 1, totalPages))
                    }
                    disabled={currentPage === totalPages}
                    className={`flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg border ${
                      currentPage === totalPages
                        ? "bg-neutral-50 text-neutral-400 border-neutral-200 cursor-not-allowed"
                        : "bg-white text-neutral-700 border-neutral-200 hover:bg-neutral-50"
                    }`}
                  >
                    下一页
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            </div>
          )}
        </DataTableShell>
      )}

      {/* 对话框组件 */}
      {dialogType && (
        <DialogPortal
          type={dialogType}
          selectedItem={selectedItem}
          onClose={() => {
            setDialogType(null);
            setSelectedItem(null);
          }}
          onAdd={handleAdd}
          onEdit={handleEdit}
          onDelete={handleDelete}
        />
      )}
    </div>
  );
};


export default ApiUserKeysPage;
