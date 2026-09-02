//! DockerManager 真实集成测试
//!
//! 依赖宿主机运行中的 Docker daemon（Docker Desktop / 原生 Docker），
//! 默认以 `#[ignore]` 标记，运行方式：
//!
//! ```sh
//! cargo test --test docker_manager_integration -- --ignored --nocapture
//! ```
//!
//! 覆盖：连接与 ping、运行中容器枚举、ps- 前缀容器过滤。

use app_lib::docker::manager::DockerManager;

#[tokio::test]
#[ignore]
async fn test_docker_ping() {
    let manager = DockerManager::new().expect("连接 Docker daemon 失败");
    manager
        .check_docker_availability()
        .await
        .expect("Docker ping 失败（daemon 未运行？）");
}

#[tokio::test]
#[ignore]
async fn test_list_all_running_containers() {
    let manager = DockerManager::new().expect("连接 Docker daemon 失败");
    let containers = manager
        .list_all_running_containers()
        .await
        .expect("列出运行中容器失败");

    eprintln!("运行中容器数: {}", containers.len());
    for c in &containers {
        eprintln!(
            "  - {} | image={} | state={} | ports={:?}",
            c.name, c.image, c.state, c.ports
        );
    }

    assert!(
        !containers.is_empty(),
        "宿主机应至少有一个运行中的容器"
    );
}

#[tokio::test]
#[ignore]
async fn test_list_ps_containers() {
    let manager = DockerManager::new().expect("连接 Docker daemon 失败");
    let containers = manager
        .list_ps_containers()
        .await
        .expect("列出 ps- 前缀容器失败");

    eprintln!("ps- 前缀容器数: {}", containers.len());
    for c in &containers {
        eprintln!("  - {} | image={} | state={}", c.name, c.image, c.state);
        assert!(
            c.name.starts_with("ps-"),
            "不应包含非 ps- 前缀容器（filter 失效）: {}",
            c.name
        );
    }
}
