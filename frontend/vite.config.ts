import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(({ mode }) => {
  const env = mode === 'development'
    ? loadEnv(mode, process.cwd(), 'NEAR_RPC_URL')
    : {};
  const nearRpcUrl = env.NEAR_RPC_URL || 'https://rpc.testnet.near.org';
  const rpcUrl = new URL(nearRpcUrl);
  const rpcBasePath = rpcUrl.pathname.replace(/\/$/, '');

  return {
    plugins: [react(), tailwindcss()],
    server: {
      proxy: {
        '/api/near-rpc': {
          target: rpcUrl.origin,
          changeOrigin: true,
          secure: true,
          rewrite: (path) => {
            const [pathname, requestQuery = ''] = path.split('?', 2);
            const suffix = pathname.replace(/^\/api\/near-rpc/, '');
            const forwardedPath = `${rpcBasePath}${suffix || (rpcBasePath ? '' : '/')}`;
            const query = [rpcUrl.search.slice(1), requestQuery].filter(Boolean).join('&');
            return `${forwardedPath}${query ? `?${query}` : ''}`;
          },
          headers: rpcUrl.username || rpcUrl.password
            ? { Authorization: `Basic ${Buffer.from(`${rpcUrl.username}:${rpcUrl.password}`).toString('base64')}` }
            : {},
        },
      },
    },
  };
});
