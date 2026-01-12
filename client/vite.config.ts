import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 3000,
    proxy: {
      '/api/indexing/sse': {
        target: 'http://127.0.0.1:3001',
        changeOrigin: true,
        // SSE requires these settings to prevent buffering
        configure: (proxy, _options) => {
          proxy.on('proxyReq', (proxyReq, _req, _res) => {
            // Force HTTP/1.1 for SSE
            proxyReq.setHeader('Accept', 'text/event-stream');
          });
          proxy.on('proxyRes', (proxyRes, _req, res) => {
            // Disable buffering for SSE
            res.setHeader('X-Accel-Buffering', 'no');
            res.setHeader('Cache-Control', 'no-cache, no-transform');
            res.setHeader('Connection', 'keep-alive');
          });
        },
      },
      '/api': {
        target: 'http://127.0.0.1:3001',
        changeOrigin: true,
      },
      '/mcp': {
        target: 'http://127.0.0.1:3002',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            // Forward original host for OAuth URL construction
            proxyReq.setHeader('X-Forwarded-Host', req.headers.host || 'localhost:3000');
            proxyReq.setHeader('X-Forwarded-Proto', 'http');
          });
        },
      },
      // OAuth stub routes for MCP clients that expect OAuth discovery
      '/.well-known/oauth-authorization-server': {
        target: 'http://127.0.0.1:3002',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            proxyReq.setHeader('X-Forwarded-Host', req.headers.host || 'localhost:3000');
            proxyReq.setHeader('X-Forwarded-Proto', 'http');
          });
        },
      },
      '/.well-known/oauth-protected-resource': {
        target: 'http://127.0.0.1:3002',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            proxyReq.setHeader('X-Forwarded-Host', req.headers.host || 'localhost:3000');
            proxyReq.setHeader('X-Forwarded-Proto', 'http');
          });
        },
      },
      '/oauth': {
        target: 'http://127.0.0.1:3002',
        changeOrigin: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            proxyReq.setHeader('X-Forwarded-Host', req.headers.host || 'localhost:3000');
            proxyReq.setHeader('X-Forwarded-Proto', 'http');
          });
        },
      },
    },
  },
})
