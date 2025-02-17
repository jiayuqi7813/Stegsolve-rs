# StegSolve-rs 
StegSolve-rs 是一个基于 Rust + egui 重构的图像隐写分析工具，复刻重构了StegSolve

> 由于跨平台原因0.2版本全面重构为egui进行开发。
>
> 主分支为egui、副分支为上个版本的gtk gui存档，

## 主要功能
java原版全功能重构

![image-20250214173053745](/img/image-20250214173053745.png)


## 功能截图

![image-20250214173152024](/img/image-20250214173152024.png)

![image-20250214173208188](/img/image-20250214173208188.png)

## 如何运行
`git clone https://github.com/jiayuqi7813/Stegsolve-rs.git`

`cargo run`

你也可以选择直接下载release版本运行，目前支持windows-x64、linux-x64、macos-arm64、macos-x64。需要更多平台可以提交issue。

linux需要如下依赖
```shell
 sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

## Why Rust+egui?
egui确实文明，gtk坑太多了（

## 已知问题

目前可能会存在一些问题，欢迎提交issue进行反馈。

- [x] 目前子ui页面大小计算有问题，导致部分页面显示不全，需要手动调整窗口大小
    - ps：目前应该都修复了，除了有些界面会突然很大，但不会出现显示不全的问题


## todo

- [x] 自动流水线打包全平台
    - [x] 全平台预编译打包支持
- [ ] 多语言支持
- [ ] 项目结构重构

## 贡献
感谢以下贡献者的贡献

[@b3nguang](https://github.com/b3nguang) 

## 许可证
[MIT License](LICENSE)
