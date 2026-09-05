; iLauncher GPUI 版安装脚本（NSIS）
;
; 消费契约（gpui-app/src/updater.rs）：
;   - 产物名：iLauncher_${VERSION}_x64-setup.exe
;   - 更新器以 /SILENT 启动（被动安装：显示进度、无交互），这里映射为 NSIS silent
;   - 安装目录与注册表卸载项对齐旧 Tauri 安装包（%LOCALAPPDATA%\Programs\iLauncher），
;     保证 gpui 更新器能原地覆盖旧版安装
;
; 构建（scripts/pack-gpui.ps1 调用）：
;   makensis /DVERSION=1.2.3 /DEXE=..\target\release\ilauncher-gpui.exe /DOUTDIR=... iLauncher.nsi

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef EXE
  !define EXE "..\target\release\ilauncher-gpui.exe"
!endif
!ifndef OUTDIR
  !define OUTDIR "..\target\release\bundle\nsis"
!endif

!define APP_NAME "iLauncher"
!define APP_EXE "iLauncher.exe"
!define INSTALL_DIR "$LOCALAPPDATA\Programs\iLauncher"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\iLauncher"

Unicode true
Name "${APP_NAME}"
OutFile "${OUTDIR}\iLauncher_${VERSION}_x64-setup.exe"
InstallDir "${INSTALL_DIR}"
RequestExecutionLevel user

; /SILENT、/VERYSILENT（更新器协议）映射为 NSIS silent，/S 原生支持
; nsExec 是内置插件（nsExec::Exec 直接可用），无需 include
!include FileFunc.nsh

Function .onInit
  ${GetOptions} "$CMDLINE" "/SILENT" $R0
  IfErrors +2 0
    SetSilent silent
  ${GetOptions} "$CMDLINE" "/VERYSILENT" $R0
  IfErrors +2 0
    SetSilent silent

  ; 覆盖安装时杀掉可能残留的旧进程，释放安装目录文件锁
  nsExec::Exec 'taskkill /F /IM ${APP_EXE}'
  nsExec::Exec 'taskkill /F /IM ilauncher-gpui.exe'
  nsExec::Exec 'taskkill /F /IM iLauncher.exe'
  Pop $R0
FunctionEnd

Page directory
Page instfiles

Section "Install"
  SetOutPath "$INSTDIR"
  SetOverwrite on
  File /oname=${APP_EXE} "${EXE}"

  ; 卸载器
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; 卸载注册表项（HKCU，与安装权限一致）
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "dulingzhi"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'

  ; 开始菜单快捷方式（静默安装也创建，与旧 Tauri 行为一致）
  CreateShortcut "$SMPROGRAMS\iLauncher.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\iLauncher.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
