import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'vite';
import { createSSRApp, h } from 'vue';
import { renderToString } from '@vue/server-renderer';

test('settings keep the service connection without source management controls', async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: 'custom' });
  try {
    const SettingsView = (await server.ssrLoadModule('/src/views/SettingsView.vue')).default;
    const { appState } = await server.ssrLoadModule('/src/state.ts');
    const { locale } = await server.ssrLoadModule('/src/i18n.ts');
    locale.value = 'zh-CN';
    for (const enabled of [false, true]) {
      appState.settings.playback_service = {
        enabled, url: 'https://example.test', token_configured: enabled, groups: [],
      };
      const html = await renderToString(createSSRApp({
        render: () => h(SettingsView, { preview: false, speed: 1, autoNext: true }),
      }));
      assert.match(html, /服务连接/);
      assert.match(html, /服务地址/);
      assert.match(html, /访问令牌/);
      assert.match(html, enabled ? /已配置，留空保持不变/ : /未连接/);
      for (const entry of ['来源管理', '播放源', 'Stremio 扩展', '添加扩展', 'TMDB']) {
        assert.ok(!html.includes(entry), `unexpected settings entry: ${entry}`);
      }
    }
  } finally {
    await server.close();
  }
});
