use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;

#[allow(dead_code)]
pub struct IconManager {
    pub app_icon: Option<TextureHandle>,
    pub add_icon: Option<TextureHandle>,
    pub extract_icon: Option<TextureHandle>,
    pub test_icon: Option<TextureHandle>,
    pub view_icon: Option<TextureHandle>,
    pub find_icon: Option<TextureHandle>,
    pub repair_icon: Option<TextureHandle>,
    pub convert_icon: Option<TextureHandle>,
    pub report_icon: Option<TextureHandle>,
    pub scan_icon: Option<TextureHandle>,
    pub benchmark_icon: Option<TextureHandle>,
    pub info_icon: Option<TextureHandle>,
    pub delete_icon: Option<TextureHandle>,
    pub folder_icon: Option<TextureHandle>,
    pub file_icon: Option<TextureHandle>,
    pub archive_icon: Option<TextureHandle>,
    pub up_icon: Option<TextureHandle>,
    pub refresh_icon: Option<TextureHandle>,
    pub lock_icon: Option<TextureHandle>,
    pub comment_icon: Option<TextureHandle>,
    pub banner_icon: Option<TextureHandle>,
}

impl IconManager {
    pub fn new(ctx: &Context) -> Self {
        Self {
            app_icon: Self::load_texture_from_bytes(ctx, "app_icon", include_bytes!("../../assets/icons/linrar.png")),
            add_icon: Self::load_texture_from_bytes(ctx, "add_icon", include_bytes!("../../assets/icons/add.png")),
            extract_icon: Self::load_texture_from_bytes(ctx, "extract_icon", include_bytes!("../../assets/icons/extract.png")),
            test_icon: Self::load_texture_from_bytes(ctx, "test_icon", include_bytes!("../../assets/icons/test.png")),
            view_icon: Self::load_texture_from_bytes(ctx, "view_icon", include_bytes!("../../assets/icons/view.png")),
            find_icon: Self::load_texture_from_bytes(ctx, "find_icon", include_bytes!("../../assets/icons/find.png")),
            repair_icon: Self::load_texture_from_bytes(ctx, "repair_icon", include_bytes!("../../assets/icons/repair.png")),
            convert_icon: Self::load_texture_from_bytes(ctx, "convert_icon", include_bytes!("../../assets/icons/convert.png")),
            report_icon: Self::load_texture_from_bytes(ctx, "report_icon", include_bytes!("../../assets/icons/report.png")),
            scan_icon: Self::load_texture_from_bytes(ctx, "scan_icon", include_bytes!("../../assets/icons/virus.png")),
            benchmark_icon: Self::load_texture_from_bytes(ctx, "benchmark_icon", include_bytes!("../../assets/icons/benchmark.png")),
            info_icon: Self::load_texture_from_bytes(ctx, "info_icon", include_bytes!("../../assets/icons/info.png")),
            delete_icon: Self::load_texture_from_bytes(ctx, "delete_icon", include_bytes!("../../assets/icons/delete.png")),
            folder_icon: Self::load_texture_from_bytes(ctx, "folder_icon", include_bytes!("../../assets/icons/folder.png")),
            file_icon: Self::load_texture_from_bytes(ctx, "file_icon", include_bytes!("../../assets/icons/file.png")),
            archive_icon: Self::load_texture_from_bytes(ctx, "archive_icon", include_bytes!("../../assets/icons/archive.png")),
            up_icon: Self::load_texture_from_bytes(ctx, "up_icon", include_bytes!("../../assets/icons/up.png")),
            refresh_icon: Self::load_texture_from_bytes(ctx, "refresh_icon", include_bytes!("../../assets/icons/refresh.png")),
            lock_icon: Self::load_texture_from_bytes(ctx, "lock_icon", include_bytes!("../../assets/icons/lock.png")),
            comment_icon: Self::load_texture_from_bytes(ctx, "comment_icon", include_bytes!("../../assets/icons/comment.png")),
            banner_icon: Self::load_texture_from_bytes(ctx, "banner_icon", include_bytes!("../../assets/icons/winrar_banner.png")),
        }
    }

    fn load_texture_from_bytes(ctx: &Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
        let img = image::load_from_memory(bytes).ok()?;
        let (width, height) = img.dimensions();
        let rgba = img.to_rgba8().into_raw();
        let color_image = ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
        Some(ctx.load_texture(name, color_image, TextureOptions::LINEAR))
    }
}
