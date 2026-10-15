import DOMPurify from 'dompurify';

/**
 * Sanitizes an SVG string using DOMPurify with strict SVG profiles.
 */
export function sanitizeSvg(svgContent: string): string {
  if (!svgContent) return '';
  return DOMPurify.sanitize(svgContent, {
    USE_PROFILES: { svg: true, svgFilters: true },
    RETURN_TRUSTED_TYPE: false,
    FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick', 'onmouseover', 'onfocus', 'onblur'],
  });
}

/**
 * Converts sanitized SVG content to a safe data URL for sandboxed <img> rendering
 */
export function svgToSafeDataUrl(svgContent: string): string {
  const clean = sanitizeSvg(svgContent);
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(clean)}`;
}
