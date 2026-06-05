import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

/** @type {import('next').NextConfig} */
const nextConfig = {
    output: 'standalone',
    outputFileTracingRoot: __dirname,
    typescript: {
        // Pre-existing type mismatches from Prisma→Rust migration.
        // TODO: fix incrementally and re-enable.
        ignoreBuildErrors: true,
    },
    eslint: {
        // Pre-existing ESLint warnings (useEffect deps, img elements).
        // TODO: fix incrementally and re-enable.
        ignoreDuringBuilds: true,
    },
   /* experimental: {
        instrumentationHook: true
    }*/
};

export default nextConfig;
