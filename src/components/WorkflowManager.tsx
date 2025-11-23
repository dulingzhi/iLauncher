// 工作流管理器组件（简化版）
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Workflow {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  trigger: any;
  steps: any[];
}

export function WorkflowManager() {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadWorkflows();
  }, []);

  const loadWorkflows = async () => {
    try {
      const data = await invoke<Workflow[]>('list_workflows');
      setWorkflows(data);
    } catch (e) {
      console.error('Failed to load workflows:', e);
    }
  };

  const executeWorkflow = async (id: string) => {
    setLoading(true);
    try {
      await invoke('execute_workflow', { id, variables: {} });
      alert('工作流执行成功！');
    } catch (e: any) {
      alert(`执行失败: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const deleteWorkflow = async (id: string) => {
    if (!confirm('确定要删除此工作流吗？')) return;
    try {
      await invoke('delete_workflow', { id });
      loadWorkflows();
    } catch (e: any) {
      alert(`删除失败: ${e}`);
    }
  };

  const selected = workflows.find((w) => w.id === selectedId);

  return (
    <div className="workflow-manager">
      <div className="header">
        <h1>工作流管理</h1>
        <button className="btn-primary">+ 新建工作流</button>
      </div>

      <div className="content">
        <div className="workflow-list">
          {workflows.length === 0 ? (
            <div className="empty">暂无工作流</div>
          ) : (
            workflows.map((workflow) => (
              <div
                key={workflow.id}
                className={`workflow-item ${selectedId === workflow.id ? 'selected' : ''}`}
                onClick={() => setSelectedId(workflow.id)}
              >
                <div className="item-header">
                  <h3>{workflow.name}</h3>
                  <span className={`status ${workflow.enabled ? 'enabled' : 'disabled'}`}>
                    {workflow.enabled ? '已启用' : '已禁用'}
                  </span>
                </div>
                <p className="description">{workflow.description}</p>
                <div className="item-actions">
                  <button onClick={(e) => { e.stopPropagation(); executeWorkflow(workflow.id); }}>
                    执行
                  </button>
                  <button onClick={(e) => { e.stopPropagation(); deleteWorkflow(workflow.id); }}>
                    删除
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        {selected && (
          <div className="workflow-detail">
            <h2>{selected.name}</h2>
            <p>{selected.description}</p>
            <div className="steps">
              <h3>步骤 ({selected.steps.length})</h3>
              {selected.steps.map((step: any, index: number) => (
                <div key={index} className="step-item">
                  <span className="step-number">{index + 1}</span>
                  <span className="step-name">{step.name}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      <style>{`
        .workflow-manager {
          padding: 20px;
          height: 100vh;
          background: var(--color-background);
          color: var(--color-text-primary);
        }

        .header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 20px;
        }

        .header h1 {
          margin: 0;
          font-size: 24px;
        }

        .btn-primary {
          padding: 10px 20px;
          background: var(--color-primary);
          color: white;
          border: none;
          border-radius: 4px;
          cursor: pointer;
        }

        .content {
          display: grid;
          grid-template-columns: 1fr 2fr;
          gap: 20px;
          height: calc(100vh - 100px);
        }

        .workflow-list {
          overflow-y: auto;
          border-right: 1px solid var(--color-border);
          padding-right: 20px;
        }

        .empty {
          text-align: center;
          padding: 40px;
          color: var(--color-text-muted);
        }

        .workflow-item {
          background: var(--color-surface);
          border: 1px solid var(--color-border);
          border-radius: 8px;
          padding: 15px;
          margin-bottom: 10px;
          cursor: pointer;
          transition: all 0.2s;
        }

        .workflow-item:hover {
          border-color: var(--color-primary);
        }

        .workflow-item.selected {
          border-color: var(--color-primary);
          background: var(--color-hover);
        }

        .item-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
        }

        .item-header h3 {
          margin: 0;
          font-size: 16px;
        }

        .status {
          font-size: 12px;
          padding: 4px 8px;
          border-radius: 4px;
        }

        .status.enabled {
          background: #4caf50;
          color: white;
        }

        .status.disabled {
          background: var(--color-surface);
          color: var(--color-text-muted);
        }

        .description {
          font-size: 14px;
          color: var(--color-text-secondary);
          margin: 8px 0;
        }

        .item-actions {
          display: flex;
          gap: 10px;
          margin-top: 10px;
        }

        .item-actions button {
          padding: 6px 12px;
          border: none;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
        }

        .item-actions button:first-child {
          background: var(--color-primary);
          color: white;
        }

        .item-actions button:last-child {
          background: #ff4444;
          color: white;
        }

        .workflow-detail {
          overflow-y: auto;
          padding-left: 20px;
        }

        .workflow-detail h2 {
          margin: 0 0 10px 0;
          font-size: 20px;
        }

        .steps {
          margin-top: 20px;
        }

        .steps h3 {
          margin: 0 0 15px 0;
          font-size: 16px;
        }

        .step-item {
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 12px;
          background: var(--color-surface);
          border: 1px solid var(--color-border);
          border-radius: 4px;
          margin-bottom: 8px;
        }

        .step-number {
          width: 24px;
          height: 24px;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--color-primary);
          color: white;
          border-radius: 50%;
          font-size: 12px;
          font-weight: bold;
        }

        .step-name {
          font-size: 14px;
        }
      `}</style>
    </div>
  );
}
