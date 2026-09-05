//! 托盘图标：程序化生成（仓库无 ico/png 资源，而 tray-icon 只收 RGBA 像素）。
//! 128×128 放大镜标志，SDF 距离场抗锯齿，Windows 托盘按通知区尺寸自行缩放。

/// 边长（正方形）
pub const SIZE: u32 = 128;
/// 品牌底色（iLauncher 蓝）
const BG: [u8; 3] = [47, 111, 246];
/// 标志前景色（白）
const FG: [u8; 3] = [255, 255, 255];
/// 圆角半径
const CORNER_RADIUS: f32 = 26.0;
/// 圆角矩形外边距
const MARGIN: f32 = 4.0;
/// 镜片圆心
const LENS_CENTER: (f32, f32) = (56.0, 56.0);
/// 镜片圆环半径
const LENS_RADIUS: f32 = 26.0;
/// 镜片圆环厚度
const LENS_THICKNESS: f32 = 11.0;
/// 手柄线段（两端点）
const HANDLE_A: (f32, f32) = (76.0, 76.0);
const HANDLE_B: (f32, f32) = (98.0, 98.0);
/// 手柄粗细（胶囊半径 = 粗细/2）
const HANDLE_RADIUS: f32 = 6.0;

/// 1px 宽的平滑过渡：d<0 内部 → 1，d>0 外部 → 0
fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// 点到圆角矩形边的有符号距离（负值在内部）
fn rounded_rect_sdf(x: f32, y: f32) -> f32 {
    let half = SIZE as f32 / 2.0 - MARGIN;
    let cx = SIZE as f32 / 2.0;
    let qx = (x - cx).abs() - (half - CORNER_RADIUS);
    let qy = (y - cx).abs() - (half - CORNER_RADIUS);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    qx.max(qy).min(0.0) + outside - CORNER_RADIUS
}

/// 点到线段的有符号距离（胶囊）
fn segment_sdf(x: f32, y: f32, (ax, ay): (f32, f32), (bx, by): (f32, f32)) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    let t = (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0);
    let px = ax + t * dx - x;
    let py = ay + t * dy - y;
    px.hypot(py)
}

/// 生成 128×128 RGBA 像素
pub fn rgba_pixels() -> Vec<u8> {
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let bg_a = coverage(rounded_rect_sdf(fx, fy));
            if bg_a <= 0.0 {
                continue;
            }
            // 圆环：到圆心距离与环半径的偏差落在厚度半径内
            let ring_d = (fx - LENS_CENTER.0).hypot(fy - LENS_CENTER.1) - LENS_RADIUS;
            let ring = coverage(ring_d.abs() - LENS_THICKNESS / 2.0);
            let handle = coverage(segment_sdf(fx, fy, HANDLE_A, HANDLE_B) - HANDLE_RADIUS);
            let fg_a = ring.max(handle).min(bg_a);
            let i = ((y * SIZE + x) * 4) as usize;
            let (r, g, b) = if fg_a > 0.0 { (FG[0], FG[1], FG[2]) } else { (BG[0], BG[1], BG[2]) };
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = (bg_a.max(fg_a) * 255.0) as u8;
        }
    }
    rgba
}

/// 构建 tray-icon 图标
pub fn build() -> tray_icon::Icon {
    tray_icon::Icon::from_rgba(rgba_pixels(), SIZE, SIZE).expect("托盘图标 RGBA 尺寸合法")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_have_expected_size() {
        assert_eq!(rgba_pixels().len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn corner_is_transparent() {
        let px = rgba_pixels();
        // (2,2) 远离圆角矩形 → 全透明
        assert_eq!(px[3], 0, "左上角应透明");
    }

    #[test]
    fn background_is_brand_blue() {
        let px = rgba_pixels();
        // (20,64)：矩形内、镜片与手柄之外 → 品牌蓝不透明
        let i = ((64 * SIZE + 20) * 4) as usize;
        assert_eq!(px[i + 3], 255, "矩形内部应不透明");
        assert_eq!((px[i], px[i + 1], px[i + 2]), (BG[0], BG[1], BG[2]));
    }

    #[test]
    fn lens_ring_is_white() {
        let px = rgba_pixels();
        // (56, 30)：镜片圆环正上方（距离圆心 26 = 环半径）→ 白色
        let i = ((30 * SIZE + 56) * 4) as usize;
        assert_eq!(px[i + 3], 255, "圆环应不透明");
        assert_eq!((px[i], px[i + 1], px[i + 2]), (FG[0], FG[1], FG[2]));
    }

    #[test]
    fn lens_interior_is_blue() {
        let px = rgba_pixels();
        // (56,56)：镜片中心（环内）→ 透出品牌蓝
        let i = ((56 * SIZE + 56) * 4) as usize;
        assert_eq!((px[i], px[i + 1], px[i + 2]), (BG[0], BG[1], BG[2]));
    }

    #[test]
    fn handle_is_white() {
        let px = rgba_pixels();
        // (87,87)：手柄线段中点 → 白色
        let i = ((87 * SIZE + 87) * 4) as usize;
        assert_eq!((px[i], px[i + 1], px[i + 2]), (FG[0], FG[1], FG[2]));
    }
}
