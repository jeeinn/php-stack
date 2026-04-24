// Test setup file for Vue components
import { config } from '@vue/test-utils'

// Global test setup - configure global properties if needed
// config.global.mocks = {}

// Mock Tauri API calls
(window as any).invoke = async (command: string, args?: any) => {
  console.log(`Mock invoke called: ${command}`, args)
  // Return mock responses based on command
  switch (command) {
    case 'check_docker':
      return { available: true, version: '20.10.0' }
    case 'list_containers':
      return []
    case 'get_env_config':
      return {}
    default:
      return null
  }
}
