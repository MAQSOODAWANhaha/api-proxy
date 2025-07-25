import request from './index'
import type { AxiosPromise } from 'axios'

// Load balancer status interface
export interface LoadBalancerStatus {
  [provider: string]: string // e.g., "OpenAI": "active"
}

// Server info interface
export interface ServerInfo {
  provider: string
  servers: Array<{
    id: number
    name: string
    host: string
    port: number
    weight: number
    use_tls: boolean
    is_healthy: boolean
  }>
}

// Strategy change request
export interface ChangeStrategyRequest {
  provider: string
  strategy: string // "round_robin" | "weighted" | "health_based"
}

// Strategy change response
export interface ChangeStrategyResponse {
  success: boolean
  message: string
  old_strategy: string | null
  new_strategy: string
}

// Server action request
export interface ServerActionRequest {
  provider: string
  server_id: number
  action: string // "enable" | "disable" | "remove"
}

// Server action response
export interface ServerActionResponse {
  success: boolean
  message: string
  affected_server: {
    id: number
    name: string
    action_taken: string
  }
}

// Load balancer metrics
export interface LoadBalancerMetrics {
  [provider: string]: {
    total_servers: number
    healthy_servers: number
    unhealthy_servers: number
    success_rate: number
    servers: Array<{
      address: string
      weight: number
      is_healthy: boolean
      success_requests: number
      failed_requests: number
      avg_response_time_ms: number
      last_health_check: string
      use_tls: boolean
    }>
  }
}

// Get load balancer status
export function getLoadBalancerStatus(): AxiosPromise<LoadBalancerStatus> {
  return request({
    url: '/loadbalancer/status',
    method: 'get'
  })
}

// List all servers across all providers
export function listAllServers(): AxiosPromise<ServerInfo[]> {
  return request({
    url: '/loadbalancer/servers',
    method: 'get'
  })
}

// Add new server
export function addServer(data: {
  provider: string
  host: string
  port: number
  weight?: number
  use_tls?: boolean
}): AxiosPromise<{ success: boolean; message: string }> {
  return request({
    url: '/loadbalancer/servers',
    method: 'post',
    data
  })
}

// Change load balancing strategy
export function changeStrategy(data: ChangeStrategyRequest): AxiosPromise<ChangeStrategyResponse> {
  return request({
    url: '/loadbalancer/strategy',
    method: 'patch',
    data
  })
}

// Perform server action (enable/disable/remove)
export function performServerAction(data: ServerActionRequest): AxiosPromise<ServerActionResponse> {
  return request({
    url: '/loadbalancer/servers/action',
    method: 'post',
    data
  })
}

// Get detailed load balancer metrics
export function getLoadBalancerMetrics(): AxiosPromise<LoadBalancerMetrics> {
  return request({
    url: '/loadbalancer/metrics',
    method: 'get'
  })
}