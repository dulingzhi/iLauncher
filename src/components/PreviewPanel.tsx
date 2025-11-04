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
      <div className="h-full flex items-center justify-center p-8" style={{ backgroundColor: 'var(--color-background)' }}>
        <div className="text-center" style={{ color: 'var(--color-text-muted)' }}>
          <File className="w-16 h-16 mx-auto mb-4 opacity-30" />
          <p className="text-sm">Select a file to preview</p>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center p-8" style={{ backgroundColor: 'var(--color-background)' }}>
        <div className="text-center" style={{ color: 'var(--color-text-secondary)' }}>
          <div className="w-8 h-8 border-2 border-t-primary rounded-full animate-spin mx-auto mb-4" 
               style={{ borderColor: 'var(--color-text-muted)', borderTopColor: 'var(--color-primary)' }} />
          <p className="text-sm">Loading preview...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center p-8" style={{ backgroundColor: 'var(--color-background)' }}>
        <div className="text-center text-red-400">
          <AlertCircle className="w-16 h-16 mx-auto mb-4" />
          <p className="text-sm font-medium mb-2">Cannot preview file</p>
          <p className="text-xs" style={{ color: 'var(--color-text-muted)' }}>{error}</p>
        </div>
      </div>
    );
  }

  if (!preview) return null;

  return (
    <div className="h-full flex flex-col" style={{ backgroundColor: 'var(--color-background)' }}>
      {/* 文件信息头部 */}
      <div className="flex-shrink-0 px-4 py-3" style={{ borderBottom: '1px solid var(--color-border)' }}>
        <div className="flex items-start gap-3">
          <div className="flex-shrink-0 mt-1">
            {preview.file_type === 'image' && <ImageIcon className="w-5 h-5" style={{ color: 'var(--color-primary)' }} />}
            {preview.file_type === 'markdown' && <FileText className="w-5 h-5" style={{ color: 'var(--color-primary)' }} />}
            {(preview.file_type === 'code' || preview.file_type === 'json') && <Code className="w-5 h-5" style={{ color: 'var(--color-primary)' }} />}
            {preview.file_type === 'text' && <FileText className="w-5 h-5" style={{ color: 'var(--color-primary)' }} />}
          </div>
          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-medium truncate mb-1" style={{ color: 'var(--color-text-primary)' }}>
              {filePath.split(/[\\/]/).pop()}
            </h3>
            <div className="flex gap-4 text-xs" style={{ color: 'var(--color-text-muted)' }}>
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
          <div className="p-6">
            <div style={{ 
              color: '#e8e8e8',
              lineHeight: '1.75',
              fontSize: '0.95rem'
            }}>
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ children }) => <h1 style={{ fontSize: '2em', fontWeight: 'bold', marginTop: '0.67em', marginBottom: '0.67em', color: '#ffffff' }}>{children}</h1>,
                  h2: ({ children }) => <h2 style={{ fontSize: '1.5em', fontWeight: 'bold', marginTop: '0.83em', marginBottom: '0.83em', color: '#ffffff' }}>{children}</h2>,
                  h3: ({ children }) => <h3 style={{ fontSize: '1.17em', fontWeight: 'bold', marginTop: '1em', marginBottom: '1em', color: '#ffffff' }}>{children}</h3>,
                  h4: ({ children }) => <h4 style={{ fontSize: '1em', fontWeight: 'bold', marginTop: '1.33em', marginBottom: '1.33em', color: '#ffffff' }}>{children}</h4>,
                  h5: ({ children }) => <h5 style={{ fontSize: '0.83em', fontWeight: 'bold', marginTop: '1.67em', marginBottom: '1.67em', color: '#ffffff' }}>{children}</h5>,
                  h6: ({ children }) => <h6 style={{ fontSize: '0.67em', fontWeight: 'bold', marginTop: '2.33em', marginBottom: '2.33em', color: '#ffffff' }}>{children}</h6>,
                  p: ({ children }) => <p style={{ marginTop: '1em', marginBottom: '1em', color: '#e8e8e8' }}>{children}</p>,
                  a: ({ children, href }) => <a href={href} style={{ color: '#58a6ff', textDecoration: 'underline' }}>{children}</a>,
                  strong: ({ children }) => <strong style={{ fontWeight: 'bold', color: '#ffffff' }}>{children}</strong>,
                  em: ({ children }) => <em style={{ fontStyle: 'italic', color: '#e8e8e8' }}>{children}</em>,
                  ul: ({ children }) => <ul style={{ marginTop: '1em', marginBottom: '1em', paddingLeft: '2em', listStyle: 'disc', color: '#e8e8e8' }}>{children}</ul>,
                  ol: ({ children }) => <ol style={{ marginTop: '1em', marginBottom: '1em', paddingLeft: '2em', listStyle: 'decimal', color: '#e8e8e8' }}>{children}</ol>,
                  li: ({ children }) => <li style={{ marginTop: '0.5em', color: '#e8e8e8' }}>{children}</li>,
                  blockquote: ({ children }) => <blockquote style={{ borderLeft: '4px solid #58a6ff', paddingLeft: '1em', marginLeft: '0', marginTop: '1em', marginBottom: '1em', color: '#b8b8b8' }}>{children}</blockquote>,
                  code({ node, inline, className, children, ...props }: any) {
                    const match = /language-(\w+)/.exec(className || '');
                    return !inline && match ? (
                      <SyntaxHighlighter
                        style={vscDarkPlus}
                        language={match[1]}
                        PreTag="div"
                        customStyle={{
                          marginTop: '1em',
                          marginBottom: '1em',
                          borderRadius: '0.5rem',
                        }}
                        {...props}
                      >
                        {String(children).replace(/\n$/, '')}
                      </SyntaxHighlighter>
                    ) : (
                      <code style={{ 
                        backgroundColor: '#2d2d2d', 
                        color: '#e8e8e8', 
                        padding: '0.2em 0.4em', 
                        borderRadius: '0.25rem',
                        fontSize: '0.9em',
                        fontFamily: 'monospace'
                      }} {...props}>
                        {children}
                      </code>
                    );
                  },
                  table: ({ children }) => <table style={{ borderCollapse: 'collapse', width: '100%', marginTop: '1em', marginBottom: '1em' }}>{children}</table>,
                  thead: ({ children }) => <thead style={{ borderBottom: '2px solid #444' }}>{children}</thead>,
                  tbody: ({ children }) => <tbody>{children}</tbody>,
                  tr: ({ children }) => <tr style={{ borderBottom: '1px solid #333' }}>{children}</tr>,
                  th: ({ children }) => <th style={{ padding: '0.75em', textAlign: 'left', fontWeight: 'bold', color: '#ffffff' }}>{children}</th>,
                  td: ({ children }) => <td style={{ padding: '0.75em', color: '#e8e8e8' }}>{children}</td>,
                }}
              >
                {preview.content}
              </ReactMarkdown>
            </div>
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
            <pre className="text-sm font-mono whitespace-pre-wrap break-words" style={{ color: '#e8e8e8' }}>
              {preview.content}
            </pre>
          </div>
        )}

        {/* 二进制文件 */}
        {preview.file_type === 'binary' && (
          <div className="p-8 text-center" style={{ color: 'var(--color-text-muted)' }}>
            <AlertCircle className="w-12 h-12 mx-auto mb-4 opacity-50" />
            <p className="text-sm">Binary file cannot be previewed</p>
            <p className="text-xs mt-2">Size: {formatFileSize(preview.size)}</p>
          </div>
        )}
      </div>
    </div>
  );
};
