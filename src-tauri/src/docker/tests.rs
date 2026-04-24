#[cfg(test)]
mod tests {
    use crate::docker::manager::DockerManager;

    #[tokio::test]
    async fn test_docker_manager_init() {
        let manager = DockerManager::new();
        assert!(manager.is_ok(), "DockerManager 应该能够初始化，即使 Docker 未运行（懒连接）");
    }
}
