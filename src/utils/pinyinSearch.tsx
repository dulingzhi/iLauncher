// 拼音搜索增强

import { pinyin } from 'pinyin-pro';

/**
 * 获取中文文本的拼音（全拼和首字母）
 */
export function getPinyin(text: string): { full: string; initial: string } {
  const full = pinyin(text, { toneType: 'none', type: 'array' }).join('');
  const initial = pinyin(text, { pattern: 'first', toneType: 'none' });
  
  return {
    full: full.toLowerCase(),
    initial: initial.toLowerCase(),
  };
}

/**
 * 检查查询是否匹配文本（支持拼音搜索）
 */
export function matchesSearch(text: string, query: string): boolean {
  if (!query) return true;
  if (!text) return false;
  
  const lowerQuery = query.toLowerCase();
  const lowerText = text.toLowerCase();
  
  // 1. 直接文本匹配
  if (lowerText.includes(lowerQuery)) {
    return true;
  }
  
  // 2. 拼音全拼匹配
  const { full, initial } = getPinyin(text);
  
  if (full.includes(lowerQuery)) {
    return true;
  }
  
  // 3. 拼音首字母匹配
  if (initial.includes(lowerQuery)) {
    return true;
  }
  
  return false;
}

/**
 * 计算匹配分数（用于排序）
 */
export function getMatchScore(text: string, query: string): number {
  if (!query) return 0;
  if (!text) return 0;
  
  const lowerQuery = query.toLowerCase();
  const lowerText = text.toLowerCase();
  
  // 完全匹配：最高分
  if (lowerText === lowerQuery) return 1000;
  
  // 开头匹配：高分
  if (lowerText.startsWith(lowerQuery)) return 800;
  
  // 包含匹配：中等分
  if (lowerText.includes(lowerQuery)) return 600;
  
  // 拼音匹配
  const { full, initial } = getPinyin(text);
  
  // 拼音全拼开头匹配
  if (full.startsWith(lowerQuery)) return 500;
  
  // 拼音首字母开头匹配
  if (initial.startsWith(lowerQuery)) return 400;
  
  // 拼音全拼包含
  if (full.includes(lowerQuery)) return 300;
  
  // 拼音首字母包含
  if (initial.includes(lowerQuery)) return 200;
  
  return 0;
}

/**
 * 高亮匹配的文本
 */
export function highlightMatch(text: string, query: string): React.ReactNode {
  if (!query || !text) return text;
  
  const lowerQuery = query.toLowerCase();
  const lowerText = text.toLowerCase();
  
  // 找到匹配位置
  const index = lowerText.indexOf(lowerQuery);
  
  if (index === -1) {
    // 没有直接匹配，可能是拼音匹配
    return text;
  }
  
  // 高亮匹配部分
  return (
    <>
      {text.substring(0, index)}
      <span className="bg-yellow-500/30 text-yellow-200 font-semibold">
        {text.substring(index, index + query.length)}
      </span>
      {text.substring(index + query.length)}
    </>
  );
}
