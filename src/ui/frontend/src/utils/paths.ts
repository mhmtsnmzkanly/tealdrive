// Absolute SPA paths: "/" is the root, no trailing slash elsewhere.

export function joinPath(dir: string, name: string): string {
  return dir === '/' ? `/${name}` : `${dir}/${name}`;
}

export function parentPath(path: string): string {
  const parent = path.substring(0, path.lastIndexOf('/'));
  return parent === '' ? '/' : parent;
}

/** "a.txt" -> "a copy.txt", then "a copy 2.txt", ... avoiding names already present. */
export function copyName(name: string, taken: Set<string>): string {
  const dot = name.lastIndexOf('.');
  const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, ''];
  for (let i = 1; ; i++) {
    const candidate = `${stem} copy${i === 1 ? '' : ` ${i}`}${ext}`;
    if (!taken.has(candidate)) return candidate;
  }
}
