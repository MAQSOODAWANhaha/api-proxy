// 导出所有API模块

export { AuthAPI } from './auth'
export { UserAPI } from './user'
export { ApiKeyAPI } from './apiKey'
export { StatisticsAPI } from './statistics'
export { SystemAPI } from './system'

// 重新导入以确保在统一API类中可用
import { AuthAPI } from './auth'
import { UserAPI } from './user'  
import { ApiKeyAPI } from './apiKey'
import { StatisticsAPI } from './statistics'
import { SystemAPI } from './system'

// 统一的API类
export class API {
  static auth = AuthAPI
  static user = UserAPI
  static apiKey = ApiKeyAPI
  static statistics = StatisticsAPI
  static system = SystemAPI
}