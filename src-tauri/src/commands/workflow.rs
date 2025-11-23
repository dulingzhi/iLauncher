// 工作流相关命令
use crate::plugin::workflow_engine::{Workflow, WorkflowEngine};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// 获取所有工作流
#[tauri::command]
pub async fn list_workflows(
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<Vec<Workflow>, String> {
    let engine = engine.read().await;
    Ok(engine.list_workflows().await)
}

/// 获取单个工作流
#[tauri::command]
pub async fn get_workflow(
    id: String,
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<Option<Workflow>, String> {
    let engine = engine.read().await;
    Ok(engine.get_workflow(&id).await)
}

/// 保存工作流
#[tauri::command]
pub async fn save_workflow(
    workflow: Workflow,
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<(), String> {
    let engine = engine.read().await;
    engine.save_workflow(workflow).await.map_err(|e| e.to_string())
}

/// 删除工作流
#[tauri::command]
pub async fn delete_workflow(
    id: String,
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<(), String> {
    let engine = engine.read().await;
    engine.delete_workflow(&id).await.map_err(|e| e.to_string())
}

/// 执行工作流
#[tauri::command]
pub async fn execute_workflow(
    id: String,
    variables: HashMap<String, serde_json::Value>,
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<HashMap<String, serde_json::Value>, String> {
    let engine = engine.read().await;
    let context = engine.execute_workflow(&id, variables).await.map_err(|e| e.to_string())?;
    Ok(context.variables)
}

/// 通过关键词查找工作流
#[tauri::command]
pub async fn find_workflows_by_keyword(
    keyword: String,
    engine: State<'_, Arc<RwLock<WorkflowEngine>>>,
) -> Result<Vec<Workflow>, String> {
    let engine = engine.read().await;
    Ok(engine.find_by_keyword(&keyword).await)
}
