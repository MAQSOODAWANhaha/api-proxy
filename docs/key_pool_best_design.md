# API 密钥池最佳设计方案

> 综合 `docs/KEY_POOL_HEALTH_CHECK_OPTIMIZATION.md` 与 `@docs/key_pool_refactor_plan.md` 的优点，并结合最新约束：健康检查器仅负责检测并更新健康状态；加权调度仅基于权重；所有调度策略在健康筛选之后运行；完整纳入 `rate_limit_reset_task` 的生命周期管理。

## 1. 设计目标

- **启动即可用**：系统启动阶段完成密钥池预热、健康检查器与限流恢复任务的初始化。
- **单职责组件**：`ApiKeyHealthChecker` 专注于检测与更新状态；调度器只处理已过滤的健康密钥。
- **统一调度语义**：无论轮询还是加权策略，都在同一健康筛选前提下执行，确保行为可预测。
- **一致的状态管理**：`RateLimitResetTask`、健康状态映射、数据库之间状态同步，避免多源冲突。
- **可观测与可配置**：关键参数可配置，运行状态可通过度量指标与管理接口查看。

## 2. 总体架构

```
key_pool/
├── mod.rs                      // Facade 与 re-export
├── service.rs                  // KeyPoolService：统一入口
├── repository.rs               // ApiKeyRepository：数据库访问封装
├── scheduler/
│   ├── mod.rs                  // SchedulerRegistry / trait
│   ├── round_robin.rs
│   └── weighted.rs
├── health/
│   ├── mod.rs                  // HealthService：编排器
│   ├── checker.rs              // ApiKeyHealthChecker（只负责检测&状态更新）
│   └── config.rs               // 配置加载
├── tasks.rs                    // 后台任务管理（含 RateLimitResetTask 适配层）
└── types.rs                    // 对外类型（无 HealthBest）
```

### 核心职责划分

- **KeyPoolService**：提供调度入口、统计信息与就绪探针；依赖仓储、调度器注册表、健康服务，同时管理所需指标。
- **ApiKeyRepository**：封装 `user_provider_keys` 查询，提供基础的过滤、排序与事务能力，不保留长驻缓存。
- **SchedulerRegistry**：注册 `RoundRobin` 与 `Weighted` 两种调度器，暴露按策略检索接口。
- **HealthService**：协调 `ApiKeyHealthChecker`、`RateLimitResetTask` 与后台周期任务；封装查询与过滤接口。
- **ApiKeyHealthChecker**：只负责执行健康检测，并将结果同步至数据库与内存健康映射；不参与调度策略。
- **RateLimitResetTask**：作为健康服务的子任务，专注处理限流过期后的状态恢复。

## 3. 启动与生命周期

1. **`dual_port_setup::initialize_shared_services`**：
   - 创建 `ApiKeyRepository`（注入 `Arc<DatabaseConnection>`）。
   - 创建 `ApiKeyHealthChecker`（注入 DB、HTTP 客户端、自定义配置）。
   - 创建 `RateLimitResetTask`（共享健康状态映射）。
   - 组合成 `HealthService`（持有 checker、reset_task、调度循环控制）。
   - 构建 `KeyPoolService` 并注入 `AppContext`（新增字段 `key_pool_service: Arc<KeyPoolService>`）。

2. **`run_dual_port_servers`**：
   - 在启动阶段调用 `context.key_pool_service.bootstrap().await?`：
     - 预热所有活跃密钥（根据配置可选择全量或延迟预热）。
     - 启动 `HealthService` 内的后台循环与 `RateLimitResetTask`。
     - 如果配置禁用预热/主动检查，则记录日志并保持被动模式。

3. **`ProxyServerBuilder::create_proxy_service` 与各业务服务**：
   - 复用 `AppContext` 中的 `Arc<KeyPoolService>`，不再创建新的 `ApiKeyHealthChecker`。
   - 从 `KeyPoolService` 获取调度结果或健康状态，不直接操作底层组件。
   - 代理对外开放流量前，可选地等待 `key_pool_service.is_ready()` 返回 `true`，确保启动预热已完成，避免冷启动窗口内的调度失败。

## 4. 调度流程

```text
请求 → 构建 SelectionContext → KeyPoolService::select(service_api, ctx)
  1. repository.load_active_keys(service_api_id)
       - 直接查询数据库获取当前启用的密钥
       - 按 provider_type、权重等字段排序，便于后续策略使用
  2. KeyPoolService::filter_usable(keys, ctx)
       - 静态校验：is_active / 授权状态 / 过期时间
       - 健康筛选：HealthService::filter_healthy(keys)
           · Healthy → 保留
           · RateLimited → 交给 RateLimitResetTask 继续追踪，无立即使用
           · Unhealthy/未知 → 过滤
       - 如果无健康密钥：返回 `error!(KeyPool, "No available keys")`
  3. SchedulerRegistry::resolve(strategy, keys)
       - Strategy 来源：service_api.strategy 或配置默认值
       - RoundRobin：针对健康且活跃的密钥轮询
       - Weighted：针对健康且活跃的密钥按权重构建队列
  4. 调度结果封装 `ApiKeySelectionResult`，追加日志、指标，并返回
```

### 调度策略约束

- `SchedulingStrategy` 仅包含 `RoundRobin` 与 `Weighted`。
- 所有调度实现要求输入已经通过健康筛选的密钥集合。
- `Weighted` 逻辑：`weight <= 0` 时按 1 处理；只影响健康密钥的轮询序列，不叠加额外健康分数。
- 若权重全部为 0，则回退至轮询策略（记录 warning）。

## 5. 健康检查流程

### 5.1 HealthService 编排

- 保存健康状态映射（`DashMap<i32, ApiKeyHealth>` 或 `RwLock<HashMap<..>>`）。
- 提供接口：
  - `async fn start(&self) -> Result<()>`：调用 `checker.start()` 及 `reset_task.start()`，启动后台循环。
  - `async fn stop(&self) -> Result<()>`：停止循环与任务。
  - `async fn filter_healthy(&self, &[ApiKeyRecord]) -> Vec<ApiKeyRecord>`：结合健康状态映射与限流信息过滤。
  - `async fn snapshot(&self) -> HealthSnapshot`：用于监控输出。
  - `async fn mark_unhealthy(&self, key_id, reason)`：手动标记。

- 后台循环（可配置）：
  1. 通过仓储查询活跃密钥。
  2. 基于 `ApiKeyHealth::should_check()` 判定需要检测的密钥集合。
  3. 调用 `checker.check_api_key(&key)` 执行检测。
  4. 根据检测结果更新健康状态映射及数据库，如遇限流则调用 `reset_task.schedule_reset`。
  5. 循环间隔由配置控制；若整体禁用健康检查，则仅维持被动状态更新。
  6. 低频增量同步（如每 5 分钟）：扫描 `user_provider_keys` 中近期更新的密钥，将新增或修改项登记到健康状态映射，确保运行期变更自动生效。

### 5.2 ApiKeyHealthChecker（单一职责）

- **输入**：`user_provider_keys::Model`。
- **行为**：
  - 调用提供商对应的健康检查接口（按配置 path/方法）。
  - 根据响应判定当前密钥是否健康。
  - 将结果写入数据库：`health_status`, `health_status_detail`, `last_check`, `consecutive_failures/successes` 等。
  - 更新内存健康映射中的 `ApiKeyHealth`。
  - 对于限流（rate_limited）响应，调用 `reset_task.schedule_reset`。
- **输出**：`ApiKeyCheckResult`，包含时间、成功标记、响应时间、错误类别等。
- 不负责选择策略、也不计算健康分数；仅提供布尔健康状态与基础统计字段。

### 5.3 RateLimitResetTask 集成

- 由 `HealthService` 构造并管理生命周期。
- 收到 `schedule_reset(key_id, resets_at)` 时：
  1. 注册延迟任务，等待限流解除时间。
  2. 到期后调用 `reset_key_status`：
     - 数据库：将 `health_status` 重置为 `healthy`，清空 `rate_limit_resets_at`。
     - 内存健康映射：同步更新 `ApiKeyHealth`，设置 `is_healthy = true`。
- 支持 `Remove` 命令（手动取消），用于密钥下线/删除场景。
- 停止服务时 graceful abort，并记录日志。

### 5.4 失败上报与分类

- 代理转发过程中捕获上游错误时，通过 `HealthService::report_failure`（或 `mark_unhealthy` 的高阶封装）进行反馈。
- 将失败划分为两类：
  - **确定性失败**：如 `401 Unauthorized`（密钥失效）、`403 Forbidden`、`429 Too Many Requests`。立即更新健康状态：`401/403` 直接标记为 `unhealthy`，`429` 标记为 `rate_limited` 并交由 `RateLimitResetTask` 调度恢复。
  - **瞬时性失败**：如 `500/502/503` 或网络超时。记录连续失败计数，仅当同一密钥在短时间内连续达到阈值（默认 3 次，可复用 `ApiKeyHealth` 的 `consecutive_failures` 字段）时才标记为 `unhealthy`，避免对暂时波动过度惩罚。
- 成功请求清零对应的连续失败计数，并更新 `last_healthy` 时间，有助于后续动态评估。
- 分类决策应记录结构化日志，便于后续分析与调优。

### 5.5 管理端变更触发流程

- 管理端的新增、更新、删除操作（`src/management/handlers/provider_keys.rs`）在业务层执行完成后，必须调用 `HealthService` 的对应钩子，保证运行态立即感知密钥变化：
  - **Create**：`HealthService::register_new_key(key_id)`，加载数据库最新数据并立即排队一次健康检测，确保新密钥快速投入使用。
  - **Update**：`HealthService::refresh_key(key_id)`，重新读取密钥详情；若状态从禁用变为启用或敏感字段（权重、配额、secret）发生改变，则重置连续失败计数并触发即时健康检查。
  - **Delete**：`HealthService::remove_key(key_id)`，从健康状态映射中移除对应条目，同时通过 `RateLimitResetTask::cancel` 取消未完成的限流恢复任务。
- `ProviderKeyService` 可以统一封装这些调用，handler 层只需按现有模式返回响应，降低重复代码。
- 每次钩子调用需输出业务日志（包含 `request_id`、操作类型、key_id`），便于审计和排障。

## 6. 配置与监控

- 继续沿用现有 `AppConfig`，健康检查、预热等参数通过代码默认值控制，无需新增配置项；如未来需要开放可再行扩展。
- 监控指标：
  - 调度次数、策略分布、数据库取数耗时与行数统计。
  - 健康检查成功/失败统计、限流恢复触发次数。
  - `RateLimitResetTask` 待恢复密钥数量、执行成功/失败计数。
  - 动态增量同步统计（扫描次数、新增/更新密钥数、同步耗时），确保运行期变更被及时感知。
- 管理端 API：
  - `GET /api/system/key-pool/stats`：输出 `KeyPoolStats`。
  - `POST /api/system/key-pool/health/check/{key_id}`：即时触发健康检查。
  - （可选）`POST /api/system/key-pool/resync`：触发一次后台增量同步，便于在运维场景下快速刷新健康状态。

## 7. 数据与兼容性

- 数据库中若存在 `health_best` 策略值，迁移脚本统一改写为 `round_robin`；接口解析时发现未知值则回退默认并记录 warning。
- 保持 `user_provider_keys`、`provider_types` 表结构不变；如后续增强健康检查参数，可通过 `provider_types` 的配置字段扩展。
- `ApiKeyHealth` 结构保留 `consecutive_failures/successes` 等统计字段，以支撑未来扩展（如黑名单策略），但当前调度仅使用布尔健康状态。

## 8. 迁移步骤

1. **模块拆分**：按新目录结构重构 `key_pool`，迁移现有逻辑并确保功能等价。
2. **Service 汇聚**：实现 `KeyPoolService`，整合原 `ApiKeyPoolManager` 对外接口。
3. **策略精简**：移除 `HealthBestApiKeySelector`，更新 `SchedulingStrategy` 枚举及相关测试。
4. **启动改造**：`AppContext`、`dual_port_setup`、`ProxyServerBuilder` 按新依赖关系调整。
5. **健康检查调整**：重写 `ApiKeyHealthChecker` 与 `HealthService` 分工，保持只负责检测与更新；整合 `RateLimitResetTask`。
6. **仓储与管理钩子**：实现统一的数据库访问层，并在 `ProviderKeyService` 中接入健康服务钩子；保证新增/修改/删除立即同步到健康状态。
   - 在 HealthService 后台循环内加入低频增量同步：依据 `updated_at` 扫描最近变更的密钥，自动刷新健康状态，减少对手动操作或重启的依赖。
7. **配置/监控**：扩展 `AppConfig` 与管理端 API；更新文档与部署脚本。
8. **回归测试**：补齐单元测试（调度、健康过滤）、集成测试（API 调用路径）、端到端启动验证。

## 9. 风险与应对

- **启动失败**：若 `bootstrap()` 异常，可通过配置关闭预热与主动检查，保留懒加载模式。
- **状态漂移**：若健康状态与数据库脱节，可通过管理端钩子或手动触发增量同步快速纠正，同时保留回退到完全被动模式的能力。
- **健康误判**：健康检查策略可配置关闭；出现误判时，`RateLimitResetTask` 仍可恢复状态，运营可手动标记。
- **性能压力**：健康检查并发数受配置限制；如需进一步优化，可引入按提供商拆分或分布式执行。
- **增量同步开销**：周期性扫描数据库可能带来额外负载，可通过调节频率、限制每次扫描窗口或在运营变更后按需触发来控制，并配合数据库索引优化。

## 10. 验证计划

- **编译与静态检查**：`cargo build`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
- **单元测试**：新增 `tests/key_pool_service.rs`，覆盖调度逻辑与健康筛选。
- **集成测试**：模拟多策略、多状态的密钥池，验证过滤与限流恢复。
- **观测验证**：在预发布环境观测调度成功率、数据库查询耗时、健康检查成功率、限流恢复日志，确保与设计一致。
- **动态同步测试**：在运行时新增/修改密钥，验证低频增量同步能在预期时间窗口内刷新健康状态。
- **就绪探针验证**：模拟预热耗时较长的场景，确认 `is_ready()` gating 能防止冷启动期间的请求失败。

---

通过上述方案，`key_pool` 模块在职责划分、健康管理、调度策略与运维工具等方面实现统一规范，解决原有启动缺陷、策略重叠、状态割裂问题，同时满足“健康检查器只负责检测&更新、调度基于健康过滤”的最新需求，为后续功能拓展提供稳定基础。
