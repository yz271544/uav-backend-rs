use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    f64::consts::PI,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time;
use tokio_stream::wrappers::BroadcastStream;
use tokio::sync::broadcast;
use uuid::Uuid;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

// 定义无人机结构
#[derive(Clone, Debug, Serialize, Deserialize)]
struct UAV {
    id: String,
    latitude: f64,
    longitude: f64,
    altitude: f64,
    timestamp: i64,
    is_dangerous: bool,
}

// 定义雷达信息结构
#[derive(Clone, Debug)]
struct Radar {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    radius: f64, // 单位：米
}

// 定义WebSocket消息类型
#[derive(Clone, Debug, Serialize, Deserialize)]
enum WSMessage {
    UavUpdate(UAV),
    UavRemove(String),
}

// 共享状态
struct AppState {
    uavs: HashMap<String, UAV>,
    radar: Radar,
}

#[tokio::main]
async fn main() {
    // 初始化日志
    env_logger::init();
    log::info!("启动无人机反制预警系统后端 - Rust版");

    // 配置雷达
    let radar = Radar {
        latitude: 37.761196,
        longitude: 112.531004,
        altitude: 200.0,
        radius: 2000.0,
    };

    // 初始化应用状态
    let app_state = Arc::new(Mutex::new(AppState {
        uavs: HashMap::new(),
        radar,
    }));

    // 创建广播通道
    let (tx, _rx) = broadcast::channel::<WSMessage>(100);
    let tx = Arc::new(tx);

    // 创建WebSocket处理程序
    let ws_route = warp::path("socket.io")
        .and(warp::ws())
        .and(with_app_state(app_state.clone()))
        .and(with_broadcaster(tx.clone()))
        .map(|ws: warp::ws::Ws, state, broadcaster| {
            ws.on_upgrade(move |socket| handle_websocket(socket, state, broadcaster))
        });

    // 设置健康检查API
    let health_route = warp::path!("api" / "health").map(move || {
        let state = app_state.lock().unwrap();
        let uav_count = state.uavs.len();
        warp::reply::json(&serde_json::json!({
            "status": "ok",
            "uavCount": uav_count
        }))
    });

    // 初始化生成一些无人机
    initialize_uavs(Arc::clone(&app_state), 5);

    // CORS配置
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["Content-Type"]);

    // 启动定时任务
    let app_state_for_add = Arc::clone(&app_state);
    let sender_for_add = tx.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(3000));
        loop {
            interval.tick().await;
            add_random_uav(Arc::clone(&app_state_for_add), sender_for_add.clone()).await;
        }
    });

    let app_state_for_update = Arc::clone(&app_state);
    let sender_for_update = tx.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(2000));
        loop {
            interval.tick().await;
            update_uavs_position(Arc::clone(&app_state_for_update), sender_for_update.clone()).await;
        }
    });

    // 组合路由
    let routes = ws_route
        .or(health_route)
        .with(cors);

    // 启动服务器
    let port = 3001;
    log::info!("服务器运行在: http://localhost:{}", port);
    
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
}

// 处理WebSocket连接
async fn handle_websocket(
    ws: WebSocket,
    state: Arc<Mutex<AppState>>,
    broadcaster: Arc<broadcast::Sender<WSMessage>>,
) {
    let (mut ws_tx, _ws_rx) = ws.split();
    let client_id = Uuid::new_v4().to_string();
    log::info!("客户端已连接: {}", client_id);

    // 发送当前所有无人机数据
    {
        let state = state.lock().unwrap();
        for uav in state.uavs.values() {
            let message = WSMessage::UavUpdate(uav.clone());
            let json = serde_json::to_string(&message).unwrap();
            if let Err(e) = ws_tx.send(Message::text(json)).await {
                log::error!("发送无人机数据失败: {}", e);
                return;
            }
        }
    }

    // 订阅广播消息
    let mut rx = BroadcastStream::new(broadcaster.subscribe());
    
    // 监听广播消息并转发到WebSocket
    while let Some(Ok(msg)) = rx.next().await {
        let json = serde_json::to_string(&msg).unwrap();
        if let Err(e) = ws_tx.send(Message::text(json)).await {
            log::error!("发送消息失败: {}", e);
            break;
        }
    }

    log::info!("客户端已断开连接: {}", client_id);
}

// 为Warp过滤器提供状态
fn with_app_state(
    state: Arc<Mutex<AppState>>,
) -> impl Filter<Extract = (Arc<Mutex<AppState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

// 为Warp过滤器提供广播者
fn with_broadcaster(
    broadcaster: Arc<broadcast::Sender<WSMessage>>,
) -> impl Filter<Extract = (Arc<broadcast::Sender<WSMessage>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || broadcaster.clone())
}

// 初始化生成无人机
fn initialize_uavs(app_state: Arc<Mutex<AppState>>, count: usize) {
    log::info!("初始化生成 {} 架无人机...", count);
    let mut state = app_state.lock().unwrap();
    
    for _ in 0..count {
        let uav = generate_random_uav(&state.radar);
        log::info!(
            "初始化无人机: {}, 危险状态: {}",
            uav.id,
            uav.is_dangerous
        );
        state.uavs.insert(uav.id.clone(), uav);
    }
}

// 随机生成无人机
fn generate_random_uav(radar: &Radar) -> UAV {
    let mut rng = thread_rng();
    
    // 在雷达范围内生成随机位置
    // 随机生成极坐标角度和距离
    let angle = rng.gen_range(0.0..2.0 * PI);
    let distance = rng.gen_range(0.0..radar.radius);
    
    // 将极坐标转换为经纬度偏移量
    // 注意：这里假设1度经度约等于111km，但实际上经度的距离会随纬度变化
    let lat_offset = (distance / 111000.0) * angle.cos();
    let lon_offset = (distance / (111000.0 * (radar.latitude * PI / 180.0).cos())) * angle.sin();
    
    // 计算最终的经纬度
    let latitude = radar.latitude + lat_offset;
    let longitude = radar.longitude + lon_offset;
    
    // 随机高度，在雷达高度上下波动
    let altitude = radar.altitude + (rng.gen::<f64>() - 0.5) * 100.0;
    
    // 随机判断是否为危险无人机
    let is_dangerous = rng.gen_bool(0.5);
    
    UAV {
        id: Uuid::new_v4().to_string(),
        latitude,
        longitude,
        altitude,
        timestamp: Utc::now().timestamp_millis(),
        is_dangerous,
    }
}

// 添加新的随机无人机
async fn add_random_uav(app_state: Arc<Mutex<AppState>>, broadcaster: Arc<broadcast::Sender<WSMessage>>) {
    let mut rng = thread_rng();
    
    // 限制最大无人机数量为15
    let should_add = {
        let state = app_state.lock().unwrap();
        state.uavs.len() < 15 && rng.gen_bool(0.5)
    };
    
    if should_add {
        let new_uav = {
            let mut state = app_state.lock().unwrap();
            let uav = generate_random_uav(&state.radar);
            state.uavs.insert(uav.id.clone(), uav.clone());
            uav
        };
        
        // 广播新无人机数据
        let message = WSMessage::UavUpdate(new_uav.clone());
        let _ = broadcaster.send(message);
        
        log::info!(
            "添加新无人机: {}, 危险状态: {}",
            new_uav.id,
            new_uav.is_dangerous
        );
    }
}

// 更新所有无人机位置
async fn update_uavs_position(app_state: Arc<Mutex<AppState>>, broadcaster: Arc<broadcast::Sender<WSMessage>>) {
    let mut rng = thread_rng();
    let mut to_remove = Vec::new();
    
    // 更新无人机位置
    {
        let mut state = app_state.lock().unwrap();
        
        for (id, uav) in state.uavs.iter_mut() {
            // 随机小幅度移动
            let lat_delta = (rng.gen::<f64>() - 0.5) * 0.005;
            let lon_delta = (rng.gen::<f64>() - 0.5) * 0.005;
            let alt_delta = (rng.gen::<f64>() - 0.5) * 5.0;
            
            // 计算新位置
            let mut new_lat = uav.latitude + lat_delta;
            let mut new_lon = uav.longitude + lon_delta;
            let new_alt = uav.altitude + alt_delta;
            
            // 计算到雷达中心的距离
            let distance = calculate_distance(
                new_lat, new_lon,
                state.radar.latitude, state.radar.longitude,
            );
            
            // 如果超出雷达范围，向雷达中心移动
            if distance > state.radar.radius {
                let dir_lat = state.radar.latitude - new_lat;
                let dir_lon = state.radar.longitude - new_lon;
                
                new_lat = uav.latitude + (rng.gen::<f64>() * 0.002 * dir_lat.signum());
                new_lon = uav.longitude + (rng.gen::<f64>() * 0.002 * dir_lon.signum());
            }
            
            // 更新无人机位置
            uav.latitude = new_lat;
            uav.longitude = new_lon;
            uav.altitude = new_alt;
            uav.timestamp = Utc::now().timestamp_millis();
            
            // 随机改变危险状态（低概率）
            if rng.gen::<f64>() > 0.95 {
                uav.is_dangerous = !uav.is_dangerous;
            }
            
            // 发送更新
            let message = WSMessage::UavUpdate(uav.clone());
            let _ = broadcaster.send(message);
            
            // 随机移除无人机（低概率）
            if rng.gen::<f64>() > 0.97 {
                to_remove.push(id.clone());
            }
        }
        
        // 移除标记的无人机
        for id in &to_remove {
            state.uavs.remove(id);
        }
    }
    
    // 发送移除通知
    for id in to_remove {
        let message = WSMessage::UavRemove(id.clone());
        let _ = broadcaster.send(message);
        log::info!("移除无人机: {}", id);
    }
}

// 计算两点间距离（单位：米）
fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371000.0; // 地球半径，单位米
    let d_lat = (lat2 - lat1) * PI / 180.0;
    let d_lon = (lon2 - lon1) * PI / 180.0;
    let a = 
        (d_lat / 2.0).sin() * (d_lat / 2.0).sin() +
        (lat1 * PI / 180.0).cos() * (lat2 * PI / 180.0).cos() * 
        (d_lon / 2.0).sin() * (d_lon / 2.0).sin();
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}
