import React from 'react';
import { X, Keyboard } from 'lucide-react';

interface HotkeyGuideProps {
  onClose: () => void;
}

interface HotkeyItem {
  keys: string[];
  description: string;
  category: string;
}

export const HotkeyGuide: React.FC<HotkeyGuideProps> = ({ onClose }) => {
  const hotkeys: HotkeyItem[] = [
    // 基础导航
    { keys: ['↑', '↓'], description: '上下选择结果', category: '导航' },
    { keys: ['Enter'], description: '执行默认操作', category: '导航' },
    { keys: ['Tab'], description: '切换操作面板', category: '导航' },
    { keys: ['Esc'], description: '隐藏窗口', category: '导航' },
    
    // 操作面板
    { keys: ['Ctrl', '1-9'], description: '快速执行操作', category: '操作' },
    { keys: ['Ctrl', 'C'], description: '复制内容', category: '操作' },
    { keys: ['Ctrl', 'V'], description: '粘贴内容', category: '操作' },
    
    // 视图切换
    { keys: ['Ctrl', ','], description: '打开设置', category: '视图' },
    { keys: ['Ctrl', 'H'], description: '剪贴板历史', category: '视图' },
    
    // 特殊功能
    { keys: ['?'], description: '显示此帮助', category: '帮助' },
    { keys: ['F1'], description: '显示此帮助', category: '帮助' },
  ];
  
  const categories = Array.from(new Set(hotkeys.map(h => h.category)));
  
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="w-[600px] max-h-[80vh] bg-[#1e1e1e] rounded-lg shadow-2xl overflow-hidden border border-[#3e3e42]">
        {/* 标题栏 */}
        <div className="flex items-center justify-between px-6 py-4 bg-[#252526] border-b border-[#3e3e42]">
          <div className="flex items-center gap-3">
            <Keyboard className="w-5 h-5 text-[#007acc]" />
            <h2 className="text-lg font-semibold text-gray-100">快捷键指南</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1 hover:bg-[#3e3e42] rounded transition-colors"
            aria-label="关闭"
          >
            <X className="w-5 h-5 text-gray-400" />
          </button>
        </div>
        
        {/* 内容区 */}
        <div className="p-6 overflow-y-auto max-h-[calc(80vh-72px)]">
          <div className="space-y-6">
            {categories.map(category => (
              <div key={category}>
                <h3 className="text-sm font-semibold text-gray-400 mb-3">{category}</h3>
                <div className="space-y-2">
                  {hotkeys
                    .filter(h => h.category === category)
                    .map((hotkey, index) => (
                      <div
                        key={index}
                        className="flex items-center justify-between py-2 px-3 bg-[#2d2d30] rounded border border-[#3e3e42] hover:bg-[#323234] transition-colors"
                      >
                        <span className="text-sm text-gray-300">{hotkey.description}</span>
                        <div className="flex items-center gap-1">
                          {hotkey.keys.map((key, kidx) => (
                            <React.Fragment key={kidx}>
                              {kidx > 0 && (
                                <span className="text-xs text-gray-500 mx-1">+</span>
                              )}
                              <kbd className="px-2 py-1 text-xs font-mono bg-[#1e1e1e] text-gray-200 border border-[#555] rounded shadow-sm min-w-[28px] text-center">
                                {key}
                              </kbd>
                            </React.Fragment>
                          ))}
                        </div>
                      </div>
                    ))}
                </div>
              </div>
            ))}
          </div>
          
          {/* 底部提示 */}
          <div className="mt-6 pt-4 border-t border-[#3e3e42]">
            <p className="text-xs text-gray-500 text-center">
              💡 提示: 按 <kbd className="px-1.5 py-0.5 text-xs bg-[#2d2d30] border border-[#555] rounded">Esc</kbd> 或点击外部区域关闭此窗口
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
