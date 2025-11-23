"""
将 SVG 图标转换为所有需要的格式
需要安装: pip install cairosvg pillow
"""

import cairosvg
from PIL import Image
import io
import os

def svg_to_png(svg_path, png_path, size):
    """将 SVG 转换为指定尺寸的 PNG"""
    png_data = cairosvg.svg2png(
        url=svg_path,
        output_width=size,
        output_height=size
    )
    
    # 保存 PNG
    with open(png_path, 'wb') as f:
        f.write(png_data)
    
    return png_path

def create_ico_from_svg(svg_path, ico_path):
    """从 SVG 创建 ICO 文件"""
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    images = []
    
    for size in sizes:
        png_data = cairosvg.svg2png(
            url=svg_path,
            output_width=size[0],
            output_height=size[1]
        )
        img = Image.open(io.BytesIO(png_data))
        images.append(img)
    
    # 保存为 ICO
    images[0].save(ico_path, format='ICO', sizes=sizes, append_images=images[1:])
    print(f"✓ 生成 icon.ico")

def create_icns_from_svg(svg_path, icns_path):
    """从 SVG 创建 ICNS 文件"""
    import struct
    
    icon_types = [
        (512, b'ic09'),
        (256, b'ic08'),
        (128, b'ic07'),
        (32, b'ic11'),
        (16, b'ic04'),
    ]
    
    icon_data = []
    
    for size, type_code in icon_types:
        png_data = cairosvg.svg2png(
            url=svg_path,
            output_width=size,
            output_height=size
        )
        
        chunk_size = 8 + len(png_data)
        chunk = type_code + struct.pack('>I', chunk_size) + png_data
        icon_data.append(chunk)
    
    with open(icns_path, 'wb') as f:
        total_size = 8 + sum(len(chunk) for chunk in icon_data)
        f.write(b'icns')
        f.write(struct.pack('>I', total_size))
        for chunk in icon_data:
            f.write(chunk)
    
    print(f"✓ 生成 icon.icns")

def main():
    icons_dir = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'icons')
    svg_path = os.path.join(icons_dir, 'icon.svg')
    
    if not os.path.exists(svg_path):
        print(f"❌ SVG 文件不存在: {svg_path}")
        return
    
    print("🎨 从 SVG 生成所有图标格式...\n")
    
    # 生成各种尺寸的 PNG
    sizes = {
        'icon.png': 512,
        '32x32.png': 32,
        '128x128.png': 128,
        '128x128@2x.png': 256,
        'Square30x30Logo.png': 30,
        'Square44x44Logo.png': 44,
        'Square71x71Logo.png': 71,
        'Square89x89Logo.png': 89,
        'Square107x107Logo.png': 107,
        'Square142x142Logo.png': 142,
        'Square150x150Logo.png': 150,
        'Square284x284Logo.png': 284,
        'Square310x310Logo.png': 310,
        'StoreLogo.png': 50,
    }
    
    print("📦 生成 PNG 文件...")
    for filename, size in sizes.items():
        output_path = os.path.join(icons_dir, filename)
        svg_to_png(svg_path, output_path, size)
        print(f"  ✓ {filename} ({size}x{size})")
    
    print("\n🪟 生成 Windows ICO...")
    ico_path = os.path.join(icons_dir, 'icon.ico')
    create_ico_from_svg(svg_path, ico_path)
    
    print("\n🍎 生成 macOS ICNS...")
    icns_path = os.path.join(icons_dir, 'icon.icns')
    create_icns_from_svg(svg_path, icns_path)
    
    print("\n✨ 所有图标生成完成！")
    print(f"📁 输出目录: {icons_dir}")

if __name__ == '__main__':
    try:
        main()
    except ImportError as e:
        print("❌ 缺少依赖库！")
        print("请运行: pip install cairosvg pillow")
        print(f"错误: {e}")
