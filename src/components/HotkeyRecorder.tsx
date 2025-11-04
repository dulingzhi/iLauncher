import React, { useState, useRef } from 'react';
import { Keyboard } from 'lucide-react';

interface HotkeyRecorderProps {
  value: string;
  onChange: (hotkey: string) => void;
  onValidation?: (isValid: boolean, message?: string) => void;
}

export const HotkeyRecorder: React.FC<HotkeyRecorderProps> = ({ value, onChange, onValidation }) => {
  const [isRecording, setIsRecording] = useState(false);
  const [pressedKeys, setPressedKeys] = useState<Set<string>>(new Set());
  const inputRef = useRef<HTMLInputElement>(null);

  const keyMap: Record<string, string> = {
    'Control': 'Ctrl',
    'Meta': 'Super',
    'ArrowUp': 'Up',
    'ArrowDown': 'Down',
    'ArrowLeft': 'Left',
    'ArrowRight': 'Right',
    ' ': 'Space',
  };

  const modifierKeys = new Set(['Control', 'Alt', 'Shift', 'Meta']);

  const normalizeKey = (key: string): string => {
    return keyMap[key] || key;
  };

  const formatHotkey = (keys: Set<string>): string => {
    const keyArray = Array.from(keys);
    const modifiers: string[] = [];
    let mainKey = '';

    keyArray.forEach(key => {
      const normalized = normalizeKey(key);
      if (modifierKeys.has(key)) {
        modifiers.push(normalized);
      } else {
        mainKey = normalized;
      }
    });

    // 排序修饰键：Ctrl -> Alt -> Shift -> Super
    const order = { 'Ctrl': 1, 'Alt': 2, 'Shift': 3, 'Super': 4 };
    modifiers.sort((a, b) => (order[a as keyof typeof order] || 9) - (order[b as keyof typeof order] || 9));

    if (mainKey) {
      return [...modifiers, mainKey].join('+');
    }
    return modifiers.join('+');
  };

  const validateHotkey = (hotkey: string): { isValid: boolean; message?: string } => {
    if (!hotkey) {
      return { isValid: false, message: 'Hotkey cannot be empty' };
    }

    const parts = hotkey.split('+');
    const hasModifier = parts.some(p => ['Ctrl', 'Alt', 'Shift', 'Super'].includes(p));
    const hasMainKey = parts.some(p => !['Ctrl', 'Alt', 'Shift', 'Super'].includes(p));

    if (!hasModifier) {
      return { isValid: false, message: 'At least one modifier key (Ctrl/Alt/Shift/Super) is required' };
    }

    if (!hasMainKey) {
      return { isValid: false, message: 'A main key is required' };
    }

    // 检查常见冲突
    const conflictingHotkeys = [
      'Ctrl+C', 'Ctrl+V', 'Ctrl+X', 'Ctrl+Z', 'Ctrl+Y',
      'Alt+F4', 'Ctrl+Alt+Delete'
    ];

    if (conflictingHotkeys.includes(hotkey)) {
      return { isValid: false, message: `"${hotkey}" is a system hotkey and cannot be used` };
    }

    return { isValid: true };
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isRecording) return;

    e.preventDefault();
    e.stopPropagation();

    const newKeys = new Set(pressedKeys);
    
    // 忽略单独的修饰键
    if (!modifierKeys.has(e.key) || pressedKeys.size > 0) {
      newKeys.add(e.key);
    }

    setPressedKeys(newKeys);

    // 如果按下了非修饰键，结束录制
    if (!modifierKeys.has(e.key)) {
      const hotkey = formatHotkey(newKeys);
      const validation = validateHotkey(hotkey);
      
      if (validation.isValid) {
        onChange(hotkey);
        onValidation?.(true);
        setIsRecording(false);
        setPressedKeys(new Set());
        inputRef.current?.blur();
      } else {
        onValidation?.(false, validation.message);
      }
    }
  };

  const handleKeyUp = (e: React.KeyboardEvent) => {
    if (!isRecording) return;

    e.preventDefault();
    e.stopPropagation();

    // 如果只剩下修饰键被释放，清空
    if (modifierKeys.has(e.key)) {
      const newKeys = new Set(pressedKeys);
      newKeys.delete(e.key);
      
      if (newKeys.size === 0) {
        setPressedKeys(new Set());
      }
    }
  };

  const handleFocus = () => {
    setIsRecording(true);
    setPressedKeys(new Set());
  };

  const handleBlur = () => {
    setIsRecording(false);
    setPressedKeys(new Set());
  };

  const displayValue = isRecording 
    ? (pressedKeys.size > 0 ? formatHotkey(pressedKeys) : 'Press keys...')
    : value || 'Click to set hotkey';

  return (
    <div className="relative">
      <div className="relative flex items-center">
        <div className="absolute left-3 pointer-events-none">
          <Keyboard className="w-4 h-4 text-gray-500" />
        </div>
        <input
          ref={inputRef}
          type="text"
          value={displayValue}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          onFocus={handleFocus}
          onBlur={handleBlur}
          readOnly
          className={`w-full pl-10 pr-3 py-2 bg-[#2d2d30] border rounded text-sm focus:outline-none transition-colors ${
            isRecording
              ? 'border-[#007acc] text-white'
              : 'border-[#3e3e42] text-gray-300 hover:border-[#555]'
          }`}
          placeholder="Click to record hotkey"
        />
      </div>
      {isRecording && (
        <p className="mt-1.5 text-xs text-[#007acc]">
          Press a key combination (e.g., Ctrl+Alt+Space)
        </p>
      )}
    </div>
  );
};
