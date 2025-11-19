import { useEffect, useRef } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { cn } from '../utils/cn';
import type { Action } from '../types';

interface ContextMenuProps {
  x: number;
  y: number;
  actions: Action[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onExecute: (actionId: string) => void;
  onClose: () => void;
}

export function ContextMenu({
  x,
  y,
  actions,
  selectedIndex,
  onSelect,
  onExecute,
  onClose,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  // 调整菜单位置防止超出屏幕
  useEffect(() => {
    if (!menuRef.current) return;

    const menu = menuRef.current;
    const rect = menu.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let adjustedX = x;
    let adjustedY = y;

    // 防止右侧超出
    if (rect.right > viewportWidth) {
      adjustedX = viewportWidth - rect.width - 10;
    }

    // 防止底部超出
    if (rect.bottom > viewportHeight) {
      adjustedY = viewportHeight - rect.height - 10;
    }

    menu.style.left = `${adjustedX}px`;
    menu.style.top = `${adjustedY}px`;
  }, [x, y]);

  return (
    <>
      {/* 遮罩层 */}
      <div
        className="fixed inset-0 z-40"
        onClick={onClose}
      />
      
      {/* 右键菜单 */}
      <div
        ref={menuRef}
        className="fixed z-50 min-w-[200px] bg-surface rounded-lg shadow-xl border border-border py-1"
        style={{ left: x, top: y }}
        onClick={(e) => e.stopPropagation()}
      >
        {actions.map((action, index) => (
          <button
            key={action.id}
            onClick={() => {
              onExecute(action.id);
              onClose();
            }}
            onMouseEnter={() => onSelect(index)}
            className={cn(
              "w-full px-4 py-2 text-left text-sm flex items-center gap-3 transition-colors",
              selectedIndex === index
                ? "bg-primary text-white"
                : "text-text-primary hover:bg-hover"
            )}
          >
            {/* 图标 */}
            {action.icon && (
              <span className="text-base flex-shrink-0">
                {action.icon.type === 'emoji' ? (
                  action.icon.data
                ) : action.icon.type === 'base64' ? (
                  <img src={action.icon.data} alt="" className="w-4 h-4 object-contain" />
                ) : action.icon.type === 'file' ? (
                  <img src={convertFileSrc(action.icon.data)} alt="" className="w-4 h-4 object-contain" />
                ) : (
                  '⚡'
                )}
              </span>
            )}
            
            {/* 名称 */}
            <span className="flex-1 font-medium">{action.name}</span>
            
            {/* 快捷键 */}
            {action.hotkey && (
              <span className={cn(
                "text-xs px-1.5 py-0.5 rounded font-mono",
                selectedIndex === index
                  ? "bg-primary/80 text-white"
                  : "bg-surface text-text-secondary"
              )}>
                {action.hotkey}
              </span>
            )}
            
            {/* 默认标记 */}
            {action.is_default && selectedIndex !== index && (
              <span className="text-xs text-text-muted">↵</span>
            )}
          </button>
        ))}
      </div>
    </>
  );
}

