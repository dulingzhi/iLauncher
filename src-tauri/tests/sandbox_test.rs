// 沙盒系统集成测试

#[cfg(test)]
mod sandbox_integration_tests {
    use ilauncher::plugin::sandbox::*;
    use std::sync::Arc;

    #[test]
    fn test_sandbox_manager_creation() {
        let manager = SandboxManager::new();
        assert!(true, "SandboxManager created successfully");
    }

    #[test]
    fn test_register_system_plugin() {
        let manager = SandboxManager::new();
        let config = SandboxConfig::system("test_plugin");
        
        assert_eq!(config.security_level, SecurityLevel::System);
        assert_eq!(config.enabled, false);
        
        manager.register(config);
        
        let retrieved = manager.get_config("test_plugin");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().plugin_id, "test_plugin");
    }

    #[test]
    fn test_register_restricted_plugin() {
        let manager = SandboxManager::new();
        let config = SandboxConfig::restricted("test_plugin");
        
        assert_eq!(config.security_level, SecurityLevel::Restricted);
        assert_eq!(config.enabled, true);
        assert_eq!(config.timeout_ms, Some(5000));
        assert_eq!(config.max_memory_mb, Some(100));
        
        manager.register(config);
    }

    #[test]
    fn test_permission_inheritance() {
        let system_perms = SecurityLevel::System.default_permissions();
        let trusted_perms = SecurityLevel::Trusted.default_permissions();
        let restricted_perms = SecurityLevel::Restricted.default_permissions();
        let sandboxed_perms = SecurityLevel::Sandboxed.default_permissions();
        
        // System has all permissions
        assert!(system_perms.len() >= trusted_perms.len());
        
        // Trusted has more than Restricted
        assert!(trusted_perms.len() > restricted_perms.len());
        
        // Restricted has more than Sandboxed
        assert!(restricted_perms.len() > sandboxed_perms.len());
    }

    #[test]
    fn test_custom_permissions() {
        let config = SandboxConfig::restricted("test_plugin")
            .with_permission(PluginPermission::ExecuteProgram)
            .with_permission(PluginPermission::NetworkAccess(NetworkScope::All));
        
        let perms = config.effective_permissions();
        assert!(perms.contains(&PluginPermission::ExecuteProgram));
    }

    #[test]
    fn test_file_permission_check() {
        let manager = SandboxManager::new();
        
        let config = SandboxConfig::restricted("test_plugin")
            .with_permission(PluginPermission::FileSystemRead(
                std::path::PathBuf::from("/home/user")
            ));
        
        manager.register(config);
        
        // Should allow reading from /home/user
        let result = manager.check_permission(
            "test_plugin",
            &PluginPermission::FileSystemRead(std::path::PathBuf::from("/home/user/file.txt"))
        );
        assert!(result.is_ok());
        
        // Should deny reading from /etc
        let result = manager.check_permission(
            "test_plugin",
            &PluginPermission::FileSystemRead(std::path::PathBuf::from("/etc/passwd"))
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_network_permission_check() {
        let manager = SandboxManager::new();
        
        let config = SandboxConfig::restricted("test_plugin")
            .with_permission(PluginPermission::NetworkAccess(
                NetworkScope::Domain("api.example.com".to_string())
            ));
        
        manager.register(config);
        
        // Should allow accessing api.example.com
        let result = manager.check_permission(
            "test_plugin",
            &PluginPermission::NetworkAccess(
                NetworkScope::Domain("api.example.com".to_string())
            )
        );
        assert!(result.is_ok());
        
        // Should deny accessing other domains
        let result = manager.check_permission(
            "test_plugin",
            &PluginPermission::NetworkAccess(
                NetworkScope::Domain("evil.com".to_string())
            )
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sandboxed_execution_success() {
        let manager = Arc::new(SandboxManager::new());
        let config = SandboxConfig::restricted("test_plugin");
        manager.register(config);
        
        let executor = SandboxedExecution::<i32>::new("test_plugin".to_string(), manager);
        
        let result = executor.execute(|| async {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(42)
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_sandboxed_execution_timeout() {
        let manager = Arc::new(SandboxManager::new());
        let config = SandboxConfig {
            plugin_id: "slow_plugin".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: None,
            enabled: true,
            timeout_ms: Some(100),
            max_memory_mb: None,
        };
        manager.register(config);
        
        let executor = SandboxedExecution::<()>::new("slow_plugin".to_string(), manager);
        
        let result = executor.execute(|| async {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            Ok(())
        }).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    #[test]
    fn test_system_plugin_bypasses_sandbox() {
        let manager = SandboxManager::new();
        let config = SandboxConfig::system("system_plugin");
        manager.register(config);
        
        // System plugins should pass all permission checks
        let result = manager.check_permission(
            "system_plugin",
            &PluginPermission::ProcessManagement
        );
        assert!(result.is_ok());
    }
}
