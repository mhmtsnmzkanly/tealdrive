import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

const distDir = path.resolve('dist');
const entry = 'index.html';
const htmlPath = path.join(distDir, entry);
const assets = [];
const fileHashes = {};

// 1. Scan and compute hashes for all asset files (excluding entry and manifest)
function scanDir(dir, prefix = '') {
  for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, item.name);
    const relPath = prefix ? `${prefix}/${item.name}` : item.name;
    if (item.isDirectory()) {
      scanDir(fullPath, relPath);
    } else if (item.isFile()) {
      if (relPath === 'manifest.json' || relPath === entry) continue;
      const content = fs.readFileSync(fullPath);
      const hash = crypto.createHash('sha256').update(content).digest('hex');
      fileHashes[relPath] = hash;
      assets.push(relPath);
    }
  }
}

scanDir(distDir);

// 2. Read dist/index.html and append ?{hash} to asset references (no hash in filename)
if (fs.existsSync(htmlPath)) {
  let html = fs.readFileSync(htmlPath, 'utf8');

  for (const [relPath, fullHash] of Object.entries(fileHashes)) {
    const shortHash = fullHash.substring(0, 10);
    // Replace e.g. /static/assets/index.js or assets/index.js with ?{hash}
    const escaped = relPath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const regex = new RegExp(`((?:src|href)=["'][^"']*?${escaped})(?:\\?[^"']*)?(["'])`, 'g');
    html = html.replace(regex, `$1?${shortHash}$2`);
  }

  fs.writeFileSync(htmlPath, html, 'utf8');
}

// 3. Compute final sha256 for index.html after query hash injection
const htmlContent = fs.readFileSync(htmlPath);
const htmlHash = crypto.createHash('sha256').update(htmlContent).digest('hex');

const sha256 = {
  ...fileHashes,
  [entry]: htmlHash,
};

const manifest = {
  manifestVersion: 1,
  name: 'tealdrive-web',
  version: '1.0.0',
  sdkMajor: 1,
  entry: 'index.html',
  assets: assets.sort(),
  sha256,
};

fs.writeFileSync(
  path.join(distDir, 'manifest.json'),
  JSON.stringify(manifest, null, 2) + '\n'
);
console.log('Generated dist/manifest.json successfully:', manifest);
