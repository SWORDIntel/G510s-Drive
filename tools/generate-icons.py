#!/usr/bin/env python3
"""
Generate multi-resolution icons, desktop assets, and favicon.ico from source image.
Resolves relative paths from repository root.
"""

import sys
import os
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parent.parent
    default_src = repo_root / "assets" / "G410S.png"
    
    # Allow passing custom source, fallback to Pictures if assets not yet populated
    if len(sys.argv) > 1 and Path(sys.argv[1]).is_file():
        src_path = Path(sys.argv[1])
    elif default_src.is_file():
        src_path = default_src
    elif Path("/home/john/Pictures/G410S.png").is_file():
        src_path = Path("/home/john/Pictures/G410S.png")
    else:
        print(f"Error: source image not found at {default_src}", file=sys.stderr)
        sys.exit(1)

    print(f"[+] Loading source icon from: {src_path}")

    try:
        from PIL import Image
    except ImportError:
        print("PIL/Pillow not found, falling back to ImageMagick convert...", file=sys.stderr)
        generate_with_imagemagick(src_path, repo_root)
        return

    img = Image.open(src_path)
    if img.mode not in ("RGB", "RGBA"):
        img = img.convert("RGBA")

    assets_dir = repo_root / "assets"
    icons_dir = assets_dir / "icons"
    assets_dir.mkdir(parents=True, exist_ok=True)
    icons_dir.mkdir(parents=True, exist_ok=True)

    # Ensure assets/G410S.png exists
    if src_path != default_src and not default_src.is_file():
        import shutil
        shutil.copy2(src_path, default_src)
        print(f"[+] Copied source image to: {default_src}")

    sizes = [16, 24, 32, 48, 64, 128, 256, 512]
    for sz in sizes:
        hicolor_dir = icons_dir / "hicolor" / f"{sz}x{sz}" / "apps"
        hicolor_dir.mkdir(parents=True, exist_ok=True)
        resized = img.resize((sz, sz), Image.Resampling.LANCZOS)
        resized.save(hicolor_dir / "g510tui.png", "PNG")
        resized.save(hicolor_dir / "g510s.png", "PNG")

    # Pixmap fallbacks
    img.resize((256, 256), Image.Resampling.LANCZOS).save(icons_dir / "g510tui.png", "PNG")
    img.resize((256, 256), Image.Resampling.LANCZOS).save(icons_dir / "g510s.png", "PNG")

    # Favicon PNGs
    img.resize((16, 16), Image.Resampling.LANCZOS).save(icons_dir / "favicon-16x16.png", "PNG")
    img.resize((32, 32), Image.Resampling.LANCZOS).save(icons_dir / "favicon-32x32.png", "PNG")

    # Favicon .ico with 16, 32, 48, 64 frames
    ico_path = icons_dir / "favicon.ico"
    root_ico_path = assets_dir / "favicon.ico"
    img.save(ico_path, format="ICO", sizes=[(16, 16), (32, 32), (48, 48), (64, 64)])
    img.save(root_ico_path, format="ICO", sizes=[(16, 16), (32, 32), (48, 48), (64, 64)])

    print(f"[+] Successfully generated icons in {icons_dir}")
    print(f"[+] Successfully generated favicon at {ico_path} and {root_ico_path}")

def generate_with_imagemagick(src_path: Path, repo_root: Path):
    import subprocess
    assets_dir = repo_root / "assets"
    icons_dir = assets_dir / "icons"
    icons_dir.mkdir(parents=True, exist_ok=True)

    sizes = [16, 24, 32, 48, 64, 128, 256, 512]
    for sz in sizes:
        hicolor_dir = icons_dir / "hicolor" / f"{sz}x{sz}" / "apps"
        hicolor_dir.mkdir(parents=True, exist_ok=True)
        subprocess.run(["convert", str(src_path), "-resize", f"{sz}x{sz}", str(hicolor_dir / "g510tui.png")], check=True)
        subprocess.run(["convert", str(src_path), "-resize", f"{sz}x{sz}", str(hicolor_dir / "g510s.png")], check=True)

    subprocess.run(["convert", str(src_path), "-resize", "256x256", str(icons_dir / "g510tui.png")], check=True)
    subprocess.run(["convert", str(src_path), "-resize", "256x256", str(icons_dir / "g510s.png")], check=True)
    subprocess.run(["convert", str(src_path), "-resize", "16x16", str(icons_dir / "favicon-16x16.png")], check=True)
    subprocess.run(["convert", str(src_path), "-resize", "32x32", str(icons_dir / "favicon-32x32.png")], check=True)
    subprocess.run(["convert", str(src_path), "-define", "icon:auto-resize=64,48,32,16", str(icons_dir / "favicon.ico")], check=True)
    subprocess.run(["cp", str(icons_dir / "favicon.ico"), str(assets_dir / "favicon.ico")], check=True)

if __name__ == "__main__":
    main()
