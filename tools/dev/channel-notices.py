#!/usr/bin/env python3
"""Collect upstream license files for the embedded, Cargo.lock-pinned WASM runtime."""
import json, subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[2]
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1','--filter-platform','wasm32-unknown-unknown'],cwd=root))
packages={p['id']:p for p in metadata['packages']}
nodes={n['id']:n for n in metadata['resolve']['nodes']}
start=next(k for k,v in packages.items() if v['name']=='tealdrive-channel')
seen=set();pending=[start]
while pending:
    ident=pending.pop()
    if ident in seen:continue
    seen.add(ident);pending.extend(d['pkg'] for d in nodes[ident]['deps'])
lines=['# Embedded Noise runtime: upstream license notices','',
       'Generated from Cargo.lock by tools/dev/channel-notices.py. Includes resolved development dependencies for completeness.','']
for package in sorted((packages[i] for i in seen if i!=start),key=lambda p:(p['name'],p['version'])):
    lines.extend([f"## {package['name']} {package['version']}",'',f"License: {package.get('license')}; source: {package.get('repository') or package.get('source')}",''])
    directory=Path(package['manifest_path']).parent
    files=sorted({p for pattern in ('LICENSE*','COPYING*','UNLICENSE*') for p in directory.glob(pattern) if p.is_file() and p.stat().st_size<100000})
    for path in files:
        lines.extend([f'### {path.name}','','```text',path.read_text(errors='replace').rstrip(),'```',''])
(root/'src/ui/sdk/noise/THIRD_PARTY_NOTICES.md').write_text('\n'.join(lines))
