"""
简化版：使用 Pillow 直接绘制高质量图标
不依赖 cairosvg，纯 Python 实现
"""

from PIL import Image, ImageDraw, ImageFilter
import os
import struct
import io

def create_modern_icon(size):
    """创建简洁扁平科幻风格的 iLauncher 图标"""
    
    # 创建高分辨率画布（2倍采样）
    render_size = size * 2
    img = Image.new('RGBA', (render_size, render_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    center = render_size // 2
    padding = render_size // 10
    
    # === 1. 扁平渐变背景圆形 - 科幻感蓝紫色 ===
    # 使用简单的双色渐变
    for i in range(50):
        ratio = i / 50
        radius = int((render_size // 2 - padding) * (1 - ratio * 0.15))
        
        # 深空蓝到电光蓝
        r = int(41 + (99 - 41) * ratio)      # #2952E8 -> #6366F1
        g = int(82 + (102 - 82) * ratio)
        b = int(232 + (241 - 232) * ratio)
        
        draw.ellipse(
            [center - radius, center - radius, center + radius, center + radius],
            fill=(r, g, b, 255)
        )
    
    # === 2. 极简搜索图标 - 细线条设计 ===
    mag_radius = int(render_size * 0.20)
    mag_thickness = max(5, render_size // 30)  # 更细的线条
    
    # 放大镜圆圈（纯白，无装饰）
    draw.ellipse(
        [center - mag_radius, center - mag_radius,
         center + mag_radius, center + mag_radius],
        outline=(255, 255, 255, 255),
        width=mag_thickness
    )
    
    # === 3. 极简手柄 ===
    import math
    angle = math.radians(45)
    handle_start_x = center + int(mag_radius * math.cos(angle))
    handle_start_y = center + int(mag_radius * math.sin(angle))
    handle_length = int(render_size * 0.24)
    handle_end_x = handle_start_x + int(handle_length * math.cos(angle))
    handle_end_y = handle_start_y + int(handle_length * math.sin(angle))
    
    # 手柄（纯白直线）
    draw.line(
        [(handle_start_x, handle_start_y), (handle_end_x, handle_end_y)],
        fill=(255, 255, 255, 255),
        width=mag_thickness
    )
    
    # === 4. 科幻点缀：扫描线效果（右上角） ===
    scan_x = center + int(render_size * 0.28)
    scan_y = center - int(render_size * 0.28)
    scan_size = int(render_size * 0.12)
    
    # 绘制简洁的扫描圆环
    for i in range(3):
        ring_radius = scan_size - i * (scan_size // 4)
        alpha = 180 - i * 60
        draw.ellipse(
            [scan_x - ring_radius, scan_y - ring_radius,
             scan_x + ring_radius, scan_y + ring_radius],
            outline=(100, 255, 218, alpha),  # 青色 #64FFDA
            width=3
        )
    
    # === 5. 科幻光点 ===
    # 右下角光点
    light_positions = [
        (center + int(render_size * 0.32), center + int(render_size * 0.20)),
        (center - int(render_size * 0.30), center - int(render_size * 0.32)),
        (center + int(render_size * 0.10), center - int(render_size * 0.38)),
    ]
    
    for x, y in light_positions:
        dot_size = 4
        # 发光效果（多层透明圆）
        for j in range(3):
            r = dot_size * (3 - j)
            alpha = 100 // (j + 1)
            draw.ellipse(
                [x - r, y - r, x + r, y + r],
                fill=(100, 255, 218, alpha)
            )
    
    # === 6. 数字感装饰线 ===
    # 左侧竖线
    line_x = center - int(render_size * 0.34)
    line_top = center - int(render_size * 0.15)
    line_bottom = center + int(render_size * 0.15)
    draw.line(
        [(line_x, line_top), (line_x, line_bottom)],
        fill=(255, 255, 255, 60),
        width=2
    )
    
    # 右侧短线组
    for i in range(3):
        line_x = center + int(render_size * 0.30)
        line_y = center - int(render_size * 0.10) + i * int(render_size * 0.10)
        line_length = 35 - i * 8
        draw.line(
            [(line_x, line_y), (line_x + line_length, line_y)],
            fill=(255, 255, 255, 50),
            width=2
        )
    
    # === 7. 缩小到目标尺寸（抗锯齿） ===
    img = img.resize((size, size), Image.Resampling.LANCZOS)
    
    return img

def create_ico_file(png_path, ico_path):
    """创建 ICO 文件"""
    img = Image.open(png_path)
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    
    images = []
    for size in sizes:
        resized = img.resize(size, Image.Resampling.LANCZOS)
        images.append(resized)
    
    images[0].save(ico_path, format='ICO', sizes=sizes, append_images=images[1:])

def create_icns_file(png_path, icns_path):
    """创建 ICNS 文件"""
    img = Image.open(png_path)
    
    icon_types = [
        (512, b'ic09'),
        (256, b'ic08'),
        (128, b'ic07'),
        (32, b'ic11'),
        (16, b'ic04'),
    ]
    
    icon_data = []
    
    for size, type_code in icon_types:
        resized = img.resize((size, size), Image.Resampling.LANCZOS)
        
        png_buffer = io.BytesIO()
        resized.save(png_buffer, format='PNG')
        png_bytes = png_buffer.getvalue()
        
        chunk_size = 8 + len(png_bytes)
        chunk = type_code + struct.pack('>I', chunk_size) + png_bytes
        icon_data.append(chunk)
    
    with open(icns_path, 'wb') as f:
        total_size = 8 + sum(len(chunk) for chunk in icon_data)
        f.write(b'icns')
        f.write(struct.pack('>I', total_size))
        for chunk in icon_data:
            f.write(chunk)

def main():
    icons_dir = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'icons')
    
    print("🎨 生成专业手绘图标...\n")
    
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
        icon = create_modern_icon(size)
        output_path = os.path.join(icons_dir, filename)
        icon.save(output_path, 'PNG')
        print(f"  ✓ {filename} ({size}x{size})")
    
    print("\n🪟 生成 Windows ICO...")
    base_icon_path = os.path.join(icons_dir, 'icon.png')
    ico_path = os.path.join(icons_dir, 'icon.ico')
    create_ico_file(base_icon_path, ico_path)
    print(f"  ✓ icon.ico")
    
    print("\n🍎 生成 macOS ICNS...")
    icns_path = os.path.join(icons_dir, 'icon.icns')
    create_icns_file(base_icon_path, icns_path)
    print(f"  ✓ icon.icns")
    
    print("\n✨ 专业图标生成完成！")
    print(f"📁 输出目录: {icons_dir}")

if __name__ == '__main__':
    main()
