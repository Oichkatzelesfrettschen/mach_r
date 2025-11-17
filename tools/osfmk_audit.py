#!/usr/bin/env python3
import os, sys, json, hashlib, tarfile, zipfile, io, gzip, bz2, argparse, re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OSFMK = Path(os.getenv("OSFMK_ROOT", str(Path.home() / "OSFMK")))

def sha256sum(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(8192), b''):
            h.update(chunk)
    return h.hexdigest()

def walk(root: Path):
    stats = {
        'root': str(root),
        'total_files': 0,
        'by_ext': {},
        'by_dir': {},
        'top_files': [],
    }
    for p, _, files in os.walk(root):
        pth = Path(p)
        rel = str(pth.relative_to(root))
        for fn in files:
            fp = pth / fn
            try:
                sz = fp.stat().st_size
            except Exception:
                continue
            ext = fp.suffix.lower()
            stats['total_files'] += 1
            stats['by_ext'][ext] = stats['by_ext'].get(ext, 0) + 1
            stats['by_dir'].setdefault(rel, {'files': 0, 'bytes': 0})
            stats['by_dir'][rel]['files'] += 1
            stats['by_dir'][rel]['bytes'] += sz
            stats['top_files'].append((sz, str(fp)))
    stats['top_files'] = [ {'bytes': b, 'path': p} for b,p in sorted(stats['top_files'], reverse=True)[:50] ]
    return stats

MAPPINGS = {
    'osfmk/ipc': ['synthesis/src/port.rs', 'synthesis/src/message.rs', 'synthesis/src/syscall.rs'],
    'osfmk/vm': ['synthesis/src/paging.rs', 'synthesis/src/memory.rs', 'synthesis/src/external_pager.rs'],
    'osfmk/kern': ['synthesis/src/task.rs', 'synthesis/src/scheduler.rs', 'synthesis/src/interrupt.rs'],
    'osfmk/arm': ['synthesis/src/arch/aarch64.rs', 'synthesis/src/arch/mod.rs'],
    'osfmk/i386': ['synthesis/src/arch/x86_64/mod.rs'],
}

def generate_report(osfmk_root: Path, inv: dict):
    lines = []
    lines.append(f"# OSFMK Audit Report\n")
    lines.append(f"Source root: {osfmk_root}\n")
    lines.append(f"Total files: {inv['total_files']}\n")
    lines.append("## By Extension\n")
    for ext, cnt in sorted(inv['by_ext'].items(), key=lambda x: (-x[1], x[0])):
        lines.append(f"- {ext or '<none>'}: {cnt}")
    lines.append("\n## Key Directories\n")
    for d in ['osfmk/ipc','osfmk/vm','osfmk/kern','osfmk/arm','osfmk/i386','bsd','iokit']:
        if d in inv['by_dir']:
            lines.append(f"- {d}: {inv['by_dir'][d]['files']} files, {inv['by_dir'][d]['bytes']} bytes")
    lines.append("\n## Suggested Translation Map\n")
    for src, targets in MAPPINGS.items():
        lines.append(f"- {src} ->")
        for t in targets:
            lines.append(f"  - {t}")
    lines.append("\n## Next Steps\n")
    lines.append("- Prioritize vm/, ipc/, kern/ for translation to Rust modules.")
    lines.append("- Extract constants and structure layouts; recreate in Rust types.")
    lines.append("- Build unit tests around rights transitions, vm faults, and scheduling.")
    return "\n".join(lines)

def main():
    parser = argparse.ArgumentParser(description='OSFMK audit/extract tool')
    parser.add_argument('root', nargs='?', default=str(DEFAULT_OSFMK))
    parser.add_argument('command', nargs='?', choices=['audit','extract'], default='audit')
    args = parser.parse_args()

    osfmk = Path(args.root)
    if not osfmk.exists():
        print(f"OSFMK root not found: {osfmk}")
        sys.exit(1)
    out_dir = ROOT / 'archive' / 'osfmk'
    out_dir.mkdir(parents=True, exist_ok=True)

    if args.command == 'audit':
        inv = walk(osfmk)
        (out_dir / 'inventory.json').write_text(json.dumps(inv, indent=2))
        archives = []
        for p, _, files in os.walk(osfmk):
            for fn in files:
                fp = Path(p) / fn
                try:
                    if tarfile.is_tarfile(fp):
                        with tarfile.open(fp, 'r:*') as tf:
                            names = tf.getnames()[:50]
                            archives.append({'path': str(fp), 'type': 'tar', 'entries': names})
                    elif zipfile.is_zipfile(fp):
                        with zipfile.ZipFile(fp) as zf:
                            names = zf.namelist()[:50]
                            archives.append({'path': str(fp), 'type': 'zip', 'entries': names})
                except Exception:
                    continue
        (out_dir / 'archives_summary.json').write_text(json.dumps(archives, indent=2))
        report = generate_report(osfmk, inv)
        rep_dir = ROOT / 'reports'
        rep_dir.mkdir(parents=True, exist_ok=True)
        (rep_dir / 'OSFMK_AUDIT.md').write_text(report)
        print(f"Wrote {out_dir / 'inventory.json'}, {out_dir / 'archives_summary.json'} and {rep_dir / 'OSFMK_AUDIT.md'}")
        return

    # extract mode
    focus_exts = {'.h', '.defs'}
    focus_dirs = [
        'osfmk/', 'mach/', 'ipc/', 'vm/', 'kern/', 'osfmk/src', 'osfmk/src/ipc', 'osfmk/src/vm', 'osfmk/src/kern', 'osfmk/src/mach'
    ]
    ref_root = out_dir / 'reference'
    ref_root.mkdir(parents=True, exist_ok=True)
    extracted = []

    core_dirs = re.compile(r"(osfmk|mach_kernel|osf\.mk|osfmk-src|osfmk-export)")
    subdirs = re.compile(r"/(mach|ipc|vm|kern)/")
    def want(name: str) -> bool:
        lname = name.lower()
        if not any(lname.endswith(e) for e in focus_exts):
            return False
        if not core_dirs.search(lname):
            return False
        return bool(subdirs.search(lname))

    for p, _, files in os.walk(osfmk):
        for fn in files:
            fp = Path(p) / fn
            try:
                if tarfile.is_tarfile(fp):
                    with tarfile.open(fp, 'r:*') as tf:
                        for m in tf.getmembers():
                            if not m.isreg():
                                continue
                            if want(m.name):
                                data = tf.extractfile(m).read()
                                dst = ref_root / Path(fp.stem).name / m.name
                                dst.parent.mkdir(parents=True, exist_ok=True)
                                dst.write_bytes(data)
                                extracted.append(str(dst.relative_to(ref_root)))
                elif zipfile.is_zipfile(fp):
                    with zipfile.ZipFile(fp) as zf:
                        for name in zf.namelist():
                            if want(name):
                                data = zf.read(name)
                                dst = ref_root / Path(fp.stem).name / name
                                dst.parent.mkdir(parents=True, exist_ok=True)
                                dst.write_bytes(data)
                                extracted.append(str(dst.relative_to(ref_root)))
            except Exception:
                continue
    idx = {'root': str(ref_root), 'files': sorted(extracted)}
    (out_dir / 'reference_index.json').write_text(json.dumps(idx, indent=2))
    rep = ROOT / 'reports' / 'OSFMK_REFERENCE_INDEX.md'
    rep.parent.mkdir(parents=True, exist_ok=True)
    with open(rep, 'w') as f:
        f.write('# OSFMK Reference Index (Extracted)\n\n')
        f.write(f'Root: {ref_root}\n\n')
        for rel in idx['files']:
            f.write(f'- {rel}\n')
    print(f"Extracted {len(extracted)} files into {ref_root}; wrote reference_index.json and OSFMK_REFERENCE_INDEX.md")

if __name__ == '__main__':
    main()
