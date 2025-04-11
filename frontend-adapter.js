/**
 * Rust后端适配器
 * 
 * 这个文件提供了一个适配层，允许现有Vue前端连接到Rust后端
 * 它模拟了socket.io的接口，但实际上使用原生WebSocket
 */

class RustBackendAdapter {
  constructor(url = 'ws://localhost:3001/socket.io') {
    this.url = url;
    this.socket = null;
    this.connected = false;
    this.eventHandlers = {
      'connect': [],
      'disconnect': [],
      'connect_error': [],
      'uav-update': [],
      'uav-remove': []
    };
    this.reconnectTimer = null;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 10;
  }

  /**
   * 连接到后端
   */
  connect() {
    if (this.socket) {
      this.socket.close();
    }

    try {
      console.log('连接到Rust后端:', this.url);
      this.socket = new WebSocket(this.url);

      this.socket.onopen = () => {
        console.log('已连接到Rust后端');
        this.connected = true;
        this.reconnectAttempts = 0;
        this._triggerEvent('connect');
      };

      this.socket.onclose = () => {
        console.log('与Rust后端断开连接');
        this.connected = false;
        this._triggerEvent('disconnect');
        this._scheduleReconnect();
      };

      this.socket.onerror = (error) => {
        console.error('WebSocket连接错误:', error);
        this._triggerEvent('connect_error', error);
      };

      this.socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          
          // Rust后端使用的是枚举消息格式
          if (data.UavUpdate) {
            this._triggerEvent('uav-update', data.UavUpdate);
          } else if (data.UavRemove) {
            this._triggerEvent('uav-remove', data.UavRemove);
          }
        } catch (e) {
          console.error('解析WebSocket消息失败:', e, event.data);
        }
      };
    } catch (error) {
      console.error('创建WebSocket连接失败:', error);
      this._triggerEvent('connect_error', error);
      this._scheduleReconnect();
    }
  }

  /**
   * 断开连接
   */
  disconnect() {
    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }
    
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  /**
   * 注册事件处理函数
   * @param {string} event 事件名称
   * @param {Function} callback 回调函数
   */
  on(event, callback) {
    if (!this.eventHandlers[event]) {
      this.eventHandlers[event] = [];
    }
    this.eventHandlers[event].push(callback);
  }

  /**
   * 触发事件
   * @param {string} event 事件名称
   * @param {any} data 事件数据
   */
  _triggerEvent(event, data) {
    if (this.eventHandlers[event]) {
      for (const handler of this.eventHandlers[event]) {
        handler(data);
      }
    }
  }

  /**
   * 安排重连
   */
  _scheduleReconnect() {
    if (this.reconnectTimer) {
      return;
    }

    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      const delay = Math.min(1000 * Math.pow(1.5, this.reconnectAttempts), 30000);
      console.log(`安排在 ${delay}ms 后重新连接`);
      
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.reconnectAttempts++;
        console.log(`重连尝试 ${this.reconnectAttempts}/${this.maxReconnectAttempts}`);
        this.connect();
      }, delay);
    } else {
      console.error('达到最大重连次数，放弃重连');
    }
  }
}

/**
 * 创建一个模拟的socket.io客户端
 * 实际上使用原生WebSocket连接Rust后端
 */
function createRustSocketClient() {
  const adapter = new RustBackendAdapter();
  
  // 模拟socket.io接口
  const mockSocketIo = {
    on: (event, callback) => adapter.on(event, callback),
    disconnect: () => adapter.disconnect(),
    // 以下方法在原始Socket.IO中使用，但此处不需要实现
    emit: () => {},
    id: 'rust-websocket-client'
  };
  
  // 立即连接
  adapter.connect();
  
  return mockSocketIo;
}

// 导出工厂函数
export { createRustSocketClient }; 