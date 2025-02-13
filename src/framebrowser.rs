use gtk::prelude::*;
use gtk::{
    Button, Label, Picture, ScrolledWindow, Window, Box, Orientation,
    FileDialog, PolicyType,
};
use image::{DynamicImage, RgbaImage};
use gdk_pixbuf::Pixbuf;
use glib::Bytes;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::Path;


// 将 RgbaImage 转换为 Pixbuf (复用 main.rs 中的函数)
fn image_to_pixbuf(img: &RgbaImage) -> Pixbuf {
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



pub struct FrameBrowser {
    window: Window,
    frames: Rc<RefCell<Vec<RgbaImage>>>,
    current_frame: Rc<RefCell<usize>>,
    picture: Picture,
    label: Label,
}


impl FrameBrowser {
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        // 创建窗口
        let window = Window::new();
        window.set_title(Some("Frame Browser"));
        window.set_default_size(500, 600);
        window.set_transient_for(Some(parent));
        window.set_modal(true);

        // 创建组件
        let vbox = Box::new(Orientation::Vertical, 5);
        vbox.set_margin_end(5);

        let label = Label::new(Some("Frame: 0 of 0"));
        vbox.append(&label);

        // 图片显示区域
        let picture = Picture::new();
        let scrolled = ScrolledWindow::new();
        scrolled.set_policy(PolicyType::Automatic, PolicyType::Automatic);
        scrolled.set_child(Some(&picture));
        scrolled.set_vexpand(true);
        vbox.append(&scrolled);

        // 按钮区域
        let button_box = Box::new(Orientation::Horizontal, 5);
        let back_btn = Button::with_label("<");
        let forward_btn = Button::with_label(">");
        let save_btn = Button::with_label("Save");
        
        button_box.append(&back_btn);
        button_box.append(&forward_btn);
        button_box.append(&save_btn);
        vbox.append(&button_box);

        window.set_child(Some(&vbox));

        let frames = Rc::new(RefCell::new(Vec::new()));
        let current_frame = Rc::new(RefCell::new(0));

        // 设置事件处理
        {
            let frames = frames.clone();
            let current = current_frame.clone();
            let picture = picture.clone();
            let label = label.clone();
            back_btn.connect_clicked(move |_| {
                let mut current = current.borrow_mut();
                let frames = frames.borrow();
                if !frames.is_empty() {
                    if *current == 0 {
                        *current = frames.len() - 1;
                    } else {
                        *current -= 1;
                    }
                    // 更新显示
                    let pixbuf = image_to_pixbuf(&frames[*current]);
                    picture.set_pixbuf(Some(&pixbuf));
                    label.set_text(&format!("Frame: {} of {}", *current + 1, frames.len()));
                }
            });
        }

        {
            let frames = frames.clone();
            let current = current_frame.clone();
            let picture = picture.clone();
            let label = label.clone();
            forward_btn.connect_clicked(move |_| {
                let mut current = current.borrow_mut();
                let frames = frames.borrow();
                if !frames.is_empty() {
                    *current = (*current + 1) % frames.len();
                    // 更新显示
                    let pixbuf = image_to_pixbuf(&frames[*current]);
                    picture.set_pixbuf(Some(&pixbuf));
                    label.set_text(&format!("Frame: {} of {}", *current + 1, frames.len()));
                }
            });
        }

        {
            let frames = frames.clone();
            let current = current_frame.clone();
            let window_clone = window.clone();
            save_btn.connect_clicked(move |_| {
                // 立即获取当前帧索引和对应的图像数据
                let current_index = *current.borrow();
                let frame = frames.borrow().get(current_index).cloned();
        
                if let Some(frame) = frame {
                    let dialog = FileDialog::new();
                    let frame_num = current_index + 1;
                    dialog.set_initial_name(Some(&format!("frame{}.png", frame_num)));
        
                    dialog.save(
                        Some(&window_clone),
                        None::<&gtk::gio::Cancellable>,
                        move |res| {
                            if let Ok(file) = res {
                                if let Some(path) = file.path() {
                                    if let Err(e) = frame.save(&path) {
                                        eprintln!("保存帧失败: {:?}", e);
                                    }
                                }
                            }
                        },
                    );
                }
            });
        }

        Self {
            window,
            frames,
            current_frame,
            picture,
            label,
        }
    }

    pub fn load_frames<P: AsRef<Path>>(&self, path: P) {
        // 清空现有帧
        self.frames.borrow_mut().clear();
        *self.current_frame.borrow_mut() = 0;

        // 读取所有帧
        if let Ok(reader) = image::io::Reader::open(path) {
            if let Ok(reader) = reader.with_guessed_format() {
                if let Ok(frames) = reader.decode() {
                    self.frames.borrow_mut().push(frames.to_rgba8());
                }
            }
        }

        // 更新显示
        let frames = self.frames.borrow();
        if !frames.is_empty() {
            let pixbuf = image_to_pixbuf(&frames[0]);
            self.picture.set_pixbuf(Some(&pixbuf));
            self.label.set_text(&format!("Frame: 1 of {}", frames.len()));
        }
    }

    pub fn show(&self) {
        self.window.show();
    }
}