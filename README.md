# StegSolve-rs 
StegSolve-rs 是一个基于 Rust + GTK4 重构的图像隐写分析工具，复刻重构了StegSolve
## 主要功能
java原版全功能重构

![功能展示](img/image2.png)


## 功能截图

![alt text](img/image.png)

![alt text](img/lsb.png)

## 如何运行
`git clone https://github.com/jiayuqi7813/Stegsolve-rs.git`

`cargo run`

目前release包存在需要依赖问题，你可以手工安装gtk4的依赖进行运行，也可以等待我发布打包好的内容，预计下个版本修复。

手工构建则不存在该问题。


## Why Rust+GTK4?
rust重构一切！
其他的gui要么基于web，要么有更多自己的组件外包，而gtk直接在原本rust和gnome基础上进行开发，简单快捷，也没那么丑。


## todo

- [x] 自动流水线打包全平台
    - [ ] 全平台预编译打包支持
- [ ] 多语言支持
- [ ] 项目结构重构

## 许可证
[MIT License](LICENSE)