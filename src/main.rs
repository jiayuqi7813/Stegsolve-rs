mod transform;
mod fileanalysis;
mod extractanlysis;
mod stereo;
mod framebrowser;  
mod combine;


use gio::prelude::*;
use gio::{Menu as GMenu, SimpleAction};
use gtk::{prelude::*, ButtonsType, MessageDialog, MessageType,AlertDialog};
use gtk::{
    Application, ApplicationWindow, Button, HeaderBar, Label, MenuButton, 
    Orientation, Scale, ScrolledWindow, Picture, Widget, 
    Adjustment, PolicyType,
};
use gdk_pixbuf::Pixbuf;
use glib::Bytes;
use std::cell::RefCell;
use std::rc::Rc;

use transform::Transform;

// ============ DPanel (简化，用 Picture 显示) ============
#[derive(Clone)]
struct DPanel {
    picture: Picture,
}

impl DPanel {
    fn new() -> Self {
        let picture = Picture::new();
        picture.set_halign(gtk::Align::Center);
        picture.set_valign(gtk::Align::Center);
        Self { picture }
    }

    fn widget(&self) -> &Picture {
        &self.picture
    }

  
    fn set_image(&self, img: &image::RgbaImage) {
        let pixbuf = image_to_pixbuf(img);
        // self.picture.set_paintable(Some(&pixbuf)); //废弃方法
        let textture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
        self.picture.set_paintable(Some(&textture));
    }

    /// 按给定缩放值(%)缩放
    fn apply_zoom(&self, zoom: f64) {
        if let Some(paintable) = self.picture.paintable() {
            if let Some(texture) = paintable.dynamic_cast::<gtk::gdk::Texture>().ok() {
                // 获取原始纹理的宽度和高度
                let original_width = texture.width() as f64;
                let original_height = texture.height() as f64;
    
                // 计算缩放后的宽度和高度
                let new_width = (original_width * zoom / 100.0) as i32;
                let new_height = (original_height * zoom / 100.0) as i32;
    
                // 设置 Picture 的显示大小
                self.picture.set_size_request(new_width, new_height);
            }
        }
    }
}

// ============ Util: 将 RgbaImage 转换为 Pixbuf ============
fn image_to_pixbuf(img: &image::RgbaImage) -> Pixbuf {
    let width = img.width() as i32;
    let height = img.height() as i32;
    let rowstride = 4 * width;
    Pixbuf::from_bytes(
        &Bytes::from(&img.as_raw()[..]),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        rowstride,
    )
}

fn main() {
    let app = Application::new(
        Some("com.example.rust-stegsolve"),
        Default::default(),
    );

    // 应用启动后，先注册动作，再创建界面
    app.connect_activate(build_ui);

    app.run();
}

fn build_ui(app: &Application) {
    // ============ 创建主窗口 ============
    let window = ApplicationWindow::new(app);
    window.set_default_size(1000, 700);
    window.set_title(Some("StegSolve (Rust + GTK4)"));

    // ============ 整体状态变量 ============
    // Transform 目前为空，等用户打开图片后才创建
    let transform_ref: Rc<RefCell<Option<Transform>>> = Rc::new(RefCell::new(None));
    let current_file_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // ============ 头部标题栏 + 菜单按钮 ============
    let header_bar = HeaderBar::new();

    // -- “文件” 菜单
    let file_menu_btn = MenuButton::new();
    file_menu_btn.set_label("文件");

    let file_menu = GMenu::new();
    file_menu.append(Some("打开"), Some("app.open"));
    file_menu.append(Some("另存为"), Some("app.save"));
    file_menu.append(Some("退出"), Some("app.quit"));
    file_menu_btn.set_menu_model(Some(&file_menu));

    header_bar.pack_start(&file_menu_btn);

    // -- “分析” 菜单
    let analyse_menu_btn = MenuButton::new();
    analyse_menu_btn.set_label("分析");

    let analyse_menu = GMenu::new();
    analyse_menu.append(Some("文件格式"), Some("app.analyse_format"));
    analyse_menu.append(Some("数据提取"), Some("app.analyse_extract"));
    analyse_menu.append(Some("立体视图"), Some("app.stereo_solve"));
    analyse_menu.append(Some("帧浏览器"), Some("app.frame_browser"));
    analyse_menu.append(Some("图像合成器"), Some("app.image_combine"));
    analyse_menu_btn.set_menu_model(Some(&analyse_menu));

    header_bar.pack_start(&analyse_menu_btn);

    // -- “帮助” 菜单
    let help_menu_btn = MenuButton::new();
    help_menu_btn.set_label("帮助");

    let help_menu = GMenu::new();
    help_menu.append(Some("关于"), Some("app.about"));
    help_menu_btn.set_menu_model(Some(&help_menu));

    header_bar.pack_start(&help_menu_btn);

    window.set_titlebar(Some(&header_bar));

    // ============ 中部：图像显示 + 滚动条 ============
    let dpanel = DPanel::new();
    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(PolicyType::Automatic, PolicyType::Automatic);

    // 允许滚动窗口本身扩展
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_min_content_width(1);
    scrolled.set_min_content_height(1);

    scrolled.set_child(Some(dpanel.widget()));

    // ============ 底部： 前后按钮 + 缩放滑条 + 状态文本 ============
    let bottom_box = gtk::Box::new(Orientation::Horizontal, 5);
    bottom_box.set_margin_top(6);
    bottom_box.set_margin_bottom(6);
    bottom_box.set_margin_start(6);
    bottom_box.set_margin_end(6);

    let back_btn = Button::with_label("<");
    let fwd_btn = Button::with_label(">");
    let label_now = Label::new(None);

    // 缩放滑条 (10 ~ 500%)
    let zoom_adjust = Adjustment::new(100.0, 10.0, 500.0, 1.0, 10.0, 0.0);
    let zoom_slider = Scale::new(Orientation::Horizontal, Some(&zoom_adjust));
    zoom_slider.set_draw_value(true);
    zoom_slider.set_value_pos(gtk::PositionType::Right);
    zoom_slider.set_hexpand(true);

    bottom_box.append(&back_btn);
    bottom_box.append(&fwd_btn);
    bottom_box.append(&label_now);
    bottom_box.append(&zoom_slider);

    // ============ 主布局 (垂直) ============
    let main_vbox = gtk::Box::new(Orientation::Vertical, 0);
    main_vbox.append(&scrolled);
    main_vbox.append(&bottom_box);

    window.set_child(Some(&main_vbox));

    // ============ 注册所需的 SimpleAction 给 app ============
    {
        // 打开
        let open_action = SimpleAction::new("open", None);
        let window_weak = window.downgrade();
        let transform_clone = transform_ref.clone();
        let current_file_path_clone = current_file_path.clone(); // 克隆状态变量
        let dpanel_clone = dpanel.clone();
        let label_clone = label_now.clone();
        let zoom_clone = zoom_slider.clone();
        open_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                let dialog = gtk::FileDialog::new();
                let transform_clone = transform_clone.clone();
                let current_file_path_clone = current_file_path_clone.clone(); // 再次克隆状态变量
                let dpanel_clone = dpanel_clone.clone();
                let label_clone = label_clone.clone();
                let zoom_clone = zoom_clone.clone();

                dialog.open(
                    Some(&win),
                    None::<&gio::Cancellable>,
                    move |res: Result<gio::File, glib::Error>| {
                        match res {
                            Ok(file) => {
                                if let Some(path) = file.path() {
                                    // 更新文件路径
                                    *current_file_path_clone.borrow_mut() = Some(path.to_string_lossy().to_string());
                                    println!("已选择文件: {}", path.display());

                                    // 打开图片并更新 UI
                                    match image::open(&path) {
                                        Ok(img) => {
                                            let rgba_img = img.to_rgba8(); // 转换为 RgbaImage
                                            let dynamic_img = image::DynamicImage::ImageRgba8(rgba_img);
                                            *transform_clone.borrow_mut() = Some(Transform::new(dynamic_img));
                                            if let Some(tf) = transform_clone.borrow().as_ref() {
                                                dpanel_clone.set_image(tf.get_image());
                                                label_clone.set_text(&tf.get_text());
                                            }
                                            zoom_clone.set_value(100.0);
                                        }
                                        Err(e) => {
                                            eprintln!("打开图片失败: {:?}", e);
                                        }
                                    }
                                } else {
                                    eprintln!("无法获取文件路径");
                                }
                            }
                            Err(e) => {
                                eprintln!("打开文件对话框失败: {:?}", e);
                            }
                        }
                    },
                );
            }
        });
        app.add_action(&open_action);

        // 另存为
        // 另存为
        let save_action = SimpleAction::new("save", None);
        let window_weak = window.downgrade();
        let transform_clone = transform_ref.clone();
        save_action.connect_activate(move |_, _| {
            let tr_opt = transform_clone.borrow();
            if tr_opt.is_none() {
                return; // 尚未打开图片
            }
            if let Some(win) = window_weak.upgrade() {
                let dialog = gtk::FileDialog::new();
                let transform_clone = transform_clone.clone();
                
                // 使用 save() 方法而不是 open()
                dialog.save(
                    Some(&win),
                    None::<&gtk::gio::Cancellable>,
                    move |res: Result<gtk::gio::File, gtk::glib::Error>| {
                        match res {
                            Ok(gfile) => {
                                if let Some(path) = gfile.path() {
                                    let tf = transform_clone.borrow();
                                    if let Some(ref transform) = *tf {
                                        stereo::save_rgba_image(transform.get_image(), path);
                                    }
                                }
                            }
                            Err(e) => {
                                // 用户可能取消了保存，不视为错误
                                if !e.matches(gtk::gio::IOErrorEnum::Cancelled) {
                                    eprintln!("保存失败: {:?}", e);
                                }
                            }
                        }
                    },
                );
            }
        });
        app.add_action(&save_action);

        // 退出
        let quit_action = SimpleAction::new("quit", None);
        let window_weak = window.downgrade();
        quit_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                win.close();
            }
        });
        app.add_action(&quit_action);

        // 分析功能（简单弹窗示意）
        // for (action_name, msg) in &[
        //     // ("analyse_format", "这里是文件格式分析界面 (待实现)"),
        //     // ("analyse_extract", "这里是数据提取界面 (待实现)"),
        //     // ("stereo_solve", "这里是立体视图界面 (待实现)"),
        //     // ("frame_browser", "这里是帧浏览器界面 (待实现)"),
        //     ("image_combine", "这里是图像合成器界面 (待实现)"),
        // ] {
        //     let a = SimpleAction::new(action_name, None);
        //     let msg_str = msg.to_string();
        //     a.connect_activate(move |_, _| {
        //         // gtk::MessageDialog::new(
        //         //     None::<&ApplicationWindow>,
        //         //     gtk::DialogFlags::MODAL,
        //         //     gtk::MessageType::Info,
        //         //     gtk::ButtonsType::Ok,
        //         //     &msg_str,
        //         // )
        //         // .run_async(|dialog, _| dialog.close());
        //         let dialog = AlertDialog::builder()
        //             .message(&msg_str)
        //             .buttons(vec!["确定"]) // 设置按钮，传入一个数组
        //             .default_button(0) // 设置默认按钮索引
        //             .build();
        //         dialog.choose(
        //             None::<&ApplicationWindow>, // 父窗口（可选）
        //             None::<&gio::Cancellable>, // 可选的细节信息
        //             |response| {
        //                 match response {
        //                     Ok(button_index) => {
        //                         if button_index == 0 {
        //                             println!("用户点击了确定");
        //                         }
        //                     }
        //                     Err(err) => eprintln!("对话框显示出错: {}", err),
        //                 }
        //             },
        //         );
        //     });
        //     app.add_action(&a);
        // }

        // 分析 - 文件格式
        let analyse_format_action = SimpleAction::new("analyse_format", None);
        let window_weak = window.downgrade();
        let current_file_path_clone = current_file_path.clone(); // 克隆状态变量
        analyse_format_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                // 获取当前文件路径
                if let Some(file_path) = current_file_path_clone.borrow().as_ref() {
                    // 调用文件分析功能
                    fileanalysis::show_file_analysis_dialog(Some(win.as_ref()), file_path);
                } else {
                    // 如果没有选择文件，显示错误提示
                    let dialog = AlertDialog::builder()
                        .message("请先打开一个图像文件")
                        .buttons(vec!["确定"])
                        .default_button(0)
                        .build();
                    dialog.show(Some(&win));
                }
            }
        });
        app.add_action(&analyse_format_action);

        // 分析 - 数据提取
        let analyse_extract_action = SimpleAction::new("analyse_extract", None);
        let window_weak = window.downgrade();
        let transform_clone = transform_ref.clone();
        analyse_extract_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                if let Some(tf) = transform_clone.borrow().as_ref() {
                    let dialog = extractanlysis::ExtractDialog::new(&win, tf.get_image());
                    dialog.show(); 
                } else {
                    // 如果没有图片，显示错误提示
                    let dialog =AlertDialog::builder()
                        .message("请先打开一个图像")
                        .buttons(vec!["确定"])
                        .default_button(0)
                        .build();
                    dialog.show(Some(&win));
                }
            }
        });
        app.add_action(&analyse_extract_action);

        // 分析 - 立体视图
        let stereo_solve_action = SimpleAction::new("stereo_solve", None);
        let window_weak = window.downgrade();
        let transform_clone = transform_ref.clone();
        stereo_solve_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                if let Some(tf) = transform_clone.borrow().as_ref() {
                    let dialog = crate::stereo::Stereo::new(&win, tf.get_image().clone());
                    dialog.show();
                } else {
                    let dialog = AlertDialog::builder()
                        .message("请先打开一个图像文件")
                        .buttons(vec!["确定"])
                        .default_button(0)
                        .build();
                    dialog.show(Some(&win));
                }
            }
        });
        app.add_action(&stereo_solve_action);

        // 分析 - 帧浏览器
        let frame_browser_action = SimpleAction::new("frame_browser", None);
        let window_weak = window.downgrade();
        let current_file_path_clone = current_file_path.clone();
        frame_browser_action.connect_activate(move|_, _| {
            if let Some(win) = window_weak.upgrade() {
                if let Some(path) = current_file_path_clone.borrow().as_ref() {
                    let browser = framebrowser::FrameBrowser::new(&win);
                    browser.load_frames(path);
                    browser.show();
                } else {
                    let dialog = AlertDialog::builder()
                        .message("请先打开一个图像文件")
                        .buttons(vec!["确定"])
                        .default_button(0)
                        .build();
                    dialog.show(Some(&win));
                }
            }
        });
        app.add_action(&frame_browser_action);

        // 分析 - 图像合成器
        let image_combine_action = SimpleAction::new("image_combine", None);
        let window_weak = window.downgrade();
        let transform_clone = transform_ref.clone();
        image_combine_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                if let Some(tf) = transform_clone.borrow().as_ref() {
                    // 打开图像选择对话框选择第二张图片
                    let dialog = combine::ImageCombiner::new(
                        &win,
                        tf.get_image().clone(),
                    );
                    dialog.show();
                } else {
                    let msg_dialog = AlertDialog::builder()
                        .message("请先打开一个图像文件")
                        .buttons(vec!["确定"])
                        .default_button(0)
                        .build();
                    msg_dialog.show(Some(&win));
                }
            }
        });
        app.add_action(&image_combine_action);



        // 帮助 - 关于
        let about_action = SimpleAction::new("about", None);
        about_action.connect_activate(move |_, _| {
            let dialog = AlertDialog::builder()
                .message("StegSolve-rs (基于原版重构)\n参考 Java 原版结构\nBy: Sn1waR")
                .buttons(vec!["确定"])
                .default_button(0)
                .build();
            dialog.show(None::<&ApplicationWindow>);
        });
        app.add_action(&about_action);
    }

    // ============ 前后按钮 ============
    {
        let transform_clone = transform_ref.clone();
        let dpanel_clone = dpanel.clone();
        let label_clone = label_now.clone();
        back_btn.connect_clicked(move |_| {
            let mut opt = transform_clone.borrow_mut();
            if let Some(ref mut tf) = *opt {
                tf.back();
                label_clone.set_text(&tf.get_text());
                dpanel_clone.set_image(tf.get_image());
            }
        });
    }
    {
        let transform_clone = transform_ref.clone();
        let dpanel_clone = dpanel.clone();
        let label_clone = label_now.clone();
        fwd_btn.connect_clicked(move |_| {
            let mut opt = transform_clone.borrow_mut();
            if let Some(ref mut tf) = *opt {
                tf.forward();
                label_clone.set_text(&tf.get_text());
                dpanel_clone.set_image(tf.get_image());
            }
        });
    }

    // ============ 缩放滑条 ============
    {
        let dpanel_clone = dpanel.clone();
        zoom_slider.connect_value_changed(move |scale| {
            let val = scale.value();
            print!("缩放: {}%\n", val);
            dpanel_clone.apply_zoom(val);
        });
    }

    window.set_visible(true);
}
