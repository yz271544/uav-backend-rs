# 无人机反制预警系统 - Rust后端

这是使用Rust语言实现的无人机反制预警系统后端服务。该系统可以模拟生成多架无人机并在指定雷达范围内移动，通过WebSocket实时推送无人机数据到前端。

## 功能特点

- 使用Rust语言开发，高性能、内存安全
- 基于Tokio异步运行时
- 使用Warp框架提供HTTP服务和WebSocket支持
- 生成和管理无人机数据
- 确保无人机在指定雷达范围内移动

## 雷达配置

当前配置的雷达信息：
- 纬度：37.761196
- 经度：112.531004
- 高度：200米
- 覆盖半径：2000米

## 安装依赖

确保已安装Rust工具链，然后执行：

```bash
cargo build
```

## 运行服务

```bash
cargo run
```

服务将在 http://localhost:3001 上运行。

## 与前端连接

前端可通过WebSocket连接到后端服务：

```javascript
const socket = new WebSocket('ws://localhost:3001/socket.io');

socket.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.UavUpdate) {
    // 处理无人机更新数据
    console.log('无人机更新:', data.UavUpdate);
  } else if (data.UavRemove) {
    // 处理无人机移除事件
    console.log('无人机移除:', data.UavRemove);
  }
};
```

## 消息格式

消息通过JSON格式的WebSocket消息发送，有两种类型：

1. 无人机更新消息:
```json
{"UavUpdate": {"id": "uuid", "latitude": 37.123, "longitude": 112.456, "altitude": 200.0, "timestamp": 1629123456789, "is_dangerous": true}}
```

2. 无人机移除消息:
```json
{"UavRemove": "uuid"}
```

## 健康检查API

- GET /api/health - 返回服务状态和当前无人机数量 