// 插件系统

pub mod calculator;
pub mod app_search;
pub mod file_search;

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;

/// 插件特征
#[async_trait]
pub trait Plugin: Send + Sync {
    /// 获取插件元数据
    fn metadata(&self) -> &PluginMetadata;
    
    /// 查询
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>>;
    
    /// 执行动作
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()>;
}

/// 插件管理器
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub async fn new() -> Self {
        let mut manager = Self {
            plugins: Vec::new(),
        };
        
        // 注册插件
        manager.register(Box::new(calculator::CalculatorPlugin::new()));
        
        let app_search = app_search::AppSearchPlugin::new();
        app_search.init().await;
        manager.register(Box::new(app_search));
        
        let file_search = file_search::FileSearchPlugin::new();
        file_search.init().await;
        manager.register(Box::new(file_search));
        
        manager
    }
    
    /// 注册插件
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
    
    /// 查询所有插件
    pub async fn query(&self, input: &str) -> Result<Vec<QueryResult>> {
        let ctx = QueryContext {
            query_type: QueryType::Input,
            trigger_keyword: String::new(),
            command: None,
            search: input.to_string(),
            raw_query: input.to_string(),
        };
        
        let mut all_results = Vec::new();
        
        for plugin in &self.plugins {
            match plugin.query(&ctx).await {
                Ok(mut results) => {
                    all_results.append(&mut results);
                }
                Err(e) => {
                    tracing::warn!("Plugin {} query failed: {}", plugin.metadata().name, e);
                }
            }
        }
        
        // 按分数排序
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(all_results)
    }
    
    /// 执行动作
    pub async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        for plugin in &self.plugins {
            if let Ok(()) = plugin.execute(result_id, action_id).await {
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!("No plugin handled the action"))
    }
    
    /// 获取所有插件元数据
    pub fn get_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins.iter().map(|p| p.metadata().clone()).collect()
    }
}
