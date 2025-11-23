"""
简化版：使用 Pillow 直接绘制高质量图标
不依赖 cairosvg，纯 Python 实现
"""

from PIL import Image, ImageDraw, ImageFilter
import os
import struct
import io

def create_modern_icon(size):
    """创建现代化的 iLauncher 图标 - 专业手绘版"""
    
    # 创建高分辨率画布（2倍采样，最后缩小以获得抗锯齿效果）
    render_size = size * 2
    img = Image.new('RGBA', (render_size, render_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    center = render_size // 2
    
    # === 1. 绘制渐变背景圆形 ===
    # 创建紫蓝色渐变
    for i in range(100):
        ratio = i / 100
        radius = int((render_size // 2 - render_size // 10) * (1 - ratio * 0.3))
        
        # 从深紫色到浅蓝紫色
        r = int(102 + (139 - 102) * ratio)    # 102 -> 139
        g = int(126 + (158 - 126) * ratio)    # 126 -> 158
        b = int(234 + (250 - 234) * ratio)    # 234 -> 250
        
        draw.ellipse(
            [center - radius, center - radius, center + radius, center + radius],
            fill=(r, g, b, 255)
        )
    
    # === 2. 添加外发光效果 ===
    glow_radius = render_size // 2 - render_size // 12
    draw.ellipse(
        [center - glow_radius, center - glow_radius, 
         center + glow_radius, center + glow_radius],
        outline=(255, 255, 255, 30),
        width=6
    )
    
    # === 3. 绘制放大镜主体 ===
    mag_offset_x = -render_size // 16
    mag_offset_y = -render_size // 16
    mag_center_x = center + mag_offset_x
    mag_center_y = center + mag_offset_y
    mag_radius = int(render_size * 0.22)
    mag_thickness = max(6, render_size // 24)
    
    # 放大镜镜片外圈（深色边框）
    draw.ellipse(
        [mag_center_x - mag_radius - 2, mag_center_y - mag_radius - 2,
         mag_center_x + mag_radius + 2, mag_center_y + mag_radius + 2],
        outline=(200, 200, 200, 100),
        width=2
    )
    
    # 放大镜镜片主体
    draw.ellipse(
        [mag_center_x - mag_radius, mag_center_y - mag_radius,
         mag_center_x + mag_radius, mag_center_y + mag_radius],
        outline=(255, 255, 255, 255),
        width=mag_thickness
    )
    
    # 镜片内部高光
    highlight_r = mag_radius // 3
    highlight_x = mag_center_x - mag_radius // 3
    highlight_y = mag_center_y - mag_radius // 3
    draw.ellipse(
        [highlight_x - highlight_r, highlight_y - highlight_r,
         highlight_x + highlight_r, highlight_y + highlight_r],
        fill=(255, 255, 255, 100)
    )
    
    # === 4. 绘制放大镜手柄 ===
    import math
    angle = math.radians(45)
    handle_start_x = mag_center_x + int(mag_radius * math.cos(angle))
    handle_start_y = mag_center_y + int(mag_radius * math.sin(angle))
    handle_length = int(render_size * 0.28)
    handle_end_x = handle_start_x + int(handle_length * math.cos(angle))
    handle_end_y = handle_start_y + int(handle_length * math.sin(angle))
    
    # 手柄阴影
    draw.line(
        [(handle_start_x + 3, handle_start_y + 3), 
         (handle_end_x + 3, handle_end_y + 3)],
        fill=(0, 0, 0, 50),
        width=mag_thickness + 2
    )
    
    # 手柄主体
    draw.line(
        [(handle_start_x, handle_start_y), (handle_end_x, handle_end_y)],
        fill=(255, 255, 255, 255),
        width=mag_thickness
    )
    
    # 手柄末端圆点
    cap_radius = mag_thickness // 2 + 2
    draw.ellipse(
        [handle_end_x - cap_radius, handle_end_y - cap_radius,
         handle_end_x + cap_radius, handle_end_y + cap_radius],
        fill=(255, 255, 255, 255)
    )
    
    # === 5. 装饰性元素：星星 ===
    # 右上角星星
    star_x = center + int(render_size * 0.30)
    star_y = center - int(render_size * 0.32)
    star_size = render_size // 20
    
    # 绘制四角星
    star_points = []
    for i in range(8):
        angle = math.radians(i * 45)
        if i % 2 == 0:
            r = star_size
        else:
            r = star_size // 3
        x = star_x + int(r * math.cos(angle))
        y = star_y + int(r * math.sin(angle))
        star_points.append((x, y))
    
    draw.polygon(star_points, fill=(255, 215, 0, 220))  # 金色
    
    # 左上角小星星
    small_star_x = center - int(render_size * 0.28)
    small_star_y = center - int(render_size * 0.30)
    small_star_size = render_size // 30
    
    star_points2 = []
    for i in range(8):
        angle = math.radians(i * 45)
        if i % 2 == 0:
            r = small_star_size
        else:
            r = small_star_size // 3
        x = small_star_x + int(r * math.cos(angle))
        y = small_star_y + int(r * math.sin(angle))
        star_points2.append((x, y))
    
    draw.polygon(star_points2, fill=(255, 215, 0, 180))
    
    # === 6. 速度线条（右侧） ===
    line_x = center + int(render_size * 0.25)
    line_start_y = center - int(render_size * 0.08)
    line_spacing = int(render_size * 0.08)
    
    for i in range(3):
        y = line_start_y + i * line_spacing
        line_length = [50, 60, 45][i]
        draw.line(
            [(line_x, y), (line_x + line_length, y)],
            fill=(255, 255, 255, 40),
            width=6
        )
    
    # === 7. 应用轻微模糊以获得更柔和的效果 ===
    img = img.filter(ImageFilter.SMOOTH)
    
    # === 8. 缩小到目标尺寸（抗锯齿） ===
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
