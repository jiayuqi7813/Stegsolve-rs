use gtk::{gdk, prelude::*};
use gtk::{self, ApplicationWindow, Button, Box, Label, Picture, ScrolledWindow, Orientation};
use gtk::FileChooserAction;
use gdk_pixbuf::Pixbuf;
use image::{DynamicImage, RgbaImage, GenericImageView};
use std::cell::RefCell;
use std::rc::Rc;
use crate::transform::Transform;


pub struct StereoTransform {
    original_image: RgbaImage,
    transform: RgbaImage,
    trans_num: i32,
}

impl StereoTransform {
    pub fn new(img: RgbaImage) -> Self {
        let mut st = Self {
            original_image: img.clone(),
            transform: RgbaImage::new(img.width(), img.height()),
            trans_num: 0,
        };
        st.calc_trans();
        st
    }

    fn calc_trans(&mut self) {
        let width = self.original_image.width() as i32;
        let height = self.original_image.height() as i32;

        for i in 0..width {
            for j in 0..height {
                let fcol = self.original_image.get_pixel(i as u32, j as u32);
                let offset = ((i + self.trans_num).rem_euclid(width)) as u32;
                let ocol = self.original_image.get_pixel(offset, j as u32);
                
                let new_pixel = image::Rgba([
                    fcol[0] ^ ocol[0],
                    fcol[1] ^ ocol[1], 
                    fcol[2] ^ ocol[2],
                    255,
                ]);
                
                self.transform.put_pixel(i as u32, j as u32, new_pixel);
            }
        }
    }

    pub fn back(&mut self) {
        self.trans_num -= 1;
        if self.trans_num < 0 {
            self.trans_num = self.original_image.width() as i32 - 1;
        }
        self.calc_trans();
    }

    pub fn forward(&mut self) {
        self.trans_num += 1;
        if self.trans_num >= self.original_image.width() as i32 {
            self.trans_num = 0;
        }
        self.calc_trans();
    }

    pub fn get_text(&self) -> String {
        format!("偏移量: {}", self.trans_num)
    }

    pub fn get_image(&self) -> &RgbaImage {
        &self.transform
    }
}

pub struct Stereo {
    window: ApplicationWindow,
    now_showing: Label,
    back_btn: Button,
    forward_btn: Button,
    save_btn: Button,
    dpanel: Picture,
    transform: Rc<RefCell<Option<StereoTransform>>>,
    key_controller: gtk::EventControllerKey,
}



impl Stereo {
    
    pub fn new(parent: &ApplicationWindow, img: RgbaImage) -> Self {
        let window = ApplicationWindow::builder()
            .transient_for(parent)
            .modal(true)
            .title("立体图分析")
            .default_width(500)
            .default_height(600)
            .build();

        let transform = Rc::new(RefCell::new(Some(StereoTransform::new(img))));

        let vbox = Box::new(Orientation::Vertical, 5);
        
        // 顶部状态标签
        let now_showing = Label::new(None);
        vbox.append(&now_showing);

        // 中间图像显示区域
        let dpanel = Picture::new();
        let scroll = ScrolledWindow::new();
        scroll.set_child(Some(&dpanel));
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);
        vbox.append(&scroll);

        // 底部按钮区域
        let button_box = Box::new(Orientation::Horizontal, 5);
        let back_btn = Button::with_label("<");
        let forward_btn = Button::with_label(">");
        let save_btn = Button::with_label("保存");

        button_box.append(&back_btn);
        button_box.append(&forward_btn); 
        button_box.append(&save_btn);
        vbox.append(&button_box);

        window.set_child(Some(&vbox));

        let key_controller = gtk::EventControllerKey::new();
        window.add_controller(
            key_controller.clone()
        );

        let stereo = Self {
            window,
            now_showing,
            back_btn,
            forward_btn,
            save_btn,
            dpanel,
            transform,
            key_controller,
        };

        stereo.connect_signals();
        stereo.update_ui();
        stereo
    }

    fn connect_signals(&self) {
        let transform = self.transform.clone();
        let dpanel = self.dpanel.clone();
        let now_showing = self.now_showing.clone();

        
        self.back_btn.connect_clicked(move |_| {
            if let Some(ref mut tf) = *transform.borrow_mut() {
                tf.back();
                if let Some(pixbuf) = image_to_pixbuf(tf.get_image()) {
                    dpanel.set_pixbuf(Some(&pixbuf));
                }
                now_showing.set_text(&tf.get_text());
            }
        });

        let transform = self.transform.clone();
        let dpanel = self.dpanel.clone();
        let now_showing = self.now_showing.clone();

        self.forward_btn.connect_clicked(move |_| {
            if let Some(ref mut tf) = *transform.borrow_mut() {
                tf.forward();
                if let Some(pixbuf) = image_to_pixbuf(tf.get_image()) {
                    dpanel.set_pixbuf(Some(&pixbuf));
                }
                now_showing.set_text(&tf.get_text());
            }
        });

        let transform = self.transform.clone();
        let window = self.window.clone();

        self.save_btn.connect_clicked(move |_| {
            if let Some(tf) = transform.borrow().as_ref() {
                let img = tf.get_image().clone();
                let window = window.clone();
        
                let dialog = gtk::FileDialog::new();
                dialog.set_initial_name(Some("solved.png"));  // 设置默认文件名
        
                // 使用 save() 方法处理保存
                let img = img.clone();
                dialog.save(
                    Some(&window),
                    None::<&gtk::gio::Cancellable>,
                    move |result| {
                        match result {
                            Ok(gfile) => {
                                if let Some(path) = gfile.path() {
                                    save_rgba_image(&img, path);
                                }
                            }
                            Err(e) if !e.matches(gtk::gio::IOErrorEnum::Cancelled) => {
                                eprintln!("保存失败: {}", e);
                            }
                            _ => {}
                        }
                    },
                );
            }
        });



        let transform = self.transform.clone();
        let dpanel = self.dpanel.clone();
        let now_showing = self.now_showing.clone();

        self.key_controller.connect_key_pressed(
            move |_, keyval, _, _| {
                let mut need_update = false;
                if let Some(ref mut tf) = *transform.borrow_mut() {
                    match keyval {
                        gdk::Key::Left => {
                            tf.back();
                            need_update = true;
                        }
                        gdk::Key::Right => {
                            tf.forward();
                            need_update = true;
                        }
                        _ => {}
                    }
                }
                
                if need_update {
                    if let Some(ref tf) = *transform.borrow() {
                        if let Some(pixbuf) = image_to_pixbuf(tf.get_image()) {
                            dpanel.set_pixbuf(Some(&pixbuf));
                        }
                        now_showing.set_text(&tf.get_text());
                    }
                }
                glib::Propagation::Proceed
            }
        );



    }

    fn update_ui(&self) {
        if let Some(ref tf) = *self.transform.borrow() {
            if let Some(pixbuf) = image_to_pixbuf(tf.get_image()) {
                self.dpanel.set_pixbuf(Some(&pixbuf));
            }
            self.now_showing.set_text(&tf.get_text());
        }
    }

    pub fn show(&self) {
        self.window.show();
    }
}

fn image_to_pixbuf(img: &RgbaImage) -> Option<Pixbuf> {
    let width = img.width() as i32;
    let height = img.height() as i32;
    Some(Pixbuf::from_bytes(
        &glib::Bytes::from(img.as_raw()),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        width * 4,
    ))
}

// 通用保存逻辑，保存RGBA图像
pub fn save_rgba_image(img: &image::RgbaImage, path: std::path::PathBuf) {
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".to_string());

    let format = match ext.as_str() {
        "png" => image::ImageFormat::Png,
        "jpg" | "jpeg" => image::ImageFormat::Jpeg,
        "bmp" => image::ImageFormat::Bmp,
        _ => image::ImageFormat::Png, // 默认PNG
    };

    if let Err(e) = img.save_with_format(&path, format) {
        eprintln!("保存失败: {}", e);
    }
}