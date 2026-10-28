"""Compares UmlCraft with Astah for every .asta file in a folder.

Usage: python -I tools/asta_compare_all.py asta-dir out-dir [first-index]

Per file: what UmlCraft's import skips, Astah's SVG (command line) and UmlCraft's SVG
(`asta-svg` example) of every diagram, and per diagram a score: the share of ink drawn by one
side only, with 1 px tolerance, after shifting UmlCraft's drawing onto Astah's. Writes
out-dir/NN/<diagram>-overlay.png (Astah red, UmlCraft blue) and prints one JSON line per file.
Build the examples first (cargo build -p umlcraft-asta --examples). Needs Inkscape, Pillow and
numpy.
"""
import json
import subprocess
import sys
from pathlib import Path

import numpy
from PIL import Image, ImageChops, ImageFilter

ASTAH = Path(r'C:\Program Files\astah-professional')
INKSCAPE = r'C:\Program Files\Inkscape\bin\inkscape.com'
TARGET = Path(__file__).resolve().parent.parent / 'target' / 'debug' / 'examples'
# Astah's command line runs elsewhere and needs absolute paths.
src, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)


def render(svg, png):
    # Inkscape now and then exits without writing the file when several run at once.
    for _ in range(3):
        subprocess.run([INKSCAPE, str(svg), '--export-type=png', '--export-dpi=96',
                        '--export-background=white', '--export-background-opacity=1',
                        f'--export-filename={png}'], capture_output=True)
        if Path(png).exists():
            break
    return Image.open(png).convert('L')


def pad(image, size):
    canvas = Image.new('L', size, 255)
    canvas.paste(image, (0, 0))
    return canvas


def score(astah_svg, ours_svg, prefix):
    a = render(astah_svg, f'{prefix}-a.png')
    b = render(ours_svg, f'{prefix}-b.png')
    size = (max(a.width, b.width), max(a.height, b.height))
    a, b = pad(a, size), pad(b, size)
    fa = numpy.array(a) < 160
    fb = numpy.array(b) < 160
    corr = numpy.fft.irfft2(numpy.fft.rfft2(fa) * numpy.conj(numpy.fft.rfft2(fb)), fa.shape)
    dy, dx = numpy.unravel_index(numpy.argmax(corr), corr.shape)
    dy = int(dy - fa.shape[0] if dy > fa.shape[0] // 2 else dy)
    dx = int(dx - fa.shape[1] if dx > fa.shape[1] // 2 else dx)
    b = ImageChops.offset(b, dx, dy)
    ink_a = numpy.array(a) < 160
    ink_b = numpy.array(b) < 160
    grow_a = numpy.array(Image.fromarray(ink_a.astype('uint8') * 255).filter(ImageFilter.MaxFilter(3))) > 0
    grow_b = numpy.array(Image.fromarray(ink_b.astype('uint8') * 255).filter(ImageFilter.MaxFilter(3))) > 0
    miss = int((ink_a & ~grow_b).sum() + (ink_b & ~grow_a).sum())
    total = int(ink_a.sum() + ink_b.sum())
    Image.merge('RGB', (b, ImageChops.multiply(a, b), a)).save(f'{prefix}-overlay.png')
    return miss / max(total, 1), (dx, dy)


def run(job):
    i, asta = job
    case = out / f'{i:02d}'
    case.mkdir(exist_ok=True)
    entry = {'file': asta.name, 'case': case.name}
    imp = subprocess.run([str(TARGET / 'asta-import.exe'), str(asta)], capture_output=True,
                         text=True, encoding='utf-8', errors='replace')
    skipped = [l for l in imp.stdout.splitlines() if l.startswith('skipped:')]
    entry['skipped'] = skipped[0][len('skipped: '):] if skipped else f'IMPORT FAILED: {imp.stderr[-300:]}'
    subprocess.run([str(ASTAH / 'jre/bin/java.exe'), '-Xmx2048m', '-Djava.awt.headless=true',
                    '-cp', str(ASTAH / 'astah-pro.jar'),
                    'com.change_vision.jude.cmdline.JudeCommandRunner', '-image', 'all',
                    '-f', str(asta), '-t', 'svg', '-o', str(case / 'astah')],
                   capture_output=True, cwd=ASTAH)
    svg = subprocess.run([str(TARGET / 'asta-svg.exe'), str(asta), str(case / 'ours')],
                         capture_output=True, text=True, encoding='utf-8', errors='replace')
    if svg.returncode:
        entry['svg_error'] = svg.stderr[-300:]
    astah_svgs = {p.stem: p for p in (case / 'astah').rglob('*.svg')}
    ours_svgs = {p.stem: p for p in (case / 'ours').glob('*.svg')} if (case / 'ours').exists() else {}
    entry['astah_only'] = sorted(set(astah_svgs) - set(ours_svgs))
    entry['diagrams'] = {}
    for name in sorted(set(astah_svgs) & set(ours_svgs)):
        try:
            share, shift = score(astah_svgs[name], ours_svgs[name], case / name)
            entry['diagrams'][name] = [round(share, 4), shift]
        except Exception as e:
            entry['diagrams'][name] = [None, str(e)[:200]]
    return entry


if __name__ == '__main__':
    from concurrent.futures import ProcessPoolExecutor
    start = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    jobs = [j for j in enumerate(sorted(src.glob('*.asta'))) if j[0] >= start]
    with ProcessPoolExecutor(6) as pool:
        for entry in pool.map(run, jobs):
            print(json.dumps(entry, ensure_ascii=False), flush=True)
