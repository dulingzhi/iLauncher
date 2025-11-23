"""
iLauncher 图标生成器
设计理念：现代化的启动器图标，结合搜索和火箭元素
"""

from PIL import Image, ImageDraw, ImageFont
import os

def create_ilauncher_icon(size):
    """创建 iLauncher 图标
    
    设计元素：
    - 渐变蓝色圆形背景（象征搜索框）
    - 白色放大镜图标（搜索功能）
    - 火箭元素融入（快速启动）
    """
    # 创建透明背景
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    # 计算中心和边距
    center = size // 2
    padding = size // 8
    
    # === 背景：渐变蓝色圆形 ===
    # 主圆形（深蓝色到浅蓝色渐变效果通过多层实现）
    circle_radius = size // 2 - padding
    
    # 外层光晕（浅蓝色）
    draw.ellipse(
        [center - circle_radius, center - circle_radius,
         center + circle_radius, center + circle_radius],
        fill=(66, 153, 225, 255)  # 明亮的蓝色 #4299E1
    )
    
    # 内层阴影效果
    inner_radius = circle_radius - size // 20
    draw.ellipse(
        [center - inner_radius, center - inner_radius,
         center + inner_radius, center + inner_radius],
        fill=(56, 178, 172, 255)  # 青色 #38B2AC
    )
    
    # === 前景：搜索图标设计 ===
    # 放大镜圆圈
    mag_center_x = center - size // 12
    mag_center_y = center - size // 12
    mag_radius = size // 5
    mag_thickness = max(2, size // 32)
    
    # 绘制放大镜圆圈（白色）
    for i in range(mag_thickness):
        draw.ellipse(
            [mag_center_x - mag_radius + i, mag_center_y - mag_radius + i,
             mag_center_x + mag_radius - i, mag_center_y + mag_radius - i],
            outline=(255, 255, 255, 255),
            width=1
        )
    
    # 绘制放大镜手柄（从右下角延伸）
    handle_start_x = mag_center_x + int(mag_radius * 0.707)
    handle_start_y = mag_center_y + int(mag_radius * 0.707)
    handle_end_x = center + size // 5
    handle_end_y = center + size // 5
    
    draw.line(
        [handle_start_x, handle_start_y, handle_end_x, handle_end_y],
        fill=(255, 255, 255, 255),
        width=mag_thickness
    )
    
    # === 点缀：小火箭元素（右上角） ===
    rocket_size = size // 6
    rocket_x = center + size // 4
    rocket_y = center - size // 3
    
    # 火箭主体（三角形）
    rocket_points = [
        (rocket_x, rocket_y - rocket_size // 2),  # 顶部
        (rocket_x - rocket_size // 4, rocket_y + rocket_size // 4),  # 左下
        (rocket_x + rocket_size // 4, rocket_y + rocket_size // 4),  # 右下
    ]
    draw.polygon(rocket_points, fill=(255, 223, 0, 255))  # 金黄色 #FFDF00
    
    # 火箭尾焰（小圆点）
    flame_y = rocket_y + rocket_size // 3
    draw.ellipse(
        [rocket_x - 2, flame_y - 2, rocket_x + 2, flame_y + 2],
        fill=(255, 107, 107, 255)  # 红色火焰
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
