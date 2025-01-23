use std::cell::RefCell;
use std::rc::Rc;
use gtk::prelude::*;
use gtk::{self, Window, Picture, Button, Label, Box as GtkBox};
use gdk_pixbuf::Pixbuf;
use image::{RgbaImage, DynamicImage, GenericImageView, ImageBuffer};

pub struct ImageCombiner {
    window: Window,
    picture: Picture,
    label: Label,
    transform_num: Rc<RefCell<i32>>,  // 改为Rc包裹
    img1: RgbaImage,
    img2: Rc<RefCell<Option<RgbaImage>>>,  // 改为Rc包裹
}

const NUM_TRANSFORMS: i32 = 13;

impl ImageCombiner {
    pub fn new(parent: &impl IsA<gtk::Window>, img1: RgbaImage) -> Self {
        // 创建窗口
        let window = Window::new();
        window.set_title(Some("图像合成器"));
        window.set_transient_for(Some(parent));
        window.set_default_size(500, 600);
        window.set_modal(true);

        // 创建垂直布局
        let vbox = GtkBox::new(gtk::Orientation::Vertical, 5);
        
        // 显示当前模式的标签
        let label = Label::new(Some("请选择第二张图片"));
        vbox.append(&label);

        // 图片显示区域
        let picture = Picture::new();
        picture.set_size_request(400, 400);
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_child(Some(&picture));
        scrolled.set_vexpand(true);
        vbox.append(&scrolled);

        // 底部按钮区域
        let button_box = GtkBox::new(gtk::Orientation::Horizontal, 5);
        button_box.set_halign(gtk::Align::Center);

        let back_btn = Button::with_label("<");
        let forward_btn = Button::with_label(">");
        let open_btn = Button::with_label("打开图片");
        let save_btn = Button::with_label("保存");

        button_box.append(&back_btn);
        button_box.append(&forward_btn); 
        button_box.append(&open_btn);
        button_box.append(&save_btn);

        vbox.append(&button_box);
        window.set_child(Some(&vbox));

        let combiner = Self {
            window,
            picture,
            label,
            transform_num: Rc::new(RefCell::new(0)),  // 改为Rc
            img1,
            img2: Rc::new(RefCell::new(None)),        // 改为Rc
        };

        // 事件处理
        let combiner_clone = combiner.clone();
            back_btn.connect_clicked(move |_| {
                combiner_clone.backward();
        });

            let combiner_clone = combiner.clone();
            forward_btn.connect_clicked(move |_| {
                combiner_clone.forward();
            });

        let c = combiner.clone();
        open_btn.connect_clicked(move |_| {
            c.open_second_image();
        });

        let c = combiner.clone();
        save_btn.connect_clicked(move |_| {
            c.save_image();
        });

        combiner
    }

    pub fn show(&self) {
        self.window.show();
    }

    fn backward(&self) {
        if self.img2.borrow().is_none() { return; }
    
        // 将修改操作放在独立的作用域中
        {
            let mut num = self.transform_num.borrow_mut();
            *num = if *num <= 0 {
                NUM_TRANSFORMS - 1
            } else {
                *num - 1
            };
        } // 这里会释放可变借用
        
        self.update_image(); // 此时可以安全获取不可变借用
    }
    
    fn forward(&self) {
        if self.img2.borrow().is_none() { return; }
        
        {
            let mut num = self.transform_num.borrow_mut();
            *num = (*num + 1) % NUM_TRANSFORMS;
        } // 释放可变借用
        
        self.update_image();
    }

    fn open_second_image(&self) {
        let dialog = gtk::FileDialog::new();
        let window_clone = self.window.clone();
        let self_clone = self.clone();

        dialog.open(Some(&self.window), None::<&gtk::gio::Cancellable>, 
            move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        if let Ok(img) = image::open(&path) {
                            *self_clone.img2.borrow_mut() = Some(img.to_rgba8());
                            self_clone.update_image();
                        } else {
                            let msg = gtk::MessageDialog::new(
                                Some(&window_clone),
                                gtk::DialogFlags::MODAL,
                                gtk::MessageType::Error,
                                gtk::ButtonsType::Ok,
                                "无法打开图片"
                            );
                            msg.run_async(|d, _| d.close());
                        }
                    }
                }
            }
        );
    }

    fn save_image(&self) {
        if self.img2.borrow().is_none() { return; }

        let dialog = gtk::FileDialog::new();
        let window_clone = self.window.clone();
        let self_clone = self.clone();

        dialog.save(
            Some(&self.window),
            None::<&gtk::gio::Cancellable>,
            move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        if let Some(combined) = self_clone.get_combined_image() {
                            if let Err(e) = combined.save(path) {
                                let msg = gtk::MessageDialog::new(
                                    Some(&window_clone),
                                    gtk::DialogFlags::MODAL,
                                    gtk::MessageType::Error,
                                    gtk::ButtonsType::Ok,
                                    &format!("保存失败: {}", e)
                                );
                                msg.run_async(|d, _| d.close());
                            }
                        }
                    }
                }
            }
        );
    }

    fn get_transform_text(&self) -> String {
        match *self.transform_num.borrow() {
            0 => "XOR",
            1 => "OR",
            2 => "AND",
            3 => "ADD",
            4 => "ADD (R,G,B separate)",
            5 => "SUB",
            6 => "SUB (R,G,B separate)", 
            7 => "MUL",
            8 => "MUL (R,G,B separate)",
            9 => "Lightest (R,G,B separate)",
            10 => "Darkest (R,G,B separate)",
            11 => "Horizontal Interlace",
            12 => "Vertical Interlace",
            _ => "???"
        }.to_string()
    }

    // 修改get_combined_image方法，提前获取转换模式的副本
    fn get_combined_image(&self) -> Option<RgbaImage> {
        let img2 = self.img2.borrow();
        let img2 = img2.as_ref()?;
        
        // 提前获取转换模式的不可变借用
        let transform_num = *self.transform_num.borrow();
        
        match transform_num {  // 使用副本而不是实时借用
            11 => self.horizontal_interlace(img2),
            12 => self.vertical_interlace(img2),
            _ => self.combine_pixels(img2, transform_num)  // 需要修改函数签名
        }
    }


    fn combine_pixels(&self, img2: &RgbaImage, transform_num: i32) -> Option<RgbaImage> {
        let width = self.img1.width().max(img2.width());
        let height = self.img1.height().max(img2.height());
        
        let mut result = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let p1 = if x < self.img1.width() && y < self.img1.height() {
                    self.img1.get_pixel(x, y)
                } else {
                    &image::Rgba([0, 0, 0, 0])
                };

                let p2 = if x < img2.width() && y < img2.height() {
                    img2.get_pixel(x, y)
                } else {
                    &image::Rgba([0, 0, 0, 0])
                };

                let combined = match transform_num  {
                    0 => [p1[0]^p2[0], p1[1]^p2[1], p1[2]^p2[2], 255], // XOR
                    1 => [p1[0]|p2[0], p1[1]|p2[1], p1[2]|p2[2], 255], // OR
                    2 => [p1[0]&p2[0], p1[1]&p2[1], p1[2]&p2[2], 255], // AND
                    3 => [ // ADD
                        p1[0].saturating_add(p2[0]),
                        p1[1].saturating_add(p2[1]),
                        p1[2].saturating_add(p2[2]),
                        255
                    ],
                    4 => [ // ADD separate
                        ((p1[0] as u16 + p2[0] as u16) % 256) as u8,
                        ((p1[1] as u16 + p2[1] as u16) % 256) as u8,
                        ((p1[2] as u16 + p2[2] as u16) % 256) as u8,
                        255
                    ],
                    5 => [ // SUB
                        p1[0].saturating_sub(p2[0]),
                        p1[1].saturating_sub(p2[1]),
                        p1[2].saturating_sub(p2[2]),
                        255
                    ],
                    6 => [ // SUB separate
                        ((256 + p1[0] as i16 - p2[0] as i16) % 256) as u8,
                        ((256 + p1[1] as i16 - p2[1] as i16) % 256) as u8,
                        ((256 + p1[2] as i16 - p2[2] as i16) % 256) as u8,
                        255
                    ],
                    7 => [ // MUL
                        ((p1[0] as u16 * p2[0] as u16) / 256) as u8,
                        ((p1[1] as u16 * p2[1] as u16) / 256) as u8,
                        ((p1[2] as u16 * p2[2] as u16) / 256) as u8,
                        255
                    ],
                    8 => [ // MUL separate
                        ((p1[0] as u16 * p2[0] as u16) % 256) as u8,
                        ((p1[1] as u16 * p2[1] as u16) % 256) as u8,
                        ((p1[2] as u16 * p2[2] as u16) % 256) as u8,
                        255
                    ],
                    9 => [ // Lightest
                        p1[0].max(p2[0]),
                        p1[1].max(p2[1]),
                        p1[2].max(p2[2]),
                        255
                    ],
                    10 => [ // Darkest
                        p1[0].min(p2[0]),
                        p1[1].min(p2[1]),
                        p1[2].min(p2[2]),
                        255
                    ],
                    _ => [p1[0], p1[1], p1[2], 255]
                };

                result.put_pixel(x, y, image::Rgba(combined));
            }
        }

        Some(result)
    }

    fn horizontal_interlace(&self, img2: &RgbaImage) -> Option<RgbaImage> {
        let width = self.img1.width().min(img2.width());
        let height = self.img1.height().min(img2.height());
        
        let mut result = ImageBuffer::new(width, height * 2);

        for y in 0..height {
            for x in 0..width {
                let p1 = self.img1.get_pixel(x, y);
                let p2 = img2.get_pixel(x, y);
                
                result.put_pixel(x, y*2, *p1);
                result.put_pixel(x, y*2+1, *p2);
            }
        }

        Some(result)
    }

    fn vertical_interlace(&self, img2: &RgbaImage) -> Option<RgbaImage> {
        let width = self.img1.width().min(img2.width());
        let height = self.img1.height().min(img2.height());
        
        let mut result = ImageBuffer::new(width * 2, height);

        for y in 0..height {
            for x in 0..width {
                let p1 = self.img1.get_pixel(x, y);
                let p2 = img2.get_pixel(x, y);
                
                result.put_pixel(x*2, y, *p1);
                result.put_pixel(x*2+1, y, *p2);
            }
        }

        Some(result)
    }

    fn update_image(&self) {
        if let Some(combined) = self.get_combined_image() {
            // 更新标签文本
            self.label.set_text(&self.get_transform_text());
            
            // 转换为 Pixbuf 并显示
            let width = combined.width() as i32;
            let height = combined.height() as i32;
            let pixbuf = Pixbuf::from_bytes(
                &glib::Bytes::from(&combined.as_raw()[..]),
                gdk_pixbuf::Colorspace::Rgb,
                true,
                8,
                width,
                height,
                width * 4,
            );
            self.picture.set_pixbuf(Some(&pixbuf));
        }
    }
}

impl Clone for ImageCombiner {
    fn clone(&self) -> Self {
        Self {
            window: self.window.clone(),
            picture: self.picture.clone(),
            label: self.label.clone(),
            transform_num: Rc::clone(&self.transform_num),  // 共享Rc
            img1: self.img1.clone(),
            img2: Rc::clone(&self.img2),  // 共享Rc
        }
    }
}