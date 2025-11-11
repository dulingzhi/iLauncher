import React, { useEffect, useState } from 'react';
import { check, type DownloadEvent } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useToast } from '../hooks/useToast';

export const UpdateChecker: React.FC = () => {
  const { showToast } = useToast();
  const [checking, setChecking] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);

  const checkForUpdates = async (silent = false) => {
    if (checking || updating) return;

    setChecking(true);
    try {
      const update = await check();
      
      if (update?.available) {
        const shouldUpdate = confirm(
          `New version ${update.version} is available!\n\n` +
          `Current version: ${update.currentVersion}\n` +
          `Release notes:\n${update.body || 'No release notes'}\n\n` +
          `Do you want to download and install it now?`
        );

        if (shouldUpdate) {
          setUpdating(true);
          showToast('Downloading update...', 'info');

          // 下载并安装更新
          await update.downloadAndInstall((event: DownloadEvent) => {
            switch (event.event) {
              case 'Started':
                setDownloadProgress(0);
                break;
              case 'Progress':
                if (event.data?.chunkLength) {
                  setDownloadProgress(event.data.chunkLength);
                  showToast(`Downloading: ${Math.round(event.data.chunkLength / 1024)}KB`, 'info');
                }
                break;
              case 'Finished':
                setDownloadProgress(100);
                showToast('Update downloaded! Restarting...', 'success');
                break;
            }
          });

          // 重启应用
          await relaunch();
        }
      } else {
        if (!silent) {
          showToast('You are using the latest version!', 'success');
        }
      }
    } catch (error) {
      console.error('Update check failed:', error);
      if (!silent) {
        showToast(`Update check failed: ${error}`, 'error');
      }
    } finally {
      setChecking(false);
      setUpdating(false);
    }
  };

  // 启动时自动检查更新（静默）
  useEffect(() => {
    // 延迟5秒后检查，避免影响启动速度
    const timer = setTimeout(() => {
      checkForUpdates(true);
    }, 5000);

    return () => clearTimeout(timer);
  }, []);

  return (
    <button
      onClick={() => checkForUpdates(false)}
      disabled={checking || updating}
      className="px-4 py-2 text-sm bg-[#0e639c] hover:bg-[#1177bb] text-white rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    >
      {checking ? 'Checking...' : updating ? `Updating... ${downloadProgress}%` : 'Check for Updates'}
    </button>
  );
};
