import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import { cn } from '../utils/cn';
import type { Action } from '../types';

interface ActionPanelProps {
  actions: Action[];
  selectedActionIndex: number;
  onActionSelect: (index: number) => void;
  onExecuteAction: (actionId: string) => void;
}

export function ActionPanel({
  actions,
  selectedActionIndex,
  onActionSelect,
  onExecuteAction,
}: ActionPanelProps) {
  const { t } = useTranslation();
  
  if (actions.length === 0) return null;

  return (
    <div className="border-t border-gray-200 bg-gray-50/95 backdrop-blur-sm">
      <div className="px-4 py-2">
        <div className="text-xs font-medium text-gray-500 mb-2">{t('actions.availableActions')}</div>
        <div className="flex flex-wrap gap-2">
          {actions.map((action, index) => (
            <button
              key={action.id}
              onClick={() => onExecuteAction(action.id)}
              onMouseEnter={() => onActionSelect(index)}
              className={cn(
                "px-3 py-1.5 text-sm rounded-md transition-all",
                "flex items-center gap-2",
                selectedActionIndex === index
                  ? "bg-blue-500 text-white shadow-sm"
                  : "bg-white text-gray-700 hover:bg-gray-100 border border-gray-200"
              )}
            >
              {action.icon && (
                action.icon.type === 'emoji' ? (
                  <span className="text-base">{action.icon.data}</span>
                ) : action.icon.type === 'base64' ? (
                  <img src={action.icon.data} alt="" className="w-4 h-4 object-contain" />
                ) : action.icon.type === 'file' ? (
                  <img src={convertFileSrc(action.icon.data)} alt="" className="w-4 h-4 object-contain" />
                ) : (
                  <span className="text-base">⚡</span>
                )
              )}
              <span className="font-medium">{action.name}</span>
              {action.hotkey && (
                <span className={cn(
                  "text-xs px-1.5 py-0.5 rounded",
                  selectedActionIndex === index
                    ? "bg-blue-600"
                    : "bg-gray-200 text-gray-600"
                )}>
                  {action.hotkey}
                </span>
              )}
              {action.is_default && (
                <span className={cn(
                  "text-xs px-1.5 py-0.5 rounded",
                  selectedActionIndex === index
                    ? "bg-blue-600"
                    : "bg-blue-100 text-blue-700"
                )}>
                  {t('actions.default')}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
