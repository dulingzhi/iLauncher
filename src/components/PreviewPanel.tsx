import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import ReactMarkdown from 'react-markdown';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import remarkGfm from 'remark-gfm';
import { FileText, Image as ImageIcon, Code, AlertCircle, File } from 'lucide-react';

interface PreviewPanelProps {
  filePath: string | null;
}

interface FilePreview {
  content: string;
  file_type: 'text' | 'image' | 'markdown' | 'json' | 'code' | 'binary';
  size: number;
  modified: string;
  extension: string;
}

export const PreviewPanel: React.FC<PreviewPanelProps> = ({ filePath }) => {
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!filePath) {
      setPreview(null);
      setError(null);
      return;
    }

    loadPreview(filePath);
  }, [filePath]);

  const loadPreview = async (path: string) => {
    setLoading(true);
    setError(null);
    
    try {
      const result = await invoke<FilePreview>('read_file_preview', { path });
      setPreview(result);
    } catch (err) {
      setError(err as string);
      setPreview(null);
    } finally {
      setLoading(false);
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  };

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const getLanguageFromExtension = (ext: string): string => {
    const map: Record<string, string> = {
      rs: 'rust',
      js: 'javascript',
      jsx: 'jsx',
      ts: 'typescript',
      tsx: 'tsx',
      py: 'python',
      go: 'go',
      java: 'java',
      cpp: 'cpp',
      c: 'c',
      cs: 'csharp',
      php: 'php',
      rb: 'ruby',
      sh: 'bash',
      yml: 'yaml',
      yaml: 'yaml',
      toml: 'toml',
      xml: 'xml',
      html: 'html',
      css: 'css',
      scss: 'scss',
      sql: 'sql',
    };
    return map[ext.toLowerCase()] || 'text';
  };

  if (!filePath) {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="text-center text-text-muted">
          <File className="w-16 h-16 mx-auto mb-4 opacity-30" />
          <p className="text-sm">Select a file to preview</p>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="text-center text-text-secondary">
          <div className="w-8 h-8 border-2 border-text-muted border-t-primary rounded-full animate-spin mx-auto mb-4" />
          <p className="text-sm">Loading preview...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="text-center text-red-400">
          <AlertCircle className="w-16 h-16 mx-auto mb-4" />
          <p className="text-sm font-medium mb-2">Cannot preview file</p>
          <p className="text-xs text-text-muted">{error}</p>
        </div>
      </div>
    );
  }

  if (!preview) return null;

  return (
    <div className="h-full flex flex-col" style={{ backgroundColor: 'var(--color-background)' }}>
      {/* 文件信息头部 */}
      <div className="flex-shrink-0 px-4 py-3 border-b border-border">
        <div className="flex items-start gap-3">
          <div className="flex-shrink-0 mt-1">
            {preview.file_type === 'image' && <ImageIcon className="w-5 h-5 text-primary" />}
            {preview.file_type === 'markdown' && <FileText className="w-5 h-5 text-primary" />}
            {(preview.file_type === 'code' || preview.file_type === 'json') && <Code className="w-5 h-5 text-primary" />}
            {preview.file_type === 'text' && <FileText className="w-5 h-5 text-primary" />}
          </div>
          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-medium text-text-primary truncate mb-1">
              {filePath.split(/[\\/]/).pop()}
            </h3>
            <div className="flex gap-4 text-xs text-text-muted">
              <span>{formatFileSize(preview.size)}</span>
              <span>{formatDate(preview.modified)}</span>
              <span className="uppercase">{preview.extension}</span>
            </div>
          </div>
        </div>
      </div>

      {/* 预览内容区 */}
      <div className="flex-1 overflow-auto">
        {/* 图片预览 */}
        {preview.file_type === 'image' && (
          <div className="p-4 flex items-center justify-center min-h-full">
            <img
              src={`file://${filePath}`}
              alt="Preview"
              className="max-w-full max-h-full object-contain rounded"
              style={{ maxHeight: 'calc(100vh - 200px)' }}
            />
          </div>
        )}

        {/* Markdown 预览 */}
        {preview.file_type === 'markdown' && (
          <div className="p-6 prose prose-invert prose-sm max-w-none">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                code({ node, inline, className, children, ...props }: any) {
                  const match = /language-(\w+)/.exec(className || '');
                  return !inline && match ? (
                    <SyntaxHighlighter
                      style={vscDarkPlus}
                      language={match[1]}
                      PreTag="div"
                      {...props}
                    >
                      {String(children).replace(/\n$/, '')}
                    </SyntaxHighlighter>
                  ) : (
                    <code className={className} {...props}>
                      {children}
                    </code>
                  );
                },
              }}
            >
              {preview.content}
            </ReactMarkdown>
          </div>
        )}

        {/* JSON 预览 */}
        {preview.file_type === 'json' && (
          <div className="p-4">
            <SyntaxHighlighter
              language="json"
              style={vscDarkPlus}
              customStyle={{
                margin: 0,
                borderRadius: '0.5rem',
                fontSize: '0.875rem',
              }}
              showLineNumbers
            >
              {preview.content}
            </SyntaxHighlighter>
          </div>
        )}

        {/* 代码预览 */}
        {preview.file_type === 'code' && (
          <div className="p-4">
            <SyntaxHighlighter
              language={getLanguageFromExtension(preview.extension)}
              style={vscDarkPlus}
              customStyle={{
                margin: 0,
                borderRadius: '0.5rem',
                fontSize: '0.875rem',
              }}
              showLineNumbers
            >
              {preview.content}
            </SyntaxHighlighter>
          </div>
        )}

        {/* 纯文本预览 */}
        {preview.file_type === 'text' && (
          <div className="p-4">
            <pre className="text-sm text-text-primary font-mono whitespace-pre-wrap break-words">
              {preview.content}
            </pre>
          </div>
        )}

        {/* 二进制文件 */}
        {preview.file_type === 'binary' && (
          <div className="p-8 text-center text-text-muted">
            <AlertCircle className="w-12 h-12 mx-auto mb-4 opacity-50" />
            <p className="text-sm">Binary file cannot be previewed</p>
            <p className="text-xs mt-2">Size: {formatFileSize(preview.size)}</p>
          </div>
        )}
      </div>
    </div>
  );
};
