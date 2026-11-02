# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · **简体中文**

在玩家自己的电脑上、用玩家自己的游戏拷贝，为新的 Ballance 引擎 [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt) 准备关卡。游戏只游玩其关卡文件夹中的内容；让关卡看起来更现代的所有繁重工作都在这里完成，游戏本身不需要其中任何东西。

```
drahtseilakt-toolbox                     （窗口）
drahtseilakt-toolbox remaster [--scale N] [--model NAME] [--relief S] [--data DIR]
drahtseilakt-toolbox relief DIR [--scale N] [--relief S]
drahtseilakt-toolbox import-maps DIR [--to DIR]
```

不带命令启动时会打开窗口（Linux X11 或 Wayland、Windows、macOS；它会在游戏数据文件夹的 `toolbox.txt` 中记住上次选择的模型和放大倍数）：你的游戏拷贝、放大倍数、Real-ESRGAN 模型、浮雕、“开始制作”按钮、按关卡和部件显示的进度（可取消）、结果，以及一张纹理处理前后的对比。界面文字有英语、俄语、德语和简体中文，按系统语言或在窗口中选择；中文使用系统已有的字体（Noto CJK、Microsoft YaHei、PingFang 等）。

`remaster` 把拷贝中的原版关卡转换到游戏的 `levels/remastered/`（开放格式），为每个关卡生成更圆的形状（写入关卡、放在原始网格旁边，物理仍使用原始网格，因此通关过程不变），并把纹理放大 N 倍（默认 4；1 表示不改动）。有 Real-ESRGAN 时纹理会补出细节，否则用 Lanczos 放大（小于 64 像素的纹理也用 Lanczos，因为它会在上面凭空编造细节）。Real-ESRGAN 先在 toolbox 程序旁的 `realesrgan/` 中查找，然后在 PATH 中查找；所用版本固定在 [third-party.txt](third-party.txt) 中。原始纹理保存在 `textures.original/`。每个关卡的部件（球、模块、变球器）也以同样方式重制到它自己的 `game/` 文件夹中，连同其天空、粒子、阴影和声音，因此关卡无需拷贝中的关卡文件也能游玩（见游戏的 open-format.md）。原版关卡每次运行都会从拷贝重新构建。作为平铺重复的纹理放大时会在四周带上其另一侧的内容，因此平铺接缝处不会出现缝隙。使用 `--relief S` 时，每个受光照的纹理还会根据其自身图像生成一张强度为 S 的法线贴图，游戏在重制关卡中使用它。默认关闭：浮雕是从图像推测出来的，会加深每一条裂缝和缝隙（强度 8 时世界看起来像被酸蚀过）；请保持较低，1 到 3。砖砌始终生成：纹理显示混凝土中的砖块（地板的砖带）时，按颜色识别砖块（米色或灰色中的赤陶色），使其凸起、砖间灰缝凹下，游戏以视差绘制，因此在掠射视角下砖块也会凸显；木材、石材和金属不受影响。`relief DIR` 为开放格式的单个关卡文件夹重新生成凹凸，而不重制它。`--model` 可选择放在其 `models/` 文件夹中的其他 Real-ESRGAN 模型（默认 realesrgan-x4plus，随附提供）。`remaster`（或单独的 `sounds`）还会用拷贝中的音效和音乐在游戏数据文件夹中生成“原版”音效主题；它属于原版，因此只留在玩家的电脑上。更新 toolbox 后再运行一次即可更新关卡。

`import-maps DIR` 把下载的自制地图合集（已解压的压缩包：任意深度的文件夹、图片、文档）整理到游戏的 `levels/community/`（或 `--to DIR`），整理成游戏列出的结构（游戏的 levels.md，“一个关卡就是一个文件夹”）：每张地图一个文件夹，包含它的 `.nmo`、旁边的图片作为 `preview.jpg`/`.png`，以及说明来源的 `level.txt`（文件夹名不得不丢掉字符时还写入完整标题）。通用文件名（“45-Degree Rotated Maps” 中的 `Level_07.NMO`）按合集命名（“45-Degree Rotated - Level 07”）；同一文件出现两次只算一个关卡；压缩包（`.tar.gz`、`.zip`……）只列出、不打开；名称在 Linux、Windows 和 macOS 上都可用（无保留名称，末尾无点或空格，最多 40 个字符，按单词截断并加“…”，第二个 “Hello Mars” 变为 “Hello Mars (2)”）。从不跟随链接，深度和数量有上限，源文件只读，每个关卡先写入隐藏文件夹再重命名到位。再次运行只添加新内容。窗口中也有同样的功能：一个输入框和 Import 按钮。

toolbox 从不联网，也从不改动玩家的原版拷贝。

## 构建

toolbox 与游戏共用关卡读取代码：需要在旁边的 `../ballance/drahtseilakt` 放一份游戏仓库的克隆。然后执行 `cargo build --release --offline`。若要使用 Real-ESRGAN，把固定版本的下载包解压到 `third_party/realesrgan/`，并把 `target/release/realesrgan` 链接到该目录。

GPL-3.0-only，与游戏相同。
