import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Bot, Send, Loader2, Settings, Trash2, Plus } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface ChatMessage {
  role: string; // 'user' | 'assistant' | 'system'
  content: string;
}

interface Conversation {
  id: string;
  title: string;
  messages: ChatMessage[];
  timestamp: number;
}

interface AIConfig {
  provider: string;
  api_key: string;
  model: string;
  base_url?: string;
  temperature: number;
  max_tokens: number;
}

interface AIChatProps {
  onClose?: () => void;
}

const AIChat: React.FC<AIChatProps> = ({ onClose }) => {
  const { t } = useTranslation();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [currentConvId, setCurrentConvId] = useState<string | null>(null);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [config, setConfig] = useState<AIConfig | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadConfig();
    loadConversations();
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [conversations, currentConvId]);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  const loadConfig = async () => {
    try {
      const cfg = await invoke<AIConfig>('get_ai_config');
      setConfig(cfg);
    } catch (error) {
      console.error('Failed to load AI config:', error);
      // 默认配置
      setConfig({
        provider: 'openai',
        api_key: '',
        model: 'gpt-3.5-turbo',
        temperature: 0.7,
        max_tokens: 2000,
      });
    }
  };

  const loadConversations = async () => {
    try {
      const convs = await invoke<Conversation[]>('get_ai_conversations');
      setConversations(convs);
      if (convs.length > 0 && !currentConvId) {
        setCurrentConvId(convs[0].id);
      }
    } catch (error) {
      console.error('Failed to load conversations:', error);
    }
  };

  const createNewConversation = async () => {
    try {
      const id = await invoke<string>('create_ai_conversation', {
        title: 'New Chat',
      });
      await loadConversations();
      setCurrentConvId(id);
    } catch (error) {
      console.error('Failed to create conversation:', error);
    }
  };

  const sendMessage = async () => {
    if (!input.trim() || !config) return;

    if (!config.api_key) {
      alert('Please configure your API key first');
      setShowSettings(true);
      return;
    }

    const userMessage = input;
    setInput('');
    setLoading(true);

    try {
      // 创建新对话（如果需要）
      if (!currentConvId) {
        await createNewConversation();
      }

      const response = await invoke<string>('send_ai_message', {
        message: userMessage,
      });

      // 重新加载对话以获取最新消息
      await loadConversations();
    } catch (error) {
      console.error('Failed to send message:', error);
      alert(`Error: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const deleteConversation = async (id: string) => {
    if (!window.confirm('Delete this conversation?')) return;

    try {
      await invoke('delete_ai_conversation', { convId: id });
      await loadConversations();
      if (currentConvId === id) {
        setCurrentConvId(null);
      }
    } catch (error) {
      console.error('Failed to delete conversation:', error);
    }
  };

  const saveConfig = async () => {
    if (!config) return;

    try {
      await invoke('save_ai_config', { config });
      setShowSettings(false);
      alert('Configuration saved');
    } catch (error) {
      console.error('Failed to save config:', error);
      alert(`Error: ${error}`);
    }
  };

  const currentConversation = conversations.find((c) => c.id === currentConvId);

  return (
    <div
      className="flex h-full"
      style={{ backgroundColor: 'var(--color-background)' }}
    >
      {/* 对话列表侧边栏 */}
      <div
        className="w-64 border-r flex flex-col"
        style={{ borderColor: 'var(--color-border)' }}
      >
        <div
          className="p-4 border-b flex items-center justify-between"
          style={{ borderColor: 'var(--color-border)' }}
        >
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Bot style={{ color: 'var(--color-accent)' }} className="w-5 h-5" />
            <span style={{ color: 'var(--color-text)' }}>AI Chat</span>
          </h2>
          <button
            onClick={createNewConversation}
            className="p-2 rounded hover:bg-opacity-10"
            style={{ color: 'var(--color-accent)' }}
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          {conversations.map((conv) => (
            <div
              key={conv.id}
              onClick={() => setCurrentConvId(conv.id)}
              className="group p-3 rounded-lg cursor-pointer mb-2 flex justify-between items-center"
              style={{
                backgroundColor:
                  currentConvId === conv.id
                    ? 'var(--color-hover)'
                    : 'transparent',
                color: 'var(--color-text)',
              }}
            >
              <div className="flex-1 min-w-0">
                <div className="font-medium truncate">{conv.title}</div>
                <div
                  className="text-xs truncate"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  {conv.messages.length} messages
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  deleteConversation(conv.id);
                }}
                className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-opacity-20"
                style={{ color: 'var(--color-danger, #e53e3e)' }}
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          ))}
        </div>

        <button
          onClick={() => setShowSettings(!showSettings)}
          className="p-4 border-t flex items-center gap-2"
          style={{
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <Settings className="w-4 h-4" />
          <span>Settings</span>
        </button>
      </div>

      {/* 主聊天区域 */}
      <div className="flex-1 flex flex-col">
        {showSettings ? (
          /* 设置面板 */
          <div className="p-6 overflow-y-auto">
            <h3
              className="text-xl font-bold mb-4"
              style={{ color: 'var(--color-text)' }}
            >
              AI Settings
            </h3>

            <div className="space-y-4 max-w-2xl">
              <div>
                <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>Provider</label>
                <select
                  value={config?.provider || 'openai'}
                  onChange={(e) =>
                    setConfig({ ...config!, provider: e.target.value })
                  }
                  className="w-full p-2 rounded border"
                  style={{
                    backgroundColor: 'var(--color-surface)',
                    color: 'var(--color-text)',
                    borderColor: 'var(--color-border)',
                  }}
                >
                  <option value="openai">OpenAI (GPT-3.5/4)</option>
                  <option value="anthropic">Anthropic (Claude)</option>
                  <option value="github">GitHub Copilot</option>
                  <option value="deepseek">DeepSeek</option>
                  <option value="gemini">Google Gemini</option>
                  <option value="ollama">Ollama (Local)</option>
                  <option value="custom">Custom Endpoint</option>
                </select>
                <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                  {config?.provider === 'github' && 'GitHub Copilot requires GitHub account'}
                  {config?.provider === 'ollama' && 'Make sure Ollama is running locally'}
                  {config?.provider === 'custom' && 'Provide your custom API base URL below'}
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>
                  API Key / Token
                </label>
                <input
                  type="password"
                  value={config?.api_key || ''}
                  onChange={(e) =>
                    setConfig({ ...config!, api_key: e.target.value })
                  }
                  placeholder={
                    config?.provider === 'github' ? 'ghu_...' :
                    config?.provider === 'anthropic' ? 'sk-ant-...' :
                    config?.provider === 'ollama' ? 'Not required for local' :
                    'sk-...'
                  }
                  disabled={config?.provider === 'ollama'}
                  className="w-full p-2 rounded border font-mono text-sm"
                  style={{
                    backgroundColor: 'var(--color-surface)',
                    color: 'var(--color-text)',
                    borderColor: 'var(--color-border)',
                    opacity: config?.provider === 'ollama' ? 0.5 : 1,
                  }}
                />
                <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                  {config?.provider === 'openai' && 'Get your API key from platform.openai.com'}
                  {config?.provider === 'anthropic' && 'Get your API key from console.anthropic.com'}
                  {config?.provider === 'github' && 'Generate token at github.com/settings/tokens (needs copilot scope)'}
                  {config?.provider === 'deepseek' && 'Get your API key from platform.deepseek.com'}
                  {config?.provider === 'gemini' && 'Get your API key from makersuite.google.com'}
                  {config?.provider === 'ollama' && 'No API key needed for local Ollama'}
                </p>
              </div>

              {(config?.provider === 'custom' || config?.provider === 'ollama' || config?.provider === 'github') && (
                <div>
                  <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>
                    Base URL
                  </label>
                  <input
                    type="text"
                    value={config?.base_url || ''}
                    onChange={(e) =>
                      setConfig({ ...config!, base_url: e.target.value })
                    }
                    placeholder={
                      config?.provider === 'ollama' ? 'http://localhost:11434' :
                      config?.provider === 'github' ? 'https://api.githubcopilot.com' :
                      'https://api.your-endpoint.com'
                    }
                    className="w-full p-2 rounded border font-mono text-sm"
                    style={{
                      backgroundColor: 'var(--color-surface)',
                      color: 'var(--color-text)',
                      borderColor: 'var(--color-border)',
                    }}
                  />
                </div>
              )}

              <div>
                <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>Model</label>
                <input
                  type="text"
                  value={config?.model || ''}
                  onChange={(e) =>
                    setConfig({ ...config!, model: e.target.value })
                  }
                  placeholder={
                    config?.provider === 'openai' ? 'gpt-3.5-turbo or gpt-4' :
                    config?.provider === 'anthropic' ? 'claude-3-opus-20240229' :
                    config?.provider === 'github' ? 'gpt-4 or copilot-chat' :
                    config?.provider === 'deepseek' ? 'deepseek-chat' :
                    config?.provider === 'gemini' ? 'gemini-pro' :
                    config?.provider === 'ollama' ? 'llama2 or mistral' :
                    'model-name'
                  }
                  className="w-full p-2 rounded border font-mono text-sm"
                  style={{
                    backgroundColor: 'var(--color-surface)',
                    color: 'var(--color-text)',
                    borderColor: 'var(--color-border)',
                  }}
                />
                <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                  Available models depend on your provider and subscription
                </p>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>
                    Temperature ({config?.temperature || 0.7})
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    value={config?.temperature || 0.7}
                    onChange={(e) =>
                      setConfig({ ...config!, temperature: parseFloat(e.target.value) })
                    }
                    className="w-full"
                  />
                  <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                    Higher = more creative, Lower = more focused
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1" style={{ color: 'var(--color-text)' }}>
                    Max Tokens
                  </label>
                  <input
                    type="number"
                    min="100"
                    max="8000"
                    step="100"
                    value={config?.max_tokens || 2000}
                    onChange={(e) =>
                      setConfig({ ...config!, max_tokens: parseInt(e.target.value) })
                    }
                    className="w-full p-2 rounded border"
                    style={{
                      backgroundColor: 'var(--color-surface)',
                      color: 'var(--color-text)',
                      borderColor: 'var(--color-border)',
                    }}
                  />
                  <p className="text-xs mt-1" style={{ color: 'var(--color-text-secondary)' }}>
                    Maximum response length
                  </p>
                </div>
              </div>

              <div className="border-t pt-4" style={{ borderColor: 'var(--color-border)' }}>
                <div className="flex gap-2">
                  <button
                    onClick={saveConfig}
                    className="px-4 py-2 rounded font-medium"
                    style={{
                      backgroundColor: 'var(--color-accent)',
                      color: 'white',
                    }}
                  >
                    Save Configuration
                  </button>
                  <button
                    onClick={() => setShowSettings(false)}
                    className="px-4 py-2 rounded"
                    style={{
                      backgroundColor: 'var(--color-surface)',
                      color: 'var(--color-text)',
                      border: '1px solid var(--color-border)',
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          </div>
        ) : (
          <>
            {/* 消息列表 */}
            <div className="flex-1 overflow-y-auto p-4">
              {currentConversation && currentConversation.messages.length > 0 ? (
                <div className="space-y-4 max-w-3xl mx-auto">
                  {currentConversation.messages.map((msg, idx) => (
                    <div
                      key={idx}
                      className={`flex ${
                        msg.role === 'user' ? 'justify-end' : 'justify-start'
                      }`}
                    >
                      <div
                        className="max-w-[80%] p-3 rounded-lg"
                        style={{
                          backgroundColor:
                            msg.role === 'user'
                              ? 'var(--color-accent)'
                              : 'var(--color-surface)',
                          color:
                            msg.role === 'user' ? 'white' : 'var(--color-text)',
                        }}
                      >
                        <div className="whitespace-pre-wrap">{msg.content}</div>
                      </div>
                    </div>
                  ))}
                  <div ref={messagesEndRef} />
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-full">
                  <Bot
                    className="w-16 h-16 mb-4 opacity-50"
                    style={{ color: 'var(--color-accent)' }}
                  />
                  <p style={{ color: 'var(--color-text-secondary)' }}>
                    Start a conversation with AI
                  </p>
                </div>
              )}
            </div>

            {/* 输入框 */}
            <div
              className="p-4 border-t"
              style={{ borderColor: 'var(--color-border)' }}
            >
              <div className="flex gap-2 max-w-3xl mx-auto">
                <input
                  type="text"
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyPress={(e) => e.key === 'Enter' && !loading && sendMessage()}
                  placeholder="Type your message..."
                  disabled={loading}
                  className="flex-1 p-3 rounded-lg border"
                  style={{
                    backgroundColor: 'var(--color-surface)',
                    color: 'var(--color-text)',
                    borderColor: 'var(--color-border)',
                  }}
                />
                <button
                  onClick={sendMessage}
                  disabled={loading || !input.trim()}
                  className="p-3 rounded-lg"
                  style={{
                    backgroundColor: 'var(--color-accent)',
                    color: 'white',
                    opacity: loading || !input.trim() ? 0.5 : 1,
                  }}
                >
                  {loading ? (
                    <Loader2 className="w-5 h-5 animate-spin" />
                  ) : (
                    <Send className="w-5 h-5" />
                  )}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default AIChat;
