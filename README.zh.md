# drahtseilakt-toolbox

[English](README.md) · [Русский](README.ru.md) · [Deutsch](README.de.md) · **简体中文**

在玩家自己的电脑上、用玩家自己的游戏拷贝，为新的 Ballance 引擎 [Drahtseilakt](https://github.com/eugene-vereshchagin/drahtseilakt) 准备关卡。游戏只游玩其关卡文件夹中的内容；让关卡看起来更现代的所有繁重工作都在这里完成，游戏本身不需要其中任何东西。

```
drahtseilakt-toolbox remaster [--scale N] [--data DIR]
```

`remaster` 把拷贝中的原版关卡转换到游戏的 `levels/remastered/`（开放格式），为每个关卡生成更圆的形状（写入关卡、放在原始网格旁边，物理仍使用原始网格，因此通关过程不变），并把纹理放大 N 倍（默认 4；1 表示不改动）。有 Real-ESRGAN 时纹理会补出细节，否则用 Lanczos 放大（小于 64 像素的纹理也用 Lanczos，因为它会在上面凭空编造细节）。Real-ESRGAN 先在 toolbox 程序旁的 `realesrgan/` 中查找，然后在 PATH 中查找；所用版本固定在 [third-party.txt](third-party.txt) 中。原始纹理保存在 `textures.original/`。所有关卡共用的部件（球、模块、变球器）也以同样方式重制到隐藏的 `levels/remastered/.shared/` 中，游戏在重制关卡中按此绘制它们。原版关卡每次运行都会从拷贝重新构建。更新 toolbox 后再运行一次即可更新关卡。

toolbox 从不联网，也从不改动玩家的原版拷贝。

## 构建

toolbox 与游戏共用关卡读取代码：需要在旁边的 `../ballance/drahtseilakt` 放一份游戏仓库的克隆。然后执行 `cargo build --release --offline`。若要使用 Real-ESRGAN，把固定版本的下载包解压到 `third_party/realesrgan/`，并把 `target/release/realesrgan` 链接到该目录。

GPL-3.0-only，与游戏相同。
