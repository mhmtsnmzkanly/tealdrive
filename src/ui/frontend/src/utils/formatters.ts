export function formatBytes(bytes: number, decimals = 1): string {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

import { t, locale } from '../i18n/index.ts';

export function formatRelativeDate(isoDateString: string): string {
  try {
    const date = new Date(isoDateString);
    const now = new Date();
    const diffSec = Math.floor((now.getTime() - date.getTime()) / 1000);

    if (diffSec < 60) return t('Just now');
    if (diffSec < 3600) return t('{n}m ago', { n: Math.floor(diffSec / 60) });
    if (diffSec < 86400) return t('{n}h ago', { n: Math.floor(diffSec / 3600) });
    if (diffSec < 604800) return t('{n}d ago', { n: Math.floor(diffSec / 86400) });

    return date.toLocaleDateString(locale(), {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    });
  } catch {
    return isoDateString;
  }
}

export function getFileCategory(name: string, isDirectory: boolean, mimeType?: string): 'folder' | 'document' | 'image' | 'media' | 'code' | 'archive' | 'other' {
  if (isDirectory) return 'folder';
  const ext = name.split('.').pop()?.toLowerCase() || '';

  if (['png', 'jpg', 'jpeg', 'svg', 'webp', 'gif', 'bmp', 'ico'].includes(ext)) return 'image';
  if (['mp3', 'wav', 'ogg', 'mp4', 'mov', 'webm', 'mkv', 'flac', 'aac'].includes(ext)) return 'media';
  if (['rs', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'json', 'toml', 'sh', 'bash', 'zsh', 'py', 'sql', 'html', 'htm', 'css', 'yaml', 'yml', 'xml', 'c', 'cpp', 'h', 'hpp', 'go', 'java', 'php', 'rb', 'lua'].includes(ext)) return 'code';
  if (['pdf', 'doc', 'docx', 'txt', 'md', 'rtf', 'csv', 'xlsx', 'log', 'env', 'conf', 'ini', 'diff', 'patch'].includes(ext)) return 'document';
  if (['zip', 'tar', 'gz', 'bz2', '7z', 'rar', 'xz', 'tgz'].includes(ext)) return 'archive';

  if (mimeType?.startsWith('image/')) return 'image';
  if (mimeType?.startsWith('audio/') || mimeType?.startsWith('video/')) return 'media';
  if (mimeType?.startsWith('text/')) return 'document';
  if (mimeType === 'application/json' || mimeType === 'application/javascript' || mimeType === 'application/xml') return 'code';

  return 'other';
}

export function isTextFile(name: string, mimeType?: string): boolean {
  const ext = name.split('.').pop()?.toLowerCase() || '';
  const textExts = [
    'txt', 'md', 'html', 'htm', 'css', 'js', 'mjs', 'cjs', 'jsx', 'ts', 'tsx', 'json',
    'xml', 'yaml', 'yml', 'toml', 'ini', 'conf', 'env', 'sh', 'bash', 'zsh',
    'rs', 'py', 'c', 'cpp', 'h', 'hpp', 'go', 'java', 'sql', 'log', 'csv', 'diff', 'patch'
  ];
  if (textExts.includes(ext)) return true;
  if (mimeType?.startsWith('text/')) return true;
  if (mimeType === 'application/json' || mimeType === 'application/javascript' || mimeType === 'application/xml') return true;
  return false;
}
