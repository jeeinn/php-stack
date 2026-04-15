#[cfg(test)]
mod tests {
    use crate::docker::manager::DockerManager;
    use crate::docker::mirror::MirrorManager;

    #[tokio::test]
    async fn test_docker_manager_init() {
        let manager = DockerManager::new();
        assert!(manager.is_ok(), "DockerManager 应该能够初始化，即使 Docker 未运行（懒连接）");
    }

    #[test]
    fn test_php_mirror_commands() {
        let aliyun_cmds = MirrorManager::get_php_mirror_commands("aliyun");
        assert_eq!(aliyun_cmds.len(), 2);
        assert!(aliyun_cmds[0].contains("aliyun.com"));

        let ustc_cmds = MirrorManager::get_php_mirror_commands("ustc");
        assert_eq!(ustc_cmds.len(), 2);
        assert!(ustc_cmds[0].contains("ustc.edu.cn"));

        let unknown_cmds = MirrorManager::get_php_mirror_commands("unknown");
        assert_eq!(unknown_cmds.len(), 0);
    }
}
