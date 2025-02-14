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

release流水线正在进行重构中，下版本发布


## Why Rust+egui?
egui确实文明，gtk坑太多了（

## 已知问题
- [ ] 目前子ui页面大小计算有问题，导致部分页面显示不全，需要手动调整窗口大小


## todo

- [x] 自动流水线打包全平台
    - [ ] 全平台预编译打包支持
- [ ] 多语言支持
- [ ] 项目结构重构

## 许可证
[MIT License](LICENSE)