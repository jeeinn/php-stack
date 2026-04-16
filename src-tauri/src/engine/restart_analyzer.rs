use std::collections::HashMap;

/// 重启影响分析结果
#[derive(Debug, Clone)]
pub struct RestartImpact {
    /// 需要重启的服务列表
    pub services_to_restart: Vec<String>,
    /// 依赖链说明（用于日志展示）
    pub dependency_chain: Vec<String>,
    /// 影响的服务数量
    pub total_affected: usize,
}

/// 重启分析器 - 负责分析服务依赖关系和影响范围
pub struct RestartAnalyzer;

impl RestartAnalyzer {
    /// 分析服务依赖关系
    /// 返回：哪些服务依赖指定的服务
    pub fn analyze_dependencies(
        target_service: &str,
        all_services: &[String],
    ) -> RestartImpact {
        let mut services_to_restart = Vec::new();
        let mut dependency_chain = Vec::new();
        
        // 1. 首先添加目标服务本身
        services_to_restart.push(target_service.to_string());
        dependency_chain.push(format!(" 目标服务: {}", target_service));
        
        // 2. 构建依赖关系图
        let dependencies = Self::build_dependency_graph();
        
        // 3. 查找所有依赖目标服务的其他服务
        let dependents = Self::find_dependents(target_service, &dependencies);
        
        // 4. 过滤出已安装的服务
        let affected = dependents
            .into_iter()
            .filter(|s| all_services.contains(s))
            .collect::<Vec<_>>();
        
        for service in &affected {
            services_to_restart.push(service.clone());
            dependency_chain.push(format!("  ↳ {} 依赖 {}", service, target_service));
        }
        
        RestartImpact {
            services_to_restart: services_to_restart.clone(),
            dependency_chain,
            total_affected: services_to_restart.len(),
        }
    }
    
    /// 构建服务依赖关系图
    /// 返回：HashMap<服务名, 该服务依赖的其他服务列表>
    fn build_dependency_graph() -> HashMap<String, Vec<String>> {
        let mut graph = HashMap::new();
        
        // PHP 依赖 MySQL 和 Redis
        graph.insert("php".to_string(), vec!["mysql".to_string(), "redis".to_string()]);
        
        // Nginx 依赖 PHP
        graph.insert("nginx".to_string(), vec!["php".to_string()]);
        
        // MySQL、Redis、MongoDB 没有依赖
        graph.insert("mysql".to_string(), vec![]);
        graph.insert("redis".to_string(), vec![]);
        graph.insert("mongodb".to_string(), vec![]);
        
        graph
    }
    
    /// 查找所有依赖目标服务的其他服务（反向查找）
    /// 例如：目标是 mysql，返回 [php]（因为 PHP 依赖 MySQL）
    fn find_dependents(target_service: &str, dependencies: &HashMap<String, Vec<String>>) -> Vec<String> {
        let mut dependents = Vec::new();
        
        for (service, deps) in dependencies {
            if deps.contains(&target_service.to_string()) {
                dependents.push(service.clone());
            }
        }
        
        dependents
    }
    
    /// 分析修改配置后的影响范围
    /// 用于前端展示影响评估
    pub fn analyze_config_change_impact(
        modified_service: &str,
        installed_services: &[String],
    ) -> RestartImpact {
        Self::analyze_dependencies(modified_service, installed_services)
    }
    
    /// 分析服务更新后的影响（如版本升级）
    /// 与配置修改类似，但影响范围可能更广
    pub fn analyze_service_update_impact(
        updated_service: &str,
        installed_services: &[String],
    ) -> RestartImpact {
        // 服务更新和配置修改的影响范围相同
        // 未来可以扩展为不同的分析逻辑
        Self::analyze_dependencies(updated_service, installed_services)
    }
    
    /// 获取服务名称（去除版本号前缀）
    /// 例如：ps-php-8-2 → php
    pub fn extract_service_name(container_name: &str) -> String {
        container_name
            .trim_start_matches("ps-")
            .split('-')
            .next()
            .unwrap_or(container_name)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_dependency_graph() {
        let graph = RestartAnalyzer::build_dependency_graph();
        
        // PHP 应该依赖 MySQL 和 Redis
        assert!(graph.get("php").unwrap().contains(&"mysql".to_string()));
        assert!(graph.get("php").unwrap().contains(&"redis".to_string()));
        
        // Nginx 应该依赖 PHP
        assert_eq!(graph.get("nginx").unwrap(), &vec!["php".to_string()]);
        
        // MySQL 没有依赖
        assert!(graph.get("mysql").unwrap().is_empty());
    }
    
    #[test]
    fn test_find_dependents() {
        let graph = RestartAnalyzer::build_dependency_graph();
        
        // MySQL 的依赖者：PHP
        let mysql_dependents = RestartAnalyzer::find_dependents("mysql", &graph);
        assert_eq!(mysql_dependents, vec!["php".to_string()]);
        
        // PHP 的依赖者：Nginx
        let php_dependents = RestartAnalyzer::find_dependents("php", &graph);
        assert_eq!(php_dependents, vec!["nginx".to_string()]);
        
        // Redis 的依赖者：PHP
        let redis_dependents = RestartAnalyzer::find_dependents("redis", &graph);
        assert_eq!(redis_dependents, vec!["php".to_string()]);
        
        // Nginx 没有依赖者
        let nginx_dependents = RestartAnalyzer::find_dependents("nginx", &graph);
        assert!(nginx_dependents.is_empty());
    }
    
    #[test]
    fn test_analyze_dependencies_mysql() {
        // 场景：MySQL 配置修改，已安装 PHP + MySQL
        let all_services = vec!["php".to_string(), "mysql".to_string()];
        let impact = RestartAnalyzer::analyze_dependencies("mysql", &all_services);
        
        // 需要重启：mysql + php（因为 PHP 依赖 MySQL）
        assert!(impact.services_to_restart.contains(&"mysql".to_string()));
        assert!(impact.services_to_restart.contains(&"php".to_string()));
        assert_eq!(impact.total_affected, 2);
    }
    
    #[test]
    fn test_analyze_dependencies_redis() {
        // 场景：Redis 配置修改，已安装 PHP + MySQL + Redis
        let all_services = vec![
            "php".to_string(),
            "mysql".to_string(),
            "redis".to_string(),
        ];
        let impact = RestartAnalyzer::analyze_dependencies("redis", &all_services);
        
        // 需要重启：redis + php（因为 PHP 依赖 Redis）
        assert!(impact.services_to_restart.contains(&"redis".to_string()));
        assert!(impact.services_to_restart.contains(&"php".to_string()));
        // MySQL 不应该被重启
        assert!(!impact.services_to_restart.contains(&"mysql".to_string()));
        assert_eq!(impact.total_affected, 2);
    }
    
    #[test]
    fn test_analyze_dependencies_nginx() {
        // 场景：Nginx 配置修改，已安装 Nginx + PHP + MySQL
        let all_services = vec![
            "nginx".to_string(),
            "php".to_string(),
            "mysql".to_string(),
        ];
        let impact = RestartAnalyzer::analyze_dependencies("nginx", &all_services);
        
        // 只需要重启 nginx（没有服务依赖 Nginx）
        assert_eq!(impact.services_to_restart, vec!["nginx".to_string()]);
        assert_eq!(impact.total_affected, 1);
    }
    
    #[test]
    fn test_extract_service_name() {
        assert_eq!(RestartAnalyzer::extract_service_name("ps-php-8-2"), "php");
        assert_eq!(RestartAnalyzer::extract_service_name("ps-mysql-5-7"), "mysql");
        assert_eq!(RestartAnalyzer::extract_service_name("ps-nginx-1-24"), "nginx");
        assert_eq!(RestartAnalyzer::extract_service_name("ps-redis-7-0"), "redis");
    }
}
