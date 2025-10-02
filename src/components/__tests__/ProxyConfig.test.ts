import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import ProxyConfig from '../ProxyConfig.vue'
import { useConfigStore } from '../../stores/config'

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

describe('ProxyConfig.vue', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('renders proxy configuration form', () => {
    const wrapper = mount(ProxyConfig)
    expect(wrapper.find('h3').text()).toBe('代理配置')
    expect(wrapper.find('.mode-selector').exists()).toBe(true)
  })

  it('displays mode options', () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    expect(modeButtons.length).toBe(4)
    expect(modeButtons[0].text()).toContain('关闭')
    expect(modeButtons[1].text()).toContain('HTTP/HTTPS')
    expect(modeButtons[2].text()).toContain('SOCKS5')
    expect(modeButtons[3].text()).toContain('系统代理')
  })

  it('allows mode selection', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Click HTTP mode
    await modeButtons[1].trigger('click')
    expect(modeButtons[1].classes()).toContain('active')
  })

  it('shows URL input when mode is http', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Initially off mode, no URL input
    expect(wrapper.find('input[placeholder*="http://"]').exists()).toBe(false)
    
    // Switch to HTTP mode
    await modeButtons[1].trigger('click')
    expect(wrapper.find('input[placeholder*="http://"]').exists()).toBe(true)
  })

  it('shows URL input when mode is socks5', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Switch to SOCKS5 mode
    await modeButtons[2].trigger('click')
    expect(wrapper.find('input[placeholder*="socks5://"]').exists()).toBe(true)
  })

  it('hides URL input when mode is system', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Switch to system mode
    await modeButtons[3].trigger('click')
    expect(wrapper.find('input[placeholder*="http://"]').exists()).toBe(false)
  })

  it('calls detect system proxy on button click', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue({
      detected: true,
      mode: 'http',
      url: 'http://proxy.example.com:8080'
    })

    const wrapper = mount(ProxyConfig)
    const detectBtn = wrapper.find('.detect-btn')
    
    await detectBtn.trigger('click')
    
    expect(mockInvoke).toHaveBeenCalledWith('detect_system_proxy')
  })

  it('shows auth fields when authentication is enabled', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Switch to HTTP mode to enable config
    await modeButtons[1].trigger('click')
    
    // Initially no auth fields
    expect(wrapper.find('input[placeholder="用户名"]').exists()).toBe(false)
    
    // Enable auth
    const authCheckbox = wrapper.find('input[type="checkbox"]')
    await authCheckbox.setValue(true)
    
    // Auth fields should appear
    expect(wrapper.find('input[placeholder="用户名"]').exists()).toBe(true)
    expect(wrapper.find('input[type="password"]').exists()).toBe(true)
  })

  it('validates empty URL when mode requires URL', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Switch to HTTP mode
    await modeButtons[1].trigger('click')
    
    // Try to save without URL
    const saveBtn = wrapper.find('.save-btn')
    await saveBtn.trigger('click')
    
    // Should show error
    expect(wrapper.find('.error-message').exists()).toBe(true)
    expect(wrapper.find('.error-message').text()).toContain('URL')
  })

  it('saves configuration to store', async () => {
    const configStore = useConfigStore()
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Configure HTTP proxy
    await modeButtons[1].trigger('click')
    const urlInput = wrapper.find('input[placeholder*="http://"]')
    await urlInput.setValue('http://proxy.example.com:8080')
    
    // Save
    const saveBtn = wrapper.find('.save-btn')
    await saveBtn.trigger('click')
    
    // Check store was updated
    expect(configStore.cfg?.proxy?.mode).toBe('http')
    expect(configStore.cfg?.proxy?.url).toBe('http://proxy.example.com:8080')
  })

  it('resets form to default values', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Change some values
    await modeButtons[1].trigger('click')
    const urlInput = wrapper.find('input[placeholder*="http://"]')
    await urlInput.setValue('http://proxy.example.com:8080')
    
    // Reset
    const resetBtn = wrapper.find('.reset-btn')
    await resetBtn.trigger('click')
    
    // Should be back to off mode
    expect(modeButtons[0].classes()).toContain('active')
  })

  it('toggles advanced options', async () => {
    const wrapper = mount(ProxyConfig)
    const modeButtons = wrapper.findAll('.mode-btn')
    
    // Switch to HTTP mode
    await modeButtons[1].trigger('click')
    
    // Advanced options should be hidden initially
    expect(wrapper.find('.advanced-options').exists()).toBe(false)
    
    // Toggle advanced
    const advancedToggle = wrapper.find('.advanced-toggle')
    await advancedToggle.trigger('click')
    
    // Advanced options should appear
    expect(wrapper.find('.advanced-options').exists()).toBe(true)
  })

  it('shows system proxy detection result', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue({
      detected: true,
      mode: 'http',
      url: 'http://detected.proxy.com:3128'
    })

    const wrapper = mount(ProxyConfig)
    const detectBtn = wrapper.find('.detect-btn')
    
    await detectBtn.trigger('click')
    await wrapper.vm.$nextTick()
    
    // Should show detection result
    expect(wrapper.text()).toContain('detected.proxy.com')
  })

  it('handles system proxy detection failure', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue({
      detected: false
    })

    const wrapper = mount(ProxyConfig)
    const detectBtn = wrapper.find('.detect-btn')
    
    await detectBtn.trigger('click')
    await wrapper.vm.$nextTick()
    
    // Should show no proxy detected message
    expect(wrapper.text()).toContain('未检测到系统代理')
  })
})
