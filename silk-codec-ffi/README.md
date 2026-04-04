# silk-codec-ffi

`silk-codec-ffi` 是单独的 C FFI crate，用来生成 C 头文件和可静态链接的库。

构建：

```bash
cargo build -p silk-codec-ffi --release
```

产物：

- `target/release/libsilk_codec_ffi.a`
- `target/ffi/silk_codec.h`

启用 ffmpeg：

```bash
cargo build -p silk-codec-ffi --release --features ffmpeg
```

C 示例编译：

```bash
cc silk-codec-ffi/examples/c_api_smoke.c \
  -Itarget/ffi \
  target/release/libsilk_codec_ffi.a \
  -o /tmp/c_api_smoke
```
