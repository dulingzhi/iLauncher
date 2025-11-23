import React, { useState } from 'react';
import { X, Rocket, Keyboard, Puzzle, Sparkles } from 'lucide-react';

interface WelcomeGuideProps {
  onClose: () => void;
}

export const WelcomeGuide: React.FC<WelcomeGuideProps> = ({ onClose }) => {
  const [currentStep, setCurrentStep] = useState(0);

  const steps = [
    {
      title: '欢迎使用 iLauncher',
      icon: <Rocket className="w-16 h-16 text-primary" />,
      content: (
        <div className="space-y-4">
          <p className="text-lg">
            iLauncher 是一个快速、轻量、优雅的应用启动器
          </p>
          <ul className="space-y-2 text-left">
            <li className="flex items-start gap-2">
              <span className="text-primary">⚡</span>
              <span>极速搜索 - 毫秒级响应</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-primary">🔍</span>
              <span>智能搜索 - 支持拼音、模糊匹配</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-primary">🎨</span>
              <span>精美主题 - 多款内置主题</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-primary">📋</span>
              <span>剪贴板历史 - 永不丢失重要内容</span>
            </li>
          </ul>
        </div>
      ),
    },
    {
      title: '快捷键速查',
      icon: <Keyboard className="w-16 h-16 text-primary" />,
      content: (
        <div className="space-y-3 text-left">
          <div className="bg-surface p-3 rounded-lg">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">显示/隐藏窗口</span>
              <kbd className="px-3 py-1 bg-hover rounded text-sm">Alt + Space</kbd>
            </div>
          </div>
          <div className="bg-surface p-3 rounded-lg">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">向上/向下选择</span>
              <div className="flex gap-2">
                <kbd className="px-3 py-1 bg-hover rounded text-sm">↑ / ↓</kbd>
                <span className="text-text-muted">或</span>
                <kbd className="px-3 py-1 bg-hover rounded text-sm">Ctrl+P / Ctrl+N</kbd>
              </div>
            </div>
          </div>
          <div className="bg-surface p-3 rounded-lg">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">执行操作</span>
              <kbd className="px-3 py-1 bg-hover rounded text-sm">Enter</kbd>
            </div>
          </div>
          <div className="bg-surface p-3 rounded-lg">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">隐藏窗口</span>
              <kbd className="px-3 py-1 bg-hover rounded text-sm">Esc</kbd>
            </div>
          </div>
          <div className="bg-surface p-3 rounded-lg">
            <div className="flex justify-between items-center">
              <span className="text-text-secondary">打开设置</span>
              <kbd className="px-3 py-1 bg-hover rounded text-sm">Ctrl + ,</kbd>
            </div>
          </div>
        </div>
      ),
    },
    {
      title: '强大的插件系统',
      icon: <Puzzle className="w-16 h-16 text-primary" />,
      content: (
        <div className="space-y-4 text-left">
          <p>iLauncher 内置了 15+ 实用插件：</p>
          <div className="grid grid-cols-2 gap-3">
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">📱 应用搜索</div>
              <div className="text-sm text-text-muted">快速启动应用</div>
            </div>
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">📂 文件搜索</div>
              <div className="text-sm text-text-muted">MFT 极速搜索</div>
            </div>
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">🧮 计算器</div>
              <div className="text-sm text-text-muted">直接输入表达式</div>
            </div>
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">📋 剪贴板</div>
              <div className="text-sm text-text-muted">历史记录管理</div>
            </div>
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">🌐 网页搜索</div>
              <div className="text-sm text-text-muted">Google/Bing 搜索</div>
            </div>
            <div className="bg-surface p-3 rounded-lg">
              <div className="text-lg mb-1">⚙️ 系统命令</div>
              <div className="text-sm text-text-muted">关机/重启/锁定</div>
            </div>
          </div>
          <p className="text-sm text-text-secondary">
            💡 提示：在设置中可以查看和配置所有插件
          </p>
        </div>
      ),
    },
    {
      title: '开始使用',
      icon: <Sparkles className="w-16 h-16 text-primary" />,
      content: (
        <div className="space-y-4">
          <p className="text-lg">现在就开始体验 iLauncher 吧！</p>
          <div className="space-y-3 text-left bg-surface p-4 rounded-lg">
            <div className="flex items-start gap-3">
              <span className="text-2xl">1️⃣</span>
              <div>
                <div className="font-medium">按下 Alt + Space</div>
                <div className="text-sm text-text-muted">随时唤起搜索框</div>
              </div>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-2xl">2️⃣</span>
              <div>
                <div className="font-medium">输入任何内容</div>
                <div className="text-sm text-text-muted">应用名、文件名、计算式...</div>
              </div>
            </div>
            <div className="flex items-start gap-3">
              <span className="text-2xl">3️⃣</span>
              <div>
                <div className="font-medium">按 Enter 执行</div>
                <div className="text-sm text-text-muted">快速打开结果</div>
              </div>
            </div>
          </div>
          <p className="text-sm text-text-secondary">
            按 Ctrl + , 打开设置，探索更多功能和自定义选项
          </p>
        </div>
      ),
    },
  ];

  const handleNext = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      // 最后一步，完成引导
      handleFinish();
    }
  };

  const handlePrevious = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleFinish = () => {
    localStorage.setItem('ilauncher_welcome_shown', 'true');
    onClose();
  };

  const handleSkip = () => {
    localStorage.setItem('ilauncher_welcome_shown', 'true');
    onClose();
  };

  const currentStepData = steps[currentStep];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-60 backdrop-blur-sm">
      <div
        className="relative w-full max-w-2xl mx-4 rounded-2xl shadow-2xl overflow-hidden"
        style={{ backgroundColor: 'var(--color-surface)', maxHeight: '90vh' }}
      >
        {/* 关闭按钮 */}
        <button
          onClick={handleSkip}
          className="absolute top-4 right-4 p-2 rounded-lg hover:bg-hover transition-colors z-10"
          style={{ color: 'var(--color-text-secondary)' }}
        >
          <X className="w-5 h-5" />
        </button>

        {/* 内容区域 */}
        <div className="p-8 flex flex-col items-center text-center">
          {/* 图标 */}
          <div className="mb-6">{currentStepData.icon}</div>

          {/* 标题 */}
          <h2
            className="text-3xl font-bold mb-6"
            style={{ color: 'var(--color-text-primary)' }}
          >
            {currentStepData.title}
          </h2>

          {/* 内容 */}
          <div
            className="w-full mb-8"
            style={{ color: 'var(--color-text-secondary)' }}
          >
            {currentStepData.content}
          </div>

          {/* 进度指示器 */}
          <div className="flex gap-2 mb-6">
            {steps.map((_, index) => (
              <div
                key={index}
                className={`h-2 rounded-full transition-all ${
                  index === currentStep ? 'w-8' : 'w-2'
                }`}
                style={{
                  backgroundColor:
                    index === currentStep
                      ? 'var(--color-primary)'
                      : 'var(--color-border)',
                }}
              />
            ))}
          </div>

          {/* 按钮组 */}
          <div className="flex gap-3 w-full justify-center">
            {currentStep > 0 && (
              <button
                onClick={handlePrevious}
                className="px-6 py-2 rounded-lg font-medium transition-colors"
                style={{
                  backgroundColor: 'var(--color-hover)',
                  color: 'var(--color-text-primary)',
                }}
              >
                上一步
              </button>
            )}
            <button
              onClick={handleNext}
              className="px-8 py-2 rounded-lg font-medium transition-colors"
              style={{
                backgroundColor: 'var(--color-primary)',
                color: '#ffffff',
              }}
            >
              {currentStep === steps.length - 1 ? '开始使用' : '下一步'}
            </button>
            {currentStep < steps.length - 1 && (
              <button
                onClick={handleSkip}
                className="px-6 py-2 rounded-lg font-medium transition-colors"
                style={{
                  color: 'var(--color-text-muted)',
                }}
              >
                跳过
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
