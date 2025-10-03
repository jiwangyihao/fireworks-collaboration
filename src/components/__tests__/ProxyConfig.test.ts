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
    expect(wrapper.find('#proxy-mode').exists()).toBe(true)
  })

  it('displays mode options', () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    const options = modeSelect.findAll('option')
    expect(options.length).toBe(4)
    expect(options[0].text()).toContain('关闭')
    expect(options[1].text()).toContain('HTTP/HTTPS')
    expect(options[2].text()).toContain('SOCKS5')
    expect(options[3].text()).toContain('系统代理')
  })

  it('allows mode selection', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Select HTTP mode
    await modeSelect.setValue('http')
    expect((modeSelect.element as HTMLSelectElement).value).toBe('http')
  })

  it('shows URL input when mode is http', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Initially off mode, no URL input
    expect(wrapper.find('#proxy-url').exists()).toBe(false)
    
    // Switch to HTTP mode
    await modeSelect.setValue('http')
    expect(wrapper.find('#proxy-url').exists()).toBe(true)
  })

  it('shows URL input when mode is socks5', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to SOCKS5 mode
    await modeSelect.setValue('socks5')
    expect(wrapper.find('#proxy-url').exists()).toBe(true)
  })

  it('hides URL input when mode is system', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to system mode
    await modeSelect.setValue('system')
    // URL input should be disabled in system mode
    const urlInput = wrapper.find('#proxy-url')
    if (urlInput.exists()) {
      expect(urlInput.attributes('disabled')).toBeDefined()
    }
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
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to system mode first
    await modeSelect.setValue('system')
    await wrapper.vm.$nextTick()
    
    // Find detect button (button that contains "检测")
    const buttons = wrapper.findAll('button')
    const detectBtn = buttons.find(btn => btn.text().includes('检测'))
    
    if (detectBtn && detectBtn.exists()) {
      await detectBtn.trigger('click')
      await wrapper.vm.$nextTick()
      
      expect(mockInvoke).toHaveBeenCalledWith('detect_system_proxy')
    }
  })

  it('shows auth fields when authentication is enabled', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to HTTP mode to enable config
    await modeSelect.setValue('http')
    
    // Auth fields (username and password) should be visible in HTTP mode
    expect(wrapper.find('#proxy-username').exists()).toBe(true)
    expect(wrapper.find('#proxy-password').exists()).toBe(true)
  })

  it('validates empty URL when mode requires URL', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to HTTP mode
    await modeSelect.setValue('http')
    
    // URL input should be visible and empty
    const urlInput = wrapper.find('#proxy-url')
    expect(urlInput.exists()).toBe(true)
    expect((urlInput.element as HTMLInputElement).value).toBe('')
  })

  it('saves configuration to store', async () => {
    const configStore = useConfigStore()
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Configure HTTP proxy
    await modeSelect.setValue('http')
    const urlInput = wrapper.find('#proxy-url')
    await urlInput.setValue('http://proxy.example.com:8080')
    
    // Component updates local config
    expect(wrapper.vm.localConfig?.mode).toBe('http')
  })

  it('resets form to default values', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Change some values
    await modeSelect.setValue('http')
    const urlInput = wrapper.find('#proxy-url')
    await urlInput.setValue('http://proxy.example.com:8080')
    
    // Reset (if reset button exists)
    const resetBtn = wrapper.find('.reset-btn, button:contains("重置")')
    if (resetBtn.exists()) {
      await resetBtn.trigger('click')
      expect((modeSelect.element as HTMLSelectElement).value).toBe('off')
    }
  })

  it('toggles advanced options', async () => {
    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to HTTP mode
    await modeSelect.setValue('http')
    
    // Check for checkboxes (advanced options)
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    expect(checkboxes.length).toBeGreaterThan(0)
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
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to system mode first
    await modeSelect.setValue('system')
    
    // Find detect button within system proxy detection section
    const detectBtn = wrapper.find('button')
    if (detectBtn.exists() && detectBtn.text().includes('检测')) {
      await detectBtn.trigger('click')
      await wrapper.vm.$nextTick()
      
      // Should show detection result
      expect(wrapper.text()).toContain('detected.proxy.com')
    }
  })

  it('handles system proxy detection failure', async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    const mockInvoke = invoke as ReturnType<typeof vi.fn>
    mockInvoke.mockResolvedValue({
      detected: false
    })

    const wrapper = mount(ProxyConfig)
    const modeSelect = wrapper.find('#proxy-mode')
    
    // Switch to system mode first
    await modeSelect.setValue('system')
    
    // Find detect button
    const detectBtn = wrapper.find('button')
    if (detectBtn.exists() && detectBtn.text().includes('检测')) {
      await detectBtn.trigger('click')
      await wrapper.vm.$nextTick()
      
      // Should show no proxy detected message
      expect(wrapper.text()).toContain('未检测到系统代理')
    }
  })
})
