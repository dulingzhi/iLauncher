// MFT 配置测试页面
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface AppConfig {
  general: {
    hotkey: string;
    search_delay: number;
    max_results: number;
    language: string;
    clear_on_hide: boolean;
  };
  appearance: {
    theme: string;
    language: string;
    window_width: number;
    window_height: number;
    font_size: number;
    transparency: number;
    show_preview: boolean;
  };
  plugins: {
    enabled_plugins: string[];
    disabled_plugins: string[];
  };
  advanced: {
    start_on_boot: boolean;
    show_tray_icon: boolean;
    enable_analytics: boolean;
    cache_enabled: boolean;
  };
}

interface MftStatus {
  is_scanning: boolean;
  is_ready: boolean;
  database_exists: boolean;
  drives: { letter: string; database_size_mb: number; estimated_files: number }[];
  total_files: number;
  message: string;
}

export function MftSettings() {
  const [pluginConfig, setPluginConfig] = useState<any>(null);
  const [mftStatus, setMftStatus] = useState<MftStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');

  // 加载配置和状态
  useEffect(() => {
    loadConfig();
    loadMftStatus();
    
    // 每 3 秒轮询 MFT 状态
    const interval = setInterval(() => {
      loadMftStatus();
    }, 3000);
    
    return () => clearInterval(interval);
  }, []);

  const loadConfig = async () => {
    try {
      const cfg = await invoke<any>('get_plugin_config', { pluginId: 'file_search' });
      setPluginConfig(cfg);
    } catch (error) {
      setMessage(`加载配置失败: ${error}`);
    }
  };
  
  const loadMftStatus = async () => {
    try {
      const status = await invoke<MftStatus>('get_mft_status');
      setMftStatus(status);
    } catch (error) {
      console.error('Failed to load MFT status:', error);
    }
  };

  const toggleMft = async (enabled: boolean) => {
    setLoading(true);
    setMessage('');

    try {
      await invoke('toggle_mft', { enabled });
      setMessage(`MFT ${enabled ? '已启用' : '已禁用'}，${enabled ? 'UAC 提示可能会弹出' : '服务已停止'}`);
      
      // 重新加载配置和状态
      await loadConfig();
      await loadMftStatus();
    } catch (error) {
      setMessage(`操作失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  if (!pluginConfig) {
    return <div className="p-4">加载中...</div>;
  }

  const useMft = pluginConfig.use_mft ?? true;

  return (
    <div className="p-6 max-w-2xl mx-auto">
      <h1 className="text-2xl font-bold mb-4">MFT 文件搜索设置</h1>

      <div className="bg-white rounded-lg shadow p-6 mb-4">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-lg font-semibold">MFT 快速扫描</h2>
            <p className="text-sm text-gray-600 mt-1">
              启用后可以毫秒级搜索 450 万+ 文件（需要管理员权限）
            </p>
          </div>
          
          <label className="relative inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              className="sr-only peer"
              checked={useMft}
              onChange={(e) => toggleMft(e.target.checked)}
              disabled={loading}
            />
            <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
          </label>
        </div>

        {message && (
          <div className={`p-3 rounded ${
            message.includes('失败') ? 'bg-red-100 text-red-700' : 'bg-green-100 text-green-700'
          }`}>
            {message}
          </div>
        )}
      </div>

      {/* MFT 状态卡片 */}
      {mftStatus && (
        <div className="bg-gray-50 rounded-lg p-6 mb-4">
          <h3 className="font-semibold mb-3 flex items-center">
            <span className={`inline-block w-3 h-3 rounded-full mr-2 ${
              mftStatus.is_ready ? 'bg-green-500' : 
              mftStatus.is_scanning ? 'bg-yellow-500 animate-pulse' : 'bg-red-500'
            }`}></span>
            扫描状态
          </h3>
          
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-600">状态:</span>
              <span className={`font-medium ${
                mftStatus.is_ready ? 'text-green-600' : 'text-yellow-600'
              }`}>
                {mftStatus.message}
              </span>
            </div>
            
            {mftStatus.drives.length > 0 && (
              <>
                <div className="flex justify-between">
                  <span className="text-gray-600">已索引盘符:</span>
                  <span className="font-medium">
                    {mftStatus.drives.map(d => d.letter).join(', ')}
                  </span>
                </div>
                
                <div className="flex justify-between">
                  <span className="text-gray-600">文件总数:</span>
                  <span className="font-medium">
                    ~{mftStatus.total_files.toLocaleString()}
                  </span>
                </div>
                
                <div className="mt-3 pt-3 border-t">
                  <p className="text-xs text-gray-500 mb-2">盘符详情:</p>
                  {mftStatus.drives.map(drive => (
                    <div key={drive.letter} className="flex justify-between text-xs mb-1">
                      <span>{drive.letter}:\</span>
                      <span className="text-gray-500">
                        {drive.database_size_mb} MB, ~{drive.estimated_files.toLocaleString()} 文件
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}
            
            {mftStatus.is_scanning && (
              <div className="mt-3 p-2 bg-yellow-50 rounded text-xs text-yellow-700">
                ⏳ 正在扫描中，请稍候...（首次扫描可能需要几秒到几分钟）
              </div>
            )}
          </div>
        </div>
      )}

      <div className="bg-gray-50 rounded-lg p-6">
        <h3 className="font-semibold mb-3">性能对比</h3>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b">
              <th className="text-left py-2">指标</th>
              <th className="text-left py-2">MFT 模式</th>
              <th className="text-left py-2">BFS 模式</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b">
              <td className="py-2">扫描 450 万文件</td>
              <td className="py-2 text-green-600 font-semibold">9 秒</td>
              <td className="py-2 text-orange-600">5-10 分钟</td>
            </tr>
            <tr className="border-b">
              <td className="py-2">搜索延迟</td>
              <td className="py-2 text-green-600 font-semibold">&lt;50ms</td>
              <td className="py-2 text-orange-600">100-500ms</td>
            </tr>
            <tr className="border-b">
              <td className="py-2">实时更新</td>
              <td className="py-2 text-green-600 font-semibold">是</td>
              <td className="py-2 text-red-600">否</td>
            </tr>
            <tr>
              <td className="py-2">权限要求</td>
              <td className="py-2 text-orange-600">管理员</td>
              <td className="py-2 text-green-600 font-semibold">普通用户</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="mt-6 text-sm text-gray-600">
        <p>💡 提示：</p>
        <ul className="list-disc ml-5 mt-2 space-y-1">
          <li>首次启用需要以管理员权限运行</li>
          <li>MFT Service 会在后台自动扫描所有 NTFS 盘符</li>
          <li>数据库保存在：%TEMP%\ilauncher_mft\*.db</li>
          <li>可以随时切换模式，搜索功能不会中断</li>
        </ul>
      </div>
    </div>
  );
}
