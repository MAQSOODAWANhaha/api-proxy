
import { useEffect, useState } from 'react';
import { api, SystemMetrics } from '@/lib/api';
import { Cpu, MemoryStick, HardDrive, Clock, RefreshCw } from 'lucide-react';
import { LoadingSpinner } from '@/components/ui/loading';
import { Skeleton } from '@/components/ui/skeleton';

const SystemInfo = () => {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  const fetchMetrics = async () => {
    try {
      const response = await api.system.getMetrics();
      if (response.success && response.data) {
        setMetrics(response.data);
        setLastUpdated(new Date());
      } else {
        setMetrics(null);
      }
    } catch (error) {
      console.error('获取系统监控信息失败:', error);
      setMetrics(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMetrics();
    // 每30秒刷新一次数据
    const interval = setInterval(fetchMetrics, 30000);
    return () => clearInterval(interval);
  }, []);

  // 获取使用率颜色
  const getUsageColor = (percentage: number) => {
    if (percentage < 60) return 'text-emerald-600';
    if (percentage < 80) return 'text-yellow-600';
    return 'text-red-600';
  };

  // 获取进度条颜色
  const getProgressColor = (percentage: number) => {
    if (percentage < 60) return 'bg-emerald-500';
    if (percentage < 80) return 'bg-yellow-500';
    return 'bg-red-500';
  };

  // 进度条组件
  const ProgressBar = ({ percentage, className = '' }: { percentage: number; className?: string }) => (
    <div className={`w-full bg-neutral-200 rounded-full h-2 overflow-hidden ${className}`}>
      <div 
        className={`h-full transition-all duration-500 ease-out ${getProgressColor(percentage)}`}
        style={{ width: `${Math.min(percentage, 100)}%` }}
      />
    </div>
  );

  if (loading) {
    return (
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold text-neutral-800">系统监控</h3>
          <LoadingSpinner size="sm" tone="muted" />
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {[...Array(4)].map((_, i) => (
            <div key={i} className="bg-white p-6 rounded-2xl border border-neutral-200">
              <div className="flex items-center gap-3 mb-4">
                <Skeleton className="h-8 w-8 rounded-lg" />
                <Skeleton className="h-4 w-20" />
              </div>
              <Skeleton className="h-8 w-16 mb-2" />
              <Skeleton className="h-2 w-full mb-2" />
              <Skeleton className="h-3 w-24" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-2xl p-6">
        <div className="flex items-center gap-3 text-red-700">
          <RefreshCw size={20} />
          <span className="font-medium">无法加载系统监控信息</span>
        </div>
        <p className="text-red-600 text-sm mt-2">请检查后端服务是否正常运行</p>
      </div>
    );
  }

  const cpuUsage = metrics.cpu_usage || 0;
  const memoryUsage = metrics.memory?.usage_percentage || 0;
  const diskUsage = metrics.disk?.usage_percentage || 0;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-neutral-800">系统监控</h3>
        <div className="flex items-center gap-2 text-xs text-neutral-500">
          <Clock size={12} />
          <span>
            {lastUpdated ? `更新于 ${lastUpdated.toLocaleTimeString()}` : '加载中...'}
          </span>
        </div>
      </div>
      
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* CPU 使用率 */}
        <div className="bg-white p-6 rounded-2xl border border-neutral-200">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-violet-100 rounded-lg">
              <Cpu size={16} className="text-violet-600" />
            </div>
            <span className="text-sm font-medium text-neutral-700">CPU 使用率</span>
          </div>
          
          <div className={`text-2xl font-bold mb-2 ${getUsageColor(cpuUsage)}`}>
            {cpuUsage.toFixed(1)}%
          </div>
          
          <ProgressBar percentage={cpuUsage} className="mb-3" />
          
          <div className="flex justify-between text-xs text-neutral-500">
            <span>负载均衡</span>
            <span className={cpuUsage > 80 ? 'text-red-500 font-medium' : ''}>
              {cpuUsage > 80 ? '高负载' : cpuUsage > 60 ? '中等' : '正常'}
            </span>
          </div>
        </div>

        {/* 内存使用率 */}
        <div className="bg-white p-6 rounded-2xl border border-neutral-200">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-emerald-100 rounded-lg">
              <MemoryStick size={16} className="text-emerald-600" />
            </div>
            <span className="text-sm font-medium text-neutral-700">内存使用率</span>
          </div>
          
          <div className={`text-2xl font-bold mb-2 ${getUsageColor(memoryUsage)}`}>
            {memoryUsage.toFixed(1)}%
          </div>
          
          <ProgressBar percentage={memoryUsage} className="mb-3" />
          
          <div className="text-xs text-neutral-500">
            <div className="flex justify-between">
              <span>已用</span>
              <span>{(metrics.memory?.used_mb || 0).toLocaleString()} MB</span>
            </div>
            <div className="flex justify-between">
              <span>总计</span>
              <span>{(metrics.memory?.total_mb || 0).toLocaleString()} MB</span>
            </div>
          </div>
        </div>

        {/* 磁盘使用率 */}
        <div className="bg-white p-6 rounded-2xl border border-neutral-200">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-blue-100 rounded-lg">
              <HardDrive size={16} className="text-blue-600" />
            </div>
            <span className="text-sm font-medium text-neutral-700">磁盘使用率</span>
          </div>
          
          <div className={`text-2xl font-bold mb-2 ${getUsageColor(diskUsage)}`}>
            {diskUsage.toFixed(1)}%
          </div>
          
          <ProgressBar percentage={diskUsage} className="mb-3" />
          
          <div className="text-xs text-neutral-500">
            <div className="flex justify-between">
              <span>已用</span>
              <span>{(metrics.disk?.used_gb || 0).toLocaleString()} GB</span>
            </div>
            <div className="flex justify-between">
              <span>总计</span>
              <span>{(metrics.disk?.total_gb || 0).toLocaleString()} GB</span>
            </div>
          </div>
        </div>

        {/* 运行时间 */}
        <div className="bg-white p-6 rounded-2xl border border-neutral-200">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 bg-orange-100 rounded-lg">
              <Clock size={16} className="text-orange-600" />
            </div>
            <span className="text-sm font-medium text-neutral-700">程序运行时间</span>
          </div>
          
          <div className="text-2xl font-bold text-orange-900 mb-2">
            {metrics.uptime || '0m'}
          </div>
          
          <div className="text-xs text-neutral-500 space-y-1">
            <div>自程序启动以来</div>
            <div className="text-orange-600 font-medium">
              运行稳定
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemInfo;
