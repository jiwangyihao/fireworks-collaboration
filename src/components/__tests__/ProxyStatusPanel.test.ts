import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import ProxyStatusPanel from '../ProxyStatusPanel.vue'
import { useConfigStore } from '../../stores/config'

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {})
}))

describe('ProxyStatusPanel.vue', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('renders status panel', () => {
    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.find('h3').text()).toBe('代理状态')
    expect(wrapper.find('.status-grid').exists()).toBe(true)
  })

  it('displays proxy mode from config', () => {
    const configStore = useConfigStore()
    configStore.cfg = {
      proxy: {
        mode: 'http',
        url: 'http://proxy.example.com:8080',
        disableCustomTransport: false,
        healthCheckUrl: 'https://www.google.com',
        healthCheckIntervalSec: 60,
        healthCheckTimeoutSec: 10,
        fallbackAfterFailures: 3,
        recoverAfterSuccesses: 2,
        fallbackCooldownSec: 300,
        debugProxyLogging: false
      }
    } as any

    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.text()).toContain('HTTP/HTTPS')
  })

  it('displays disabled state when mode is off', () => {
    const configStore = useConfigStore()
    configStore.cfg = {
      proxy: {
        mode: 'off'
      }
    } as any

    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.text()).toContain('已禁用')
  })

  it('shows proxy URL when configured', () => {
    const configStore = useConfigStore()
    configStore.cfg = {
      proxy: {
        mode: 'http',
        url: 'http://proxy.example.com:8080'
      }
    } as any

    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.text()).toContain('proxy.example.com')
  })

  it('sanitizes proxy URL to hide credentials', () => {
    const configStore = useConfigStore()
    configStore.cfg = {
      proxy: {
        mode: 'http',
        url: 'http://user:pass@proxy.example.com:8080'
      }
    } as any

    const wrapper = mount(ProxyStatusPanel)
    // Should show host but not credentials
    expect(wrapper.text()).toContain('proxy.example.com')
    expect(wrapper.text()).not.toContain('user')
    expect(wrapper.text()).not.toContain('pass')
  })

  it('shows fallback button when state is enabled', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    // Manually set state to enabled
    await wrapper.vm.$data.proxyState = 'enabled'
    await wrapper.vm.$nextTick()
    
    const fallbackBtn = wrapper.find('.fallback-btn')
    expect(fallbackBtn.exists()).toBe(true)
    expect(fallbackBtn.text()).toContain('强制降级')
  })

  it('shows recovery button when state is fallback', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    // Set state to fallback
    wrapper.vm.proxyState = 'fallback'
    await wrapper.vm.$nextTick()
    
    const recoveryBtn = wrapper.find('.recovery-btn')
    expect(recoveryBtn.exists()).toBe(true)
    expect(recoveryBtn.text()).toContain('强制恢复')
  })

  it('calls force_proxy_fallback on fallback button click', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue(undefined)

    const wrapper = mount(ProxyStatusPanel)
    wrapper.vm.proxyState = 'enabled'
    await wrapper.vm.$nextTick()
    
    const fallbackBtn = wrapper.find('.fallback-btn')
    await fallbackBtn.trigger('click')
    
    expect(mockInvoke).toHaveBeenCalledWith('force_proxy_fallback', {
      reason: '用户手动触发降级'
    })
  })

  it('calls force_proxy_recovery on recovery button click', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue(undefined)

    const wrapper = mount(ProxyStatusPanel)
    wrapper.vm.proxyState = 'fallback'
    await wrapper.vm.$nextTick()
    
    const recoveryBtn = wrapper.find('.recovery-btn')
    await recoveryBtn.trigger('click')
    
    expect(mockInvoke).toHaveBeenCalledWith('force_proxy_recovery')
  })

  it('displays fallback reason when in fallback state', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    wrapper.vm.proxyState = 'fallback'
    wrapper.vm.fallbackReason = '连接超时'
    await wrapper.vm.$nextTick()
    
    expect(wrapper.text()).toContain('连接超时')
  })

  it('displays failure count in fallback state', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    wrapper.vm.proxyState = 'fallback'
    wrapper.vm.failureCount = 5
    await wrapper.vm.$nextTick()
    
    expect(wrapper.text()).toContain('失败次数')
    expect(wrapper.text()).toContain('5')
  })

  it('shows health check stats when available', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    wrapper.vm.healthCheckSuccessRate = 0.85
    await wrapper.vm.$nextTick()
    
    expect(wrapper.text()).toContain('健康检查成功率')
    expect(wrapper.text()).toContain('85.0%')
  })

  it('shows next health check countdown in recovering state', async () => {
    const wrapper = mount(ProxyStatusPanel)
    
    wrapper.vm.proxyState = 'recovering'
    wrapper.vm.nextHealthCheckIn = 30
    await wrapper.vm.$nextTick()
    
    expect(wrapper.text()).toContain('下次健康检查')
    expect(wrapper.text()).toContain('30秒后')
  })

  it('displays custom transport status', () => {
    const configStore = useConfigStore()
    configStore.cfg = {
      proxy: {
        mode: 'http',
        disableCustomTransport: true
      }
    } as any

    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.text()).toContain('已禁用')
  })

  it('listens for proxy state events on mount', async () => {
    const { listen } = await import('@tauri-apps/api/event')
    const mockListen = listen as ReturnType<typeof vi.fn>

    mount(ProxyStatusPanel)
    
    expect(mockListen).toHaveBeenCalledWith('proxy://state', expect.any(Function))
  })

  it('updates state from proxy event', async () => {
    const { listen } = await import('@tauri-apps/api/event')
    const mockListen = listen as ReturnType<typeof vi.fn>
    
    let eventHandler: any
    mockListen.mockImplementation((eventName, handler) => {
      if (eventName === 'proxy://state') {
        eventHandler = handler
      }
      return Promise.resolve(() => {})
    })

    const wrapper = mount(ProxyStatusPanel)
    await wrapper.vm.$nextTick()
    
    // Simulate event
    eventHandler({
      payload: {
        proxy_state: 'Fallback',
        fallback_reason: 'Health check failed',
        failure_count: 3
      }
    })
    
    await wrapper.vm.$nextTick()
    
    expect(wrapper.vm.proxyState).toBe('fallback')
    expect(wrapper.vm.fallbackReason).toBe('Health check failed')
    expect(wrapper.vm.failureCount).toBe(3)
  })

  it('disables control buttons while controlling', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockImplementation(() => new Promise(resolve => setTimeout(resolve, 100)))

    const wrapper = mount(ProxyStatusPanel)
    wrapper.vm.proxyState = 'enabled'
    await wrapper.vm.$nextTick()
    
    const fallbackBtn = wrapper.find('.fallback-btn')
    await fallbackBtn.trigger('click')
    
    // Button should be disabled during operation
    expect(fallbackBtn.attributes('disabled')).toBeDefined()
  })

  it('applies correct health check status class', () => {
    const wrapper = mount(ProxyStatusPanel)
    
    // High success rate - success class
    expect(wrapper.vm.getHealthCheckClass(0.9)).toBe('success')
    
    // Medium success rate - warning class
    expect(wrapper.vm.getHealthCheckClass(0.6)).toBe('warning')
    
    // Low success rate - error class
    expect(wrapper.vm.getHealthCheckClass(0.3)).toBe('error')
  })

  it('handles null health check rate', () => {
    const wrapper = mount(ProxyStatusPanel)
    expect(wrapper.vm.getHealthCheckClass(null)).toBe('')
  })
})
