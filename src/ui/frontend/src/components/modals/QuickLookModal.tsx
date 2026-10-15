import React, { useState, useEffect } from 'react';
import type { FileEntry } from '../../sdk/types.ts';
import { useTealDrive } from '../../context/TealDriveContext.tsx';
import { getFileCategory, formatBytes, isTextFile } from '../../utils/formatters.ts';
import { svgToSafeDataUrl } from '../../utils/safeSvg.ts';
import { tealdrive } from '../../lib/tealdrive.ts';
import {
  X,
  Download,
  ZoomIn,
  ZoomOut,
  RotateCw,
  FileCode,
  FileText,
  Volume2,
  Globe,
  Code2,
  File,
  Loader2,
} from 'lucide-react';

interface QuickLookModalProps {
  file: FileEntry;
  onClose: () => void;
}

export const QuickLookModal: React.FC<QuickLookModalProps> = ({ file, onClose }) => {
  const { downloadFile, setEditorFile } = useTealDrive();
  const category = getFileCategory(file.name, file.isDirectory, file.mimeType);

  // Zoom & rotation state for images
  const [zoom, setZoom] = useState(1);
  const [rotation, setRotation] = useState(0);
  const [fileUrl, setFileUrl] = useState<string>('');
  const [loadingMedia, setLoadingMedia] = useState(false);
  const [mediaError, setMediaError] = useState<string | null>(null);

  // Text / Code preview state
  const isHtml = /\.html?$/i.test(file.name) || file.mimeType === 'text/html';
  const isTextLike =
    isTextFile(file.name, file.mimeType) ||
    category === 'code' ||
    (category === 'document' && file.mimeType !== 'application/pdf');

  const [textContent, setTextContent] = useState<string>(file.content || '');
  const [loadingText, setLoadingText] = useState(false);
  const [textError, setTextError] = useState<string | null>(null);
  const [htmlViewMode, setHtmlViewMode] = useState<'preview' | 'source'>('preview');

  // Load binary media (images, audio, video)
  useEffect(() => {
    let active = true;

    const prepareUrl = async () => {
      setMediaError(null);

      if (category === 'image' && file.content?.includes('<svg')) {
        setFileUrl(svgToSafeDataUrl(file.content));
        return;
      }

      const isMedia =
        category === 'image' ||
        category === 'media' ||
        file.mimeType.startsWith('image/') ||
        file.mimeType.startsWith('audio/') ||
        file.mimeType.startsWith('video/') ||
        file.mimeType === 'application/pdf';

      if (isMedia) {
        if (file.size === 0) {
          setMediaError('Dosya boyutu 0 bayt (boş dosya). Önizleme oluşturulamıyor.');
          return;
        }

        setLoadingMedia(true);
        try {
          const previewUrl = (tealdrive.files as any).getPreviewUrl({ path: file.path });
          if (active) {
            setFileUrl(previewUrl);
          }
        } catch (err: any) {
          console.warn('Could not construct preview URL:', err);
          if (active) {
            setMediaError(err?.message || 'Önizleme yüklenemedi');
            setLoadingMedia(false);
          }
        }
      }
    };

    prepareUrl();

    return () => {
      active = false;
    };
  }, [file.path, file.content, category, file.mimeType, file.size]);

  // Load text / code content if not present
  useEffect(() => {
    if (!isTextLike) return;

    if (file.content !== undefined && file.content !== null) {
      setTextContent(file.content);
      return;
    }

    let active = true;
    setLoadingText(true);
    setTextError(null);

    const fetchText = async () => {
      try {
        const res = await tealdrive.files.readText({ path: file.path });
        if (active) {
          setTextContent(res.content ?? '');
        }
      } catch (err: any) {
        // Fallback: try download endpoint and decode as text
        try {
          const dl = await tealdrive.files.download({ path: file.path });
          const response = await fetch(dl.blobUrl);
          const txt = await response.text();
          if (active) {
            setTextContent(txt);
          }
        } catch (fallbackErr: any) {
          if (active) {
            setTextError(err?.message || 'Unable to read file content.');
          }
        }
      } finally {
        if (active) {
          setLoadingText(false);
        }
      }
    };

    fetchText();

    return () => {
      active = false;
    };
  }, [file.path, file.content, isTextLike]);

  const handleZoomIn = () => setZoom((z) => Math.min(3, z + 0.25));
  const handleZoomOut = () => setZoom((z) => Math.max(0.5, z - 0.25));
  const handleRotate = () => setRotation((r) => (r + 90) % 360);

  const isVideo =
    file.mimeType.startsWith('video/') ||
    /\.(mp4|webm|mov|mkv|avi)$/i.test(file.name);
  const isAudio =
    !isVideo &&
    (file.mimeType.startsWith('audio/') || /\.(mp3|wav|ogg|aac|flac)$/i.test(file.name));

  const lines = textContent.split('\n');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/90 backdrop-blur-md animate-in fade-in duration-150">
      <div className="w-full max-w-4xl h-[84vh] bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl flex flex-col overflow-hidden text-slate-100">
        {/* Header */}
        <div className="h-14 px-5 bg-slate-950/70 border-b border-slate-800 flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5 min-w-0">
            <span className="text-xs font-semibold text-slate-200 truncate">{file.name}</span>
            <span className="text-[10px] font-mono text-slate-500">
              ({formatBytes(file.size)} · {file.mode})
            </span>

            {/* HTML View Switcher */}
            {isHtml && (
              <div className="flex items-center bg-slate-800/80 border border-slate-700 rounded-lg p-0.5 ml-2">
                <button
                  onClick={() => setHtmlViewMode('preview')}
                  className={`flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium transition-colors ${
                    htmlViewMode === 'preview'
                      ? 'bg-teal-600 text-slate-950 font-semibold shadow'
                      : 'text-slate-400 hover:text-slate-200'
                  }`}
                >
                  <Globe className="w-3 h-3" />
                  <span>Browser Preview</span>
                </button>
                <button
                  onClick={() => setHtmlViewMode('source')}
                  className={`flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium transition-colors ${
                    htmlViewMode === 'source'
                      ? 'bg-teal-600 text-slate-950 font-semibold shadow'
                      : 'text-slate-400 hover:text-slate-200'
                  }`}
                >
                  <Code2 className="w-3 h-3" />
                  <span>Source Code</span>
                </button>
              </div>
            )}
          </div>

          <div className="flex items-center gap-2">
            {isTextLike && (
              <button
                onClick={() => {
                  onClose();
                  setEditorFile(file);
                }}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-teal-950 hover:bg-teal-900 border border-teal-700/60 text-teal-300 text-xs font-medium transition-colors shadow-sm"
              >
                <FileCode className="w-3.5 h-3.5" />
                <span>Open in Editor</span>
              </button>
            )}

            <button
              onClick={() => downloadFile(file.path)}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
              title="Download file"
            >
              <Download className="w-4 h-4" />
            </button>

            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Content Viewer Body */}
        <div className="flex-1 bg-[#090d16] flex items-center justify-center overflow-auto p-4 relative">
          {/* Images */}
          {category === 'image' && (
            <div className="flex flex-col items-center justify-center w-full h-full">
              {fileUrl && !mediaError ? (
                <div className="relative flex items-center justify-center max-h-[70vh]">
                  {loadingMedia && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-slate-400 bg-slate-950/60 backdrop-blur-sm rounded-xl z-10">
                      <Loader2 className="w-8 h-8 animate-spin text-teal-400" />
                      <span className="text-xs font-mono">Önizleme yükleniyor...</span>
                    </div>
                  )}
                  <img
                    src={fileUrl}
                    alt={file.name}
                    onLoad={() => setLoadingMedia(false)}
                    onError={() => {
                      setLoadingMedia(false);
                      setMediaError('Görsel biçimi desteklenmiyor veya dosya yüklenemedi.');
                    }}
                    style={{
                      transform: `scale(${zoom}) rotate(${rotation}deg)`,
                    }}
                    className={`max-h-[70vh] object-contain transition-transform duration-200 select-none ${
                      loadingMedia ? 'opacity-0' : 'opacity-100'
                    }`}
                  />
                </div>
              ) : (
                <div className="w-full max-w-xl h-80 rounded-2xl bg-gradient-to-br from-teal-950/60 via-slate-900 to-slate-950 border border-slate-700 flex flex-col items-center justify-center p-8 text-center">
                  <FileText className="w-12 h-12 text-teal-400 mb-3" />
                  <h4 className="text-base font-semibold text-slate-200">{file.name}</h4>
                  <p className="text-xs text-slate-400 mt-1">
                    {loadingMedia
                      ? 'Önizleme yükleniyor...'
                      : mediaError || 'Görsel önizlenemiyor'}
                  </p>
                  {file.size === 0 && (
                    <div className="mt-3 px-3 py-1.5 rounded-lg bg-amber-500/10 border border-amber-500/20 text-[11px] text-amber-300">
                      Bu dosya sunucuya 0 bayt olarak kaydedilmiş. Lütfen dosyayı tekrar yükleyin.
                    </div>
                  )}
                  {!loadingMedia && (
                    <button
                      onClick={() => downloadFile(file.path)}
                      className="mt-4 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-xs rounded-lg text-slate-200 transition-colors"
                    >
                      Dosyayı İndir
                    </button>
                  )}
                </div>
              )}

              {/* Lightbox Toolbar */}
              {fileUrl && !mediaError && (
                <div className="absolute bottom-6 flex items-center gap-2 p-1.5 bg-slate-900/90 border border-slate-700 rounded-xl backdrop-blur-md shadow-xl">
                  <button
                    onClick={handleZoomIn}
                    className="p-1.5 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800"
                    title="Yakınlaştır"
                  >
                    <ZoomIn className="w-4 h-4" />
                  </button>
                  <span className="text-xs font-mono px-2 text-slate-400">
                    {Math.round(zoom * 100)}%
                  </span>
                  <button
                    onClick={handleZoomOut}
                    className="p-1.5 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800"
                    title="Uzaklaştır"
                  >
                    <ZoomOut className="w-4 h-4" />
                  </button>
                  <div className="h-4 w-px bg-slate-800" />
                  <button
                    onClick={handleRotate}
                    className="p-1.5 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800"
                    title="90° Döndür"
                  >
                    <RotateCw className="w-4 h-4" />
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Video Player */}
          {isVideo && (
            <div className="w-full max-w-3xl flex flex-col items-center">
              {fileUrl && !mediaError ? (
                <>
                  <video
                    controls
                    autoPlay={false}
                    src={fileUrl}
                    onLoadedData={() => setLoadingMedia(false)}
                    onError={() => {
                      setLoadingMedia(false);
                      setMediaError('Video formatı desteklenmiyor veya dosya bozuk.');
                    }}
                    className="max-h-[70vh] w-full rounded-xl bg-slate-950 shadow-2xl border border-slate-800"
                  >
                    Tarayıcınız video önizlemesini desteklemiyor.
                  </video>
                  <div className="mt-3 text-xs text-slate-400 font-mono">
                    {file.name} · {formatBytes(file.size)}
                  </div>
                </>
              ) : (
                <div className="w-full max-w-xl h-80 rounded-2xl bg-gradient-to-br from-teal-950/60 via-slate-900 to-slate-950 border border-slate-700 flex flex-col items-center justify-center p-8 text-center">
                  <FileText className="w-12 h-12 text-teal-400 mb-3" />
                  <h4 className="text-base font-semibold text-slate-200">{file.name}</h4>
                  <p className="text-xs text-slate-400 mt-1">
                    {loadingMedia ? 'Video yükleniyor...' : mediaError || 'Video önizlenemiyor'}
                  </p>
                </div>
              )}
            </div>
          )}

          {/* Audio Player */}
          {isAudio && (
            <div className="w-full max-w-md p-6 bg-slate-900 border border-slate-700 rounded-2xl shadow-xl flex flex-col items-center">
              <div className="w-16 h-16 rounded-full bg-sky-500/10 border border-sky-500/30 flex items-center justify-center text-sky-400 mb-4">
                <Volume2 className="w-8 h-8" />
              </div>
              <div className="font-semibold text-sm text-slate-200 mb-1">{file.name}</div>
              <div className="text-xs text-slate-500 font-mono mb-4">
                {file.mimeType} · {formatBytes(file.size)}
              </div>
              {fileUrl && !mediaError ? (
                <audio
                  controls
                  src={fileUrl}
                  onCanPlay={() => setLoadingMedia(false)}
                  onError={() => {
                    setLoadingMedia(false);
                    setMediaError('Ses formatı oynatılamadı.');
                  }}
                  className="w-full rounded-lg"
                />
              ) : (
                <p className="text-xs text-slate-400">
                  {loadingMedia ? 'Ses dosyası yükleniyor...' : mediaError || 'Ses önizlenemiyor'}
                </p>
              )}
            </div>
          )}

          {/* PDF Viewer */}
          {category === 'document' && file.mimeType === 'application/pdf' && (
            <div className="w-full h-full flex flex-col items-center justify-center max-w-2xl">
              <div className="w-full bg-slate-900 border border-slate-700 rounded-2xl p-8 flex flex-col items-center text-center shadow-xl">
                <div className="w-16 h-16 rounded-2xl bg-rose-500/10 border border-rose-500/30 flex items-center justify-center text-rose-400 mb-4">
                  <FileText className="w-8 h-8" />
                </div>
                <h4 className="text-base font-semibold text-slate-100">{file.name}</h4>
                <p className="text-xs text-slate-400 mt-1 max-w-md">
                  PDF Document (Signed &amp; Timestamped by TealDrive Core)
                </p>

                <div className="my-6 p-4 w-full bg-slate-950 rounded-xl border border-slate-800 text-left text-xs font-mono text-slate-400 space-y-1">
                  <div>Classification: User Space Document</div>
                  <div>Size: {formatBytes(file.size)}</div>
                  <div>Permissions: {file.mode}</div>
                </div>

                <button
                  onClick={() => downloadFile(file.path)}
                  className="px-4 py-2 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-xl shadow transition-colors"
                >
                  Download Complete PDF ({formatBytes(file.size)})
                </button>
              </div>
            </div>
          )}

          {/* Code / Text / HTML Preview */}
          {isTextLike && (
            <div className="w-full h-full flex flex-col overflow-hidden">
              {loadingText ? (
                <div className="w-full h-full flex flex-col items-center justify-center gap-3 text-slate-400">
                  <Loader2 className="w-6 h-6 animate-spin text-teal-400" />
                  <span className="text-xs font-mono">Loading document content...</span>
                </div>
              ) : textError ? (
                <div className="w-full h-full flex flex-col items-center justify-center gap-3 text-slate-400 p-6 text-center">
                  <File className="w-10 h-10 text-rose-400" />
                  <div className="text-sm font-semibold text-slate-200">{textError}</div>
                  <button
                    onClick={() => downloadFile(file.path)}
                    className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-xs rounded-lg text-slate-200 transition-colors"
                  >
                    Download File Instead
                  </button>
                </div>
              ) : isHtml && htmlViewMode === 'preview' ? (
                <div className="w-full h-full rounded-xl overflow-hidden border border-slate-800 bg-white">
                  <iframe
                    srcDoc={textContent}
                    sandbox="allow-scripts"
                    className="w-full h-full border-0"
                    title={`Preview of ${file.name}`}
                  />
                </div>
              ) : (
                <div className="w-full h-full flex overflow-hidden bg-slate-950 border border-slate-800/90 rounded-xl font-mono text-xs text-slate-300">
                  <div className="w-12 py-4 bg-slate-950/80 border-r border-slate-800 text-right pr-3 select-none text-slate-600 font-mono text-[11px] overflow-hidden shrink-0">
                    {lines.map((_, i) => (
                      <div key={i} className="leading-5">
                        {i + 1}
                      </div>
                    ))}
                  </div>
                  <pre className="flex-1 p-4 overflow-auto whitespace-pre leading-5 selection:bg-teal-500/30 selection:text-teal-200 font-mono text-slate-200">
                    {textContent || (file.size === 0 ? '// Empty document' : '')}
                  </pre>
                </div>
              )}
            </div>
          )}

          {/* Binary Document Fallback (not image, audio, video, pdf, or text) */}
          {!category && !isTextLike && !isVideo && !isAudio && category !== 'image' && (
            <div className="w-full max-w-md bg-slate-900 border border-slate-800 rounded-2xl p-8 flex flex-col items-center text-center shadow-xl">
              <div className="w-16 h-16 rounded-2xl bg-slate-800 flex items-center justify-center text-slate-400 mb-4">
                <File className="w-8 h-8" />
              </div>
              <h4 className="text-base font-semibold text-slate-100">{file.name}</h4>
              <p className="text-xs text-slate-400 mt-1">
                {file.mimeType} · {formatBytes(file.size)}
              </p>
              <button
                onClick={() => downloadFile(file.path)}
                className="mt-6 px-4 py-2 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-xl shadow transition-colors"
              >
                Download File
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
