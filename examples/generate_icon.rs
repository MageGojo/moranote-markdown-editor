use image::{Rgba, RgbaImage, imageops::FilterType};
use std::fs;
use std::path::PathBuf;

const MASTER: u32 = 1024;
const ICON_DIR: &str = "assets/app-icons";
const ICONSET: &str = "assets/app-icons/MoraNote.iconset";

fn main() -> anyhow::Result<()> {
    fs::create_dir_all(ICONSET)?;

    let master = render_master_icon();
    master.save(PathBuf::from(ICON_DIR).join("moranote-1024.png"))?;

    let entries = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];

    for (name, size) in entries {
        let resized = image::imageops::resize(&master, size, size, FilterType::Lanczos3);
        resized.save(PathBuf::from(ICONSET).join(name))?;
    }

    Ok(())
}

fn render_master_icon() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(MASTER, MASTER, Rgba([0, 0, 0, 0]));

    for layer in 0..18 {
        let inset = 74 + layer * 2;
        let alpha = (32 - layer).max(2) as u8;
        draw_rounded_rect(
            &mut image,
            inset,
            92 + layer,
            MASTER - inset,
            MASTER - 58 + layer,
            184,
            [52, 64, 50, alpha],
        );
    }

    draw_rounded_gradient_rect(
        &mut image,
        84,
        70,
        940,
        928,
        182,
        [251, 253, 251, 255],
        [221, 230, 223, 255],
    );
    draw_rounded_rect(&mut image, 84, 70, 940, 928, 182, [126, 163, 136, 36]);

    for layer in 0..12 {
        draw_rounded_rect(
            &mut image,
            246 + layer,
            190 + layer * 2,
            814 + layer,
            846 + layer * 2,
            58,
            [58, 49, 45, (18 - layer) as u8],
        );
    }

    draw_rounded_gradient_rect(
        &mut image,
        232,
        168,
        792,
        820,
        54,
        [255, 253, 247, 255],
        [238, 244, 239, 255],
    );
    draw_rounded_rect(&mut image, 232, 168, 792, 820, 54, [126, 163, 136, 42]);
    draw_polygon(
        &mut image,
        &[(666, 168), (792, 296), (666, 296)],
        [221, 230, 223, 255],
    );
    draw_line(&mut image, 666, 168, 792, 296, 10.0, [126, 163, 136, 48]);

    draw_rounded_rect(&mut image, 282, 168, 326, 820, 22, [80, 105, 86, 255]);
    draw_rounded_rect(&mut image, 352, 612, 674, 654, 18, [122, 95, 82, 230]);

    let green = [80, 105, 86, 255];
    draw_rounded_rect(&mut image, 360, 398, 410, 578, 22, green);
    draw_rounded_rect(&mut image, 558, 398, 608, 578, 22, green);
    draw_line(&mut image, 406, 404, 484, 542, 50.0, green);
    draw_line(&mut image, 562, 404, 484, 542, 50.0, green);

    let brown = [122, 95, 82, 255];
    draw_line(&mut image, 672, 392, 672, 544, 52.0, brown);
    draw_polygon(&mut image, &[(612, 530), (732, 530), (672, 616)], brown);

    draw_rounded_rect(&mut image, 370, 690, 654, 722, 16, [122, 95, 82, 180]);
    draw_rounded_rect(&mut image, 370, 744, 592, 770, 13, [111, 127, 136, 150]);

    image
}

fn draw_rounded_gradient_rect(
    image: &mut RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    radius: u32,
    top: [u8; 4],
    bottom: [u8; 4],
) {
    for y in y0..y1 {
        let t = (y - y0) as f32 / (y1 - y0).max(1) as f32;
        let color = [
            lerp(top[0], bottom[0], t),
            lerp(top[1], bottom[1], t),
            lerp(top[2], bottom[2], t),
            lerp(top[3], bottom[3], t),
        ];
        for x in x0..x1 {
            if inside_rounded_rect(x, y, x0, y0, x1, y1, radius) {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn draw_rounded_rect(
    image: &mut RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    radius: u32,
    color: [u8; 4],
) {
    for y in y0..y1 {
        for x in x0..x1 {
            if inside_rounded_rect(x, y, x0, y0, x1, y1, radius) {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn inside_rounded_rect(x: u32, y: u32, x0: u32, y0: u32, x1: u32, y1: u32, radius: u32) -> bool {
    let max_radius = (x1 - x0).saturating_sub(1).min((y1 - y0).saturating_sub(1)) / 2;
    let r = radius.min(max_radius) as i64;
    let xi = x as i64;
    let yi = y as i64;
    let left = x0 as i64 + r;
    let right = x1 as i64 - r - 1;
    let top = y0 as i64 + r;
    let bottom = y1 as i64 - r - 1;
    let cx = xi.clamp(left, right);
    let cy = yi.clamp(top, bottom);
    let dx = xi - cx;
    let dy = yi - cy;
    dx * dx + dy * dy <= r * r
}

fn draw_line(
    image: &mut RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    width: f32,
    color: [u8; 4],
) {
    let min_x = x0.min(x1).saturating_sub(width as u32 + 2);
    let max_x = (x0.max(x1) + width as u32 + 2).min(MASTER - 1);
    let min_y = y0.min(y1).saturating_sub(width as u32 + 2);
    let max_y = (y0.max(y1) + width as u32 + 2).min(MASTER - 1);
    let radius = width / 2.0;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let distance = distance_to_segment(
                x as f32 + 0.5,
                y as f32 + 0.5,
                x0 as f32,
                y0 as f32,
                x1 as f32,
                y1 as f32,
            );
            let coverage = (radius + 1.0 - distance).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let mut c = color;
                c[3] = (c[3] as f32 * coverage) as u8;
                blend_pixel(image, x, y, c);
            }
        }
    }
}

fn draw_polygon(image: &mut RgbaImage, points: &[(u32, u32)], color: [u8; 4]) {
    let min_x = points.iter().map(|point| point.0).min().unwrap_or(0);
    let max_x = points.iter().map(|point| point.0).max().unwrap_or(0);
    let min_y = points.iter().map(|point| point.1).min().unwrap_or(0);
    let max_y = points.iter().map(|point| point.1).max().unwrap_or(0);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if point_in_polygon(x as f32 + 0.5, y as f32 + 0.5, points) {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn point_in_polygon(x: f32, y: f32, points: &[(u32, u32)]) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for current in 0..points.len() {
        let (xi, yi) = (points[current].0 as f32, points[current].1 as f32);
        let (xj, yj) = (points[previous].0 as f32, points[previous].1 as f32);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn distance_to_segment(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let length_sq = dx * dx + dy * dy;
    if length_sq == 0.0 {
        return ((px - x0).powi(2) + (py - y0).powi(2)).sqrt();
    }
    let t = (((px - x0) * dx + (py - y0) * dy) / length_sq).clamp(0.0, 1.0);
    let cx = x0 + t * dx;
    let cy = y0 + t * dy;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, source: [u8; 4]) {
    let destination = image.get_pixel_mut(x, y);
    let alpha = source[3] as f32 / 255.0;
    let inverse = 1.0 - alpha;
    for channel in 0..3 {
        destination[channel] =
            (source[channel] as f32 * alpha + destination[channel] as f32 * inverse) as u8;
    }
    destination[3] = (source[3] as f32 + destination[3] as f32 * inverse).min(255.0) as u8;
}

fn lerp(start: u8, end: u8, t: f32) -> u8 {
    (start as f32 + (end as f32 - start as f32) * t) as u8
}
