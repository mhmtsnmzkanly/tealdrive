import React from 'react';
import {
  Folder,
  FileText,
  FileCode,
  FileImage,
  FileAudio,
  FileVideo,
  FileArchive,
  File,
} from 'lucide-react';
import { getFileCategory } from '../../utils/formatters.ts';

interface FileItemIconProps {
  name: string;
  isDirectory: boolean;
  mimeType?: string;
  size?: 'sm' | 'md' | 'lg';
}

export const FileItemIcon: React.FC<FileItemIconProps> = ({
  name,
  isDirectory,
  mimeType,
  size = 'md',
}) => {
  const category = getFileCategory(name, isDirectory, mimeType);

  const iconSizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-6 h-6',
    lg: 'w-10 h-10',
  }[size];

  if (isDirectory) {
    return (
      <div className="text-teal-400 group-hover:text-teal-300 transition-colors">
        <Folder className={iconSizeClasses} fill="currentColor" fillOpacity={0.15} />
      </div>
    );
  }

  switch (category) {
    case 'code':
      return (
        <div className="text-emerald-400">
          <FileCode className={iconSizeClasses} />
        </div>
      );
    case 'document':
      return (
        <div className="text-rose-400">
          <FileText className={iconSizeClasses} />
        </div>
      );
    case 'image':
      return (
        <div className="text-cyan-400">
          <FileImage className={iconSizeClasses} />
        </div>
      );
    case 'media':
      return (
        <div className="text-sky-400">
          <FileAudio className={iconSizeClasses} />
        </div>
      );
    case 'archive':
      return (
        <div className="text-amber-400">
          <FileArchive className={iconSizeClasses} />
        </div>
      );
    default:
      return (
        <div className="text-slate-400">
          <File className={iconSizeClasses} />
        </div>
      );
  }
};
