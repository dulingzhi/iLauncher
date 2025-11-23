"""
iLauncher 图标生成器
设计理念：现代化的启动器图标，结合搜索和火箭元素
"""

from PIL import Image, ImageDraw, ImageFont
import os

def create_ilauncher_icon(size):
    """创建 iLauncher 图标 - 极简现代风格
    
    设计理念：
    - 简洁的渐变圆形背景
    - 优雅的搜索图标
    - 闪电符号代表快速
    """
    # 创建透明背景
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    center = size // 2
    padding = size // 10
    
    # === 背景：现代渐变圆形 ===
    # 创建从中心向外的径向渐变效果
    for i in range(100, 0, -1):
        ratio = i / 100
        radius = int((size // 2 - padding) * ratio)
        
        # 从深蓝紫色渐变到亮蓝色
        r = int(59 + (96 - 59) * (1 - ratio))      # 59 -> 96
        g = int(130 + (165 - 130) * (1 - ratio))   # 130 -> 165
        b = int(246 + (250 - 246) * (1 - ratio))   # 246 -> 250
        alpha = 255
        
        draw.ellipse(
            [center - radius, center - radius, center + radius, center + radius],
            fill=(r, g, b, alpha)
        )
    
    # === 主图标：优雅的搜索放大镜 ===
    # 放大镜参数
    mag_radius = int(size * 0.22)
    mag_thickness = max(3, size // 24)
    mag_center_offset = -int(size * 0.06)
    
    # 放大镜镜片（圆环）
    for thickness in range(mag_thickness):
        draw.ellipse(
            [center + mag_center_offset - mag_radius + thickness,
             center + mag_center_offset - mag_radius + thickness,
             center + mag_center_offset + mag_radius - thickness,
             center + mag_center_offset + mag_radius - thickness],
            outline=(255, 255, 255, 255),
            width=1
        )
    
    # 放大镜手柄（圆角矩形）
    handle_length = int(size * 0.25)
    handle_width = mag_thickness
    handle_start_angle = 45  # 45度角
    
    import math
    angle_rad = math.radians(handle_start_angle)
    handle_start_x = center + mag_center_offset + int(mag_radius * math.cos(angle_rad))
    handle_start_y = center + mag_center_offset + int(mag_radius * math.sin(angle_rad))
    handle_end_x = handle_start_x + int(handle_length * math.cos(angle_rad))
    handle_end_y = handle_start_y + int(handle_length * math.sin(angle_rad))
    
    # 绘制圆润的手柄
    draw.line(
        [(handle_start_x, handle_start_y), (handle_end_x, handle_end_y)],
        fill=(255, 255, 255, 255),
        width=handle_width
    )
    
    # 手柄末端圆点（圆润效果）
    cap_radius = handle_width // 2
    draw.ellipse(
        [handle_end_x - cap_radius, handle_end_y - cap_radius,
         handle_end_x + cap_radius, handle_end_y + cap_radius],
        fill=(255, 255, 255, 255)
    )
    
    # === 点缀元素：闪电符号（代表快速） ===
    lightning_size = int(size * 0.15)
    lightning_x = center + int(size * 0.25)
    lightning_y = center - int(size * 0.25)
    
    # 绘制简化的闪电图标
    lightning_points = [
        (lightning_x, lightning_y - lightning_size // 3),
        (lightning_x - lightning_size // 4, lightning_y),
        (lightning_x, lightning_y),
        (lightning_x - lightning_size // 5, lightning_y + lightning_size // 3),
    ]
    
    # 使用渐变金色
    draw.polygon(
        [
            (lightning_x + 1, lightning_y - lightning_size // 2),
            (lightning_x - lightning_size // 3, lightning_y + 2),
            (lightning_x + 3, lightning_y + 2),
            (lightning_x - lightning_size // 4, lightning_y + lightning_size // 2),
        ],
        fill=(255, 215, 0, 255)  # 金色 #FFD700
    )
    
    # 添加高光效果（小白点）
    highlight_size = max(2, size // 40)
    highlight_x = center + mag_center_offset - int(mag_radius * 0.4)
    highlight_y = center + mag_center_offset - int(mag_radius * 0.4)
    
    draw.ellipse(
        [highlight_x - highlight_size, highlight_y - highlight_size,
         highlight_x + highlight_size, highlight_y + highlight_size],
        fill=(255, 255, 255, 180)
    )
    
    return img

def create_ico_file(png_path, ico_path):
    """将 PNG 转换为 ICO（多尺寸）"""
    img = Image.open(png_path)
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    
    # 创建多尺寸图标
    icon_images = []
    for size in sizes:
        resized = img.resize(size, Image.Resampling.LANCZOS)
        icon_images.append(resized)
    
    # 保存为 ICO
    icon_images[0].save(ico_path, format='ICO', sizes=sizes, append_images=icon_images[1:])

def main():
    # 输出目录
    icons_dir = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'icons')
    os.makedirs(icons_dir, exist_ok=True)
    
    print("🎨 开始生成 iLauncher 图标...")
    
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
    
    for filename, size in sizes.items():
        icon = create_ilauncher_icon(size)
        output_path = os.path.join(icons_dir, filename)
        icon.save(output_path, 'PNG')
        print(f"✓ 生成 {filename} ({size}x{size})")
    
    # 生成 ICO 文件（Windows）
    print("\n🪟 生成 Windows ICO 文件...")
    base_icon_path = os.path.join(icons_dir, 'icon.png')
    ico_path = os.path.join(icons_dir, 'icon.ico')
    create_ico_file(base_icon_path, ico_path)
    print(f"✓ 生成 icon.ico")
    
    # 生成 ICNS 文件（macOS）需要额外工具
    print("\n🍎 macOS ICNS 文件需要手动转换：")
    print("   方法1: 使用在线工具 https://cloudconvert.com/png-to-icns")
    print("   方法2: macOS 上运行: iconutil -c icns icon.iconset")
    print(f"   上传文件: {base_icon_path}")
    
    print("\n✨ 图标生成完成！")
    print(f"📁 输出目录: {icons_dir}")

if __name__ == '__main__':
    main()
