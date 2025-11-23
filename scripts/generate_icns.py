"""
生成 macOS ICNS 文件
使用简单的二进制打包方式生成 ICNS（跨平台兼容）
"""

from PIL import Image
import os
import struct
import io

def create_icns_manually(png_path, icns_path):
    """手动创建 ICNS 文件（跨平台）"""
    
    img = Image.open(png_path)
    
    # ICNS 需要的标准尺寸和类型代码
    icon_types = [
        (512, b'ic09'),  # 512x512
        (256, b'ic08'),  # 256x256
        (128, b'ic07'),  # 128x128
        (32, b'ic11'),   # 32x32 (retina)
        (16, b'ic04'),   # 16x16
    ]
    
    # 存储所有图标数据
    icon_data = []
    
    print(f"🔨 手动打包 ICNS 文件...")
    
    for size, type_code in icon_types:
        # 调整图片大小
        resized = img.resize((size, size), Image.Resampling.LANCZOS)
        
        # 转换为 PNG 字节流
        png_buffer = io.BytesIO()
        resized.save(png_buffer, format='PNG')
        png_bytes = png_buffer.getvalue()
        
        # ICNS 块格式：4字节类型 + 4字节长度 + 数据
        chunk_size = 8 + len(png_bytes)
        chunk = type_code + struct.pack('>I', chunk_size) + png_bytes
        
        icon_data.append(chunk)
        print(f"  ✓ {size}x{size} ({type_code.decode()}) - {len(png_bytes)} bytes")
    
    # 写入 ICNS 文件
    with open(icns_path, 'wb') as f:
        # ICNS 文件头：'icns' + 总大小
        total_size = 8 + sum(len(chunk) for chunk in icon_data)
        f.write(b'icns')
        f.write(struct.pack('>I', total_size))
        
        # 写入所有图标块
        for chunk in icon_data:
            f.write(chunk)
    
    print(f"\n✅ 成功生成 ICNS: {icns_path}")
    print(f"   文件大小: {total_size} bytes")

def main():
    icons_dir = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'icons')
    png_path = os.path.join(icons_dir, 'icon.png')
    icns_path = os.path.join(icons_dir, 'icon.icns')
    
    print("🍎 生成 macOS ICNS 文件（跨平台方法）\n")
    create_icns_manually(png_path, icns_path)
    print("\n✨ 完成！")

if __name__ == '__main__':
    main()
