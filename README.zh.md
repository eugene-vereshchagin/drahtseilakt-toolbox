# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · **简体中文**

在玩家自己的电脑上、用玩家自己的游戏拷贝，为新的 Ballance 引擎 [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt) 准备关卡。游戏只游玩其关卡文件夹中的内容；让关卡看起来更现代的所有繁重工作都在这里完成，游戏本身不需要其中任何东西。

```
drahtseilakt-toolbox                     （窗口）
drahtseilakt-toolbox remaster [--levels LIST] [--scale N] [--model NAME] [--relief S] [--data DIR]
drahtseilakt-toolbox remaster-open DIR [--scale N] [--model NAME] [--relief S]
drahtseilakt-toolbox remaster-nmo FILE.nmo [--scale N] [--model NAME] [--relief S]
drahtseilakt-toolbox relief DIR [--scale N] [--relief S]
drahtseilakt-toolbox import-maps DIR [--to DIR] [--catalogue FILE] [--unsorted DIR]
drahtseilakt-toolbox import-maps --refresh DIR --catalogue FILE [--source DIR] [--unsorted DIR]
```

**文件夹（2026-11-12）。** 游戏的 `levels/` 中有 `ballance-original/`（你的副本）、`ballance-remaster/`（`remaster` 写入这里）和 `user-levels/`（`import-maps` 默认写入这里；旧名称 `original/`、`remastered/`、`downloaded/` 由游戏在首次启动时改名）。只有当一个关卡位于自己的命名文件夹中，且该文件夹里有一个 `.nmo` 文件或一个开放格式关卡，再加上描述文件 `level.txt`，它才算关卡：`import-maps` 正是这样整理的。

`remaster-open` 就地重制一个开放格式关卡文件夹（更圆的形状、更大的纹理、砖墙浮雕；朴素的原图保留在 `textures.original/`，再次运行不会重复放大）。`remaster-nmo` 在社区地图旁生成 `FILE.remaster/`：其外观与图片，游戏会将其贴到地图的网格上。“回到2004年”模组会以朴素外观显示两者。

不带命令启动时会打开窗口（Linux X11 或 Wayland、Windows、macOS；它会在游戏数据文件夹的 `toolbox.txt` 中记住上次选择的模型和放大倍数）：你的游戏拷贝、放大倍数、Real-ESRGAN 模型、浮雕、“开始制作”按钮、按关卡和部件显示的进度（可取消）、结果，以及一张纹理处理前后的对比。界面文字有英语、俄语、德语和简体中文，按系统语言或在窗口中选择；中文使用系统已有的字体（Noto CJK、Microsoft YaHei、PingFang 等）。

`remaster` 把拷贝中的原版关卡转换到游戏的 `levels/ballance-remaster/`（开放格式），为每个关卡生成更圆的形状（写入关卡、放在原始网格旁边，物理仍使用原始网格，因此通关过程不变），并把纹理放大 N 倍（默认 4；1 表示不改动）。有 Real-ESRGAN 时纹理会补出细节，否则用 Lanczos 放大（小于 64 像素的纹理也用 Lanczos，因为它会在上面凭空编造细节）。Real-ESRGAN 先在 toolbox 程序旁的 `realesrgan/` 中查找，然后在 PATH 中查找；所用版本固定在 [third-party.txt](third-party.txt) 中。原始纹理保存在 `textures.original/`。每个关卡的部件（球、模块、变球器）也以同样方式重制到它自己的 `game/` 文件夹中，连同其天空、粒子、阴影和声音，因此关卡无需拷贝中的关卡文件也能游玩（见游戏的 open-format.md）。原版关卡每次运行都会从拷贝重新构建。作为平铺重复的纹理放大时会在四周带上其另一侧的内容，因此平铺接缝处不会出现缝隙。使用 `--relief S` 时，每个受光照的纹理还会根据其自身图像生成一张强度为 S 的法线贴图，游戏在重制关卡中使用它。默认关闭：浮雕是从图像推测出来的，会加深每一条裂缝和缝隙（强度 8 时世界看起来像被酸蚀过）；请保持较低，1 到 3。砖砌始终生成：纹理显示混凝土中的砖块（地板的砖带）时，按颜色识别砖块（米色或灰色中的赤陶色），使其凸起、砖间灰缝凹下，游戏以视差绘制，因此在掠射视角下砖块也会凸显；木材、石材和金属不受影响。`relief DIR` 为开放格式的单个关卡文件夹重新生成凹凸，而不重制它。`--model` 可选择放在其 `models/` 文件夹中的其他 Real-ESRGAN 模型（默认 realesrgan-x4plus，随附提供）。`remaster`（或单独的 `sounds`）还会用拷贝中的音效和音乐在游戏数据文件夹中生成“原版”音效主题；它属于原版，因此只留在玩家的电脑上。更新 toolbox 后再运行一次即可更新关卡。

在每张大图片（512 x 512 及以上）旁边，`remaster` 还会写出为显卡准备好的版本（`name.dtx`：不透明彩色图片用 BC1，带 alpha 的图片和法线贴图用 BC7，包含全部 mip 级；编码器是我们自己的）。游戏会加载它们而不是 PNG：4K 的关卡几秒内即可加载，显存只占约五分之一。PNG 仍然保留（供下次运行使用，游戏在需要时也会退回使用它们）；一次运行中放大过的图片会被复制给每个带有相同图片的部件，因此运行更快，游戏也只上传一次。旧版 toolbox 制作的关卡没有这些文件：请再运行一次 `remaster`。

`import-maps DIR` 把下载的自制地图合集（已解压的压缩包：任意深度的文件夹、图片、文档）整理到游戏的 `levels/user-levels/`（或 `--to DIR`），整理成游戏列出的结构（游戏的 levels.md，“一个关卡就是一个文件夹”）：每张地图一个文件夹，包含它的 `.nmo`、旁边的图片作为 `preview.jpg`/`.png`，以及说明来源的 `level.txt`（文件夹名不得不丢掉字符时还写入完整标题）。通用文件名（“45-Degree Rotated Maps” 中的 `Level_07.NMO`）按合集命名（“45-Degree Rotated - Level 07”）；同一文件出现两次只算一个关卡；压缩包（`.tar.gz`、`.zip`……）不打开，而是搁置（见下文）；名称在 Linux、Windows 和 macOS 上都可用（无保留名称，末尾无点或空格，最多 40 个字符，按单词截断并加“…”，第二个 “Hello Mars” 变为 “Hello Mars (2)”）。从不跟随链接，深度和数量有上限，源文件只读，每个关卡先写入隐藏文件夹再重命名到位。再次运行只添加新内容。窗口中也有同样的功能：一个输入框和 Import 按钮。请在 `levels/user-levels/` 之外（例如下载文件夹）解压下载内容：位于导入目标文件夹内的源会被拒绝。下载内容中无法读取的文件夹会被跳过并列出；打印的名称中的控制字符会被转义。
窗口中每个路径输入框都有“浏览…”按钮（游戏副本另有“镜像…”，用于光盘镜像：.cue、.bin、.iso），打开系统选择器；所选路径与手动输入的一样被检查，并记住该文件夹。Linux 上需要带后端（-gtk、-kde、-gnome）的 xdg-desktop-portal；没有时输入框仍可手动输入。

地图生成后,工具箱会请游戏(旁边的 `drahtseilakt` 或 PATH 中的)检查每一张:结果为每张地图给出结论(完全支持、可玩但有差异、可能无法通关、无法加载),以及游戏尚不支持的内容,例如未知的脚本块、由脚本驱动而静止的部件、作者未附带的贴图、脚本的声音。检查结果以 `report.txt` 保存在地图旁,并显示在游戏的已下载地图列表中。找不到游戏时地图仍会导入;之后可运行 `drahtseilakt --audit-report DIR`。

使用 `--catalogue FILE`（社区目录，[BallanceCustomMaps](https://github.com/pseudoacetyl/BallanceCustomMaps) 的 `CustomMapList.md`）时，每张地图按其文件名和所在文件夹名查找（大小写、标点、空格、全角字符以及 “v1.0”/“V1.0”/“1.0” 的写法都不影响；目录中没有的版本号会去掉后再试，但绝不当作另一个版本；两张地图共用的名称或少于三个字符的名称不匹配任何地图；文件夹名只对其中唯一的一张地图有效）。找到的地图在 `level.txt` 中写入 `title =`（英文名）、`title.zh =`（原名，当它不同且含中文时）、`author =` 和 `difficulty =`：游戏的难度范围是 1–10，因此**目录的 1–5（含半级）乘以 2**（3.5 变为 7；像 2.5–3 这样的范围取中点并向上取整：6）；目录没有难度时不写。使用目录时，既不在目录中、旁边也没有 readme（`.txt`、`.md` 或 `.nfo`，或以 readme 或地图名命名的文档）的地图不会被导入：与压缩包一样，它会按其在下载中的路径被复制到**待整理文件夹**（`--unsorted DIR`，默认是关卡文件夹旁边的 `community-unsorted`），那里的 `README.txt` 列出每一项及其被搁置的原因，供手动查看。有 readme 但不在目录中的地图会被导入，标题取自其名称，不写难度。目录按陌生人的文件读取：最多 4 MiB、20 000 行、每格 1024 个字符，格式错误的行会被跳过，值会被清理。

`import-maps --refresh DIR --catalogue FILE --source DIR` 就地把之前导入的关卡文件夹（`levels/user-levels`）更新为目录中的信息：每个关卡的 `level.txt` 按目录重写（按其 `source =` 行和文件夹名查找；注释、`source =` 行和其他行保留）；不是单个关卡的文件夹（没有地图或有多张），或其地图既不在目录中也没有 readme（在其文件夹中，或在合集 `--source DIR` 中其来源旁边，`source =` 的路径相对于该合集）的关卡文件夹，会被移动到待整理文件夹（在同一磁盘上重命名，从不删除）。只会移动由 import-maps 创建的文件夹（`level.txt` 中有 `source =` 行）；无法查看来源的关卡保留不动。目录中没有对应值的行（例如你自己写的 `author =`）会保留。

toolbox 从不联网，也从不改动玩家的原版拷贝。

## 构建

toolbox 与游戏共用关卡读取代码：需要在旁边的 `../drahtseilakt` 放一份游戏仓库的克隆。然后执行 `cargo build --release --offline`。若要使用 Real-ESRGAN，把固定版本的下载包解压到 `third_party/realesrgan/`，并把 `target/release/realesrgan` 链接到该目录。

## 谁做的

代码和文档由 **Claude**（Anthropic 的 AI 模型，在 Claude Code 中工作）编写。**Veresk** 主持项目：想法、决定、计划，以及判断什么是对的。提交记录中署有两人的名字。

GPL-3.0-only，与游戏相同。
