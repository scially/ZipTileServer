# 非常实用的紧凑型切片服务器ZipTileServer

## 介绍
非常实用的紧凑型切片服务器，ZipTileServer采用Rust编写，支持异步请求访问，提高请求效率。另外对于紧凑型文件，方便管理备份，可以有效减少拷贝移动时间。
ZIPServer支持以下两种切片：
1. ZIP文件，可以把切片压缩成zip文件，3DTiles、TMS、XYZ等等都可以；
2. PAK文件，由CesiumLab实验室生成的紧凑型切片格式

## 使用方法
1. 配置文件config.yaml,如果没有配置文件，默认端口为8080，切片路径是./tile
2. 配置文件主要就是三个字段，path是切片目录地址，port是运行端口，host是监听IP，一般默认即可。
```
path: "./tile"
port: 60231
host: 127.0.0.1
```
3. Windows下直接双击ZipTileServer，或者在终端下运行`ZipTileServer`命令，如果是成功运行，会有以下信息输出：
```
[2022-11-08T03:00:44Z INFO  ZipTileServer] ZipTileServer listen on 127.0.0.1:60231
[2022-11-08T03:00:44Z INFO  actix_server::builder] Starting 8 workers
[2022-11-08T03:00:44Z INFO  actix_server::server] Actix runtime found; starting in Actix runtime
```
## 服务地址
1. 如果是ZIP文件，请求地址按服务类型分别是：
    1. DEM服务 http://ip:host/tile/{tile_name_without_extension}/layer.json
    2. 3DTiles服务 http://ip:host/tile/{tile_name_without_extension}/tileset.json
    3. TMS服务 http://ip:host/tile/{tile_name_without_extension}/tilemapresource.xml
2. 如果是PAK文件，支持TMS和XYZ两种请求方式：
    1. XYZ服务 http://ip:host/tile/{pak_name_without_extension}/xyz/{z}/{x}/{y}.png
    2. TMS服务 http://ip:host/tile/{pak_name_without_extension}/tms/tilemapresource.xml

## 编译
1. 安装Rust
2. 在工程目录下运行`cargo build --release`，编译后的程序在当前目录的target/release下