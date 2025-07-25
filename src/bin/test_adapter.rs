//! 测试适配器管理器

use api_proxy::providers::manager::AdapterManager;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("创建 AdapterManager 实例...");
    let adapter_manager = AdapterManager::new();
    
    println!("获取适配器统计信息...");
    let stats = adapter_manager.get_adapter_stats();
    
    println!("适配器统计信息:");
    for (name, stat) in &stats {
        println!("- {}: {} 端点, 上游类型: {}", 
                 name, 
                 stat.supported_endpoints, 
                 stat.upstream_type);
        println!("  端点列表: {:?}", stat.endpoints);
    }
    
    println!("总共找到 {} 个适配器", stats.len());
}